# Proxy

Reverse proxy HTTP berbasis Pingora. Daftarkan backend dengan mengulangi `--upstream`. Proxy membagikan request secara round robin sesuai urutan alamat yang diberikan. Dengan dua alamat, urutannya adalah A, B, A, B, termasuk untuk request berurutan pada satu koneksi HTTP.

## Mencoba dua backend lokal

Jalankan dari root repo dengan toolchain Rust stabil yang mendukung edition 2024. Gunakan dua direktori crate sebagai backend contoh agar isi respons menunjukkan server yang melayani request.

Di dua terminal terpisah, jalankan satu server Python 3 per terminal.

```sh
python3 -m http.server 8000 --bind 127.0.0.1 --directory crates/proxy-http
```

```sh
python3 -m http.server 8001 --bind 127.0.0.1 --directory crates/proxy-balancer
```

Di terminal ketiga, jalankan proxy.

```sh
cargo run --locked -- --upstream 127.0.0.1:8000 --upstream 127.0.0.1:8001
```

Di terminal keempat, kirim enam request ke path yang sama. Nama package dalam respons akan menampilkan `proxy-http` lalu `proxy-balancer`, berulang tiga kali.

```sh
for i in 1 2 3 4 5 6; do
  curl -fsS http://127.0.0.1:8080/Cargo.toml | sed -n 's/^name = "\(.*\)"/\1/p'
done
```

### Mencoba failover dan pemulihan

1. Tekan Ctrl+C di terminal backend pada port `8001`. Biarkan proxy dan backend pada port `8000` berjalan.
2. Jalankan kembali enam request di atas. Semua respons sekarang berasal dari `proxy-http`.
3. Nyalakan kembali backend pada port `8001` dengan perintah yang sama. Setelah probe berikutnya berhasil, kedua nama kembali muncul bergantian tanpa restart proxy.

Proxy memeriksa semua backend secara paralel dengan koneksi TCP sebelum listener menerima trafik, lalu mengulang pemeriksaan setiap satu detik. Batas waktu koneksi probe adalah satu detik. Satu probe gagal mengeluarkan backend dari rotasi dan satu probe sukses mengembalikannya. Pemeriksaan ini mengukur apakah port backend menerima koneksi TCP.

Jika backend mati di antara dua probe dan koneksi gagal sebelum request dikirim, proxy mencoba backend lain yang tersedia. Jika semua backend mati, proxy mengembalikan `502 Bad Gateway` dan tetap memeriksa pemulihannya.

## Konfigurasi satu backend

`cargo run --locked` menggunakan listener `127.0.0.1:8080` dan satu backend `127.0.0.1:8000`. Gunakan `--listen` dan `--upstream` untuk mengganti alamat IPv4 atau IPv6.

```sh
cargo run --locked -- --listen 127.0.0.1:9000 --upstream 127.0.0.1:8000
```

## Aturan akses IP

Gunakan `--allow-ip` dan `--deny-ip` saat menjalankan proxy. Tiap opsi menerima satu alamat IPv4 atau IPv6 persis dan bisa diulang. Contoh berikut mengizinkan koneksi dari `127.0.0.1`.

```sh
cargo run --locked -- --allow-ip 127.0.0.1 --deny-ip 192.0.2.10
```

| Kondisi | Hasil |
|---|---|
| IP ada di denylist, termasuk jika juga ada di allowlist | Proxy langsung mengembalikan `403 Forbidden`. |
| Allowlist terisi dan IP klien tidak tercantum | Proxy langsung mengembalikan `403 Forbidden`. |
| IP lolos dari kedua pemeriksaan | Proxy meneruskan request ke backend sesuai giliran round robin. |

Secara default kedua daftar kosong dan semua klien diizinkan. Pemeriksaan menggunakan IP peer TCP. Jika ada proxy lain di depan aplikasi, IP yang diperiksa adalah IP proxy tersebut. Header HTTP seperti `X-Forwarded-For` tidak mengubah keputusan akses.

## Mengembangkan fitur

Untuk memilih lokasi fitur dan melihat hubungan antar-crate, baca [arsitektur workspace](docs/architecture.md). Untuk gaya Rust, penempatan tes, dan perintah verifikasi, baca [panduan pengembangan](docs/development.md). Agen mulai dari [panduan agen](AGENT.md).

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

Secara default, proxy memeriksa apakah port TCP tiap backend menerima koneksi, dengan batas waktu satu detik. Probe berjalan paralel sebelum listener menerima trafik dan diulang setiap satu detik. Satu probe gagal mengeluarkan backend dari rotasi dan satu probe sukses mengembalikannya.

Jika backend mati di antara dua probe dan koneksi gagal sebelum request dikirim, proxy mencoba backend lain yang tersedia. Jika semua backend mati, proxy mengembalikan `502 Bad Gateway` dan tetap memeriksa pemulihannya.

### Memeriksa kesiapan aplikasi lewat HTTP

Jika kedua backend menyediakan endpoint `/health`, aktifkan pemeriksaan HTTP saat menjalankan proxy.

```sh
cargo run --locked -- --health-check http \
  --upstream 127.0.0.1:8000 --upstream 127.0.0.1:8001
```

Proxy mengirim `GET /health` langsung ke setiap backend dengan header `Host` berisi alamat dan port tujuan. Hanya respons `200 OK` yang menandakan backend sehat. Respons `500`, status lain, atau probe yang gagal atau melebihi dua detik mengeluarkan backend dari rotasi. Request berikutnya menuju backend sehat. Jika semuanya gagal, klien mendapat `502 Bad Gateway`.

Probe tetap berjalan selama backend berada di luar rotasi. Setelah satu probe mendapat `200 OK` dan selesai, backend kembali melayani request secara round robin tanpa restart. Untuk mengulangi skenario `200 → 500 → 200` dengan dua backend lokal, jalankan `cargo test -p proxy --test http_health --locked`.

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

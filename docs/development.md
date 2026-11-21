# Pengembangan

## Menulis kode Rust

Ikuti format `rustfmt`. Gunakan `snake_case` untuk modul dan fungsi, serta `UpperCamelCase` untuk tipe. Pertahankan item tetap privat selama hanya digunakan dalam modulnya. Gunakan `pub(crate)` ketika modul lain dalam crate membutuhkannya dan `pub` untuk kontrak antar-crate.

Gunakan `Result` untuk kegagalan yang perlu ditangani pemanggil. Terjemahkan keputusan kebijakan dan kegagalan menjadi respons pada lapisan HTTP yang memiliki konteks request.

## Menempatkan tes

Tempatkan unit test di modul pemilik dalam `#[cfg(test)] mod tests`. Letakkan integration test yang menggunakan API publik atau binary di direktori `tests/` milik package terkait.

| Perubahan | Perilaku yang diuji | Pemeriksaan terfokus |
|---|---|---|
| Listener dan forwarding | Binary meneruskan metode, path, query, header, dan body ke backend HTTP lokal. Respons sukses dan backend offline diuji lewat jaringan loopback. | `cargo test -p proxy --test forwarding --locked` |
| Round robin | Dua backend lokal menerima sepuluh request bergantian pada satu koneksi klien. Periksa identitas backend di setiap respons. | `cargo test -p proxy --test forwarding --locked alternates_backends` |
| Aturan IP | Periksa allowlist, denylist, prioritas deny, alamat IPv4-mapped IPv6, dan peer tanpa IP. | `cargo test -p proxy-security --locked` |
| Health check TCP | Periksa hasil probe TCP untuk pool yang sebagian atau seluruhnya offline. Uji binary dengan menghentikan backend lalu menyalakannya lagi di port yang sama dan periksa urutan respons. | `cargo test -p proxy-balancer --locked` dan `cargo test -p proxy --test forwarding --locked routes_to_survivor` |
| Health check HTTP | Biarkan kedua backend menyala. Ubah status `/health` salah satunya dari 200 ke 500 lalu kembali ke 200. Tunggu hasil probe, periksa urutan respons klien, dan hitung request aplikasi yang diterima tiap backend. Uji juga respons 502 ketika semua probe gagal sejak startup. | `cargo test -p proxy --test http_health --locked` |
| Timeout HTTP | Jalankan binary dengan path `/ready`, interval, dan timeout eksplisit. Tahan respons probe satu backend sambil tetap melayani request aplikasi. Periksa backend itu keluar dari rotasi dan kembali setelah probe mendapat 200 tepat waktu. Uji juga bahwa timeout yang diperpanjang mengizinkan respons lambat. | `cargo test -p proxy --test http_health --locked ready` |
| Ambang probe HTTP | Kendalikan satu respons `/ready` per putaran. Tahan probe berikutnya saat memeriksa urutan backend melalui listener. Periksa trafik sebelum dan sesudah ambang gagal serta ambang sukses tercapai. Sisipkan hasil berlawanan untuk menguji bahwa tiap rentetan terputus. | `cargo test -p proxy --test http_health --locked applies_consecutive_thresholds_to_ready_probes` |
| Backend belum siap saat startup | Tahan probe pertama kedua backend dan periksa HTTP 502 melalui listener. Kendalikan respons `/ready` tiap backend, sisipkan kegagalan di antara keberhasilan, lalu periksa trafik saat satu dan kemudian kedua backend mencapai ambang sukses. | `cargo test -p proxy --test http_health --locked starts_unhealthy_until_each_ready_endpoint_passes_twice` |
| Status backend | Baca `/status` di antara putaran probe. Cocokkan alamat dan nilai `healthy` dalam JSON dengan urutan backend yang melayani request. Periksa HTTP 200 saat semua backend belum siap dan 403 untuk klien yang diblokir. | `cargo test -p proxy --test http_health --locked` |
| Penolakan request HTTP | Klien yang diblokir mendapat 403, backend tidak menerima request tersebut, dan POST yang diizinkan mencapai backend. | `cargo test -p proxy --test forwarding --locked` |

## Memeriksa perubahan

Jalankan perintah berikut dari root sebelum menyelesaikan perubahan kode atau manifest. Opsi `--workspace` mencakup semua package karena target default dari root adalah binary.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

Setelah mengubah anggota workspace atau dependensi internal, periksa daftar package dan dependensi langsung dengan `cargo metadata --no-deps --format-version 1 --locked`. Jika dependensi berubah, perbarui lockfile melalui Cargo sebelum menjalankan pemeriksaan terkunci. Setelah mengubah startup, jalankan `cargo run --locked` dan amati keluaran serta status keluar binary.

Untuk perubahan dokumentasi, periksa tujuan tautan relatif dan cocokkan contoh perintah dengan package yang disebutkan. Jika menambahkan aturan, letakkan di dokumen yang menjelaskan keputusan tersebut dan tautkan dari panduan agen bila diperlukan.

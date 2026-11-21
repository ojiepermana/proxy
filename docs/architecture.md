# Arsitektur workspace

Workspace menghasilkan satu binary. Tempatkan perubahan di crate yang memiliki keputusan atau perilaku tersebut.

| Lokasi | Tanggung jawab | Dependensi langsung |
|---|---|---|
| `apps/proxy` | Merakit pool, kebijakan IP, layanan health check, dan listener. Secara bawaan listener menunggu putaran probe pertama. Dengan `--health-check-start-unhealthy`, listener dapat menerima trafik sejak awal. | `clap`, `proxy-http`, `proxy-balancer`, `proxy-security`, Pingora dengan fitur `proxy` |
| `crates/proxy-http` | Handler `ReverseProxy` melayani `/status` dan menyimpan alamat terpilih di konteks request yang diteruskan. Jika koneksi gagal sebelum pengiriman, handler memilih alamat sehat lain dan melewati alamat yang sudah gagal untuk request tersebut. | `async-trait`, `serde_json`, `proxy-security`, `proxy-balancer`, Pingora dengan fitur `proxy` |
| `crates/proxy-balancer` | `RoundRobinPool` menyimpan urutan konfigurasi dan giliran bersama. Saat memilih alamat, pool melewati backend yang ditandai tidak sehat oleh Pingora. | `async-trait`, `futures`, `tokio`, Pingora dengan fitur `lb` |
| `crates/proxy-security` | `IpAccessPolicy` mengevaluasi IP peer terhadap allowlist dan denylist. IPv4 dan representasi IPv4-mapped IPv6 dicocokkan sebagai IP yang sama. | Pustaka standar Rust |

## Menempatkan perubahan

- **Aturan IP** berada di `proxy-security`. Gunakan IP peer dari koneksi TCP. Jika IP peer tidak tersedia saat ada aturan aktif, tolak request.
- **Health check backend** berada di `proxy-balancer`. Binary meneruskan pengaturan CLI sebagai `HttpHealthOptions`. Modul `health` menerapkan path dan timeout pada probe GET tiap backend dengan header `Host` sesuai alamatnya. `RoundRobinPool` mengatur interval pada `HealthCheckService`. Pool dan layanan berbagi satu `HealthRegistry`. Registry Pingora menghitung hasil berturut-turut per backend dan mengubah status saat mencapai ambang yang diberikan oleh `HealthCheck::health_threshold`.
- **Status awal backend HTTP** ditetapkan setelah discovery statis dan sebelum listener dimulai. Jika opsi startup aktif, `StartupAdmission` menonaktifkan setiap backend lewat `Backends::set_enable`. Probe tetap berjalan. Saat rentetan sukses mencapai ambang, modul mengaktifkan backend dan menghapus penghitung startup miliknya. `HealthRegistry` menangani perubahan kesehatan berikutnya.
- **Respons HTTP untuk akses yang ditolak** berada di `proxy-http`. Evaluasi kebijakan dalam `request_filter` dan akhiri request yang ditolak dengan HTTP 403 sebelum mengambil giliran dari `proxy-balancer`.
- **Status backend** dibaca lewat `RoundRobinPool::backend_statuses`, yang menggunakan `Backends::ready` seperti pemilihan upstream. Setelah aturan IP mengizinkan request, `proxy-http` membentuk respons JSON untuk `GET /status` langsung di listener.

## Mengembangkan struktur

Mulai dengan modul di crate pemilik. Pisahkan menjadi crate baru ketika tugasnya dapat dijelaskan secara mandiri dan arah dependensinya jelas. Daftarkan crate baru di `workspace.members` pada manifest root. Pertahankan arah dependensi pada tabel agar kedua crate fitur bisa diuji sendiri.

Deklarasikan versi dependensi eksternal bersama dan jalur dependensi internal di `[workspace.dependencies]` pada manifest root. Setiap package mewarisi dependensi yang dipakainya dengan `workspace = true` dan mengaktifkan fitur yang dibutuhkan di manifest miliknya. Simpan satu `Cargo.lock` di root.

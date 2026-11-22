Baca [AGENT.md](AGENT.md) sebelum mengerjakan tugas di repo ini.

## graphify

Saat pengguna mengetik `/graphify`, ikuti panduan skill Graphify.

- Jika `graphify-out/graph.json` tersedia, awali pertanyaan tentang kode dengan `graphify query "<pertanyaan>"`. Gunakan `graphify path "<A>" "<B>"` untuk menelusuri hubungan dan `graphify explain "<konsep>"` untuk memeriksa satu konsep.
- Tetap gunakan graf saat ada perubahan lokal di `graphify-out/`. Periksa sumber langsung jika sedang menyelidiki graf yang usang atau keliru, atau jika pengguna meminta untuk melewati Graphify.
- Untuk menjelajahi repo, mulai dari `graphify-out/wiki/index.md` jika tersedia. Baca `graphify-out/GRAPH_REPORT.md` untuk tinjauan arsitektur atau jika hasil kueri belum cukup.
- Setelah mengubah kode, jalankan `graphify update .` dari root repo untuk memperbarui graf dari AST lokal.
- Jika dokumentasi berubah, jalankan `/graphify . --update` di Codex. Tandai hasil ekstraksi semantik dengan `_origin: "semantic"` sebelum menggabungkannya dengan AST agar pembaruan kode mempertahankan hubungan dari dokumentasi.
- Sertakan perubahan di `graphify-out/` dalam commit yang sama dengan sumbernya. Setelah `git pull`, graf siap dikueri dan `graphify-out/graph.html` bisa dibuka di browser.

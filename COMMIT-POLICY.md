# KEBIJAKAN GIT COMMIT & PUSH (COMMIT POLICY)

> ⚠️ **ATURAN PENGEMBANGAN / AI ASSISTANT:**
> 1. **DILARANG COMMIT / PUSH OTOMATIS:** Jangan pernah menjalankan perintah `git commit` maupun `git push` ke repositori remote secara otomatis di akhir fase atau setelah membuat file, **kecuali jika pengguna secara eksplisit meminta** (contoh: *"lakukan atomic commit dan push"*).
> 2. **ATOMIC COMMIT STANDARDS:** Setiap commit harus terisolasi secara logis per fitur/fase (*atomic*), menggunakan format Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
> 3. **VERIFIKASI SEBELUM COMMIT:** Seluruh kode wajib lulus kompilasi dan pengujian (`cargo test`) sebelum di-commit.

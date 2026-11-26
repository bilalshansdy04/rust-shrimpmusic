# Changelog — ShrimpMusic Player v1.2.0

**Tanggal Rilis:** 2026-11-26  
**Upgrade dari:** `v1.1.0`

## Apa yang Baru di v1.2.0

### 1. Waveform Seekbar Dinamis (RMS + Dynamic Range Expansion)
- Mengganti algoritma ekstraksi puncak waveform di `src/audio/waveform.rs` dari *Max Peak* menjadi **RMS (*Root Mean Square*)** per paket audio dan per bucket (120 bar).
- Menambahkan *floor subtraction* dan kurva kontras (`powf(1.45)`) agar lagu modern yang di-mastering keras tetap menampilkan perbedaan dinamika yang jelas antara intro, verse, bridge, dan chorus.

### 2. Reset Otomatis Cache Waveform Lama
- Menambahkan migrasi satu kali berbasis `user_preferences` (`waveform_algo_version = '2'`) di `src/db/schema.rs`.
- Saat aplikasi `v1.2.0` dijalankan pada database lama, kolom `waveform_data` otomatis dikosongkan satu kali agar seluruh lagu digenerate ulang dengan bentuk gelombang RMS dinamis yang baru.

### 3. Tampilan Versi Aplikasi di Pengaturan → Info
- Menambahkan tampilan **`Versi v1.2.0`** pada kartu identitas aplikasi di tab **Pengaturan → Info** (`ui/views/settings.slint`).
- Nilai versi diikat secara dinamis dari `CARGO_PKG_VERSION` (`src/ui/app.rs` & `ui/appwindow.slint`) sehingga selalu sinkron dengan versi rilis di `Cargo.toml`.

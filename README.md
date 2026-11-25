<div align="center">
  <img src="assets/logo_small.png" alt="ShrimpMusic Player Logo" width="96" height="96" />
  <h1>ShrimpMusic Player</h1>
  <p><strong>Pemutar Musik Audiophile Beresolusi Tinggi untuk Windows</strong></p>
  <p>
    Bit-Perfect WASAPI • Ringan (&lt; 50 MB RAM) • Single-Binary Native Rust &amp; Slint UI
  </p>

  <p>
    <a href="https://github.com/bilalshansdy04/rust-shrimpmusic/releases/latest">
      <img src="https://img.shields.io/github/v/release/bilalshansdy04/rust-shrimpmusic?style=for-the-badge&color=FF5722&label=Download" alt="Latest Release" />
    </a>
    <img src="https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(x64)-0078D6?style=for-the-badge&logo=windows" alt="Windows 10/11" />
    <img src="https://img.shields.io/badge/Built%20With-Rust%20%2B%20Slint-000000?style=for-the-badge&logo=rust" alt="Rust + Slint" />
    <img src="https://img.shields.io/badge/Audio-WASAPI%20Bit--Perfect-16A34A?style=for-the-badge" alt="WASAPI Bit-Perfect" />
  </p>
</div>

---

## Mengapa ShrimpMusic Player?

Kebanyakan pemutar musik modern berjalan di atas browser (Electron) yang memakan ratusan MB RAM dan mencampur audio melalui mixer sistem operasi. **ShrimpMusic Player** dibangun murni dengan **Rust** untuk menghadirkan pengalaman mendengarkan musik *lossless* yang murni, responsif, dan sangat hemat sumber daya.

### Fitur Unggulan

- **Audio Engine Audiophile (WASAPI Shared & Exclusive):**
  - Dukungan penuh format lossless & hi-res: **FLAC, ALAC, WAV/PCM, AAC/M4A, MP3** hingga **32-bit / 384 kHz**.
  - **WASAPI Exclusive Mode (Bit-Perfect):** Mengirim sinyal audio langsung ke DAC eksternal / IEM tanpa resampler Windows.
  - **Smooth Seek Crossfade:** Transisi *fade-out / fade-in* mikro saat menggeser durasi (*scrubbing*) agar bebas dari suara *pop* atau *click*.
  - **USB DAC Hotplug Detection:** Otomatis mendeteksi saat DAC/dongle Type-C dicolokkan atau dilepas.

- **DSP & Equalizer Presisi:**
  - **10-Band Parametric Equalizer** dengan preset siap pakai (*Flat, Acoustic, Bass Boost, Vocal, Rock, Electronic*).
  - **Volume Balance (ReplayGain / Soft Limiter)** untuk menjaga tingkat kekencangan suara antar lagu tetap nyaman tanpa *clipping*.

- **Lirik Kinetik & Romanisasi Cerdas (Korea & Jepang):**
  - Membaca lirik tersinkronisasi (`.lrc` eksternal maupun *embedded* `LYRICS` / `USLT` / `UNSYNCEDLYRICS`).
  - Dukungan format timestamp `[mm:ss.xx]` dan `[mm:ss:xx]` dengan auto-scroll halus serta klik baris lirik untuk langsung melompat ke bagian lagu tersebut.
  - **Romanizer Bawaan:** Mengubah teks **Hangul (Korea), Hiragana, Katakana, dan Kanji (Jepang)** ke huruf Latin secara opsional (dapat diatur terpisah untuk Judul, Artis, dan Lirik).

- **Manajemen Pustaka & Playlist Ala Spotify:**
  - **Scoped Library:** Kelola banyak folder musik dengan filter per folder, pengurutan dropdown (*Title, Artist, Album, Recently added, Duration*), dan pencarian real-time berbasis **SQLite FTS / WAL Mode**.
  - **Smart Playlist & Batch Select:** Pilih banyak lagu sekaligus, tambahkan ke antrean atau playlist dengan **Sweet Alert deteksi duplikat** (*Lewati* atau *Tetap Tambahkan*), serta **Impor/Ekspor `.m3u8`**.
  - **Antrean Pintar:** Mode *Shuffle* (*Anti-Ulang Pintar, Acak Normal, Favorit*), rotasi antrean berurutan dari lagu yang diputar, dan *Kosongkan Antrean* tanpa menghentikan lagu aktif.

- **Integrasi Native Windows Taskbar:**
  - **Taskbar Hover Media Controls:** Sorot ikon di Taskbar Windows untuk melihat lagu yang sedang diputar lengkap dengan tombol **Previous, Play/Pause, dan Next**.
  - **Taskbar Jump List (Recently Played):** Klik kanan ikon di Taskbar untuk langsung memutar kembali 5 lagu terakhir yang Anda dengarkan.
  - **System Tray & Hearing Safety Guard:** Perlindungan volume awal untuk pengguna IEM sensitif.

---

## Download

Unduh versi terbaru melalui halaman [**GitHub Releases**](https://github.com/bilalshansdy04/rust-shrimpmusic/releases/latest):

| Paket | Deskripsi | Cocok Untuk |
| :--- | :--- | :--- |
| **Installer (`.exe`)** | Paket instalasi Windows standar lengkap dengan *Start Menu shortcut* & *Uninstaller*. | Penggunaan harian di PC/Laptop |
| **Portable (`.zip`)** | Berisi file tunggal `shrimp-music-player.exe` siap jalan tanpa instalasi. | Flashdisk / Power User |

> **Catatan Windows SmartScreen:**
> Karena rilis indie ini belum menggunakan sertifikat *Code Signing* komersial, Windows mungkin menampilkan peringatan *"Windows protected your PC"* saat pertama kali dijalankan. Klik **More info** → **Run anyway** untuk melanjutkan. Anda juga dapat memverifikasi keaslian file menggunakan `SHA256SUMS.txt` yang tersedia di halaman Release.

---

## Arsitektur & Teknologi

| Komponen | Teknologi |
| :--- | :--- |
| **Bahasa** | Rust (Edition 2024) |
| **Antarmuka (GUI)** | Slint 1.19 (Software Renderer — Low RAM Footprint) |
| **Audio Output** | Windows WASAPI (`wasapi` crate) |
| **Audio Decoding & DSP** | `symphonia` + `rubato` + Custom 10-Band Biquad EQ |
| **Metadata & Artwork** | `lofty` + `image` + `fast_image_resize` |
| **Database & Pencarian** | SQLite (`rusqlite` bundled, WAL Mode) |
| **Packaging** | `cargo-packager` (NSIS Installer) |

---

## Build dari Source

Pastikan Anda sudah menginstall **Rust Toolchain** (`rustup`) dan **Visual Studio C++ Build Tools** di Windows:

```powershell
# Clone repositori
git clone https://github.com/bilalshansdy04/rust-shrimpmusic.git
cd rust-shrimpmusic

# Jalankan seluruh test suite (Unit, Audio, DB, & Slint E2E)
cargo test

# Build binary release
cargo build --release
```

Binary hasil kompilasi akan tersedia di `target/release/shrimp-music-player.exe`.

---

<div align="center">
  <sub>Dibuat oleh <strong>Bilal Shandyarta Syamsudin</strong> • ShrimpMusic Player</sub>
</div>

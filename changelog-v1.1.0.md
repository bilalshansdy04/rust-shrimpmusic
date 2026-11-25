# Changelog — ShrimpMusic Player v1.1.0

Dokumen ini merangkum secara lengkap dan rinci seluruh fitur, arsitektur, alur pemindaian pustaka, konversi/romanisasi teks, pemrosesan audio, manajemen antrean & playlist, integrasi sistem Windows, serta seluruh isi panel **Pengaturan** pada **ShrimpMusic Player**.

---

## 1. Arsitektur & Performa Aplikasi
- **Teknologi Inti**: Dibangun dengan bahasa **Rust** (Edition 2024) dan **Slint GUI** (`1.19.0`) menggunakan komponen Material + ikon vektor **Lucide**.
- **Renderer Ringan**: Secara default menggunakan `SLINT_BACKEND=software` (bisa di-override lewat environment variable) agar penggunaan RAM tetap rendah (`< 50 MB`).
- **Build Release Optimal**: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, dan `strip = true` dengan `windows_subsystem = "windows"` di mode release.
- **Mode CLI Headless (`--headless` / `-h`)**: Mendukung pengecekan cepat via terminal untuk menampilkan daftar perangkat output WASAPI, status inisialisasi audio engine, lokasi database SQLite, daftar folder terpantau, sampel lagu, dan direktori cache thumbnail.
- **Database Lokal SQLite (WAL Mode)**:
  - Menggunakan `rusqlite` (bundled) dengan `journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, `cache_size = -4000`, dan `temp_store = MEMORY`.
  - Terdiri dari 6 tabel utama: `library_folders`, `tracks`, `lyrics`, `playlists`, `playlist_tracks`, dan `user_preferences`, lengkap dengan index pada `folder_id`, `artist`, `album`, `is_active`, serta `(playlist_id, sort_order)`.

---

## 2. Pemindaian Pustaka, Rescan & Ekstraksi Metadata (`Scanner` & `Metadata`)
- **Format Audio yang Didukung**: `.flac`, `.mp3`, `.m4a`, `.alac`, `.aac`, `.wav`, dan `.ogg`.
- **Ekstraksi Tag & Spesifikasi Teknis (`lofty`)**:
  - Membaca `title`, `artist`, `album`, `album_artist`, `genre`, `year` (dari `Year` atau `RecordingDate`), `track_number`, `disc_number`, `duration_ms`, `sample_rate`, `bit_depth`, dan format ekstensi.
  - Jika tag judul kosong, otomatis menggunakan nama file (*file stem*), serta fallback ke `"Unknown Artist"` / `"Unknown Album"`.
- **Pemindaian Cerdas (*Fast Skip*) & Rescan Paksa (*Force Refresh*)**:
  - **Scan Normal**: Melewati file yang `file_size` dan `modified_time`-nya belum berubah di database (kecuali jika file `.lrc` eksternal baru/lebih baru ditemukan).
  - **Force Rescan / Refresh Semua / Rescan Per-Folder**: Mengabaikan cache `file_size` & `modified_time` (`force_refresh = true`) untuk memaksa pembacaan ulang seluruh metadata, lirik tertanam, dan artwork.
  - **Sinkronisasi Folder & Pruning**: Jika file berpindah antar-folder terdaftar, `folder_id` diperbarui otomatis; file yang sudah dihapus dari disk akan di-prune dari folder terkait, dan statistik `total_tracks` serta `last_scanned_at` diperbarui.
  - **Banner Progres Real-Time**: Menampilkan banner progres di UI (`is_scanning` & `scan_status_text`) berisi jumlah file yang sedang dipindai (`current / total`) hingga selesai.
- **Ekstraksi & Caching Cover Art (`artwork.rs`)**:
  - Memprioritaskan gambar bertipe `CoverFront` (jika tidak ada, mengambil gambar pertama di tag).
  - Memotong gambar menjadi rasio persegi (*center square crop*) lalu melakukan downsampling ke **500×500 px** menggunakan **SIMD Lanczos3** (`fast_image_resize`) untuk ketajaman High-DPI tanpa membebani RAM.
  - Disimpan sebagai JPEG di folder cache lokal (`thumbnails/<sha256>.jpg`) berdasarkan hash `album-artist-title`.

---

## 3. Lirik Lagu & Sinkronisasi Real-Time (`Lyrics`)
- **Prioritas Sumber Lirik**:
  1. **Embedded Lyrics (Primary Tag)**: Membaca `ItemKey::Lyrics` (Vorbis `LYRICS`, MP4 `©lyr`, APE `Lyrics`) dan `ItemKey::UnsyncLyrics` (ID3v2 `USLT`, Vorbis `UNSYNCEDLYRICS`, MP4 `©lyr`).
  2. **Fallback Semua Tag**: Jika primary tag tidak memiliki lirik (misalnya FLAC yang memiliki tag ganda ID3v2 + Vorbis), scanner memeriksa seluruh tag pada file.
  3. **File Eksternal `.lrc`**: Jika tidak ada lirik tertanam, otomatis membaca file `.lrc` dengan nama yang sama di direktori lagu.
- **Parser Format LRC Fleksibel**:
  - Mendukung format timestamp standar `[mm:ss.xx]` / `[mm:ss.xxx]` / `[mm:ss.x]` / `[mm:ss]` maupun format titik dua `[mm:ss:xx]` (mis. `[00:00:07]`).
  - Mendukung **multi-timestamp** pada satu baris (mis. `[00:12.00][01:05.00]Chorus`).
  - Mendukung **Inline LRC** (seluruh lirik tersimpan dalam satu baris tanpa newline `\n` atau `\r` dengan banyak tag `[mm:ss...]`) melalui normalisasi pemecahan baris otomatis.
- **Tampilan & Interaksi Lirik (`LyricsView`)**:
  - Menyorot baris lirik aktif secara sinkron dengan posisi pemutaran (`active_lyric_index`).
  - Mendeteksi status **Instrumental** atau lagu tanpa lirik.
  - Mendukung scroll manual pengguna (`is_user_scrolling_lyrics`) dengan tombol **Resync** untuk kembali mengikuti posisi lagu aktif, serta klik baris lirik untuk melompat ke timestamp tersebut.

---

## 4. Konversi & Romanisasi Teks Korea & Jepang (`romanize.rs`)
- **Deteksi Skrip Otomatis (`ScriptDetector`)**:
  - Mendeteksi karakter **Hangul Korea** (`U+AC00..=U+D7A3`, `U+1100..=U+11FF`, `U+3130..=U+318F`), **Hiragana** (`U+3040..=U+309F`), **Katakana** (`U+30A0..=U+30FF`), dan **Kanji / CJK Ideographs** (`U+4E00..=U+9FFF`, `U+3400..=U+4DBF`, `U+F900..=U+FAFF`).
- **Mesin Konversi**:
  - **Korea (Hangul → Latin)**: Menggunakan standar *Revised Romanization (RR)* dengan dekomposisi matematis suku kata Hangul (19 *Choseong*, 21 *Jungseong*, 28 *Jongseong*).
  - **Jepang (Hiragana, Katakana & Kanji → Romaji)**: Menggunakan transliterasi Hepburn untuk Kana (termasuk *sokuon* `っ/ッ`, kombinasi *yoon* seperti `kyo/sha/cha`, serta *dakuten/handakuten*) dan `kakasi` untuk konversi pembacaan **Kanji** menjadi Romaji/Latin.
- **Kontrol Granular Ruang Lingkup Konversi**:
  - Master toggle **Konversi ke Huruf Latin (ABC)** (`show_romanized`).
  - Tiga sakelar independen:
    1. **Judul Musik (Track Title)** (`romanize_title`)
    2. **Nama Artis / Penyanyi** (`romanize_artist`)
    3. **Lirik Lagu (Lyrics)** (`romanize_lyrics`) — meromanisasi baris lirik tersinkron maupun tidak tersinkron secara real-time.
- **Dukungan Pencarian**: Query pencarian dapat mencocokkan teks asli maupun hasil romanisasinya.

---

## 5. Mesin Audio, DSP, & Keamanan Pendengaran (`Audio Engine`)
- **Decoder Multi-Format (`SymphoniaDecoder`)**: Mendekode FLAC, MP3, AAC, ALAC, WAV, dan OGG/Vorbis secara streaming dengan pre-buffering (16k frame preload) untuk mencegah stutter saat background playback.
- **Output WASAPI (`WasapiStream`)**:
  - **WASAPI Shared Mode**: Kompatibel dengan seluruh perangkat audio Windows (Speaker internal, Jack 3.5mm, Bluetooth, Virtual Audio).
  - **WASAPI Exclusive Mode (Bit-Perfect)**: Akses langsung ke hardware DAC dengan buffer hardware ~100ms, melewati Windows audio mixer/resampler.
- **Deteksi & Hotplug Perangkat Audio (`DeviceManager` & `DeviceWatcher`)**:
  - Klasifikasi perangkat otomatis menjadi: `USB DAC`, `Type-C DAC`, `USB Audio`, `Analog Jack`, `Speaker`, `Bluetooth`, `Virtual`, dan `Audio`.
  - Mengenali chip & brand DAC eksternal populer (FiiO, JadeAudio JA11, iFi, Topping, Moondrop, iBasso, Shanling, Tempotec, Chord, AudioQuest, Questyle, JCALLY, ALC5686, CX31993, CS43131, ES9038/ES9280, AK4493/AK4499, KT0210, Realtek USB, dll.).
  - **Bit-Perfect Guard**: Mengaktifkan/mengizinkan `Exclusive Mode` hanya pada perangkat yang mendukung bit-perfect (`supports_bit_perfect`) serta memantau colok/cabut USB DAC & Type-C converter secara real-time.
- **10-Band Biquad Parametric Equalizer & DSP (`DspProcessor`)**:
  - 10 pita frekuensi standar: `31.25 Hz`, `62.5 Hz`, `125 Hz`, `250 Hz`, `500 Hz`, `1 kHz`, `2 kHz`, `4 kHz`, `8 kHz`, dan `16 kHz`.
  - 5 preset bawaan (**Flat**, **Bass**, **Vokal**, **Akustik**, **Treble**) + mode **Custom** via slider per-pita.
  - **Rekomendasi Audiophile (`AudiophileRecommendationModal`)**: Saat WASAPI Exclusive Mode diaktifkan sementara EQ aktif, muncul dialog penawaran untuk **Matikan EQ (Rekomendasi)** demi sinyal 0% distorsi digital atau **Tetap Gunakan EQ**.
- **Keseimbangan Volume / ReplayGain (`VolumeBalanceMode`)**:
  - 4 pilihan level: **Mati** (`1.0x`), **Pelan (-18 LUFS)** (`0.72x`), **Normal (-14 LUFS)** (`1.0x`), dan **Keras (-11 LUFS)** (`1.35x`).
- **Waveform Seekbar & Seek Crossfade**:
  - Menghasilkan puncak gelombang audio (`waveform_data`) yang disimpan di SQLite untuk tampilan seekbar interaktif dengan transisi crossfade saat melakukan *seek*.
- **Hearing Safety Handshake (`HearingSafetyModal`)**:
  - Modal keamanan pendengaran dengan pengatur **Windows Master Volume** dan tombol **Putar Nada Uji 10 Detik** (`SafetyTestSignal`: chime multi-harmonik A4/E5/A5 terkalibrasi pada `-18 dBFS` dengan envelope decay setiap 2 detik) sebelum memulai pemutaran.

---

## 6. Pustaka, Pencarian, Antrean & Playlist
- **Navigasi 4 Tampilan Utama (`PillDock`)**:
  1. **Pustaka (`LibraryView`)**: Filter cakupan per-folder (*Semua Folder* atau folder spesifik), tombol **Tambah Folder**, status badge **Hi-Res**, pencarian real-time (termasuk prefix `lirik:` / `lyric:`, `artist:`, `album:`), serta dropdown pengurutan ala Spotify (*Title*, *Artist*, *Album*, *Recently added*, *Duration* — Ascending/Descending).
  2. **Playlist (`PlaylistView`)**: Buat, ubah nama, dan hapus playlist; impor & ekspor format standar **Extended UTF-8 `.m3u8`**; custom artwork playlist (500×500 Lanczos3); pencarian di dalam playlist; pengurutan playlist (*Custom order*, *Title*, *Artist*, *Album*, *Recently added*, *Release date*, *Duration*); tombol **Putar Semua**; dan pemutaran double-click yang membangun antrean khusus dari isi playlist.
  3. **Lirik (`LyricsView`)**: Tampilan lirik sinkron & romanisasi penuh.
  4. **Antrean (`QueueView`)**: Menampilkan lagu *Now Playing* yang tersemat di atas, daftar *Up Next* yang 100% sinkron 1:1 dengan urutan pemutaran, geser/pindah urutan antrean, hapus item dari antrean, tombol **Acak Ulang (Reshuffle)**, dan **Bersihkan Antrean** (tanpa memutus lagu yang sedang diputar).
- **Context Menu & Seleksi Multi-Lagu**:
  - Klik kanan pada lagu: **Putar Lagu**, **Tambahkan ke Antrean**, **Tambahkan ke Playlist**, **Buat Playlist Baru...**, dan **Hapus Lagu dari Pustaka**.
  - Mode multi-seleksi dengan toolbar aksi massal: **+ Ke Antrean**, **+ Ke Playlist**, **Hapus**, **Pilih Semua**, dan **Batal**.
  - **Sweet Alert Duplikat Playlist**: Saat menambahkan satu atau banyak lagu ke playlist yang sudah berisi sebagian/seluruh lagu tersebut, muncul dialog konfirmasi dengan opsi **Batal**, **Lewati** (hanya tambah lagu baru), atau **Tetap Tambahkan**.
- **Kontrol Transport & Mode Putar**:
  - Play/Pause, Previous, Next, Volume slider, **Loop Mode** (`Off` → `Loop All` → `Loop One`), dan **Shuffle** dengan 3 gaya: **Anti-Ulang (Pintar)** (`ReducedRepeat`), **Normal (Acak)** (`Normal`), dan **Favffle (Favorit)** (`Favffle`).
  - Isolasi sumber antrean: memutar lagu dari Pustaka/Folder membangun antrean terotasi dari cakupan folder tersebut; memutar dari Playlist membangun antrean khusus playlist tersebut.

---

## 7. Integrasi Native Windows & Window Chrome
- **Frameless Custom Window (`no-frame: true`)**: Titlebar kustom dengan drag area, tombol Minimize, Maximize/Restore, dan Close.
- **System Tray (`TrayController`)**: Menutup jendela akan meminimalkan aplikasi ke System Tray (dengan ikon ter-embed di binary). Klik/double-click ikon tray untuk membuka kembali jendela; klik kanan tray menyediakan menu **Buka ShrimpMusic**, **Putar / Jeda**, dan **Keluar dari Aplikasi**.
- **Windows Taskbar Thumbnail Controls (`ITaskbarList3`)**: Saat mengarahkan kursor ke ikon ShrimpMusic Player di Windows Taskbar, muncul tombol kontrol **Previous**, **Play/Pause** (ikon dinamis), dan **Next**.
- **Windows Taskbar Jump List (`ICustomDestinationList`)**: Menampilkan daftar **Recently Played** di menu klik kanan ikon Taskbar. Mengklik lagu di Jump List saat aplikasi sudah berjalan akan meneruskan perintah `--play-track <id>` ke instance aktif melalui **Single-Instance IPC (`WM_COPYDATA`)** tanpa membuka jendela ganda.
- **Metadata PE & Identitas Aplikasi**: `AppUserModelID = "ShrimpMusic.Player"` dan informasi versi/ikon Windows (`winres`) agar tampil sebagai **ShrimpMusic Player** di Taskbar dan Task Manager.

---

## 8. Rincian Lengkap Panel Pengaturan (`SettingsDrawer` — 5 Tab)
Seluruh perubahan pengaturan disimpan secara persisten di tabel `user_preferences` SQLite:

1. **Tab `Audio`**:
   - **WASAPI Exclusive Mode**: Toggle akses DAC langsung (Bit-Perfect audiophile), lengkap dengan status ketersediaan dan pesan petunjuk perangkat (`exclusive_hint`).
   - **PERANGKAT OUTPUT AUDIO**: Daftar seluruh perangkat output yang terdeteksi lengkap dengan badge klasifikasi (`USB DAC`, `Type-C DAC`, `Bluetooth`, `Speaker`, dll.), penanda perangkat default OS, status dukungan bit-perfect, dan pemilihan perangkat aktif.
2. **Tab `DSP & EQ`**:
   - **10-Band Biquad Equalizer**: Master toggle aktif/nonaktif EQ.
   - **PRESET EKUALISER**: 5 tombol preset cepat (*Flat*, *Bass*, *Vokal*, *Akustik*, *Treble*) serta 10 slider vertikal frekuensi (`31 Hz` – `16 kHz`).
   - **KESEIMBANGAN VOLUME (REPLAYGAIN)**: Pilihan *Mati*, *Pelan (-18)*, *Normal (-14)*, dan *Keras (-11)*.
3. **Tab `Pustaka`**:
   - **FOLDER TERDAFTAR**: Daftar semua folder musik yang dipantau beserta jumlah lagu per folder.
   - Aksi per-folder: Toggle aktif/nonaktif folder (`toggle_folder`), tombol **Rescan** per folder (`force_refresh`), dan tombol **Hapus** folder.
   - Aksi global: Tombol **Refresh Semua** dan **Tambah Folder**.
4. **Tab `Tema`**:
   - **PILIHAN TEMA APLIKASI**: Pilihan visual **OLED Dark** (*Pitch black minimalis*) dan **Soft Daylight** (*Terang, bersih & elegan*).
   - **GAYA PENGACAKAN (SHUFFLE)**: Pilihan *Anti-Ulang (Pintar)*, *Normal (Acak)*, dan *Favffle (Favorit)*.
   - **ROMANISASI TEKS (KOREA & JEPANG)**: Master toggle **Konversi ke Huruf Latin (ABC)** serta 3 sakelar **RUANG LINGKUP KONVERSI** (*Judul Musik*, *Nama Artis / Penyanyi*, *Lirik Lagu*).
5. **Tab `Info`**:
   - Identitas aplikasi **ShrimpMusic Player** (*Audiophile High-Resolution Audio Player — Arsitektur Windows Native WASAPI & Slint GUI*).
   - **STATUS AUDIO ENGINE**: Menampilkan *Engine Mode* aktif (*WASAPI Exclusive* / *WASAPI Shared*), format, sample rate, dan bit depth yang sedang berjalan.

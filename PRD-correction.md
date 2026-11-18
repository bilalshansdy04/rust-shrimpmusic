# DOKUMEN KOREKSI & ADENDUM PRD (PRD-CORRECTION.MD)

**Proyek:** ShrimpMusic Native (v2.0)  
**Tipe Dokumen:** Suplemen & Adendum Resmi terhadap `PRD.md`  
**Status:** Wajib Dibaca & Dipatuhi Bersama `PRD.md`

---

> ### ⚠️ ATURAN KEPATUHAN ADENDUM (WAJIB DIBACA)
> 1. **DOKUMEN INI ADALAH EKSTENSI RESMI DARI PRD.MD:** Setiap kali memulai sesi kerja, pengembang atau AI Assistant **WAJIB membaca `PRD.md` terlebih dahulu, lalu langsung membaca seluruh isi `PRD-correction.md` ini**.
> 2. **HIERARKI SUMBER KEBENARAN:** Jika terdapat perbedaan atau detail tambahan antara dokumen ini dan `PRD.md`, maka spesifikasi di dalam `PRD-correction.md` ini berlaku sebagai penyempurna tanpa melanggar batasan sistem utama (RAM < 100 MB, Rust + Slint UI, WASAPI Exclusive Engine).

---

## 0. Matriks Korelasi Adendum terhadap Fase Utama PRD

Tabel pemetaan implementasi antara adendum ini dengan tahapan pengembangan di [PRD.md](file:///d:/Projects/music/rust/PRD.md#L224-L230):

| Bagian Adendum | Topik Fitur | Terhubung ke Fase PRD | Modul Kode Terkait |
| :--- | :--- | :--- | :--- |
| **Section 1** | Multi-Folder Management & Sakelar ON/OFF | **Fase 2 (Backend)** & **Fase 3 (UI)** | `src/library/folders.rs`, `src/db/` |
| **Section 1.2** | Strict Stop Audio saat Folder Dimatikan | **Fase 1/2 (Audio Core)** & **Fase 3 (UI)** | `src/audio/player.rs`, `AppState` |
| **Section 2** | Scoped Music List & Sistem Sortir | **Fase 2 (SQLite)** & **Fase 3 (Slint UI)** | `src/library/`, `ui/views/library.slint` |
| **Section 3** | Playlist Multi-Folder, Custom Artwork & M3U8 | **Fase 2 (DB/File)** & **Fase 4 (Interaktivitas)** | `src/library/playlist.rs`, `ui/` |
| **Section 4** | 100% Embedded Metadata & Cover Art First | **Fase 2 (Lofty Parser & Cache)** | `src/library/metadata.rs` |
| **Section 5** | UX Pencarian Khusus Berbasis Lirik | **Fase 2 (DB FTS)** & **Fase 4 (Pill Dock)** | `src/library/search.rs`, `ui/dock.slint` |
| **Section 6** | Volume Persistence & Best Practice | **Fase 2 (Settings DB)** & **Fase 3 (Theme/Shell)** | `src/db/settings.rs`, `src/audio/dsp.rs` |
| **Section 7** | Tiga Gaya Acak (Shuffle) & Kombinasi Loop | **Fase 2 (Queue Logic)** & **Fase 4 (Transport)** | `src/audio/queue.rs`, `ui/stage.slint` |
| **Section 8** | Hearing Safety Alert & 10s Universal Audio Test | **Fase 1/2 (Device Event & Audio Generator)** & **Fase 3 (Safety Modal)** | `src/audio/safety.rs`, `ui/dialogs/safety.slint` |

---

## 1. Multi-Folder Management & Sakelar Aktif/Nonaktif (Folder Switch)
*(Korelasi: Fase 2 Backend & Fase 3 UI)*

### 1.1. Perilaku UX Pengaturan Folder (`Settings -> Library -> Music Folders`)
* **Dukungan Multi-Folder:** Pengguna dapat menambahkan lebih dari 1 direktori musik (Contoh: `D:\Lossless`, `E:\MyFavMusic`, `C:\Users\User\Music`).
* **Sakelar Toggle Tiap Folder (`is_enabled: bool`):**
  * Setiap entri folder di daftar pengaturan memiliki tombol sakelar (*Switch Toggle* `ON` / `OFF`).
  * **Status ON (Default saat folder ditambahkan):** Folder dipindai dan seluruh lagunya dimasukkan ke dalam indeks pustaka aktif.
  * **Status OFF:** Seluruh lagu dari folder tersebut langsung disembunyikan (*hidden*) dari seluruh tampilan antarmuka (Semua Musik, Artis, Album, dan Antrean), **tanpa** menghapus data/konfigurasi folder dari database. Saat diaktifkan kembali (`ON`), lagu-lagu langsung muncul kembali tanpa perlu pemindaian ulang penuh.
* **Tindakan Tambahan:** Tombol *Remove Folder* (hapus permanen dari daftar pantau) dan tombol *Rescan Folder* per individu folder.

### 1.2. Perilaku Pemutusan Audio saat Folder OFF (*Strict Stop on Folder Disable*)
* Jika pengguna mematikan sakelar folder (`OFF`) yang lagunya **sedang aktif diputar**:
  1. *Audio thread* **wajib langsung menghentikan playback (`Stop`) seketika (< 50ms)** dan melepaskan *handle* audio WASAPI.
  2. Alasan: Folder yang dimatikan dianggap tidak aktif/terputus sumber medianya, sehingga musik tidak boleh terus berjalan.
  3. Seluruh lagu dari folder tersebut yang ada di dalam antrean pemutaran (*Queue / Up Next*) langsung dihapus dari antrean aktif.
  4. Antarmuka (*Acoustic Stage*) langsung kembali ke status *Stopped / Idle*.

### 1.3. Skema Tabel SQLite Pendukung
```sql
CREATE TABLE IF NOT EXISTS library_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_path TEXT NOT NULL UNIQUE,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_scanned_at INTEGER,
    total_tracks INTEGER DEFAULT 0
);
```

---

## 2. Navigasi Pustaka Terfilter, Berskala & Sistem Sortir
*(Korelasi: Fase 2 Database & Fase 3 Slint UI)*

Antarmuka tampilan pustaka (*Dynamic Canvas - View B: Library View*) menyediakan pemisahan cakupan (*Scope Selector*) dan opsi penyortiran yang fleksibel:

### 2.1. Filter Cakupan Folder (*Folder Scope Selector*)
* Di bagian atas daftar pustaka terdapat filter dropdown / pill selector:
  * **"Semua Folder" (*All Folders - Default*):** Menggabungkan seluruh lagu dari semua folder yang statusnya `ON` menjadi satu kesatuan daftar pustaka.
  * **"Pilihan Folder Tertentu" (*Per-Folder Mode*):** Pengguna dapat memilih satu folder spesifik (misal hanya melihat lagu di `D:\Lossless`).

### 2.2. Mode Tampilan Pustaka (*Library Views*)
Filter cakupan di atas berlaku secara konsisten di seluruh sub-tampilan berikut:
1. **Semua Musik (*Tracks View*):**
   * *All Folders:* Seluruh lagu dari semua folder aktif disatukan dalam satu daftar tervirtualisasi (`ListView`).
   * *Per Folder:* Hanya menampilkan lagu dari folder yang sedang dipilih.
2. **Per Artis (*Artists View*):**
   * *All Folders:* Agregasi seluruh artis dari semua folder aktif.
   * *Per Folder:* Hanya menampilkan artis yang memiliki lagu di folder terpilih.
3. **Per Album (*Albums View*):**
   * *All Folders:* Agregasi seluruh album dari semua folder aktif dengan sampul album.
   * *Per Folder:* Hanya menampilkan album dari folder terpilih.

### 2.3. Sistem Penyortiran Fleksibel (*Flexible Sorting Options*)
* Pengguna diberikan kontrol penuh untuk mengurutkan daftar musik:
  * **Pilihan Parameter Sortir:**
    1. **Judul Lagu (Title)** -> **Default Awal: Judul A-Z**
    2. **Nama Artis (Artist)** -> A-Z / Z-A
    3. **Nama Album (Album)** -> A-Z / Z-A
    4. **Nomor Trek (Track Number)** -> Mengikuti urutan album asli
    5. **Durasi (Duration)** -> Terpendek / Terpanjang
    6. **Tanggal Ditambahkan (Date Added)** -> Terbaru / Terlama
    7. **Format & Kualitas Audio (Audio Specs)** -> Sample Rate / Bit Depth tertinggi
* **Persistensi Preferensi:** Pilihan urutan terakhir pengguna disimpan otomatis ke database konfigurasi lokal, sehingga saat aplikasi dibuka kembali urutan tidak ter-reset ke default jika pengguna sudah memilih urutan lain.

---

## 3. Sistem Pembuatan, Kustomisasi & Import/Export Playlist
*(Korelasi: Fase 2 Backend & Fase 4 Interaktivitas)*

### 3.1. Dukungan Lintas-Folder (*Cross-Folder Playlists*)
* Playlist bersifat global independen: Pengguna dapat memasukkan lagu ke dalam playlist dari folder aktif mana pun tanpa batasan.
* Jika salah satu folder dinonaktifkan (`switch: OFF`), lagu dari folder tersebut pada playlist akan berstatus *unavailable* (tampilan redup) dan otomatis dilewati saat pemutaran.

### 3.2. Kustomisasi Ikon / Gambar Sampul Playlist (*Custom Playlist Artwork*)
* **Ikon Default:** Jika pengguna tidak memilih gambar khusus, sistem menampilkan kolase 4 sampul lagu pertama di dalam playlist atau ikon grafis minimalis bawaan tema.
* **Custom Artwork Picker:** Pengguna dapat mengklik kartu playlist dan memilih gambar kustom dari komputer (`.jpg`, `.png`, `.webp`).
* **Penyimpanan Gambar Playlist (Anti-Bloat RAM):**
  * Gambar kustom playlist wajib di-*resize* ke **250×250 piksel** menggunakan filter `Lanczos3` dan disimpan di direktori *cache*:
    `%LOCALAPPDATA%/ShrimpMusic/playlists/{playlist_id}.jpg`.
  * Dilarang menyimpan file gambar resolusi asli ke memori UI atau database.

### 3.3. Fitur Universal Import & Export Playlist (`.m3u8`)
Agar playlist dapat dibaca oleh aplikasi pemutar musik lain (MusicBee, foobar2000, VLC, Apple Music, AIMP, dll):
* **Standar Universal:** Menggunakan format **Extended M3U8 (`.m3u8`) dengan encoding UTF-8**.
* **Ekspor Playlist (*Export to .m3u8*):**
  * Menuliskan *header* `#EXTM3U`.
  * Setiap lagu ditulis dengan *directive* `#EXTINF:durasi_detik,Nama Artis - Judul Lagu`, diikuti path absolut file lagu pada baris berikutnya.
* **Impor Playlist (*Import from .m3u8*):**
  * Pengguna memilih file `.m3u8` / `.m3u`.
  * Parser membaca path lagu: Jika file lagu sudah ada di database, langsung ditautkan ke playlist baru. Jika belum terindeks tetapi berada dalam folder aktif, file dipindai otomatis.

### 3.4. Skema Database Playlist
```sql
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    custom_artwork_path TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL
);
```

---

## 4. Spesifikasi Penanganan Metadata & Format Audio
*(Korelasi: Fase 2 Lofty Parser & Scanner)*

### 4.1. Prioritas Ekstraksi Tag & Artwork (100% Embedded First)
Karena koleksi musik pengguna sudah memiliki metadata lengkap:
1. **Metadata Teks:** Menggunakan `lofty` untuk membaca metadata langsung dari tag internal: Judul, Artis, Album, Album Artist, Genre, Tahun, Track Number, Disc Number.
2. **Cover Art:** Ekstrak gambar sampul tertanam (*front cover*), langsung *downscale* ke 250×250px via crate `image`, dan simpan ke folder *cache* lokal.
3. **Prioritas Lirik:**
   * **Langkah 1 (Embedded):** Ekstrak tag lirik tersinkronisasi / tidak tersinkronisasi di dalam file:
     * Tag `©lyr` untuk format M4A/AAC/ALAC.
     * Tag `LYRICS`, `UNSYNCEDLYRICS`, atau frame ID3 `USLT` / `SYLT` untuk FLAC dan MP3.
   * **Langkah 2 (Fallback File `.lrc`):** Hanya jika file audio sama sekali tidak memiliki tag lirik internal, *scanner* mencari file lirik pendamping eksternal dengan nama yang sama persis di folder tersebut (Contoh: `Song.flac` -> `Song.lrc`).

### 4.2. Kebijakan Format Audio (.ogg & .opus)
* `symphonia` sudah mengaktifkan decoder *Vorbis* secara native tanpa biaya RAM tambahan (0% *idle overhead*). Jika di masa depan pengguna memasukkan file `.ogg` (Vorbis), sistem otomatis dapat membacanya.
* Decoder `.opus` eksternal yang berat tidak dipaksakan saat ini demi mematuhi target anggaran RAM ketat (< 100 MB).

---

## 5. UX Pencarian Cerdas & Pencarian Khusus Berbasis Lirik
*(Korelasi: Fase 2 Full-Text Search SQLite & Fase 4 UI Pill Dock)*

### 5.1. Pemisahan Kategori Hasil Pencarian (*Categorized Dropdown*)
Saat pengguna mengetik di bilah *Quick Search* (*Floating Pill Dock*):
1. **Grup 1: Lagu & Metadata (Utama - Prioritas Atas):**
   * Menampilkan hasil pencocokan instan pada Judul Lagu, Nama Artis, dan Nama Album.
2. **Grup 2: Kecocokan Lirik (*Lyrics Match* - Terpisah di Bawah):**
   * Diletakkan pada seksi terpisah dengan label jelas: `Kecocokan Lirik (Lyrics Match)`.
   * Menampilkan judul lagu beserta cuplikan baris lirik yang cocok dengan kata yang dicari (diberi warna aksen penyorot).
   * Menekan item ini akan langsung memutar lagu dan melompatkan durasi (*seek*) tepat ke baris lirik tersebut.

### 5.2. Prefix Pencarian Cepat (*Search Modifiers*)
Pengguna dapat memfilter pencarian secara eksplisit dengan mengetik:
* `lirik:<kata>` atau `lyric:<kata>` -> Membatasi pencarian **hanya** pada konten lirik lagu.
* `artist:<nama>` -> Membatasi pencarian hanya pada nama artis.
* `album:<nama>` -> Membatasi pencarian hanya pada judul album.

---

## 6. Persistensi Volume & Pengaturan Suara (*Volume Persistence*)
*(Korelasi: Fase 2 Settings DB & Fase 3 UI Shell)*

* **Prinsip Best Practice:** Tingkat volume terakhir yang diatur pengguna serta status mute **wajib disimpan secara persisten** di tabel SQLite `user_preferences`.
* **Perilaku Startup:**
  * Saat aplikasi dibuka kembali, volume di-*load* ke posisi terakhir (Default awal jika baru pertama diinstal: `0.7` atau 70%).
  * Menghindari efek kejut audio (*audio shock / ear-rape*) akibat volume melonjak ke 100% saat pengguna menyalakan aplikasi.
* **Volume Ramp-Up Halus:** Saat pertama kali memulai playback lagu setelah startup, *gain* audio dinaikkan dengan *micro-ramp* (`15ms`) untuk mencegah bunyi *pop/click* digital pada DAC.

---

## 7. Sistem & Gaya Pengacakan (*Shuffle Engine*) serta Interaksi Loop
*(Korelasi: Fase 2 Queue Engine & Fase 4 Transport Controls)*

Pengguna dapat memilih gaya acak di menu **Settings -> Playback -> Shuffle Style**:

### 7.1. Tiga Mode Gaya Acak (*Shuffle Modes*)
1. **Mode 1 — Lebih Sedikit Pengulangan (*Smart De-duplicate History* - Default):**
   * Mengacak antrean lagu dengan mencatat riwayat lagu yang baru saja diputar ke dalam *history ring buffer* (20 lagu terakhir atau 25% panjang playlist).
   * Menjamin lagu yang baru saja diputar tidak akan terulang kembali dalam waktu dekat.
2. **Mode 2 — Normal (*Pure Uniform Random*):**
   * Pengacakan murni (*pseudo-random* seragam). Setiap lagu memiliki peluang statistik yang sama untuk terpilih di setiap giliran pemutaran.
3. **Mode 3 — Favffle (*Favorite-Weighted Shuffle*):**
   * Pengacakan berbobot frekuensi pemutaran (*play_count*).
   * Lagu-lagu yang paling sering diputar (*most played*) memiliki probabilitas kemunculan lebih tinggi di barisan awal antrean acak, disusul lagu-lagu yang jarang diputar.

### 7.2. Aturan Matriks Kombinasi: Shuffle + Loop (Repeat)
Kombinasi tombol **Shuffle** dan **Loop** pada *Acoustic Stage* diatur dengan logika berikut:

| Mode Shuffle | Mode Loop (Repeat) | Perilaku Playback & Antrean (*Queue*) |
| :--- | :--- | :--- |
| **Shuffle ON** | **Loop OFF** | Memutar antrean yang teracak hingga lagu terakhir habis, lalu pemutaran **otomatis berhenti (*Stop*)** di akhir antrean. |
| **Shuffle ON** | **Loop ALL (♾️)** | Memutar antrean teracak. Ketika lagu terakhir selesai, seluruh daftar di-reshuffle kembali dan pemutaran **berlanjut tanpa henti**. |
| **Shuffle ON** | **Loop 1 (🔂)** | **Hanya mengulang 1 lagu aktif yang sedang berputar**. Ketika pengguna menekan tombol *Next* atau mematikan Loop 1, pemutaran kembali melanjutkan urutan antrean acak sebelumnya. |
| **Shuffle OFF** | *(Normal)* | Mengikuti urutan normal playlist/album (Loop OFF berhenti di akhir, Loop ALL berulang dari awal, Loop 1 mengulang 1 lagu). |

---

## 8. Protokol Keamanan Pendengaran & Dialog Tes Audio 10 Detik (*Hearing Safety Handshake*)
*(Korelasi: Fase 1/2 Audio Event & Fase 3 Safety Modal Dialog)*

Untuk melindungi telinga pengguna dari lonjakan volume mendadak (*hearing protection*):

### 8.1. Pemicu Kemunculan Pop-Up Alert (*Safety Check Modal*)
Dialog konfirmasi keamanan pendengaran **wajib muncul secara modal** (mengunci interaksi lain hingga tombol OK ditekan) pada dua kondisi:
1. **Setiap Kali Aplikasi Dibuka (*On Application Launch*):** Memastikan pengguna memeriksa level volume headphone/speaker sebelum memulai sesi dengar.
2. **Setelah Perangkat Audio Dipasang / Berganti (*On Audio Device Plug/Switch*):** Ketika sistem mendeteksi USB DAC eksternal baru dicolokkan atau perangkat output berganti di Windows.

### 8.2. Elemen Antarmuka Pop-Up Alert
* **Pesan Peringatan:**
  > 🛡️ **Peringatan Keamanan Pendengaran (Hearing Safety)**  
  > *Pastikan tingkat volume pada perangkat/DAC dan headphone Anda berada pada batas aman sebelum memulai pemutaran musik.*
* **Fitur Tes Audio Universal 10 Detik (*10s Universal Audio Test*):**
  * Di dalam pop-up alert disediakan tombol: **"▶ Tes Audio (10 Detik)"**.
  * Saat diklik, sistem memutar sampel audio uji berdurasi 10 detik dengan nada harmonik lembut (*gentle acoustic chime / calibrated pink noise*) yang dinormalisasi pada level aman (-18 dBFS).
  * Pengguna dapat mengatur slider volume langsung di dalam modal untuk memastikan suara nyaman di telinga.
  * Terdapat tombol **"⏹ Berhenti"** atau otomatis berhenti setelah 10 detik.
* **Tombol Konfirmasi Utama:** Tombol **"OK, Saya Sudah Memeriksa Volume"** yang menutup pop-up alert dan membuka akses penuh ke antarmuka aplikasi.

---

## 9. Ringkasan Tambahan Kontrak Backend ↔ Slint Frontend

### 9.1. State Tambahan (`AppState`)
* `library_folders: [FolderInfoStruct]` (id, path, display_name, is_enabled, track_count).
* `active_folder_filter: int` (-1 untuk *All Folders*, atau ID folder spesifik).
* `sort_criteria: int` (0: Title A-Z, 1: Artist, 2: Album, 3: TrackNo, 4: Duration, 5: DateAdded).
* `sort_ascending: bool`
* `shuffle_mode: int` (0: Reduced Repeat, 1: Normal, 2: Favffle).
* `loop_mode: int` (0: Off, 1: Loop All, 2: Loop One).
* `playlists_list: [PlaylistSummaryStruct]` (id, name, track_count, artwork_path).
* `lyrics_search_results: [LyricSearchHitStruct]` (track_id, line_index, timestamp_ms, snippet_text).
* `is_safety_modal_visible: bool` (status tampilnya modal peringatan keamanan audio).
* `is_testing_audio: bool` (status apakah tes audio 10 detik sedang aktif berbunyi).

### 9.2. Callbacks Tambahan (`LibraryCommands` / `SafetyCommands`)
* `on_add_folder()` -> Membuka dialog pemilih folder Windows.
* `on_toggle_folder(folder_id: int, is_enabled: bool)` -> Mengubah visibilitas & memutus lagu jika sedang diputar dari folder tersebut.
* `on_remove_folder(folder_id: int)`
* `on_set_folder_filter(folder_id: int)`
* `on_set_sorting(criteria: int, ascending: bool)`
* `on_set_shuffle_style(style_index: int)`
* `on_toggle_shuffle()`
* `on_cycle_loop_mode()`
* `on_create_playlist(name: string, custom_icon_path: string)`
* `on_export_playlist_m3u8(playlist_id: int, target_file_path: string)`
* `on_import_playlist_m3u8(file_path: string)`
* `on_search_query(query: string)`
* `on_confirm_safety_check()` -> Pengguna menekan tombol OK konfirmasi audio aman.
* `on_play_test_audio()` -> Memutar audio uji 10 detik pada level -18 dBFS.
* `on_stop_test_audio()` -> Menghentikan pemutaran audio uji.

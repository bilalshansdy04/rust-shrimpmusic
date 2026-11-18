# PRODUCT REQUIREMENTS DOCUMENT (PRD)

**Nama Proyek:** ShrimpMusic Native (v2.0)

**Platform:** Windows Desktop (Windows 10/11 x64)

**Arsitektur Utama:** Rust + Slint UI + WASAPI Exclusive Audio Engine

**Target Anggaran Memori (RAM):** < 100 MB (Target Ideal: 45 – 75 MB saat *playback* aktif)

---

> ### ⚠️ PERINGATAN KERAS & ATURAN KEPATUHAN ARSITEKTUR (WAJIB DIBACA)
> 
> 
> **DOKUMEN INI ADALAH SUMBER KEBENARAN TUNGGAL (*SINGLE SOURCE OF TRUTH*).**
> 1. **WAJIB BACA ULANG PRD & ADENDUM:** Setiap kali memulai sesi pengerjaan fitur baru, melakukan *refactoring*, atau memperbaiki *bug*, pengembang **WAJIB membaca ulang seluruh bagian dokumen PRD ini** dari awal hingga akhir, dilanjutkan membaca seluruh isi file adendum **`PRD-correction.md`** sebelum menulis atau mengubah satu baris kode pun.
> 2. **LARANGAN DEVIASI ARSITEKTUR:** Dilarang keras mengganti *tech stack*, menambahkan *dependency* pihak ketiga yang membengkakkan RAM (seperti WebView, Chromium, Electron, atau mesin peramban apa pun), atau mengubah batas kontrak antara *Backend* dan *Frontend* tanpa memperbarui dokumen ini terlebih dahulu.
> 3. **KONSISTENSI MUTLAK:** Seluruh penamaan struktur data, warna tema, perilaku *audio thread*, dan alur *state* antara Rust (*Backend*) dan Slint (*Frontend*) harus 100% konsisten dengan spesifikasi di bawah ini serta adendum `PRD-correction.md`. Pelanggaran terhadap batas konsumsi RAM (> 100 MB) atau kegagalan pelepasan *handle* USB DAC dianggap sebagai **kegagalan kritis (*Critical Bug*)**.
> 
> 

---

## 1. Ringkasan Eksekutif & Tujuan Produk

Membangun aplikasi pemutar musik lokal kelas audiophile untuk Windows yang menggabungkan efisiensi memori ekstrem ala MusicBee (< 100 MB RAM) dengan antarmuka pengguna (*UI/UX*) modern, interaktif, dan beranimasi halus. Aplikasi ini dirancang khusus untuk memutar koleksi audio beresolusi tinggi (M4A/AAC hingga FLAC *lossless*), membaca lirik tertanam (*embedded lyrics*), dan menyalurkan sinyal audio secara *bit-perfect* ke perangkat USB DAC eksternal melalui **WASAPI Exclusive Mode**.

---

## 2. Spesifikasi Tech Stack & Batasan Sistem (Strict Stack)

| Komponen | Teknologi / Crate | Versi / Konfigurasi Wajib | Alasan & Batasan Teknis |
| --- | --- | --- | --- |
| **Bahasa Pemrograman** | Rust | Edition 2021 / Stable terbaru | Manajemen memori deterministik tanpa *Garbage Collector*. |
| **UI Framework** | `slint` | Backend renderer: `femtovg` / `skia` (GPU) | Dikompilasi langsung ke *binary* native; mendukung animasi 60/120 FPS tanpa *browser engine*. |
| **Audio Decoding** | `symphonia` | Fitur: `flac`, `aac`, `mp3`, `alac`, `isomp4` | *Pure-Rust* decoder cepat tanpa ketergantungan FFmpeg eksternal yang berat. |
| **Audio Output (OS)** | `wasapi` crate | Windows Audio Session API (COM) | Kontrol penuh atas `AUDCLNT_SHAREMODE_EXCLUSIVE` dan *Event-Driven Mode*. |
| **Resampling (Fallback)** | `rubato` | FFT / Sinc resampler | Hanya aktif jika DAC menolak *sample rate* asli *file* saat mode eksklusif. |
| **Metadata & Lirik** | `lofty` | Dukungan: ID3v2, Vorbis, MP4 Atoms | Ekstraksi cepat tag `©lyr` (M4A), `LYRICS` (FLAC), dan *embedded artwork*. |
| **Database Lokal** | `rusqlite` | Mode: `bundled` + WAL Mode | Penyimpanan indeks pustaka, *path* lagu, dan *cache* metadata agar RAM tetap rendah. |
| **Pemrosesan Gambar** | `image` | Resize filter: `Lanczos3` / `Triangle` | Wajib mengecilkan *cover art* ke 250×250px sebelum dikirim ke memori UI. |
| **Integrasi Windows** | `windows` crate | WinRT `SystemMediaTransportControls` | Sinkronisasi kontrol media dengan Windows Quick Settings & tombol multimedia keyboard. |

---

## 3. Arsitektur Backend (Rust Core Engine)

*Backend* berjalan terpisah dari *UI Thread* dan dibagi menjadi 4 modul utama yang berkomunikasi menggunakan *lock-free ring buffer* dan *asynchronous channels* (`tokio` / `crossbeam-channel`).

### 3.1. Modul Audio Engine (`src/audio/`)

* **Dual-Mode Output:**
* **WASAPI Shared Mode (Default):** Menggunakan *Windows Audio Mixer* standar untuk pemutaran kasual.
* **WASAPI Exclusive Mode (Bit-Perfect USB Audio):** Mengambil alih perangkat DAC eksternal secara eksklusif (`AUDCLNT_SHAREMODE_EXCLUSIVE`) menggunakan *Event-Driven Buffer* untuk melewati proses *resampling* OS.


* **Hardware Format Handshake:**
1. Saat lagu dimuat, *engine* membaca *sample rate* (misal: 44.1kHz, 48kHz, 96kHz, 192kHz) dan *bit-depth* (16-bit, 24-bit, 32-bit) dari `symphonia`.
2. *Engine* memanggil `IAudioClient::IsFormatSupported` ke perangkat DAC terpilih.
3. Jika DAC mendukung format asli, data PCM dikirim mentah (*bit-perfect*). Jika tidak didukung, *engine* otomatis melakukan *bit-depth padding* atau mengaktifkan `rubato` *resampler* ke format terdekat yang didukung DAC.


* **Kontrol Volume DSP:** Karena volume Windows terkunci saat mode eksklusif, kontrol volume dilakukan di *audio thread* menggunakan kurva logaritmik: $\text{Gain} = (\text{Volume}_{0..1})^2$ yang dikalikan langsung ke sampel *floating-point* sebelum konversi ke format output DAC.
* **Aturan Pelepasan Perangkat (*Strict Release*):** Saat tombol **Stop** ditekan, perangkat output diganti, atau aplikasi ditutup (`Drop` trait), *audio thread* **wajib** langsung memanggil `IAudioClient::Stop()` dan melepaskan *handle* COM dalam waktu < 50ms agar perangkat DAC kembali tersedia untuk aplikasi Windows lainnya.

### 3.2. Modul Library Scanner & Metadata (`src/library/`)

* **Pemindaian Inkremental:** Memindai direktori musik pilihan pengguna di *background thread*. Hanya memproses ulang *file* jika `last_modified_timestamp` atau ukuran *file* berubah dibanding catatan di SQLite.
* **Prioritas Ekstraksi Lirik:**
1. Cek *embedded lyrics* di dalam tag *file* menggunakan `lofty` (tag `©lyr` untuk M4A/AAC, tag `LYRICS` atau `UNSYNCEDLYRICS` untuk FLAC).
2. Jika tidak ada, cari *file* `.lrc` eksternal dengan nama yang sama di folder lagu tersebut.
3. *Parser* LRC wajib mendukung format standar `[mm:ss.xx]` maupun `[mm:ss.xxx]` dan mengurutkan *timestamp* secara menaik.


* **Manajemen Memori Cover Art (Anti-Bloat):**
* Dilarang menyimpan gambar *cover art* resolusi penuh ke dalam SQLite atau memori UI untuk daftar pustaka.
* Saat pemindaian, *cover art* diekstrak, di-*resize* menjadi **250×250 piksel** (format WebP/JPEG terkompresi), dan disimpan di folder *cache* lokal (`%LOCALAPPDATA%/ShrimpMusic/thumbnails/{hash}.jpg`).
* Gambar resolusi asli hanya dimuat ke RAM untuk **1 lagu** yang sedang aktif diputar di *Acoustic Stage*.



### 3.3. Modul DSP & Equalizer (`src/dsp/`)

* **10-Band Parametric Equalizer:** Implementasi filter *Biquad IIR* (Peaking, Low-Shelf, High-Shelf) yang memproses sampel audio secara *real-time* sebelum dikirim ke *ring buffer* WASAPI.
* **Pre-Amp & Clipping Protection:** Penggeser *Pre-Amp* (-12 dB hingga +12 dB) dengan *soft-limiter* opsional untuk mencegah distorsi digital (*clipping*) saat frekuensi bass dinaikkan.
* **Gapless Playback:** *Decoder* wajib melakukan *pre-buffering* lagu berikutnya 3 detik sebelum lagu saat ini berakhir agar transisi antar-lagu tanpa jeda hening.

### 3.4. Modul Integrasi OS (`src/os/`)

* **Windows SMTC:** Mendaftarkan judul lagu, artis, album, *thumbnail* sampul, status *Play/Pause*, serta menerima perintah tombol media dari sistem operasi.
* **Single Instance Lock:** Menggunakan Windows Named Mutex agar membuka *file* musik dari Windows Explorer tidak membuka jendela aplikasi ganda, melainkan mengirim *path* lagu ke aplikasi yang sudah berjalan.

---

## 4. Arsitektur Frontend & Rencana UI/UX (`ui/`)

Antarmuka dibangun menggunakan bahasa deklaratif **Slint** dengan arsitektur tata letak **Asymmetric Dual-Pane Stage + Floating Pill Dock** (meninggalkan pola *sidebar* konvensional).

### 4.1. Struktur Tata Letak Utama (*Main Window Layout*)

* **Frameless Custom Window:** Menggunakan jendela tanpa *titlebar* bawaan Windows (`no-frame: true`), dilengkapi tombol kontrol jendela kustom (*Minimize, Maximize, Close*) di sudut kanan atas dan area *drag* jendela yang responsif.
* **1. Floating Pill Dock (Navigasi Melayang):**
* Posisi: Melayang di bagian atas-tengah jendela (*margin-top*: `16px`) dengan sudut membulat penuh (*border-radius*: `999px`).
* Isi: Tombol alih cepat untuk **Pustaka (*Library*)**, **Lirik (*Lyrics*)**, **Antrean (*Queue*)**, **Equalizer**, dan **Pengaturan (*Settings*)**, ditambah bilah pencarian cepat (*Quick Search*).
* Perilaku: Menyusut menjadi indikator ringkas saat kursor tidak berada di area navigasi selama 4 detik di mode *Immersive Lyrics*.


* **2. Sisi Kiri — The Acoustic Stage (Lebar: 40%):**
* **Interactive Album Card:** Menampilkan *cover art* lagu aktif dengan rasio 1:1, bayangan lembut, dan efek kemiringan halus (*subtle tilt*) saat disorot kursor.
* **Hardware Audio Badge:** Lencana status teknis di bawah *cover art* yang menampilkan format file, *bit-depth*, *sample rate*, dan status perangkat keras (Contoh: `FLAC • 24-BIT / 96kHz • WASAPI EXCLUSIVE` dengan titik indikator LED berdenyut pelan saat aktif).
* **Waveform Seekbar:** Visualisasi bentuk gelombang audio asli dari lagu yang sedang diputar. Mengarahkan kursor menampilkan *tooltip* waktu presisi; menggeser (*scrubbing*) menggunakan animasi *spring damping*.
* **Transport & Precision Volume:** Tombol kontrol pemutaran (*Shuffle, Prev, Play/Pause, Next, Repeat*) dan *slider* volume presisi dengan indikator angka desibel/persentase.


* **3. Sisi Kanan — Dynamic Canvas (Lebar: 60%):**
* Area konten utama yang berpindah antar-tampilan menggunakan animasi transisi *slide + fade* (`220ms`, *cubic-bezier*):
* **Tampilan A — Kinetic Lyrics View:** Menampilkan lirik tersinkronisasi. Baris aktif membesar (`1.15x` *scale*), menggunakan warna aksen utama dengan ketebalan *Bold*, dan otomatis bergulir halus ke tengah layar. Klik pada baris lirik mana pun akan langsung melompatkan durasi lagu (*seek-to-lyric*).
* **Tampilan B — Virtualized Library View:** Menampilkan daftar lagu/album menggunakan komponen `ListView` tervirtualisasi (hanya merender item yang tampak di *viewport* layar demi efisiensi RAM). Dilengkapi pengurutan berdasarkan Judul, Artis, Album, Format, dan Tanggal Ditambahkan.
* **Tampilan C — Queue & Up Next:** Daftar antrean pemutaran yang mendukung pengaturan ulang urutan (*reorder*).




* **4. Slide-Over Drawer (Settings & Equalizer):**
* Panel pengaturan dan *Equalizer* muncul meluncur dari sisi kanan layar di atas *Dynamic Canvas* tanpa menghentikan tampilan *Acoustic Stage* di sisi kiri.



### 4.2. Sistem Tema & Palet Warna Dinamis (`ui/theme.slint`)

Seluruh komponen UI wajib merujuk pada objek global `ThemePalette` (dilarang menulis kode warna *hardcoded* di dalam komponen):

| Token Warna | Mode Dark (OLED Pitch Black) | Mode Light (Soft Daylight) | Fungsi Penggunaan |
| --- | --- | --- | --- |
| `bg-base` | `#000000` | `#F5F5F7` | Latar belakang utama jendela aplikasi. |
| `bg-surface` | `#0E0E11` | `#FFFFFF` | Latar kartu album, panel daftar lagu, dan *drawer*. |
| `bg-elevated` | `#18181D` | `#EAEAEF` | Latar *Floating Pill Dock*, *hover state*, dan *tooltip*. |
| `border-subtle` | `#24242B` | `#DCDCE2` | Garis tepi `1px` pemisah elemen agar tetap tajam. |
| `text-primary` | `#F4F4F6` | `#121215` | Judul lagu, lirik baris aktif, dan teks utama. |
| `text-secondary` | `#8E8E98` | `#686873` | Nama artis, album, lirik tidak aktif, dan label info. |
| `accent-primary` | *Dinamis (Default: `#FF5A36`)* | *Dinamis (Default: `#E64825`)* | Sorotan lirik aktif, *waveform progress*, dan tombol utama. |

* **3 Mode Kustomisasi Palet Aksen:**
1. **Preset Palettes:** Pilihan cepat 1-klik (*Coral Orange*, *Cyber Mint*, *Electric Violet*, *Crimson Red*, *Amber Gold*, *Monochrome*).
2. **Adaptive Chameleon Mode:** *Backend* Rust mengekstrak warna dominan dari *cover art* lagu yang sedang diputar (menggunakan algoritma *color quantization* sederhana) dan mengirimkan kode warnanya ke `ThemePalette.accent-primary` dengan transisi gradasi halus selama `400ms`.
3. **Custom Hex Color Picker:** Pengguna dapat memasukkan kode Hex kustom untuk warna aksen serta menyimpan palet buatan sendiri ke konfigurasi lokal.



### 4.3. Daftar Lengkap Pengaturan Aplikasi (*Settings Specifications*)

Menu pengaturan wajib mencakup 4 kategori berikut:

1. **Audio & Output:**
* Pemilihan perangkat output (*Output Device Dropdown*: Default Windows, USB DAC spesifik).
* Sakelar **WASAPI Exclusive Mode** (On/Off) + opsi *Event-Driven Mode*.
* Pilihan *Resampler Quality* (jika perangkat tidak mendukung *sample rate* asli).
* Pengaturan *Buffer Size* (dalam milidetik, misal: `20ms` – `100ms`).
* *Gapless Playback* (On/Off) & *ReplayGain* (Track / Album / Off).


2. **Library & Metadata:**
* Manajemen folder musik (*Add Folder*, *Remove Folder*, tombol *Rescan Library*).
* Prioritas sumber lirik (*Embedded Tags First* vs *External .lrc File First*).
* Tombol bersihkan *cache* gambar (*Clear Thumbnail Cache*).


3. **Appearance & Theme:**
* Sakelar instan **Dark Mode (OLED)** / **Light Mode**.
* Pilihan mode aksen (*Preset*, *Adaptive Cover Art*, *Custom Hex*).
* Pengaturan intensitas animasi (*Full Animations*, *Reduced Motion*, *Off*).
* Pilihan ukuran font lirik (*Compact*, *Normal*, *Large*).


4. **System & Shortcuts:**
* Integrasi *Windows Media Controls (SMTC)* (On/Off).
* Perilaku tombol *Close* (*Exit Application* vs *Minimize to System Tray*).
* Daftar pintasan keyboard (*Global & Local Hotkeys*: `Space` Play/Pause, `Ctrl+F` Search, `Ctrl+E` Toggle Exclusive Mode, `Arrow Left/Right` Seek 5 detik).



---

## 5. Kontrak Komunikasi Backend ↔ Frontend (State & Callbacks)

Untuk mencegah kondisi balapan (*race condition*) dan menjaga konsistensi antara Rust dan Slint, komunikasi dibatasi secara ketat melalui jembatan properti dan *callback* berikut:

### 5.1. State dari Rust ke Slint (`AppState` Global)

* `is_playing: bool`
* `current_track: TrackInfoStruct` (judul, artis, album, format, *sample_rate*, *bit_depth*, durasi total, *image* *cover art* aktif).
* `position_ms: int` (diperbarui setiap `50ms` selama pemutaran berlangsung untuk animasi *seekbar* dan lirik yang mulus).
* `waveform_peaks: [float]` (array berisi `120` titik amplitudo ternormalisasi `0.0..1.0` untuk menggambar *waveform seekbar*).
* `lyrics_lines: [LyricLineStruct]` (daftar baris lirik beserta `timestamp_ms` dan `text`).
* `active_lyric_index: int` (indeks baris lirik yang sedang aktif berdasarkan `position_ms`).
* `is_exclusive_active: bool` (indikator status keberhasilan penguncian WASAPI Exclusive).
* `active_device_name: string`

### 5.2. Callbacks dari Slint ke Rust (`AudioCommands` Global)

* `on_play_pause()`
* `on_seek_to(target_ms: int)`
* `on_select_track(track_id: int)`
* `on_set_volume(volume: float)`
* `on_toggle_exclusive_mode(enabled: bool)`
* `on_select_output_device(device_id: string)`
* `on_update_eq_band(band_index: int, gain_db: float)`
* `on_theme_changed(is_dark: bool, accent_hex: string, adaptive_mode: bool)`

---

## 6. Standar Pengujian & Kriteria Kelulusan (*Definition of Done*)

Sebuah fitur atau rilis tidak dianggap selesai sebelum lulus pengujian metrik berikut pada Windows 10/11:

1. **Uji Konsumsi Memori (RAM):** Penggunaan *Working Set* di Windows Task Manager tidak boleh melebihi **85 MB** saat memutar *file* FLAC 24-bit/96kHz dengan tampilan lirik aktif dan pustaka berisi 1.000 lagu.
2. **Uji Stabilitas USB DAC:** Mengaktifkan dan menonaktifkan *WASAPI Exclusive Mode* berulang kali saat lagu berjalan tidak boleh menyebabkan aplikasi *crash* atau membuat perangkat audio Windows terkunci setelah aplikasi ditutup.
3. **Uji Akurasi Lirik:** *Embedded lyrics* dari *file* `.m4a` (AAC) dan `.flac` terbaca tanpa karakter rusak (UTF-8 penuh untuk lirik berbahasa Indonesia, Inggris, Jepang, dan Korea) dengan sinkronisasi meleset tidak lebih dari `±30ms`.
4. **Uji Responsivitas UI:** Pergantian antara *OLED Dark Mode* dan *Light Mode*, perubahan warna palet, serta pengguliran lirik berjalan konsisten di **60 FPS** tanpa *frame drop*.

---

## 7. Tahapan Eksekusi Pengembangan (*Milestones*)

* **Fase 1 — Core Audio & WASAPI Engine (CLI/Headless):** Implementasi dekoder `symphonia`, pemutar `wasapi` (Shared & Exclusive), kontrol volume DSP, dan verifikasi *output bit-perfect* pada DAC eksternal.
* **Fase 2 — Library Scanner, SQLite & Lofty Parser:** Implementasi pemindaian folder, ekstraksi tag FLAC/M4A + *embedded lyrics*, serta *pipeline* pembuat *thumbnail cover art* 250×250px.
* **Fase 3 — Slint UI Shell & Theme Engine:** Membangun *Dual-Pane Stage*, *Floating Pill Dock*, *ThemePalette* (Dark/Light + Adaptive Color), dan menyambungkan kontrak `AppState` dengan *Backend* Rust.
* **Fase 4 — Interaktivitas Lanjut & DSP:** Implementasi *Waveform Seekbar*, *Kinetic Lyrics* (*click-to-seek*), 10-Band Equalizer, Windows SMTC, dan optimasi profil memori akhir.

---

> ### 🔒 PENGINGAT AKHIR UNTUK PENGEMBANG / AI ASSISTANT
> 
> 
> **STOP DAN PERIKSA KEMBALI:** Sebelum menghasilkan kode untuk Fase 1 maupun fase berikutnya, pastikan setiap nama *crate*, struktur `AppState`, aturan *downscaling* gambar 250×250px, dan mekanisme pelepasan *handle* WASAPI mematuhi dokumen PRD ini tanpa pengecualian. Jika ditemukan kebutuhan perubahan teknis di tengah pengerjaan, **perbarui dokumen PRD ini terlebih dahulu** sebelum mengubah kode program.
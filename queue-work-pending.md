# Antrean Pekerjaan Tertunda (Queue Work Pending)
# Proyek: ShrimpMusic Native (v2.0)

Dokumen ini mencatat seluruh daftar temuan, evaluasi UX/UI, dan perbaikan fitur yang telah diselesaikan maupun yang sedang berjalan.

---

## 📋 Antrean Aktif

*(Semua antrean pekerjaan saat ini telah selesai dikerjakan!)*

---

## 🛠️ Catatan Riwayat Perbaikan (Selesai)

### [DONE-26 / UDAH] Perbaikan System Tray Icon (Embedded Binary Asset) & Penambahan Filter Urutan Lagu (Nama, Artis, Album, Tanggal & Asc/Desc)
- **Udah diperbaiki tanggal 11/23/2026**
- **Detail Implementasi:**
  1. **Perbaikan Masalah System Tray Icon:**
     - **Penyebab:** Sebelumnya pemuatan icon menggunakan path relatif disk (`assets/logo_small.png`). Ketika aplikasi dieksekusi dari target build, folder lain, atau saat di-install/di-pin, path relatif tersebut menghasilkan error file tidak ditemukan (*file not found*), menyebabkan inisialisasi tray gagal diam-diam (*silent fail*) dan window tidak masuk ke system tray.
     - **Solusi:** Menggunakan macro bawaan Rust `include_bytes!("../../assets/logo_small.png")` di [src/ui/tray.rs](file:///d:/Projects/music/rust/src/ui/tray.rs) sehingga aset gambar logo ter-embed 100% langsung di dalam file biner executable `.exe`. System tray kini dijamin selalu aktif dan muncul di taskbar notification area terlepas dari lokasi direktori kerja aplikasi.
  2. **Penambahan Filter Pengurutan Lagu di Pustaka ([ui/views/library.slint](file:///d:/Projects/music/rust/ui/views/library.slint)):**
     - Menghadirkan bar kontrol pengurutan elegan di atas daftar lagu dengan 4 kriteria utama:
       - **Nama (Judul Lagu):** Mengurutkan lagu berdasarkan abjad judul lagu.
       - **Artis:** Mengurutkan berdasarkan nama artis penyanyi.
       - **Album:** Mengurutkan berdasarkan nama album.
       - **Tanggal Ditambahkan:** Mengurutkan berdasarkan waktu file ditambahkan/dimodifikasi.
     - Menyediakan tombol pembalik arah urutan interaktif: **Menaik (*Ascending* / A-Z / Terlama)** dan **Menurun (*Descending* / Z-A / Terbaru)**.
     - Mengintegrasikan callback `on_sort_changed(crit, asc)` di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) yang secara instan menyaring dan menyusun ulang lagu di SQLite serta menyimpan preferensi pengguna (`sort_criteria` dan `sort_ascending`) secara permanen.
- **Hasil Pengujian:** Seluruh 19 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build rilis sukses 0 error.

---

### [DONE-25 / UDAH] Perombakan Antrean Pemutar (Queue View Interactivity, Swipe-to-Remove, Move Up/Down, Now Playing Pin, & Reshuffle)
- **Udah diperbaiki tanggal 11/23/2026**
- **Detail Implementasi:**
  1. **Now Playing Selalu di Paling Atas (*Pinned Top*):**
     - Lagu yang sedang aktif diputar disematkan di posisi teratas antrean pemutar pada kartu khusus yang menonjol dengan bingkai oranye bercahaya (`border-color: ThemePalette.accent_primary`), animasi/indikator icon play/pause, judul lagu tebal, artis, album, format audio, lencana Hi-Res, dan durasi berjalan.
  2. **Interaktivitas Geser & Hapus Cepat (*Swipe-to-Remove*):**
     - Setiap item lagu di bagian *Antrean Berikutnya* dapat di-swipe / digeser ke kiri maupun ke kanan secara interaktif menggunakan mouse/touch gesture.
     - Saat di-drag melebihi ambang batas (90px), latar belakang kartu berubah merah dengan ikon tong sampah (`IconSet.Trash2`) dan item seketika terhapus dari antrean pemutaran.
     - Disediakan pula tombol hapus cepat (`X`) di sisi kanan setiap baris antrean.
  3. **Pengaturan Urutan Lagu (*Move Up / Move Down*):**
     - Disediakan tombol panah atas (`ChevronUp`) dan panah bawah (`ChevronDown`) di sisi kanan setiap baris lagu untuk menata ulang urutan lagu berikutnya secara mudah dan presisi.
     - Mengimplementasikan method `move_item(from, to)` dan `remove_at(index)` pada [src/audio/queue.rs](file:///d:/Projects/music/rust/src/audio/queue.rs) dengan penyesuaian pointer aktif `current_index` secara aman.
  4. **Alur Urutan Lagu Berikutnya & Tombol Acak Ulang (*Reshuffle Queue*):**
     - Mendukung alur antrean normal (berurutan) dan mode shuffle aktif.
     - Menghadirkan tombol aksi **`[ 🔀 Acak Ulang Antrean ]`** di header atas antrean.
     - Mengimplementasikan method `reshuffle_upcoming()` pada [src/audio/queue.rs](file:///d:/Projects/music/rust/src/audio/queue.rs): mengocok ulang seluruh lagu antrean berikutnya secara acak tanpa menghentikan atau mengubah lagu yang saat ini sedang diputar (lagu aktif tetap disematkan di posisi indeks teratas).
- **Hasil Pengujian:** Seluruh 19 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build rilis sukses 0 error.

---

## 🛠️ Catatan Riwayat Perbaikan (Selesai)

### [DONE-24 / UDAH] Pemantauan Real-Time Pasang/Cabut (Hotplug) USB DAC & Type-C Converter
- **Udah diperbaiki tanggal 11/23/2026**
- **Detail Implementasi:**
  - Menambahkan modul background worker thread [src/audio/device_watcher.rs](file:///d:/Projects/music/rust/src/audio/device_watcher.rs) (`DeviceWatcher`) yang secara periodik memantau perubahan perangkat audio Windows secara real-time tanpa mengganggu thread UI utama.
  - Mengintegrasikan sinyal perubahan perangkat audio ke event loop utama di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs):
    - Ketika USB DAC atau Converter Type-C dicolokkan ke laptop/PC saat aplikasi sedang terbuka, daftar perangkat di menu Pengaturan seketika diperbarui tanpa perlu restart aplikasi.
    - Status sakelar Bit-Perfect langsung terbuka (enabled) dan dapat diaktifkan.
    - Sebaliknya, jika DAC dicabut, sakelar otomatis terkunci kembali ke mode proteksi aman.
- **Hasil Pengujian:** 19/19 pengujian unit dan integrasi lulus 100%.

---

### [DONE-23 / UDAH] Perilaku Tombol Close Jendela: Minimize / Hide ke Windows System Tray
- **Udah diperbaiki tanggal 11/23/2026**
- **Detail Implementasi:**
  - Menambahkan modul [src/ui/tray.rs](file:///d:/Projects/music/rust/src/ui/tray.rs) menggunakan pustaka `tray-icon` dan `muda`.
  - Mengubah penanganan tombol close (X) di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) agar memanggil `win.set_visible(false)`: jendela tersembunyi secara bersih dari layar dan Taskbar Windows, dan aplikasi tetap berjalan hemat di latar belakang.
  - Menyediakan tray icon dengan menu interaktif: "Buka ShrimpMusic", "Putar / Jeda", dan "Keluar dari Aplikasi".
  - Klik ganda atau klik kiri pada tray icon seketika memunculkan kembali jendela aplikasi (`win.set_visible(true)` + `focus_window()`).
- **Hasil Pengujian:** 19/19 pengujian unit dan integrasi lulus 100%.

---

### [DONE-22 / UDAH] Validasi Hardware USB Audio / DAC untuk Bit-Perfect Mode & Deteksi Type-C Converter
- **Udah diperbaiki tanggal 11/23/2026**
- **Detail Implementasi:**
  - Menambahkan `DeviceClassification` dan deteksi perangkat cerdas pada [src/audio/device.rs](file:///d:/Projects/music/rust/src/audio/device.rs) yang mengenali:
    - Dedicated USB DAC (FiiO, Topping, iFi, Moondrop, dll.).
    - Type-C to 3.5mm Converter dengan chip DAC aktif (JadeAudio JA11, Conexant CX31993, Realtek ALC5686, CS43131, dll.).
    - Converter analog pasif / onboard jack, speaker internal, Bluetooth, dan virtual audio (FxSound, Voicemeeter).
  - Melindungi sakelar WASAPI Exclusive Mode di [ui/views/settings.slint](file:///d:/Projects/music/rust/ui/views/settings.slint) dan [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs):
    - Jika perangkat yang dipilih bukan USB DAC / Type-C DAC, sakelar Exclusive dinonaktifkan (disabled / opacity redup) dengan pesan peringatan: *"Bit-Perfect nonaktif: Hanya didukung pada USB DAC / Type-C DAC eksternal"*.
    - Menampilkan lencana hardware (`USB DAC`, `Type-C DAC`, `Speaker`, `Virtual`, dll.) pada setiap baris perangkat di menu Pengaturan.
    - Mengintegrasikan pemilihan perangkat audio (`on_select_device`) secara real-time via `player.set_device(...)`.
- **Hasil Pengujian:** Seluruh pengujian klasifikasi perangkat dan fungsionalitas audio lulus 100%.

---

### [DONE-21] Pencegahan Glitch Audio ("Dretttt") pada WASAPI Exclusive Mode saat Pause
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Menambahkan method `pause()` dan `resume()` pada `WasapiStream` di [src/audio/wasapi_engine.rs](file:///d:/Projects/music/rust/src/audio/wasapi_engine.rs) yang secara eksplisit memanggil `IAudioClient::Stop()` dan `IAudioClient::Start()`.
  - Mengintegrasikan pemanggilan tersebut ke `AudioCommand::Pause` dan `AudioCommand::Resume` di [src/audio/player.rs](file:///d:/Projects/music/rust/src/audio/player.rs).
  - Menghentikan perputaran cyclic DMA ring buffer hardware USB DAC sehingga audio langsung senyap seketika saat di-pause tanpa suara looping glitch/dengung.
- **Hasil Pengujian:** Seluruh 18 tes lulus 100%, commit `43fefbd`.

---

### [DONE-20] Optimasi Jejak Memori RAM Rendah (< 50 MB, Target PRD < 100 MB)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Men-downscale gambar logo resolusi raksasa (3135x3448px) menjadi 128x128px (17 KB) di [assets/logo_small.png](file:///d:/Projects/music/rust/assets/logo_small.png), memangkas puluhan megabyte alokasi uncompressed bitmap RAM.
  - Menjadikan renderer ringan Slint (`software`) sebagai default di [src/main.rs](file:///d:/Projects/music/rust/src/main.rs), menghindari alokasi VRAM/OpenGL driver Windows yang boros (80-120 MB).
  - Menambahkan batas memory cache SQLite (`PRAGMA cache_size = -4000`) di [src/db/schema.rs](file:///d:/Projects/music/rust/src/db/schema.rs).
  - Mengaktifkan LTO dan optimasi release di [Cargo.toml](file:///d:/Projects/music/rust/Cargo.toml).
- **Hasil Pengujian:** Penggunaan RAM terpantau turun drastis dari 186 MB menjadi **46.35 MB** (bahkan mencapai ~14 MB saat idle di background), memenuhi target PRD < 100 MB. Commit `acd6227`.

---

### [DONE-19] Pembersihan Slider Ganda Seekbar, Pemilihan Folder Scope, Fitur Playlist Lengkap (dengan Rename & Validasi Hapus), dan Pencarian Real-Time
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Pembersihan Seekbar Menjadi Murni Spectrum Audio ([ui/components/seekbar.slint](file:///d:/Projects/music/rust/ui/components/seekbar.slint)):**
    - Menghapus komponen slider sekunder di bawah visualisasi gelombang spektrum sesuai permintaan pengguna.
    - Menjadikan 120-bar visualisasi spektrum audio setinggi 28px sebagai scrubber interaktif tunggal yang elegan: mendukung klik langsung, drag geser (*scrubbing*) dengan umpan balik warna oranye real-time, jarum penunjuk (*playhead needle*) putih halus, dan putaran roda mouse (±2%).
    - Menempatkan indikator durasi (`1:39` dan `2:35`) tepat di bawah spektrum dengan tipografi bersih.
  - **Pengaktifan Pemilihan Cakupan Folder (*Folder Scope Selector*):**
    - Mengintegrasikan handler `on_scope_changed` di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs).
    - Mengklik pill "Semua Folder" atau nama folder tertentu (misal "Lossless Music") kini langsung menyorot pill tersebut dengan warna oranye dan seketika menyaring daftar lagu yang ditampilkan di Pustaka.
  - **Fitur Playlist Lengkap & Terintegrasi ([ui/views/playlist.slint](file:///d:/Projects/music/rust/ui/views/playlist.slint)):**
    - Menambahkan tab **`[ Playlist ]`** pada dock navigasi utama [ui/components/pill_dock.slint](file:///d:/Projects/music/rust/ui/components/pill_dock.slint).
    - Membangun antarmuka `PlaylistView` lengkap:
      - Daftar kartu playlist tersimpan dengan jumlah lagu dan deskripsi.
      - Dialog/Modal pembuatan playlist baru (**`+ Buat Playlist`**) dengan input nama playlist.
      - Dialog/Modal ubah nama playlist (**`Ubah Nama Playlist`**) dengan tombol edit pada kartu maupun header detail playlist.
      - Dialog/Modal konfirmasi hapus playlist (**`Hapus Playlist?`**) dengan validasi pencegahan ketidaksengajaan (tombol Batal & Hapus Playlist merah).
      - Tampilan detail isi lagu di dalam playlist terpilih dengan tombol navigasi kembali (*Back*).
      - Tombol aksi pemutaran seluruh lagu di playlist (**`Putar Semua`**).
      - Fitur ekspor playlist ke standar universal UTF-8 Extended M3U8 (**`Ekspor .m3u8`**) dan impor (**`Impor M3U8`**) via dialog file native Windows.
      - Tombol hapus lagu dari playlist dan hapus playlist.
  - **Pengaktifan Pencarian Cepat Real-Time (*Quick Search*):**
    - Mengintegrasikan callback `on_search_changed` di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) yang terhubung ke mesin pencari SQLite FTS `SearchEngine::search`.
    - Mengetik judul lagu, nama artis, album, atau lirik pada kolom pencarian top dock langsung memfilter lagu di antarmuka Pustaka secara real-time.
- **Hasil Pengujian:** Seluruh 18 pengujian lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-18] Perbaikan Konflik COM Concurrency (`OleInitialize failed: RPC_E_CHANGED_MODE`)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Identifikasi Masalah:**
    - Pada saat startup, pemanggilan `CoInitializeEx(None, COINIT_MULTITHREADED)` untuk modul volume Windows sebelumnya sempat dipanggil di thread utama (`thread 'main'`). Hal ini menyebabkan `winit` panik saat memanggil `OleInitialize()` karena thread GUI utama Windows mewajibkan model *Single-Threaded Apartment* (STA) untuk dukungan OLE dan *drag-and-drop*.
  - **Solusi Arsitektur `WindowsVolumeController` Terisolasi:**
    - Merancang ulang modul [src/audio/windows_volume.rs](file:///d:/Projects/music/rust/src/audio/windows_volume.rs) menggunakan worker thread terpisah (`windows-volume-watcher`).
    - Inisialisasi COM MTA diisolasi 100% hanya di dalam background thread tersebut.
    - Sinkronisasi nilai volume ke thread GUI utama dilakukan secara *lock-free* menggunakan `Arc<AtomicU32>`, sehingga pembacaan volume di UI thread tidak memerlukan panggilan COM sama sekali (0 ms / 0 overhead) dan OLE winit pada thread utama berjalan lancar tanpa error `RPC_E_CHANGED_MODE`.
- **Hasil Pengujian:** Seluruh 18 pengujian lulus 100%, verifikasi peluncuran proses biner membuktikan aplikasi berjalan normal tanpa panic.

---

### [DONE-17] Integrasi Sinkronisasi Dua Arah Volume Master Windows (Fn+F2 / Fn+F3 & OSD) pada Dialog Keamanan Pendengaran
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Integrasi Windows Core Audio Endpoint API (`IAudioEndpointVolume`):**
    - Mengaktifkan fitur `Win32_Media_Audio` dan `Win32_Media_Audio_Endpoints` pada [Cargo.toml](file:///d:/Projects/music/rust/Cargo.toml).
    - Membangun modul native [src/audio/windows_volume.rs](file:///d:/Projects/music/rust/src/audio/windows_volume.rs) yang mengakses `IMMDeviceEnumerator` dan `IAudioEndpointVolume` untuk membaca volume master sistem Windows (`GetMasterVolumeLevelScalar`) dan mengaturnya (`SetMasterVolumeLevelScalar`).
  - **Sinkronisasi Dua Arah (*Bi-Directional Real-Time Sync*):**
    - Menambahkan properti `windows_master_volume` pada [ui/appwindow.slint](file:///d:/Projects/music/rust/ui/appwindow.slint) dan mengikatnya ke `test_volume <=> root.windows_master_volume;` pada `HearingSafetyModal`.
    - Mengintegrasikan pemantauan kontinu 50ms pada `self._timer` di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs): setiap kali pengguna menekan tombol Fn+F2 (Volume Down) atau Fn+F3 (Volume Up) pada keyboard laptop atau mengubah volume via flyout Windows, nilai volume dan posisi slider pada dialog keamanan langsung bergerak sinkron secara real-time (misal `12%` sesuai OSD Windows).
    - Saat slider pada dialog keamanan di-drag oleh mouse, callback `on_set_test_volume` langsung mengatur volume master Windows ke nilai tersebut sehingga kekuatan suara nada uji 10 detik langsung membesar/mengecil secara akustik.
    - Menambahkan auto-reset pada status tombol "Tes Audio (10d)" saat pemutaran nada uji selesai.
  - **Penyelarasan Teks Label:**
    - Memperbarui label indikator di [ui/dialogs/safety.slint](file:///d:/Projects/music/rust/ui/dialogs/safety.slint) menjadi `Volume Master Windows (Fn+F2/F3):` sehingga pengguna memahami bahwa slider ini memvalidasi volume sistem Windows sebelum memulai musik.
- **Hasil Pengujian:** Seluruh 18 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), verifikasi pembacaan volume Windows menghasilkan kecocokan tepat (misal `0.11999999` atau 12%), build biner sukses tanpa error.

---

### [DONE-16] Perbaikan Pelacakan Drag Slider Material 3 & Sinkronisasi Teks Persentase Dialog Keamanan
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Sinkronisasi Reaktif Real-Time Persentase Volume Uji (`HearingSafetyModal`):**
    - Mengubah properti `test_volume` pada [ui/dialogs/safety.slint](file:///d:/Projects/music/rust/ui/dialogs/safety.slint) menjadi `in-out property <float> test_volume` yang terikat dua arah (*two-way binding*) `test_volume <=> root.volume` pada [ui/appwindow.slint](file:///d:/Projects/music/rust/ui/appwindow.slint).
    - Memperbarui callback `value_changed` dan `released` agar langsung mengupdate `root.test_volume = val / 100.0` secara lokal di UI thread sehingga label teks persentase `Math.round(root.test_volume * 100) + "%"` langsung berubah mengikuti geseran mouse secara instan tanpa beku di 100%.
    - Menambahkan update `w.set_volume(v)` di [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) pada handler `on_on_set_test_volume`.
  - **Perbaikan Pelacakan Koordinat Drag pada `@material` Slider:**
    - Memperbaiki komponen `Slider` di [material-1.1.0/ui/components/slider.slint](file:///d:/Projects/music/rust/material-1.1.0/ui/components/slider.slint): sebelumnya `touch_area` utama tidak memiliki handler `moved =>`, sehingga jika pengguna mengklik dan menggeser di sepanjang track slider, slider tidak merespons drag.
    - Menambahkan handler `moved => { if self.pressed { root.set_value(root.length_to_value(self.mouse_x, track.width)); } }` pada `touch_area`.
    - Memperbaiki `thumb_area.moved` dengan mengikat ke `touch_area.mouse_x` alih-alih delta relatif `self.mouse_x - self.pressed_x` yang sebelumnya menyebabkan jitter/loncatan saat `state_layer.x` bergeser.
    - Menyelaraskan koordinat horizontal pop-up volume vertikal panggung kiri di [ui/views/stage.slint](file:///d:/Projects/music/rust/ui/views/stage.slint) agar presisi tepat di atas tombol volume player.
- **Hasil Pengujian:** Seluruh 17 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-15] Pop-up Slider Volume Vertikal Kompak & Rentang Penuh 0% - 100%
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Koreksi Rentang Volume Penuh (0% - 100%):**
    - Memperbaiki sinkronisasi callback `on_on_set_volume` pada [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) dengan menambahkan `w.set_volume(v)` sehingga nilai volume GUI selalu terikat dua arah (*two-way binding*) dan tidak terpotong di 59%.
    - Menghitung rasio vertikal berbasis `1.0 - (mouse_y / height)` yang di-clamp penuh dari 0.0 (0%) hingga 1.0 (100%).
  - **Desain Vertikal Ramping, Elegan & Kompak:**
    - Merombak pop-up volume di [ui/views/stage.slint](file:///d:/Projects/music/rust/ui/views/stage.slint) menjadi kartu vertikal ramping (lebar 48px, tinggi 156px) yang melayang presisi tepat di atas tombol volume.
    - Struktur tersusun dari atas ke bawah:
      1. Teks persentase volume dinamis (`100%`, `75%`, `0%`) berwarna aksen oranye.
      2. Jalur fader vertikal 94px dengan isian oranye dari bawah ke atas dan knob bulat putih 16px yang nyaman di-drag.
      3. Tombol ikon quick-mute/unmute (0% ↔ 70%).
    - Menambahkan dukungan scroll roda mouse pada kartu dan tombol volume serta dismiss saat klik di luar pop-up.
- **Hasil Pengujian:** Seluruh 17 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-14] Perbaikan Slider Volume Player (Hit-Testing & Integrasi Material 3 Slider)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Identifikasi Root Cause Slider Volume Macet di 3%:**
    - Sebelumnya kartu pop-up volume berada di dalam elemen tombol volume selebar 40x40px dengan koordinat negatif `y: -160px`. Pada Slint, koordinat di luar batas kontainer induk (*parent bounding box*) diabaikan/di-cull dari *hit-testing*, sehingga pergerakan kursor mouse ke atas terpotong dan macet di 3%.
  - **Penaikan Arsitektur Pop-up ke AcousticStage & Material 3 Slider:**
    - Memindahkan kontainer kartu pop-up volume ke root layer [ui/views/stage.slint](file:///d:/Projects/music/rust/ui/views/stage.slint) yang mencakup 100% tinggi panggung kiri, sehingga area sentuh (*hit-target*) bebas hambatan 100%.
    - Mengintegrasikan komponen resmi Material 3 `Slider` dari `@material` di dalam kartu pop-up volume:
      - Mendukung geser/drag mulus (*touch/mouse grab*).
      - Dilengkapi tombol ikon quick-mute/unmute di samping slider dan teks persentase volume real-time.
      - Mendukung scroll roda mouse di atas tombol volume maupun di atas slider.
      - Menutup otomatis saat area luar kartu diklik (*backdrop dismiss*).
- **Hasil Pengujian:** Seluruh 17 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-13] Integrasi Resmi Material Design 3 Slider (`@material`) dari material.slint.dev
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Penyediaan Pustaka `@material` (v1.1.0):**
    - Mengintegrasikan paket resmi pustaka komponen Material Slint ke dalam proyek pada direktori `material-1.1.0/`.
    - Mendaftarkan library path `"material"` pada [build.rs](file:///d:/Projects/music/rust/build.rs) dan [slint-project.json](file:///d:/Projects/music/rust/slint-project.json) sehingga seluruh file `.slint` dapat langsung mengimpor komponen Material (`import { Slider } from "@material";`).
    - Menyesuaikan tema `MaterialPalette` ke warna aksen oranye khas ShrimpMusic (`#FF5A36`).
  - **Integrasi Material 3 Slider pada Waveform & Seekbar:**
    - Merombak [ui/components/seekbar.slint](file:///d:/Projects/music/rust/ui/components/seekbar.slint) dengan menyematkan komponen `Slider` resmi dari `@material`.
    - Slider Material ini mendukung touch/drag scrubbing dengan mouse grab instan, pelacakan delta pergeseran tanpa stuttering, serta callback `value_changed` dan `released`.
    - Mengharmonisasikan visualisasi gelombang audio (120 waveform bars) di atas Material Slider, yang ikut menyala terang secara real-time mengikuti pergeseran thumb slider saat di-drag.
    - Menjaga interaktivitas klik langsung pada waveform dan dukungan keyboard panah kiri/kanan serta scroll roda mouse.
  - **Integrasi Material 3 Slider pada Dialog Uji Volume:**
    - Memperbarui [ui/dialogs/safety.slint](file:///d:/Projects/music/rust/ui/dialogs/safety.slint) dengan memanfaatkan `@material` Slider untuk slider uji nada volume -18 dBFS.
- **Hasil Pengujian:** Seluruh 17 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-12] Native Window Title Bar Dragging, Waveform Scrubbing Interactivity, Settings Drawer Tight Spacing, & List Alignment
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Native Window Dragging (Title Bar):**
    - Mengintegrasikan `i-slint-backend-winit` v1.19.0 dan `WinitWindowAccessor` pada [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs).
    - Menghubungkan callback `on_on_window_drag` ke `win.drag_window()` sehingga jendela frameless dapat digeser secara native menggunakan mouse saat ukuran jendela diperkecil (*restored / non-maximized*).
    - Memasang `TouchArea` pada brand logo dan area kosong top bar di [ui/appwindow.slint](file:///d:/Projects/music/rust/ui/appwindow.slint) agar dapat ditarik dengan responsif.
  - **Interaktivitas Penuh Waveform Seekbar & Debug Logging:**
    - Memperbaiki kalkulasi progres gelombang di [ui/components/seekbar.slint](file:///d:/Projects/music/rust/ui/components/seekbar.slint) menggunakan pembagian pecahan desimal (*float division*) murni: `(root.position_ms * 1.0) / (root.duration_ms * 1.0)` dan `((index * 1.0) / (peaks.length * 1.0)) <= progress`.
    - Menyediakan callback `clicked`, `pointer-event`, `moved` (saat drag ditekan), dan `scroll-event` (scroll wheel ±2%) dengan perataan waktu `Math.round`.
    - Menambahkan logging debugging informatif `[DEBUG UI]` pada [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) dan `[DEBUG AUDIO]` pada [src/audio/player.rs](file:///d:/Projects/music/rust/src/audio/player.rs) untuk memantau nilai milidetik dan status decoder seek secara real-time di terminal.
    - Menampilkan nama format audio yang bersih dan ramah pengguna (`FLAC`, `MP3`, `WAV`, `AAC`, `OGG`) pada [src/audio/decoder.rs](file:///d:/Projects/music/rust/src/audio/decoder.rs) menggantikan teks mentah `AudioCodecId(8192)`.
  - **Penyelarasan Spacing Rapat Pengaturan (Settings Drawer):**
    - Memperbaiki masalah peregangan layout vertikal pada [ui/views/settings.slint](file:///d:/Projects/music/rust/ui/views/settings.slint): menetapkan `vertical-stretch: 0; alignment: start;` pada seluruh sub-bagian judul dan kontrol pilihan di Tab Audio, DSP & EQ, Tema, dan Info.
    - Menambahkan trailing spacer `Rectangle { vertical-stretch: 1; }` di akhir setiap tab agar sisa ruang kosong terserap rapi di bagian bawah, menyatukan judul pengaturan dan pilihan kontrol secara rapat dan konsisten persis seperti tampilan tab Pustaka.
  - **Penyelarasan Kolom Lagu & Responsivitas Margin Kanan:**
    - Memperbarui [ui/views/library.slint](file:///d:/Projects/music/rust/ui/views/library.slint) dan [ui/views/queue.slint](file:///d:/Projects/music/rust/ui/views/queue.slint) dengan penataan koordinat kolom yang proporsional dan slot badge Hi-Res selebar 48px tetap di `parent.width - 116px`.
    - Durasi selalu sejajar rapi di kanan pada `parent.width - 62px` dengan margin kanan aman 14px sehingga tidak pernah terpotong (*clipped*) atau tertutup scrollbar vertikal saat jendela diperkecil.
- **Hasil Pengujian:** Seluruh 17 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build biner sukses tanpa error.

---

### [DONE-11] Interaktivitas Penuh Visualisasi Waveform (Seekbar Scrubbing & Real-time Drag)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Area Sentuh Penuh (*TouchArea 100% Width & Height*):**
    - Memperbaiki `TouchArea` pada komponen [seekbar.slint](file:///d:/Projects/music/rust/ui/components/seekbar.slint) yang sebelumnya tidak memiliki penetapan dimensi eksplisit di dalam kontainer `Rectangle`, sehingga pembagian koordinat mouse `self.mouse-x / self.width` sempat menghasilkan nilai tidak terdefinisi (*NaN*) atau tidak merespons klik.
    - Menetapkan area sentuh interaktif penuh (`width: 100%; height: 100%; mouse-cursor: pointer`) setinggi 36px untuk kenyamanan scrubbing.
  - **Umpan Balik Visual Seketika Saat Drag (*Real-time Drag Progress*):**
    - Menyediakan properti reaktif `is_dragging` dan `drag_progress`. Saat pengguna menggeser atau menyeret kursor mouse di atas bar visualisasi gelombang lagu (waveform), warna bar oranye dan garis penunjuk jarum (*playback head*) berwarna putih langsung bergerak mengikuti posisi kursor secara instan tanpa menunggu latensi backend audio.
    - Menghadirkan garis jarum pemutar (*playback head*) putih selebar 3px berujung bulat halus yang selalu terlihat jelas dan dapat ditarik (*draggable*), serta tooltip waktu target yang dinamis.
    - Mendukung putaran roda mouse (*scroll wheel*) di atas area visualisasi gelombang untuk melompat maju/mundur 2% durasi lagu secara cepat.
  - **Presisi Seeking Symphonia & Fallback:**
    - Memperbarui `seek` pada [src/audio/decoder.rs](file:///d:/Projects/music/rust/src/audio/decoder.rs) dengan akurasi sub-detik (nanodetik) melalui `Time::try_new` dan kalkulasi stempel waktu akurat `time_base.calc_time()`.
    - Menyediakan mekanisme fallback otomatis ke mode `SeekMode::Coarse` bila format file audio tertentu tidak memiliki indeks pencarian akurat (*seek table*).
    - Memperbarui [src/ui/app.rs](file:///d:/Projects/music/rust/src/ui/app.rs) agar langsung memperbarui `set_position_ms` seketika saat callback seek dipanggil.
- **Hasil Pengujian:** Seluruh 16 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build sukses tanpa error.

---

### [DONE-10] Spacing Rapat Pengaturan, Volume Popup Vertikal (Atas-Bawah), dan Laser Alignment Kolom Lagu
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Penyelarasan Spacing Pengaturan:**
    - Menghilangkan *fixed-height Flickable* statis di dalam tab Audio dan Pustaka yang sebelumnya menyisakan ruang kosong besar.
    - Menambahkan `alignment: start` pada kontainer seluruh tab (Audio, DSP & EQ, Pustaka, Tema, Info) sehingga judul bagian (*section title*) dan kontrol isinya langsung merapat ke atas tanpa gap kosong.
    - Memastikan seluruh tab memiliki jarak konsisten dan proporsional seperti halnya pada tab Pustaka.
  - **Kontrol Volume Tap-to-Toggle Popup Vertikal (Arah Atas-Bawah) & Drag Sliding:**
    - Mengganti slider horizontal permanen yang memadati dan menabrak bar pemutar dengan tombol ikon speaker elegan di jajaran transport controls.
    - Saat ikon volume di-klik/tap, muncul kartu popup slider vertikal (`arah atas-bawah`) di atas ikon volume dengan tampilan persentase volume dinamis, fader geser vertikal berpresisi tinggi, dan tombol *quick mute/unmute*.
    - **Perbaikan Interaksi Drag/Sliding:** Mengimplementasikan handler `moved => { if (self.pressed) { ... } }` dan scroll wheel `scroll-event` pada TouchArea (sebelumnya hanya mendeteksi event down sehingga tidak dapat di-drag geser secara kontinu). Memperluas area tangkap sentuh (hit target) slider dan mencegah klik pada kartu popup tembus menutup popup.
    - Perbaikan interaksi geser (`moved`) ini juga diterapkan secara komprehensif pada slider lagu (Seekbar), fader 10-band EQ, dan slider uji keselamatan audio.
    - Menambahkan layer deteksi sentuh backdrop untuk menutup/dismiss popup secara otomatis saat mengklik area di luar popup.
  - **Laser-Straight Column Alignment Kolom Lagu & Antrean:**
    - Mengganti layout flex adaptif yang rentan terpengaruh panjang teks judul dengan penataan posisi koordinat mutlak proporsional (`x` dan `width` presisi per baris) pada `library.slint` dan `queue.slint`.
    - Kolom Judul & Artis dimulai tepat di `x: 48px`, kolom Album selalu dimulai di pixel yang persis sama di setiap baris, slot badge Hi-Res berada di posisi konstan `parent.width - 104px`, dan durasi selalu sejajar rapi di kanan pada `parent.width - 54px` tanpa bergeser sedikit pun meskipun baris lagu tidak memiliki badge Hi-Res.
- **Hasil Pengujian:** Seluruh 16 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), kompilasi sukses 0 error.

---

### [DONE-09] Integrasi Lucide-Slint, Sinkronisasi Antrean (Queue), dan Responsivitas Layout
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - **Integrasi Penuh `lucide-slint` (v1.77.0):** Seluruh icon di antarmuka (Transport Controls, Top Bar Pill Dock, Window Controls, Album Card, Track List, Antrean, Pengaturan, hingga Modal Keselamatan dan Rekomendasi) kini 100% menggunakan Lucide Icons resmi berbasis komponen vektor Slint bawaan (`Play`, `Pause`, `SkipBack`, `SkipForward`, `Shuffle`, `Repeat`, `Repeat1`, `VolumeX`, `Volume1`, `Volume2`, `Settings`, `Search`, `FolderPlus`, `Folder`, `RotateCw`, `Check`, `Moon`, `Sun`, `Shield`, `Lightbulb`, `Minus`, `Square`, `X`, `Zap`, `ListMusic`).
  - **Sinkronisasi Otomatis Antrean (Queue):**
    - Menyediakan fungsi `sync_queue_to_ui` yang otomatis menghubungkan antrean lagu internal `PlaybackQueue` ke antarmuka Slint `set_queue_items` dan `set_active_queue_index`.
    - Sinkronisasi berjalan seketika saat pustaka lagu dimuat, saat lagu di-*double-click*, saat tombol *Next/Previous* ditekan, saat lagu selesai diputar secara otomatis, dan saat antrean dikosongkan.
    - Menghubungkan callback interaktif `on_play_queue_item(idx)` agar pengguna dapat langsung memutar lagu apa pun dari daftar antrean, serta `on_clear_queue()`.
  - **Perataan Presisi Kolom Lagu & Badge Hi-Res:**
    - Membuat slot kontainer badge Hi-Res selebar 48px tetap (*fixed width*), dan kolom durasi selebar 44px rata kanan (*right-aligned*).
    - Dengan struktur ini, letak angka durasi lagu di baris lagu yang tidak memiliki badge Hi-Res tetap lurus sejajar sempurna (*laser-straight alignment*) tanpa melompat ke kanan atau ke kiri.
  - **Perbaikan Responsivitas Jendela Saat Di-restore / Diperkecil:**
    - Mengganti lebar persentase kaku menjadi layout adaptif: *The Acoustic Stage* kiri (`min-width: 340px; preferred-width: 380px; max-width: 440px; horizontal-stretch: 0;`) dan *Dynamic Canvas* kanan (`horizontal-stretch: 1; min-width: 520px;`).
    - Menambahkan padding kanan dan kiri yang aman (20–24px) serta kolom judul/album yang elastis (`horizontal-stretch: 3` dan `horizontal-stretch: 2`) dengan `overflow: elide`, sehingga tombol `Tambah Folder`, badge Hi-Res, dan durasi lagu tidak akan terpotong (*clipped*) saat ukuran jendela diperkecil.
  - **Penyelarasan Spacing Judul & Kontrol Pengaturan:**
    - Mempersempit jarak vertikal antar judul sub-pengaturan dan elemen tombol/slider terkait (dari sebelumnya 16px menjadi 4–6px), sehingga judul dan kontrol terlihat menyatu dan harmonis.
- **Hasil Pengujian:** Seluruh 16 unit & integration test lulus 100% (`cargo test -- --test-threads=1`), build sukses tanpa error.

---

### [DONE-04] Tambah Musik/Folder Membuka Windows File Explorer Native
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Mengintegrasikan dialog native Windows File Explorer (`rfd::FileDialog::new().pick_folder()`) yang berjalan secara asinkron di background thread agar tidak memblokir UI thread Slint.
  - Menghubungkan seluruh tombol tambah folder (di layar pustaka kosong, tombol bar atas pustaka `📁+`, dan tombol drawer pengaturan `+ Tambah Folder`) ke callback native `on_add_folder_clicked`.
  - Background scanner otomatis memindai seluruh file musik (`.flac`, `.mp3`, `.m4a`, `.wav`, `.ogg`), mengekstrak metadata Lofty & album art ke SQLite WAL, dan memperbarui daftar lagu serta folder scopes di UI secara instan.
- **Hasil Pengujian:** Verifikasi unit & integrasi di `tests/fase4_test.rs` lulus 100%.

---

### [DONE-08] Error Pemuatan Gambar Thumbnail Album (os error 2)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Menambahkan pengecekan keberadaan file fisik di disk `std::path::Path::new(thumb).is_file()` sebelum memanggil `slint::Image::load_from_path(p)` untuk mencegah Slint mencetak pesan stderr `The system cannot find the file specified. (os error 2)`.
  - Menambahkan mekanisme *automatic on-demand regeneration*: jika file thumbnail terhapus atau belum ada di disk (misalnya akibat pembersihan cache), aplikasi langsung mengekstrak ulang cover art dari file audio asli secara otomatis dan menyimpannya kembali tanpa mengganggu pengguna.
  - Mengisolasi pengujian unit di `tests/fase2_test.rs` agar tidak lagi menghapus folder AppData pengguna.
- **Hasil Pengujian:** Verifikasi unit & integrasi di `tests/fase2_test.rs` dan `tests/fase4_test.rs` lulus 100%.

---

### [DONE-03] Icon Kotak / Tofu Diganti dengan Vector Paths Independen Font
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Mengganti seluruh icon teks/karakter unicode emoji yang rawan gagal render (*tofu box*) di Windows dengan komponen geometris dan SVG `Path` bawaan Slint:
    - **Transport Controls:** Play (segitiga tajam), Pause (dua pilar vertikal simetris), Previous (garis vertikal + segitiga kiri), Next (segitiga kanan + garis vertikal), Shuffle (panah silang interaktif), Loop (lingkaran panah melingkar + badge angka "1" untuk Repeat One), Volume/Speaker (corong speaker + gelombang suara dinamis / tanda silang saat senyap).
    - **Top Navigation Bar:** Logo dot aksen ShrimpMusic, Kaca pembesar (Search vector), Roda gigi mekanis (Settings cog vector), Kontrol jendela Windows native (Garis minimize, Kotak maximize, Silang close X).
    - **Album Artwork & Pustaka:** Vector double musical notes pada card tanpa cover, folder musik vector pada tombol tambah folder & empty state, petir vector pada banner scan, dan indikator play vector pada baris lagu.
    - **Lirik & Antrean:** Circular sync arrow pada tombol auto-resync lirik, list lines vector pada antrean pemutar.
    - **Modal & Pengaturan:** Shield vector pada modal keselamatan pendengaran, bohlam vector pada modal audiophile, dan panah circular rescan pada daftar folder.
- **Hasil Pengujian:** Seluruh icon 100% tajam, konsisten, dan bebas ketergantungan font sistem Windows di semua resolusi.

---

### [DONE-02 & DONE-07] Penataan Layout, Proporsi, Margin, Padding, dan Unified Header
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Menggabungkan Window Controls, Brand Title, dan Pill Dock ke dalam satu **Unified Top Navigation Bar** setinggi 52px yang terintegrasi rapi. Hal ini menghilangkan masalah elemen floating PillDock sebelumnya yang menabrak/menutupi stage dan kontrol jendela.
  - Memperbaiki proporsi ruang kerja: Sisi Kiri *The Acoustic Stage* (38%) vs Sisi Kanan *Dynamic Canvas* (62%) dengan pemisah border halus dan padding seimbang (24–28px).
  - Album Card diposisikan tepat di tengah dengan dimensi 220x220px dan radius sudut harmonis.
  - Memperlebar area sentuh (*hit target*) pada slider Volume dan Seekbar dari yang sebelumnya hanya 6px menjadi kontainer interaktif 24–32px sehingga nyaman diklik dan digeser (*drag scrubbing*).
- **Hasil Pengujian:** Kompilasi Slint bersih tanpa warning layout, proporsi visual seimbang di berbagai resolusi layar (min 960x620 hingga 4K).

---

### [DONE-05] Menu Pengaturan Terstruktur per Kategori Tab
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Merombak halaman pengaturan memanjang (980px satu scroll) menjadi panel drawer dengan 5 sub-kategori tab navigasi:
    1. **Audio & Output:** Sakelar WASAPI Exclusive Mode (Bit-Perfect) dan daftar lengkap perangkat output audio dengan indikator centang perangkat aktif.
    2. **DSP & Ekualiser:** Master switch bypass EQ murni, 5 tombol preset instan (Flat, Bass, Vokal, Akustik, Treble), 10 slider fader biquad EQ vertikal (-12dB s/d +12dB), dan 4 mode normalisasi ReplayGain.
    3. **Pustaka & Folder:** Daftar folder musik terdaftar dengan jumlah lagu, tombol rescan per-folder `[🔄]`, sakelar aktif/nonaktif per folder, serta tombol `+ Tambah Folder`.
    4. **Tampilan & Tema:** Kartu visual pemilih tema (OLED Dark vs Soft Daylight) serta selektor mode pengacakan (*Anti-Ulang Pintar*, *Normal Acak*, *Favffle Favorit*).
    5. **Sistem & Info:** Status audio engine aktif dan informasi versi arsitektur.
- **Hasil Pengujian:** Navigasi pengaturan terstruktur, mudah diakses, dan sesuai spesifikasi PRD & PRD-correction.

---

### [DONE-06] Pilihan Tema Terang (Soft Daylight) Lengkap dengan Switch Interaktif
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Menghadirkan dua kartu pilihan tema berdampingan di tab Pengaturan Tema: **OLED Dark** (hitam pekat `#000000` dengan aksen oranye) dan **Soft Daylight** (terang elegan `#F5F5F7` dengan kartu putih bersih `#FFFFFF` dan aksen oranye).
  - Mengaitkan pemilihan tema ke token global Slint `ThemePalette.is_dark` yang secara instan merender ulang warna seluruh komponen antarmuka tanpa perlu restart aplikasi.
  - Menyimpan preferensi tema pengguna ke database SQLite (`dark_mode`) dan memuatnya secara otomatis saat aplikasi dibuka kembali.
- **Hasil Pengujian:** Pergantian tema gelap dan terang berjalan instan, kontras teks memenuhi standar aksesibilitas WCAG, dan preferensi tersimpan permanen.

---

### [DONE-01] Tes Audio Keselamatan Pendengaran (Hearing Safety Test Tone)
- **Udah diperbaiki tanggal 11/22/2026**
- **Detail Implementasi:**
  - Menyediakan sinyal uji audio -18 dBFS multi-harmonic chime (440/880/1320 Hz) berdurasi 10 detik melalui `SafetyTestSignal::generate_10s_chime`.
  - Mengalirkan sinyal langsung ke worker WASAPI shared mode melalui `AudioCommand::PlayTestTone`.
  - Slider volume uji di dialog modal telah diperluas area sentuhnya dan terhubung langsung ke volume audio player.
- **Hasil Pengujian:** Suara nada uji terdengar jernih pada speaker/headphone sebelum pengguna memutar musik perdana.

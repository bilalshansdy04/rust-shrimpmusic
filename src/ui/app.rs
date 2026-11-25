use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use i_slint_backend_winit::WinitWindowAccessor;

use crate::audio::{
    AudioDeviceInfo, AudioEvent, AudioPlayer, DeviceManager, LoopMode, PlaybackQueue, PlayerState,
    ShuffleStyle,
};
use crate::audio::dsp::{EqPreset, VolumeBalanceMode};
use crate::audio::waveform::generate_waveform_peaks;
use crate::db::{default_db_path, open_database, Database, SortCriteria, TrackRecord};
use crate::library::scanner::{ScanProgress, Scanner};
use crate::library::playlist::PlaylistManager;
use crate::library::search::SearchEngine;
use super::taskbar::{TaskbarAction, TaskbarController};
use super::tray::{TrayAction, TrayController};

slint::include_modules!();

pub struct ShrimpMusicApp {
    window: AppWindow,
    db: Rc<Database>,
    player: Rc<AudioPlayer>,
    queue: Rc<RefCell<PlaybackQueue>>,
    taskbar: Rc<RefCell<TaskbarController>>,
    tray: Rc<RefCell<Option<TrayController>>>,
    active_scope: Rc<Cell<Option<i64>>>,
    sort_criteria: Rc<Cell<SortCriteria>>,
    sort_ascending: Rc<Cell<bool>>,
    show_romanized: Rc<Cell<bool>>,
    romanize_title: Rc<Cell<bool>>,
    romanize_artist: Rc<Cell<bool>>,
    romanize_lyrics: Rc<Cell<bool>>,
    last_lyrics_scroll_time: Rc<Cell<Option<Instant>>>,
    _timer: slint::Timer,
    win_volume: crate::audio::WindowsVolumeController,
    device_watcher: crate::audio::DeviceWatcher,
}

impl ShrimpMusicApp {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let window = AppWindow::new()?;
        let db_path = default_db_path();
        let db = Rc::new(open_database(&db_path)?);

        let initial_volume: f32 = db
            .get_preference("volume")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.7);

        let initial_shuffle_style = db
            .get_preference("shuffle_style")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i32>().ok())
            .map(ShuffleStyle::from_i32)
            .unwrap_or(ShuffleStyle::ReducedRepeat);

        let initial_loop_mode = db
            .get_preference("loop_mode")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i32>().ok())
            .map(LoopMode::from_i32)
            .unwrap_or(LoopMode::Off);

        let is_dark_mode = db
            .get_preference("dark_mode")
            .ok()
            .flatten()
            .map(|v| v == "1" || v == "true")
            .unwrap_or(true);

        let is_eq_enabled = db
            .get_preference("eq_enabled")
            .ok()
            .flatten()
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        let volume_balance_mode_idx: i32 = db
            .get_preference("volume_balance")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        window.set_volume(initial_volume);
        window.set_shuffle_style(initial_shuffle_style.to_i32());
        window.set_loop_mode(initial_loop_mode.to_i32());
        window.set_is_eq_enabled(is_eq_enabled);
        window.set_volume_balance_mode(volume_balance_mode_idx);
        window.global::<ThemePalette>().set_is_dark(is_dark_mode);

        window.set_is_safety_modal_visible(true);

        let player = Rc::new(AudioPlayer::new()?);
        let _ = player.set_volume(initial_volume);
        let _ = player.set_eq_enabled(is_eq_enabled);
        let vol_mode = match volume_balance_mode_idx {
            1 => VolumeBalanceMode::Quiet,
            2 => VolumeBalanceMode::Balanced,
            3 => VolumeBalanceMode::Punchy,
            _ => VolumeBalanceMode::Off,
        };
        let _ = player.set_volume_balance(vol_mode);

        let mut initial_queue = PlaybackQueue::new();
        initial_queue.set_shuffle_style(initial_shuffle_style);
        initial_queue.set_loop_mode(initial_loop_mode);
        let queue = Rc::new(RefCell::new(initial_queue));

        let taskbar = Rc::new(RefCell::new(TaskbarController::new()));
        let last_lyrics_scroll_time = Rc::new(Cell::new(None));
        let timer = slint::Timer::default();
        let win_volume = crate::audio::WindowsVolumeController::new();
        window.set_windows_master_volume(win_volume.get_volume());
        let device_watcher = crate::audio::DeviceWatcher::new();

        let tray = Rc::new(RefCell::new(TrayController::new().ok()));

        let sort_criteria = db
            .get_preference("sort_criteria")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i32>().ok())
            .map(SortCriteria::from_i32)
            .unwrap_or(SortCriteria::Title);

        let sort_ascending = db
            .get_preference("sort_ascending")
            .ok()
            .flatten()
            .map(|s| s != "0")
            .unwrap_or(true);

        let show_romanized = db
            .get_preference("show_romanized")
            .ok()
            .flatten()
            .map(|s| s == "1" || s == "true")
            .unwrap_or(true); // Default true so Korean/Japanese songs instantly show Romanized!

        let romanize_title = db
            .get_preference("romanize_title")
            .ok()
            .flatten()
            .map(|s| s != "0")
            .unwrap_or(true);

        let romanize_artist = db
            .get_preference("romanize_artist")
            .ok()
            .flatten()
            .map(|s| s != "0")
            .unwrap_or(true);

        let romanize_lyrics = db
            .get_preference("romanize_lyrics")
            .ok()
            .flatten()
            .map(|s| s != "0")
            .unwrap_or(true);

        window.set_library_sort_criteria(sort_criteria.to_i32());
        window.set_library_sort_ascending(sort_ascending);
        window.set_show_romanized(show_romanized);
        window.set_romanize_title(romanize_title);
        window.set_romanize_artist(romanize_artist);
        window.set_romanize_lyrics(romanize_lyrics);

        let app = Self {
            window,
            db,
            player,
            queue,
            taskbar,
            tray,
            active_scope: Rc::new(Cell::new(None)),
            sort_criteria: Rc::new(Cell::new(sort_criteria)),
            sort_ascending: Rc::new(Cell::new(sort_ascending)),
            show_romanized: Rc::new(Cell::new(show_romanized)),
            romanize_title: Rc::new(Cell::new(romanize_title)),
            romanize_artist: Rc::new(Cell::new(romanize_artist)),
            romanize_lyrics: Rc::new(Cell::new(romanize_lyrics)),
            last_lyrics_scroll_time,
            _timer: timer,
            win_volume,
            device_watcher,
        };

        app.refresh_devices();
        app.refresh_folders();
        app.refresh_tracks();
        app.refresh_playlists();
        app.setup_callbacks();
        app.setup_event_loop();
        Self::refresh_jump_list_async(&app.db);

        Ok(app)
    }

    pub fn play_track_by_id(&self, track_id: i64) {
        Self::play_track_from_jump_list(
            track_id,
            &self.db,
            &self.queue,
            &self.player,
            &self.window.as_weak(),
            self.active_scope.get(),
            self.sort_criteria.get(),
            self.sort_ascending.get(),
        );
    }

    fn refresh_jump_list_async(db: &Database) {
        if let Ok(recent) = db.get_recently_played(5) {
            let items: Vec<(i64, String, String)> = recent
                .into_iter()
                .map(|t| (t.id, t.title, t.artist))
                .collect();
            std::thread::spawn(move || {
                let tb = TaskbarController::new();
                tb.update_jump_list(&items);
            });
        }
    }

    fn play_track_from_jump_list(
        track_id: i64,
        db: &Database,
        queue: &RefCell<PlaybackQueue>,
        player: &AudioPlayer,
        win_weak: &slint::Weak<AppWindow>,
        active_scope: Option<i64>,
        sort_criteria: SortCriteria,
        sort_ascending: bool,
    ) {
        if let Ok(Some(track)) = db.get_track_by_id(track_id) {
            let scope = active_scope.or(Some(track.folder_id));
            let mut queue_tracks = db
                .list_tracks(scope, sort_criteria, sort_ascending)
                .unwrap_or_default();
            let mut start_idx = queue_tracks
                .iter()
                .position(|t| t.id == track.id)
                .unwrap_or(0);
            if queue_tracks.is_empty() {
                queue_tracks = vec![track.clone()];
                start_idx = 0;
            }
            {
                let mut q = queue.borrow_mut();
                q.set_items(queue_tracks, Some(start_idx));
                if let Some(w) = win_weak.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
            }
            Self::play_track_record(player, &track, win_weak, db);
        }
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }

    fn refresh_devices(&self) {
        Self::update_devices_in_ui(&self.window, None);
    }

    fn apply_device_models_to_ui(window: &AppWindow, devices: Vec<AudioDeviceInfo>, selected_id: Option<&str>) {
        let mut can_enable_exclusive = false;

        // Check which device is currently active/selected
        let target_id = selected_id.map(|s| s.to_string());
        let models: Vec<OutputDeviceModel> = devices
            .into_iter()
            .map(|d| {
                let is_sel = match &target_id {
                    Some(sel) => d.id == *sel,
                    None => d.is_default,
                };
                if is_sel {
                    can_enable_exclusive = d.supports_bit_perfect;
                }
                OutputDeviceModel {
                    id: SharedString::from(d.id),
                    name: SharedString::from(d.name),
                    is_default: d.is_default,
                    is_selected: is_sel,
                    badge: SharedString::from(d.classification.display_badge()),
                    supports_bit_perfect: d.supports_bit_perfect,
                }
            })
            .collect();

        let hint = if can_enable_exclusive {
            SharedString::from("Perangkat mendukung bit-perfect WASAPI Exclusive.")
        } else {
            SharedString::from("Bit-Perfect nonaktif: Hanya didukung pada USB DAC / Type-C DAC eksternal.")
        };

        window.set_can_enable_exclusive(can_enable_exclusive);
        window.set_exclusive_hint(hint);

        // If current device does not support bit-perfect, force disable exclusive mode
        if !can_enable_exclusive && window.get_is_exclusive() {
            window.set_is_exclusive(false);
        }

        window.set_settings_devices(ModelRc::new(VecModel::from(models)));
    }

    fn update_devices_in_ui(window: &AppWindow, selected_id: Option<&str>) {
        if let Ok(devices) = DeviceManager::list_output_devices_isolated() {
            Self::apply_device_models_to_ui(window, devices, selected_id);
        }
    }

    fn sync_folders_and_scopes_to_ui(db: &Database, window: &AppWindow, active_scope_id: Option<i64>) {
        if let Ok(folders) = db.get_all_folders() {
            let mut folder_models = Vec::new();
            let mut scope_models = vec![FolderScopeModel {
                id: -1,
                display_name: SharedString::from("Semua Folder"),
                is_selected: active_scope_id.is_none(),
            }];

            for f in &folders {
                folder_models.push(FolderEntryModel {
                    id: f.id as i32,
                    path: SharedString::from(f.folder_path.clone()),
                    track_count: f.total_tracks as i32,
                    is_enabled: f.is_enabled,
                });

                if f.is_enabled {
                    let folder_name = std::path::Path::new(&f.folder_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.folder_path);

                    scope_models.push(FolderScopeModel {
                        id: f.id as i32,
                        display_name: SharedString::from(folder_name.to_string()),
                        is_selected: active_scope_id == Some(f.id),
                    });
                }
            }

            window.set_settings_folders(ModelRc::new(VecModel::from(folder_models)));
            window.set_folder_scopes(ModelRc::new(VecModel::from(scope_models)));
        }
    }

    fn refresh_folders(&self) {
        Self::sync_folders_and_scopes_to_ui(&self.db, &self.window, self.active_scope.get());
    }

    pub fn format_text_display(original: &str, romanize: bool) -> String {
        if !romanize || original.is_empty() {
            return original.to_string();
        }
        let rom = crate::library::romanize::romanize_text(original);
        if rom != original {
            // Text contained Korean/Japanese characters and was converted
            // E.g. "하츠투하츠" -> "하츠투하츠 (hacheutuhacheu)"
            // or if original is "Hearts2Hearts (하츠투하츠)" -> "Hearts2Hearts (hacheutuhacheu)"
            rom
        } else {
            original.to_string()
        }
    }

    pub fn convert_tracks_to_models(tracks: &[TrackRecord]) -> Vec<TrackItemModel> {
        Self::convert_tracks_to_models_with_option(tracks, false)
    }

    pub fn convert_tracks_to_models_with_option(tracks: &[TrackRecord], romanize: bool) -> Vec<TrackItemModel> {
        Self::convert_tracks_to_models_detailed(tracks, romanize, true, true)
    }

    pub fn convert_tracks_for_window(tracks: &[TrackRecord], window: &AppWindow) -> Vec<TrackItemModel> {
        Self::convert_tracks_to_models_detailed(
            tracks,
            window.get_show_romanized(),
            window.get_romanize_title(),
            window.get_romanize_artist(),
        )
    }

    pub fn convert_tracks_to_models_detailed(
        tracks: &[TrackRecord],
        romanize_master: bool,
        romanize_title: bool,
        romanize_artist: bool,
    ) -> Vec<TrackItemModel> {
        tracks
            .iter()
            .map(|t| {
                let duration_sec = t.duration_ms / 1000;
                let mins = duration_sec / 60;
                let secs = duration_sec % 60;

                let title = Self::format_text_display(&t.title, romanize_master && romanize_title);
                let artist = Self::format_text_display(&t.artist, romanize_master && romanize_artist);
                let album = Self::format_text_display(&t.album, romanize_master && romanize_title);

                TrackItemModel {
                    id: t.id as i32,
                    title: SharedString::from(title),
                    artist: SharedString::from(artist),
                    album: SharedString::from(album),
                    duration_str: SharedString::from(format!("{}:{:02}", mins, secs)),
                    format: SharedString::from(t.format.clone()),
                    is_hi_res: t.sample_rate >= 88200 || t.bit_depth >= 24,
                    is_active: t.is_active,
                    is_selected: false,
                }
            })
            .collect()
    }

    fn refresh_tracks(&self) {
        let scope = self.active_scope.get();
        let sort = self.sort_criteria.get();
        let asc = self.sort_ascending.get();
        let rom = self.show_romanized.get();
        let rom_title = self.romanize_title.get();
        let rom_artist = self.romanize_artist.get();

        if let Ok(tracks) = self.db.list_tracks(scope, sort, asc) {
            let models = Self::convert_tracks_to_models_detailed(&tracks, rom, rom_title, rom_artist);

            let mut q = self.queue.borrow_mut();
            if q.is_empty() {
                q.set_items(tracks, None);
            }
            Self::sync_queue_to_ui_detailed(&q, &self.window, rom, rom_title, rom_artist);

            self.window.set_library_tracks(ModelRc::new(VecModel::from(models)));
        }
    }

    fn refresh_playlists(&self) {
        Self::sync_playlists_to_ui(&self.db, &self.window);
    }

    fn sync_playlists_to_ui(db: &Database, window: &AppWindow) {
        if let Ok(playlists) = db.get_playlists() {
            let models: Vec<PlaylistItemModel> = playlists
                .into_iter()
                .map(|p| PlaylistItemModel {
                    id: p.id as i32,
                    name: SharedString::from(p.name),
                    description: SharedString::from(p.description.unwrap_or_default()),
                    track_count: p.track_count as i32,
                    artwork_path: SharedString::from(p.custom_artwork_path.unwrap_or_default()),
                })
                .collect();
            window.set_playlists(ModelRc::new(VecModel::from(models)));
        }
    }

    fn sync_queue_to_ui(queue: &PlaybackQueue, window: &AppWindow) {
        Self::sync_queue_to_ui_detailed(
            queue,
            window,
            window.get_show_romanized(),
            window.get_romanize_title(),
            window.get_romanize_artist(),
        )
    }

    fn sync_queue_to_ui_detailed(
        queue: &PlaybackQueue,
        window: &AppWindow,
        romanize_master: bool,
        romanize_title: bool,
        romanize_artist: bool,
    ) {
        let models = Self::convert_tracks_to_models_detailed(
            queue.items(),
            romanize_master,
            romanize_title,
            romanize_artist,
        );
        window.set_queue_items(ModelRc::new(VecModel::from(models)));
        if let Some(idx) = queue.current_index() {
            window.set_active_queue_index(idx as i32);
        }
    }

    fn setup_callbacks(&self) {
        let player = self.player.clone();
        let q_ref = self.queue.clone();
        let win_weak = self.window.as_weak();
        let db = self.db.clone();

        // 1. Play / Pause
        let p1 = player.clone();
        let w1 = win_weak.clone();
        self.window.on_on_play_pause(move || {
            if let Some(w) = w1.upgrade() {
                if w.get_is_playing() {
                    let _ = p1.pause();
                } else {
                    let _ = p1.resume();
                }
            }
        });

        // 2. Volume
        let p2 = player.clone();
        let db_vol = db.clone();
        let w_vol = win_weak.clone();
        self.window.on_on_set_volume(move |v| {
            let _ = p2.set_volume(v);
            let _ = db_vol.set_preference("volume", &format!("{:.2}", v));
            if let Some(w) = w_vol.upgrade() {
                w.set_volume(v);
            }
        });

        // 3. Seek
        let p3 = player.clone();
        let w3 = win_weak.clone();
        self.window.on_on_seek_to(move |ms| {
            println!("[DEBUG UI] on_seek_to called with ms={}", ms);
            if let Some(w) = w3.upgrade() {
                w.set_position_ms(ms);
            }
            let _ = p3.seek(Duration::from_millis(ms as u64));
        });

        // 4. Next Track
        let p4 = player.clone();
        let q4 = q_ref.clone();
        let w4 = win_weak.clone();
        let db4 = db.clone();
        self.window.on_on_next(move || {
            let mut q = q4.borrow_mut();
            if let Some(track) = q.next().cloned() {
                if let Some(w) = w4.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
                drop(q);
                Self::play_track_record(&p4, &track, &w4, &db4);
            }
        });

        // 5. Previous Track
        let p5 = player.clone();
        let q5 = q_ref.clone();
        let w5 = win_weak.clone();
        let db5 = db.clone();
        self.window.on_on_previous(move || {
            let mut q = q5.borrow_mut();
            if let Some(track) = q.previous().cloned() {
                if let Some(w) = w5.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
                drop(q);
                Self::play_track_record(&p5, &track, &w5, &db5);
            }
        });

        // 6. Toggle Shuffle
        let q6 = q_ref.clone();
        let w6 = win_weak.clone();
        self.window.on_on_toggle_shuffle(move || {
            let mut q = q6.borrow_mut();
            let new_shuf = !q.is_shuffle();
            q.set_shuffle(new_shuf);
            if new_shuf {
                // When enabling shuffle, re-order upcoming items so the queue view reflects the randomized order!
                q.reshuffle_upcoming();
            } else {
                // When disabling shuffle, restore upcoming items to original sorted library/playlist order!
                q.restore_original_order();
            }
            if let Some(w) = w6.upgrade() {
                w.set_is_shuffle(new_shuf);
                Self::sync_queue_to_ui(&q, &w);
            }
        });

        // 7. Cycle Loop Mode
        let q7 = q_ref.clone();
        let w7 = win_weak.clone();
        let db7 = db.clone();
        self.window.on_on_cycle_loop_mode(move || {
            let mut q = q7.borrow_mut();
            let next_mode = match q.loop_mode() {
                LoopMode::Off => LoopMode::All,
                LoopMode::All => LoopMode::One,
                LoopMode::One => LoopMode::Off,
            };
            q.set_loop_mode(next_mode);
            let mode_val = next_mode.to_i32();
            let _ = db7.set_preference("loop_mode", &mode_val.to_string());
            if let Some(w) = w7.upgrade() {
                w.set_loop_mode(mode_val);
            }
        });

        // 8. Track selection & double-click
        let w8 = win_weak.clone();
        self.window.on_on_track_selected(move |id| {
            if let Some(w) = w8.upgrade() {
                w.set_selected_track_id(id);
            }
        });

        let p9 = player.clone();
        let q9 = q_ref.clone();
        let w9 = win_weak.clone();
        let db9 = db.clone();
        let app_scope_play = self.active_scope.clone();
        let app_sort_play = self.sort_criteria.clone();
        let app_asc_play = self.sort_ascending.clone();
        self.window.on_on_track_double_clicked(move |id| {
            if let Ok(Some(track)) = db9.get_track_by_id(id as i64) {
                let mut q = q9.borrow_mut();
                let mut queue_tracks = Vec::new();
                let mut start_idx = 0;

                // Cek konteks tampilan: apakah user sedang berada di dalam Playlist View?
                if let Some(w) = w9.upgrade() {
                    let active_view = w.get_active_view();
                    let sel_pl_id = w.get_selected_playlist_id();

                    if active_view == 1 && sel_pl_id >= 0 {
                        // User berada di dalam playlist! Bangun antrean HANYA dari lagu-lagu di playlist ini (sesuai filter & urutan aktif)
                        let crit_id = w.get_playlist_sort_criteria();
                        let asc = w.get_playlist_sort_ascending();
                        let q_str = w.get_playlist_search_query().to_string();
                        let search_param = if q_str.trim().is_empty() { None } else { Some(q_str.as_str()) };

                        if let Ok(pl_items) = db9.get_playlist_tracks_filtered(sel_pl_id as i64, search_param, crit_id, asc) {
                            let pl_tracks: Vec<TrackRecord> = pl_items.into_iter().map(|item| item.track).collect();
                            if let Some(idx) = pl_tracks.iter().position(|t| t.id == id as i64) {
                                start_idx = idx;
                            }
                            queue_tracks = pl_tracks;
                        }
                    }
                }

                // Jika bukan dari playlist view, gunakan logika folder scope perpustakaan
                if queue_tracks.is_empty() {
                    let scope = app_scope_play.get().or(Some(track.folder_id));
                    let sort = app_sort_play.get();
                    let asc = app_asc_play.get();

                    if let Ok(current_view_tracks) = db9.list_tracks(scope, sort, asc) {
                        if let Some(idx) = current_view_tracks.iter().position(|t| t.id == id as i64) {
                            start_idx = idx;
                        }
                        queue_tracks = current_view_tracks;
                    }
                }

                if queue_tracks.is_empty() {
                    queue_tracks = vec![track.clone()];
                    start_idx = 0;
                }

                q.set_items(queue_tracks, Some(start_idx));

                if let Some(w) = w9.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
                drop(q);
                Self::play_track_record(&p9, &track, &w9, &db9);
            }
        });

        // 8a. Scope changed (Filter by folder or "Semua Folder")
        let w_scope = win_weak.clone();
        let db_scope = db.clone();
        let app_scope = self.active_scope.clone();
        let app_sort = self.sort_criteria.clone();
        let app_asc = self.sort_ascending.clone();
        let app_rom = self.show_romanized.clone();
        self.window.on_on_scope_changed(move |scope_id| {
            let target_scope = if scope_id < 0 { None } else { Some(scope_id as i64) };
            app_scope.set(target_scope);

            if let Some(w) = w_scope.upgrade() {
                let scopes_model = w.get_folder_scopes();
                for i in 0..scopes_model.row_count() {
                    if let Some(mut sc) = scopes_model.row_data(i) {
                        let is_sel = (scope_id < 0 && sc.id < 0) || (sc.id == scope_id);
                        if sc.is_selected != is_sel {
                            sc.is_selected = is_sel;
                            scopes_model.set_row_data(i, sc);
                        }
                    }
                }

                if let Ok(tracks) = db_scope.list_tracks(target_scope, app_sort.get(), app_asc.get()) {
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                    w.set_selected_count(0);
                }
            }
        });

        // 8a2. Sort changed (Title, Artist, Album, DateAdded & Ascending/Descending)
        let w_sort = win_weak.clone();
        let db_sort = db.clone();
        let app_scope_sort = self.active_scope.clone();
        let app_sort_crit = self.sort_criteria.clone();
        let app_sort_asc = self.sort_ascending.clone();
        let app_rom_sort = self.show_romanized.clone();
        self.window.on_on_sort_changed(move |crit_id, asc| {
            let criteria = SortCriteria::from_i32(crit_id);
            app_sort_crit.set(criteria);
            app_sort_asc.set(asc);

            let _ = db_sort.set_preference("sort_criteria", &crit_id.to_string());
            let _ = db_sort.set_preference("sort_ascending", if asc { "1" } else { "0" });

            if let Some(w) = w_sort.upgrade() {
                w.set_library_sort_criteria(crit_id);
                w.set_library_sort_ascending(asc);

                if let Ok(tracks) = db_sort.list_tracks(app_scope_sort.get(), criteria, asc) {
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                    w.set_selected_count(0);
                }
            }
        });

        // 8b. Real-time Search query (tracks and lyrics)
        let w_search = win_weak.clone();
        let db_search = db.clone();
        let app_scope_s = self.active_scope.clone();
        let app_sort_s = self.sort_criteria.clone();
        let app_asc_s = self.sort_ascending.clone();
        let app_rom_s = self.show_romanized.clone();
        self.window.on_on_search_changed(move |query| {
            if let Some(w) = w_search.upgrade() {
                let trimmed = query.trim().to_string();
                if trimmed.is_empty() {
                    if let Ok(tracks) = db_search.list_tracks(app_scope_s.get(), app_sort_s.get(), app_asc_s.get()) {
                        let models = Self::convert_tracks_for_window(&tracks, &w);
                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                    }
                } else {
                    if let Ok(results) = SearchEngine::search(&db_search, &trimmed) {
                        let models = Self::convert_tracks_for_window(&results.tracks, &w);
                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                        w.set_active_view(0); // Switch to library view to see search results
                    }
                }
            }
        });

        // 8c. Playlist Management Callbacks
        let db_pl_create = db.clone();
        let w_pl_create = win_weak.clone();
        self.window.on_on_create_playlist(move |name| {
            let name_str = name.trim().to_string();
            if !name_str.is_empty() {
                let _ = db_pl_create.create_playlist(&name_str, None, None);
                if let Some(w) = w_pl_create.upgrade() {
                    Self::sync_playlists_to_ui(&db_pl_create, &w);
                }
            }
        });

        let db_pl_create_add = db.clone();
        let w_pl_create_add = win_weak.clone();
        self.window.on_on_create_playlist_and_add_track(move |name, track_id| {
            let name_str = name.trim().to_string();
            if !name_str.is_empty() {
                if let Ok(pl_id) = db_pl_create_add.create_playlist(&name_str, None, None) {
                    if track_id >= 0 {
                        let _ = db_pl_create_add.add_track_to_playlist(pl_id, track_id as i64);
                    }
                    if let Some(w) = w_pl_create_add.upgrade() {
                        Self::sync_playlists_to_ui(&db_pl_create_add, &w);
                    }
                }
            }
        });

        let db_pl_rename = db.clone();
        let w_pl_rename = win_weak.clone();
        self.window.on_on_rename_playlist(move |pl_id, new_name| {
            let name_str = new_name.trim().to_string();
            if !name_str.is_empty() {
                let _ = db_pl_rename.update_playlist_name(pl_id as i64, &name_str);
                if let Some(w) = w_pl_rename.upgrade() {
                    if w.get_selected_playlist_id() == pl_id {
                        w.set_selected_playlist_name(SharedString::from(name_str));
                    }
                    Self::sync_playlists_to_ui(&db_pl_rename, &w);
                }
            }
        });

        let db_pl_del = db.clone();
        let w_pl_del = win_weak.clone();
        self.window.on_on_delete_playlist(move |pl_id| {
            let _ = db_pl_del.delete_playlist(pl_id as i64);
            if let Some(w) = w_pl_del.upgrade() {
                if w.get_selected_playlist_id() == pl_id {
                    w.set_selected_playlist_id(-1);
                }
                Self::sync_playlists_to_ui(&db_pl_del, &w);
            }
        });

        let db_pl_sel = db.clone();
        let w_pl_sel = win_weak.clone();
        let app_rom_pl_sel = self.show_romanized.clone();
        self.window.on_on_select_playlist(move |pl_id| {
            if let Some(w) = w_pl_sel.upgrade() {
                if let Ok(Some(pl)) = db_pl_sel.get_playlist(pl_id as i64) {
                    w.set_selected_playlist_id(pl_id);
                    w.set_selected_playlist_name(SharedString::from(pl.name));
                    w.set_playlist_search_query(SharedString::from(""));
                    w.set_playlist_sort_criteria(0);
                    w.set_playlist_sort_ascending(true);
                    if let Ok(items) = db_pl_sel.get_playlist_tracks(pl_id as i64) {
                        let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                        let models = Self::convert_tracks_for_window(&tracks, &w);
                        w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                    }
                }
            }
        });

        let db_pl_play = db.clone();
        let q_pl_play = q_ref.clone();
        let p_pl_play = player.clone();
        let w_pl_play = win_weak.clone();
        self.window.on_on_play_playlist(move |pl_id| {
            if let Ok(items) = db_pl_play.get_playlist_tracks(pl_id as i64) {
                let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                if !tracks.is_empty() {
                    let first_track = tracks[0].clone();
                    let mut q = q_pl_play.borrow_mut();
                    q.set_items(tracks, Some(0));
                    if let Some(w) = w_pl_play.upgrade() {
                        Self::sync_queue_to_ui(&q, &w);
                    }
                    drop(q);
                    Self::play_track_record(&p_pl_play, &first_track, &w_pl_play, &db_pl_play);
                }
            }
        });

        self.window.on_on_export_playlist(move |pl_id| {
            std::thread::spawn(move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Extended M3U8 Playlist", &["m3u8"])
                    .set_file_name("playlist.m3u8")
                    .save_file()
                {
                    let db_path = default_db_path();
                    if let Ok(worker_db) = open_database(&db_path) {
                        let _ = PlaylistManager::export_m3u8(&worker_db, pl_id as i64, &path);
                    }
                }
            });
        });

        let w_pl_imp = win_weak.clone();
        self.window.on_on_import_playlist(move || {
            let w_c = w_pl_imp.clone();
            std::thread::spawn(move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Extended M3U8 Playlist", &["m3u8", "m3u"])
                    .pick_file()
                {
                    let db_path = default_db_path();
                    if let Ok(worker_db) = open_database(&db_path) {
                        if let Ok((_pl_id, _count)) = PlaylistManager::import_m3u8(&worker_db, &path, None) {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = w_c.upgrade() {
                                    Self::sync_playlists_to_ui(&worker_db, &w);
                                }
                            });
                        }
                    }
                }
            });
        });

        let db_pl_rm = db.clone();
        let w_pl_rm = win_weak.clone();
        self.window.on_on_remove_track_from_playlist(move |pl_id, track_id| {
            let _ = db_pl_rm.remove_track_from_playlist(pl_id as i64, track_id as i64);
            if let Some(w) = w_pl_rm.upgrade() {
                if let Ok(items) = db_pl_rm.get_playlist_tracks(pl_id as i64) {
                    let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                }
                Self::sync_playlists_to_ui(&db_pl_rm, &w);
            }
        });

        // 8c-pl. Playlist Sort & Search Callbacks
        let db_pl_sort = db.clone();
        let w_pl_sort = win_weak.clone();
        let app_rom_pl_sort = self.show_romanized.clone();
        self.window.on_on_playlist_sort_changed(move |crit_id, asc| {
            if let Some(w) = w_pl_sort.upgrade() {
                w.set_playlist_sort_criteria(crit_id);
                w.set_playlist_sort_ascending(asc);
                let pl_id = w.get_selected_playlist_id();
                let q_str = w.get_playlist_search_query().to_string();
                let search_param = if q_str.trim().is_empty() { None } else { Some(q_str.as_str()) };

                if pl_id >= 0 {
                    if let Ok(items) = db_pl_sort.get_playlist_tracks_filtered(pl_id as i64, search_param, crit_id, asc) {
                        let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                        let models = Self::convert_tracks_for_window(&tracks, &w);
                        w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                    }
                }
            }
        });

        let db_pl_search = db.clone();
        let w_pl_search = win_weak.clone();
        let app_rom_pl_s = self.show_romanized.clone();
        self.window.on_on_playlist_search_changed(move |query| {
            if let Some(w) = w_pl_search.upgrade() {
                let pl_id = w.get_selected_playlist_id();
                let crit_id = w.get_playlist_sort_criteria();
                let asc = w.get_playlist_sort_ascending();
                let q_str = query.to_string();
                let search_param = if q_str.trim().is_empty() { None } else { Some(q_str.as_str()) };

                if pl_id >= 0 {
                    if let Ok(items) = db_pl_search.get_playlist_tracks_filtered(pl_id as i64, search_param, crit_id, asc) {
                        let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                        let models = Self::convert_tracks_for_window(&tracks, &w);
                        w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                    }
                }
            }
        });

        // 8c. Context Menu and Batch Action Callbacks
        let q_ctx_q = q_ref.clone();
        let w_ctx_q = win_weak.clone();
        let db_ctx_q = db.clone();
        self.window.on_on_add_to_queue(move |track_id| {
            if let Ok(Some(track)) = db_ctx_q.get_track_by_id(track_id as i64) {
                let mut q = q_ctx_q.borrow_mut();
                q.append_tracks(vec![track]);
                if let Some(w) = w_ctx_q.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
            }
        });

        let pending_pl_add: Rc<RefCell<Option<(i64, Vec<i64>)>>> = Rc::new(RefCell::new(None));

        let db_ctx_pl = db.clone();
        let w_ctx_pl = win_weak.clone();
        let pend_ctx_pl = pending_pl_add.clone();
        self.window.on_on_add_to_playlist(move |playlist_id, track_id| {
            let pl_id = playlist_id as i64;
            let t_id = track_id as i64;
            if let Some(w) = w_ctx_pl.upgrade() {
                Self::try_add_tracks_to_playlist(&db_ctx_pl, &w, &pend_ctx_pl, pl_id, vec![t_id]);
            }
        });

        let db_ctx_del = db.clone();
        let w_ctx_del = win_weak.clone();
        let q_ctx_del = q_ref.clone();
        let app_scope_del = self.active_scope.clone();
        let app_sort_del = self.sort_criteria.clone();
        let app_asc_del = self.sort_ascending.clone();
        let app_rom_del = self.show_romanized.clone();
        self.window.on_on_delete_track_from_db(move |track_id| {
            let _ = db_ctx_del.delete_track(track_id as i64);
            // Also remove from playback queue if present
            let mut q = q_ctx_del.borrow_mut();
            if let Some(pos) = q.items().iter().position(|t| t.id == track_id as i64) {
                q.remove_at(pos);
            }
            if let Some(w) = w_ctx_del.upgrade() {
                Self::sync_queue_to_ui(&q, &w);
                if let Ok(tracks) = db_ctx_del.list_tracks(app_scope_del.get(), app_sort_del.get(), app_asc_del.get()) {
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                }
                Self::sync_playlists_to_ui(&db_ctx_del, &w);
            }
        });

        // 8d. Multi-Selection Checkbox & Batch Callbacks
        let w_sel_toggle = win_weak.clone();
        self.window.on_on_toggle_select_track(move |track_id| {
            if let Some(w) = w_sel_toggle.upgrade() {
                let model = w.get_library_tracks();
                let mut sel_count = 0;
                for i in 0..model.row_count() {
                    if let Some(mut tr) = model.row_data(i) {
                        if tr.id == track_id {
                            tr.is_selected = !tr.is_selected;
                            model.set_row_data(i, tr.clone());
                        }
                        if tr.is_selected {
                            sel_count += 1;
                        }
                    }
                }
                w.set_selected_count(sel_count);
            }
        });

        let w_sel_all = win_weak.clone();
        self.window.on_on_select_all_tracks(move || {
            if let Some(w) = w_sel_all.upgrade() {
                let model = w.get_library_tracks();
                let total = model.row_count();
                for i in 0..total {
                    if let Some(mut tr) = model.row_data(i) {
                        tr.is_selected = true;
                        model.set_row_data(i, tr);
                    }
                }
                w.set_selected_count(total as i32);
            }
        });

        let w_sel_clear = win_weak.clone();
        self.window.on_on_clear_selection(move || {
            if let Some(w) = w_sel_clear.upgrade() {
                let model = w.get_library_tracks();
                for i in 0..model.row_count() {
                    if let Some(mut tr) = model.row_data(i) {
                        if tr.is_selected {
                            tr.is_selected = false;
                            model.set_row_data(i, tr);
                        }
                    }
                }
                w.set_selected_count(0);
            }
        });

        let w_batch_q = win_weak.clone();
        let db_batch_q = db.clone();
        let q_batch_q = q_ref.clone();
        self.window.on_on_add_selected_to_queue(move || {
            if let Some(w) = w_batch_q.upgrade() {
                let model = w.get_library_tracks();
                let mut to_add = Vec::new();
                for i in 0..model.row_count() {
                    if let Some(tr) = model.row_data(i) {
                        if tr.is_selected {
                            if let Ok(Some(track)) = db_batch_q.get_track_by_id(tr.id as i64) {
                                to_add.push(track);
                            }
                        }
                    }
                }
                if !to_add.is_empty() {
                    let mut q = q_batch_q.borrow_mut();
                    q.append_tracks(to_add);
                    Self::sync_queue_to_ui(&q, &w);
                }
            }
        });

        let w_batch_pl = win_weak.clone();
        let db_batch_pl = db.clone();
        let pend_batch_pl = pending_pl_add.clone();
        self.window.on_on_add_selected_to_playlist(move |playlist_id| {
            if let Some(w) = w_batch_pl.upgrade() {
                let model = w.get_library_tracks();
                let mut track_ids = Vec::new();
                for i in 0..model.row_count() {
                    if let Some(tr) = model.row_data(i) {
                        if tr.is_selected {
                            track_ids.push(tr.id as i64);
                        }
                    }
                }
                if !track_ids.is_empty() {
                    Self::try_add_tracks_to_playlist(
                        &db_batch_pl,
                        &w,
                        &pend_batch_pl,
                        playlist_id as i64,
                        track_ids,
                    );
                }
            }
        });

        let w_batch_create_pl = win_weak.clone();
        let db_batch_create_pl = db.clone();
        self.window.on_on_create_playlist_and_add_selected(move |name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            if let Some(w) = w_batch_create_pl.upgrade() {
                let model = w.get_library_tracks();
                let mut track_ids = Vec::new();
                for i in 0..model.row_count() {
                    if let Some(tr) = model.row_data(i) {
                        if tr.is_selected {
                            track_ids.push(tr.id as i64);
                        }
                    }
                }
                if let Ok(new_pl_id) = db_batch_create_pl.create_playlist(trimmed, None, None) {
                    if !track_ids.is_empty() {
                        let _ = db_batch_create_pl.add_tracks_to_playlist_batch(
                            new_pl_id,
                            &track_ids,
                            false,
                        );
                    }
                    Self::sync_playlists_to_ui(&db_batch_create_pl, &w);
                }
            }
        });

        let w_dup_res = win_weak.clone();
        let db_dup_res = db.clone();
        let pend_dup_res = pending_pl_add.clone();
        self.window.on_on_resolve_playlist_duplicate(move |action| {
            let pending = pend_dup_res.borrow_mut().take();
            if let Some((pl_id, track_ids)) = pending {
                match action {
                    1 => {
                        // Skip duplicates
                        let _ = db_dup_res.add_tracks_to_playlist_batch(pl_id, &track_ids, true);
                    }
                    2 => {
                        // Add anyway
                        let _ = db_dup_res.add_tracks_to_playlist_batch(pl_id, &track_ids, false);
                    }
                    _ => {
                        // Cancel (0)
                    }
                }
                if action == 1 || action == 2 {
                    if let Some(w) = w_dup_res.upgrade() {
                        Self::sync_playlists_to_ui(&db_dup_res, &w);
                        if w.get_selected_playlist_id() == pl_id as i32 {
                            if let Ok(items) = db_dup_res.get_playlist_tracks(pl_id) {
                                let tracks: Vec<TrackRecord> =
                                    items.into_iter().map(|item| item.track).collect();
                                let models = Self::convert_tracks_for_window(&tracks, &w);
                                w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                            }
                        }
                    }
                }
            }
        });

        let w_batch_del = win_weak.clone();
        let db_batch_del = db.clone();
        let q_batch_del = q_ref.clone();
        let app_scope_bdel = self.active_scope.clone();
        let app_sort_bdel = self.sort_criteria.clone();
        let app_asc_bdel = self.sort_ascending.clone();
        let app_rom_bdel = self.show_romanized.clone();
        self.window.on_on_delete_selected_from_db(move || {
            if let Some(w) = w_batch_del.upgrade() {
                let model = w.get_library_tracks();
                let mut del_ids = Vec::new();
                for i in 0..model.row_count() {
                    if let Some(tr) = model.row_data(i) {
                        if tr.is_selected {
                            del_ids.push(tr.id as i64);
                        }
                    }
                }
                let mut q = q_batch_del.borrow_mut();
                for id in del_ids {
                    let _ = db_batch_del.delete_track(id);
                    if let Some(pos) = q.items().iter().position(|t| t.id == id) {
                        q.remove_at(pos);
                    }
                }
                Self::sync_queue_to_ui(&q, &w);
                if let Ok(tracks) = db_batch_del.list_tracks(app_scope_bdel.get(), app_sort_bdel.get(), app_asc_bdel.get()) {
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                }
                w.set_selected_count(0);
                Self::sync_playlists_to_ui(&db_batch_del, &w);
            }
        });

        // 9. Folder switch toggle (Strict Stop < 50ms & instant UI update)
        let db10 = db.clone();
        let p10 = player.clone();
        let q10 = q_ref.clone();
        let w10 = win_weak.clone();
        let app_scope10 = self.active_scope.clone();
        let app_sort10 = self.sort_criteria.clone();
        let app_asc10 = self.sort_ascending.clone();
        self.window.on_on_toggle_folder(move |folder_id, enabled| {
            let _ = db10.set_folder_enabled(folder_id as i64, enabled);
            if !enabled {
                let mut q = q10.borrow_mut();
                let active_removed = q.remove_folder_tracks(folder_id as i64);
                if active_removed {
                    let _ = p10.stop();
                    if let Some(w) = w10.upgrade() {
                        w.set_is_playing(false);
                        w.set_track_title(SharedString::from("Pemutaran Dihentikan"));
                        w.set_track_artist(SharedString::from("Folder dinonaktifkan"));
                    }
                }
            }

            // Instantly refresh folders and tracks in UI
            if let Some(w) = w10.upgrade() {
                Self::sync_folders_and_scopes_to_ui(&db10, &w, app_scope10.get());
                if let Ok(tracks) = db10.list_tracks(app_scope10.get(), app_sort10.get(), app_asc10.get()) {
                    let models = Self::convert_tracks_for_window(&tracks, &w);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                }
            }
        });

        // 10. Hearing Safety Modal confirmation
        let w11 = win_weak.clone();
        let p11 = player.clone();
        self.window.on_on_confirm_safety_modal(move || {
            if let Some(w) = w11.upgrade() {
                w.set_is_safety_modal_visible(false);
                w.set_is_testing_audio(false);
            }
            let _ = p11.stop();
        });

        // 11. 10-Second Test Audio Toggle & Test Volume
        let w12 = win_weak.clone();
        let p12 = player.clone();
        self.window.on_on_toggle_test_audio(move || {
            if let Some(w) = w12.upgrade() {
                let is_testing = !w.get_is_testing_audio();
                w.set_is_testing_audio(is_testing);
                if is_testing {
                    let _ = p12.play_test_tone();
                } else {
                    let _ = p12.stop();
                }
            }
        });

        let w_tvol = win_weak.clone();
        let vol_ctrl = self.win_volume.clone();
        self.window.on_on_set_test_volume(move |v| {
            vol_ctrl.set_volume(v);
            if let Some(w) = w_tvol.upgrade() {
                w.set_windows_master_volume(v);
            }
        });

        // 12. Window controls
        let w13 = win_weak.clone();
        self.window.on_on_window_minimize(move || {
            if let Some(w) = w13.upgrade() {
                w.window().set_minimized(true);
            }
        });

        let w14 = win_weak.clone();
        self.window.on_on_window_maximize(move || {
            if let Some(w) = w14.upgrade() {
                w.window().set_maximized(!w.window().is_maximized());
            }
        });

        let w15 = win_weak.clone();
        self.window.on_on_window_close(move || {
            if let Some(w) = w15.upgrade() {
                // Sembunyikan window dari layar dan taskbar ke Windows System Tray
                let _ = w.window().with_winit_window(|win| {
                    win.set_visible(false);
                });
            }
        });

        let w_drag = win_weak.clone();
        self.window.on_on_window_drag(move || {
            if let Some(w) = w_drag.upgrade() {
                let _ = w.window().with_winit_window(|win| {
                    let _ = win.drag_window();
                });
            }
        });

        // 13. Exclusive mode toggle with Audiophile Advisory Dialog
        let p16 = player.clone();
        let w16 = win_weak.clone();
        self.window.on_on_toggle_exclusive(move |en| {
            if let Some(w) = w16.upgrade() {
                // Jangan izinkan aktif jika device tidak mendukung bit-perfect
                if en && !w.get_can_enable_exclusive() {
                    w.set_is_exclusive(false);
                    return;
                }
                let _ = p16.set_exclusive(en);
                w.set_is_exclusive(en);
                // Jika user mengaktifkan Exclusive dan EQ sedang nyala, tampilkan rekomendasi popup
                if en && w.get_is_eq_enabled() {
                    w.set_is_audiophile_modal_visible(true);
                }
            }
        });

        // 13b. Select Audio Output Device
        let w_sel_dev = win_weak.clone();
        let p_sel_dev = player.clone();
        self.window.on_on_select_device(move |dev_id| {
            let id_str = dev_id.to_string();
            let _ = p_sel_dev.set_device(Some(id_str.clone()));
            if let Some(w) = w_sel_dev.upgrade() {
                Self::update_devices_in_ui(&w, Some(&id_str));
            }
        });

        // 14. Audiophile Dialog confirmations
        let p_bypass = player.clone();
        let w_bypass = win_weak.clone();
        let db_bypass = db.clone();
        self.window.on_on_confirm_bypass_eq(move || {
            let _ = p_bypass.set_eq_enabled(false);
            let _ = db_bypass.set_preference("eq_enabled", "0");
            if let Some(w) = w_bypass.upgrade() {
                w.set_is_eq_enabled(false);
                w.set_is_audiophile_modal_visible(false);
            }
        });

        let w_keep = win_weak.clone();
        self.window.on_on_confirm_keep_eq(move || {
            if let Some(w) = w_keep.upgrade() {
                w.set_is_audiophile_modal_visible(false);
            }
        });

        // 15. EQ Toggle & Bands & Presets
        let p_eq = player.clone();
        let w_eq = win_weak.clone();
        let db_eq = db.clone();
        self.window.on_on_toggle_eq(move |en| {
            let _ = p_eq.set_eq_enabled(en);
            let _ = db_eq.set_preference("eq_enabled", if en { "1" } else { "0" });
            if let Some(w) = w_eq.upgrade() {
                w.set_is_eq_enabled(en);
            }
        });

        let p_band = player.clone();
        self.window.on_on_set_eq_band(move |band_idx, gain_db| {
            let _ = p_band.set_eq_band(band_idx as usize, gain_db);
        });

        let p_pre = player.clone();
        let w_pre = win_weak.clone();
        self.window.on_on_select_eq_preset(move |preset_idx| {
            let preset = match preset_idx {
                0 => EqPreset::Flat,
                1 => EqPreset::BassBoost,
                2 => EqPreset::VocalClear,
                3 => EqPreset::Acoustic,
                4 => EqPreset::TrebleBoost,
                _ => EqPreset::Custom,
            };
            let _ = p_pre.set_eq_preset(preset);
            let gains = preset.gains();
            if let Some(w) = w_pre.upgrade() {
                w.set_eq_bands(ModelRc::new(VecModel::from(gains.to_vec())));
            }
        });

        // 16. Volume Balance (ReplayGain) Selector
        let p_bal = player.clone();
        let w_bal = win_weak.clone();
        let db_bal = db.clone();
        self.window.on_on_set_volume_balance(move |mode_idx| {
            let mode = match mode_idx {
                1 => VolumeBalanceMode::Quiet,
                2 => VolumeBalanceMode::Balanced,
                3 => VolumeBalanceMode::Punchy,
                _ => VolumeBalanceMode::Off,
            };
            let _ = p_bal.set_volume_balance(mode);
            let _ = db_bal.set_preference("volume_balance", &mode_idx.to_string());
            if let Some(w) = w_bal.upgrade() {
                w.set_volume_balance_mode(mode_idx);
            }
        });

        // 17. Rescan single folder (with full progress notification and folder stats sync)
        let w_rescan = win_weak.clone();
        let app_scope_rescan = self.active_scope.clone();
        let app_sort_rescan = self.sort_criteria.clone();
        let app_asc_rescan = self.sort_ascending.clone();
        let app_rom_rescan = self.show_romanized.clone();
        self.window.on_on_rescan_folder(move |folder_id| {
            let win_c = w_rescan.clone();
            let db_path = default_db_path();
            let cur_scope = app_scope_rescan.get();
            let cur_sort = app_sort_rescan.get();
            let cur_asc = app_asc_rescan.get();
            let cur_rom = app_rom_rescan.get();

            std::thread::spawn(move || {
                if let Ok(worker_db) = open_database(&db_path) {
                    if let Ok(folders) = worker_db.get_all_folders() {
                        if let Some(folder) = folders.iter().find(|f| f.id == folder_id as i64) {
                            let folder_path = std::path::PathBuf::from(&folder.folder_path);
                            let folder_name_display = folder_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&folder.folder_path)
                                .to_string();

                            // Show scanning banner
                            let win_start = win_c.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_start.upgrade() {
                                    w.set_is_scanning(true);
                                    w.set_scan_status_text(SharedString::from(format!(
                                        "Memindai ulang folder '{}'...",
                                        folder_name_display
                                    )));
                                }
                            });

                            let (prog_tx, prog_rx) = crossbeam_channel::unbounded();
                            let win_prog = win_c.clone();

                            let monitor_handle = std::thread::spawn(move || {
                                while let Ok(msg) = prog_rx.recv() {
                                    match msg {
                                        ScanProgress::Started { total_estimated } => {
                                            let w_p = win_prog.clone();
                                            let _ = slint::invoke_from_event_loop(move || {
                                                if let Some(w) = w_p.upgrade() {
                                                    w.set_scan_status_text(SharedString::from(format!(
                                                        "Ditemukan {} file musik. Memperbarui metadata...",
                                                        total_estimated
                                                    )));
                                                }
                                            });
                                        }
                                        ScanProgress::FileScanned { current, total, .. } => {
                                            if current % 3 == 0 || current == total {
                                                let w_p = win_prog.clone();
                                                let _ = slint::invoke_from_event_loop(move || {
                                                    if let Some(w) = w_p.upgrade() {
                                                        w.set_scan_status_text(SharedString::from(format!(
                                                            "Membaca lagu {} dari {}...",
                                                            current, total
                                                        )));
                                                    }
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            });

                            let scanner = Scanner::new();
                            let _ = scanner.scan_folder_with_options(
                                &worker_db,
                                folder.id,
                                &folder_path,
                                None,
                                Some(&prog_tx),
                                true,
                            );

                            drop(prog_tx);
                            let _ = monitor_handle.join();

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_c.upgrade() {
                                    w.set_is_scanning(false);
                                    Self::sync_folders_and_scopes_to_ui(&worker_db, &w, cur_scope);
                                    if let Ok(tracks) = worker_db.list_tracks(cur_scope, cur_sort, cur_asc) {
                                        let models = Self::convert_tracks_for_window(&tracks, &w);
                                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                                    }
                                    Self::reload_active_lyrics_if_playing(&worker_db, &w);
                                }
                            });
                        }
                    }
                }
            });
        });

        let w_rescan_all = win_weak.clone();
        let app_scope_all = self.active_scope.clone();
        let app_sort_all = self.sort_criteria.clone();
        let app_asc_all = self.sort_ascending.clone();
        let app_rom_all = self.show_romanized.clone();
        self.window.on_on_rescan_all_folders(move || {
            let win_c = w_rescan_all.clone();
            let db_path = default_db_path();
            let cur_scope = app_scope_all.get();
            let cur_sort = app_sort_all.get();
            let cur_asc = app_asc_all.get();
            let cur_rom = app_rom_all.get();

            std::thread::spawn(move || {
                if let Ok(worker_db) = open_database(&db_path) {
                    if let Ok(folders) = worker_db.get_all_folders() {
                        let enabled_folders: Vec<_> =
                            folders.into_iter().filter(|f| f.is_enabled).collect();
                        if enabled_folders.is_empty() {
                            return;
                        }

                        let win_start = win_c.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win_start.upgrade() {
                                w.set_is_scanning(true);
                                w.set_scan_status_text(SharedString::from(
                                    "Menyegarkan ulang seluruh pustaka & lirik...",
                                ));
                            }
                        });

                        let scanner = Scanner::new();
                        for (idx, folder) in enabled_folders.iter().enumerate() {
                            let folder_path = std::path::PathBuf::from(&folder.folder_path);
                            if !folder_path.exists() {
                                continue;
                            }
                            let w_p = win_c.clone();
                            let total_f = enabled_folders.len();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = w_p.upgrade() {
                                    w.set_scan_status_text(SharedString::from(format!(
                                        "Menyegarkan folder {} dari {}...",
                                        idx + 1,
                                        total_f
                                    )));
                                }
                            });
                            let _ = scanner.scan_folder_with_options(
                                &worker_db,
                                folder.id,
                                &folder_path,
                                None,
                                None,
                                true,
                            );
                        }

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win_c.upgrade() {
                                w.set_is_scanning(false);
                                Self::sync_folders_and_scopes_to_ui(&worker_db, &w, cur_scope);
                                if let Ok(tracks) = worker_db.list_tracks(cur_scope, cur_sort, cur_asc) {
                                    let models = Self::convert_tracks_for_window(&tracks, &w);
                                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                                }
                                Self::reload_active_lyrics_if_playing(&worker_db, &w);
                            }
                        });
                    }
                }
            });
        });

        // 18. Lyrics Kinetic Scroll Callbacks
        let scroll_cell = self.last_lyrics_scroll_time.clone();
        let w_scroll = win_weak.clone();
        self.window.on_on_lyrics_user_scrolled(move || {
            scroll_cell.set(Some(Instant::now()));
            if let Some(w) = w_scroll.upgrade() {
                w.set_is_user_scrolling_lyrics(true);
            }
        });

        let scroll_resync = self.last_lyrics_scroll_time.clone();
        let w_resync = win_weak.clone();
        self.window.on_on_lyrics_resync_clicked(move || {
            scroll_resync.set(None);
            if let Some(w) = w_resync.upgrade() {
                w.set_is_user_scrolling_lyrics(false);
            }
        });

        // 19. Shuffle style selector
        let q17 = q_ref.clone();
        let db17 = db.clone();
        let w17 = win_weak.clone();
        self.window.on_on_select_shuffle_style(move |style_idx| {
            let style = ShuffleStyle::from_i32(style_idx);
            q17.borrow_mut().set_shuffle_style(style);
            let _ = db17.set_preference("shuffle_style", &style_idx.to_string());
            if let Some(w) = w17.upgrade() {
                w.set_shuffle_style(style_idx);
            }
        });

        // 20. Dark mode toggle
        let db18 = db.clone();
        self.window.on_on_toggle_dark_mode(move |dark| {
            let _ = db18.set_preference("dark_mode", if dark { "1" } else { "0" });
        });

        // 20b. Romanized text toggle
        let db_rom = db.clone();
        let w_rom = win_weak.clone();
        let app_rom_toggle = self.show_romanized.clone();
        let app_rom_title = self.romanize_title.clone();
        let app_rom_artist = self.romanize_artist.clone();
        let app_rom_lyrics = self.romanize_lyrics.clone();
        let app_scope_rom = self.active_scope.clone();
        let app_sort_rom = self.sort_criteria.clone();
        let app_asc_rom = self.sort_ascending.clone();
        let q_rom = q_ref.clone();

        let refresh_ui_romanization = {
            let db = db_rom.clone();
            let app_scope = app_scope_rom.clone();
            let app_sort = app_sort_rom.clone();
            let app_asc = app_asc_rom.clone();
            let q_ref = q_rom.clone();
            move |w: &AppWindow, en: bool, r_title: bool, r_artist: bool, r_lyrics: bool| {
                if let Ok(tracks) = db.list_tracks(app_scope.get(), app_sort.get(), app_asc.get()) {
                    let models = Self::convert_tracks_to_models_detailed(&tracks, en, r_title, r_artist);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                }
                let sel_pl_id = w.get_selected_playlist_id();
                if sel_pl_id >= 0 {
                    let crit_id = w.get_playlist_sort_criteria();
                    let asc = w.get_playlist_sort_ascending();
                    let q_str = w.get_playlist_search_query().to_string();
                    let search_param = if q_str.trim().is_empty() { None } else { Some(q_str.as_str()) };
                    if let Ok(items) = db.get_playlist_tracks_filtered(sel_pl_id as i64, search_param, crit_id, asc) {
                        let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                        let models = Self::convert_tracks_to_models_detailed(&tracks, en, r_title, r_artist);
                        w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                    }
                }
                let q = q_ref.borrow();
                Self::sync_queue_to_ui_detailed(&q, w, en, r_title, r_artist);

                let playing_id = w.get_active_playing_id();
                if playing_id > 0 {
                    if let Ok(Some(track)) = db.get_track_by_id(playing_id as i64) {
                        let title = Self::format_text_display(&track.title, en && r_title);
                        let artist = Self::format_text_display(&track.artist, en && r_artist);
                        let album = Self::format_text_display(&track.album, en && r_title);
                        w.set_track_title(SharedString::from(title));
                        w.set_track_artist(SharedString::from(artist));
                        w.set_track_album(SharedString::from(album));

                        if let Ok(Some(lyrics_rec)) = db.get_lyrics(track.id) {
                            let line_models: Vec<LyricLineModel> = lyrics_rec
                                .lines
                                .iter()
                                .map(|l| {
                                    let text = Self::format_text_display(&l.text, en && r_lyrics);
                                    LyricLineModel {
                                        timestamp_ms: l.timestamp_ms as i32,
                                        text: SharedString::from(text),
                                        is_active: false,
                                    }
                                })
                                .collect();
                            w.set_lyrics_lines(ModelRc::new(VecModel::from(line_models)));
                        }
                    }
                }
            }
        };

        let refresh_fn_1 = refresh_ui_romanization.clone();
        let db_rom_1 = db_rom.clone();
        let w_rom_1 = w_rom.clone();
        let app_rom_title_1 = app_rom_title.clone();
        let app_rom_artist_1 = app_rom_artist.clone();
        let app_rom_lyrics_1 = app_rom_lyrics.clone();
        self.window.on_on_toggle_romanized(move |en| {
            app_rom_toggle.set(en);
            let _ = db_rom_1.set_preference("show_romanized", if en { "1" } else { "0" });
            if let Some(w) = w_rom_1.upgrade() {
                w.set_show_romanized(en);
                refresh_fn_1(&w, en, app_rom_title_1.get(), app_rom_artist_1.get(), app_rom_lyrics_1.get());
            }
        });

        let db_rom_2 = db_rom.clone();
        let w_rom_2 = w_rom.clone();
        let app_rom_toggle_2 = self.show_romanized.clone();
        self.window.on_on_toggle_romanize_scope(move |scope_type, en| {
            match scope_type {
                0 => {
                    app_rom_title.set(en);
                    let _ = db_rom_2.set_preference("romanize_title", if en { "1" } else { "0" });
                }
                1 => {
                    app_rom_artist.set(en);
                    let _ = db_rom_2.set_preference("romanize_artist", if en { "1" } else { "0" });
                }
                2 => {
                    app_rom_lyrics.set(en);
                    let _ = db_rom_2.set_preference("romanize_lyrics", if en { "1" } else { "0" });
                }
                _ => {}
            }

            if let Some(w) = w_rom_2.upgrade() {
                w.set_romanize_title(app_rom_title.get());
                w.set_romanize_artist(app_rom_artist.get());
                w.set_romanize_lyrics(app_rom_lyrics.get());
                refresh_ui_romanization(&w, app_rom_toggle_2.get(), app_rom_title.get(), app_rom_artist.get(), app_rom_lyrics.get());
            }
        });

        // 21. Native Windows File Explorer - Add Music Folder Dialog
        let win_add = win_weak.clone();
        let app_scope_add = self.active_scope.clone();
        let app_sort_add = self.sort_criteria.clone();
        let app_asc_add = self.sort_ascending.clone();
        let app_rom_add = self.show_romanized.clone();

        self.window.on_on_add_folder_clicked(move || {
            let win_c = win_add.clone();
            let cur_scope = app_scope_add.get();
            let cur_sort = app_sort_add.get();
            let cur_asc = app_asc_add.get();
            let cur_rom = app_rom_add.get();

            std::thread::spawn(move || {
                if let Some(folder_path) = rfd::FileDialog::new()
                    .set_title("Pilih Folder Musik")
                    .pick_folder()
                {
                    let path_str = folder_path.to_string_lossy().to_string();
                    let db_path = default_db_path();

                    // Tampilkan status scanning seketika di UI
                    let win_start = win_c.clone();
                    let folder_name_display = folder_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&path_str)
                        .to_string();

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win_start.upgrade() {
                            w.set_is_scanning(true);
                            w.set_scan_status_text(SharedString::from(format!(
                                "Mencari file musik di folder '{}'...",
                                folder_name_display
                            )));
                        }
                    });

                    if let Ok(worker_db) = open_database(&db_path) {
                        let folder_id = match worker_db.add_folder(&path_str) {
                            Ok(id) => id,
                            Err(_) => {
                                worker_db.get_all_folders().ok()
                                    .and_then(|folders| folders.into_iter().find(|f| f.folder_path == path_str))
                                    .map(|f| f.id)
                                    .unwrap_or(0)
                            }
                        };

                        if folder_id > 0 {
                            let (prog_tx, prog_rx) = crossbeam_channel::unbounded();
                            let win_prog = win_c.clone();

                            let monitor_handle = std::thread::spawn(move || {
                                while let Ok(msg) = prog_rx.recv() {
                                    match msg {
                                        ScanProgress::Started { total_estimated } => {
                                            let w_p = win_prog.clone();
                                            let _ = slint::invoke_from_event_loop(move || {
                                                if let Some(w) = w_p.upgrade() {
                                                    w.set_scan_status_text(SharedString::from(format!(
                                                        "Ditemukan {} file musik. Membaca metadata & cover art...",
                                                        total_estimated
                                                    )));
                                                }
                                            });
                                        }
                                        ScanProgress::FileScanned { current, total, .. } => {
                                            if current % 3 == 0 || current == total {
                                                let w_p = win_prog.clone();
                                                let _ = slint::invoke_from_event_loop(move || {
                                                    if let Some(w) = w_p.upgrade() {
                                                        w.set_scan_status_text(SharedString::from(format!(
                                                            "Membaca lagu {} dari {}...",
                                                            current, total
                                                        )));
                                                    }
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            });

                            let scanner = Scanner::new();
                            let _ = scanner.scan_folder(
                                &worker_db,
                                folder_id,
                                &folder_path,
                                None,
                                Some(&prog_tx),
                            );

                            drop(prog_tx);
                            let _ = monitor_handle.join();

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_c.upgrade() {
                                    w.set_is_scanning(false);

                                    // 1. Refresh folder scopes & settings folders
                                    if let Ok(folders) = worker_db.get_all_folders() {
                                        let mut folder_models = Vec::new();
                                        let mut scope_models = vec![FolderScopeModel {
                                            id: -1,
                                            display_name: SharedString::from("Semua Folder"),
                                            is_selected: cur_scope.is_none(),
                                        }];

                                        for f in &folders {
                                            folder_models.push(FolderEntryModel {
                                                id: f.id as i32,
                                                path: SharedString::from(f.folder_path.clone()),
                                                track_count: f.total_tracks as i32,
                                                is_enabled: f.is_enabled,
                                            });

                                            if f.is_enabled {
                                                let folder_name = std::path::Path::new(&f.folder_path)
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or(&f.folder_path);

                                                scope_models.push(FolderScopeModel {
                                                    id: f.id as i32,
                                                    display_name: SharedString::from(folder_name.to_string()),
                                                    is_selected: cur_scope == Some(f.id),
                                                });
                                            }
                                        }

                                        w.set_settings_folders(ModelRc::new(VecModel::from(folder_models)));
                                        w.set_folder_scopes(ModelRc::new(VecModel::from(scope_models)));
                                    }

                                    // 2. Refresh library tracks
                                    if let Ok(tracks) = worker_db.list_tracks(cur_scope, cur_sort, cur_asc) {
                                        let models = Self::convert_tracks_for_window(&tracks, &w);
                                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                                    }
                                }
                            });
                        }
                    }
                }
            });
        });

        // 22. Queue item double-clicked / selected to play
        let p_q = player.clone();
        let q_q = q_ref.clone();
        let w_q = win_weak.clone();
        let db_q = db.clone();
        self.window.on_on_play_queue_item(move |idx| {
            let mut q = q_q.borrow_mut();
            if let Some(track) = q.set_current_index(idx as usize).cloned() {
                if let Some(w) = w_q.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
                drop(q);
                Self::play_track_record(&p_q, &track, &w_q, &db_q);
            }
        });

        // 22b. Remove item from queue (via Swipe or Delete button)
        let q_rm = q_ref.clone();
        let w_rm = win_weak.clone();
        self.window.on_on_remove_queue_item(move |idx| {
            if idx >= 0 {
                let mut q = q_rm.borrow_mut();
                q.remove_at(idx as usize);
                if let Some(w) = w_rm.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
            }
        });

        // 22c. Move / Reorder item in queue (Up/Down)
        let q_mv = q_ref.clone();
        let w_mv = win_weak.clone();
        self.window.on_on_move_queue_item(move |from_idx, to_idx| {
            if from_idx >= 0 && to_idx >= 0 {
                let mut q = q_mv.borrow_mut();
                q.move_item(from_idx as usize, to_idx as usize);
                if let Some(w) = w_mv.upgrade() {
                    Self::sync_queue_to_ui(&q, &w);
                }
            }
        });

        // 22d. Reshuffle upcoming queue on demand
        let q_rshuf = q_ref.clone();
        let w_rshuf = win_weak.clone();
        self.window.on_on_reshuffle_queue(move || {
            let mut q = q_rshuf.borrow_mut();
            q.reshuffle_upcoming();
            if let Some(w) = w_rshuf.upgrade() {
                Self::sync_queue_to_ui(&q, &w);
            }
        });

        // 23. Clear queue (Keep currently playing song intact!)
        let q_clear = q_ref.clone();
        let w_clear = win_weak.clone();
        self.window.on_on_clear_queue(move || {
            let mut q = q_clear.borrow_mut();
            q.clear_upcoming();
            if let Some(w) = w_clear.upgrade() {
                Self::sync_queue_to_ui(&q, &w);
            }
        });
    }

    fn try_add_tracks_to_playlist(
        db: &Database,
        w: &AppWindow,
        pending_slot: &Rc<RefCell<Option<(i64, Vec<i64>)>>>,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) {
        if track_ids.is_empty() {
            return;
        }
        let mut dup_count = 0usize;
        for &tid in &track_ids {
            if db.is_track_in_playlist(playlist_id, tid).unwrap_or(false) {
                dup_count += 1;
            }
        }

        if dup_count == 0 {
            let _ = db.add_tracks_to_playlist_batch(playlist_id, &track_ids, false);
            Self::sync_playlists_to_ui(db, w);
            if w.get_selected_playlist_id() == playlist_id as i32 {
                if let Ok(items) = db.get_playlist_tracks(playlist_id) {
                    let tracks: Vec<TrackRecord> =
                        items.into_iter().map(|item| item.track).collect();
                    let models = Self::convert_tracks_for_window(&tracks, w);
                    w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                }
            }
        } else {
            let pl_name = db
                .get_playlist(playlist_id)
                .ok()
                .flatten()
                .map(|p| p.name)
                .unwrap_or_else(|| "Playlist".to_string());

            let msg = if track_ids.len() == 1 {
                format!(
                    "Lagu yang dipilih sudah ada di playlist \"{}\". Tetap tambahkan atau lewati?",
                    pl_name
                )
            } else {
                format!(
                    "{} dari {} lagu yang dipilih sudah ada di playlist \"{}\". Lewati lagu yang sama atau tetap tambahkan semua?",
                    dup_count,
                    track_ids.len(),
                    pl_name
                )
            };

            *pending_slot.borrow_mut() = Some((playlist_id, track_ids));
            w.set_duplicate_alert_message(SharedString::from(msg));
            w.set_show_duplicate_alert(true);
        }
    }

    fn reload_active_lyrics_if_playing(db: &Database, w: &AppWindow) {
        let active_id = w.get_active_playing_id();
        if active_id < 0 {
            return;
        }
        let rom = w.get_show_romanized();
        let rom_lyrics = w.get_romanize_lyrics();
        if let Ok(Some(lyrics_rec)) = db.get_lyrics(active_id as i64) {
            w.set_has_lyrics(true);
            let line_models: Vec<LyricLineModel> = lyrics_rec
                .lines
                .iter()
                .map(|l| {
                    let text = Self::format_text_display(&l.text, rom && rom_lyrics);
                    LyricLineModel {
                        timestamp_ms: l.timestamp_ms as i32,
                        text: SharedString::from(text),
                        is_active: false,
                    }
                })
                .collect();
            w.set_lyrics_lines(ModelRc::new(VecModel::from(line_models)));
        } else {
            w.set_has_lyrics(false);
            w.set_lyrics_lines(ModelRc::new(VecModel::from(Vec::<LyricLineModel>::new())));
        }
    }

    fn play_track_record(
        player: &AudioPlayer,
        track: &TrackRecord,
        window: &slint::Weak<AppWindow>,
        db: &Database,
    ) {
        let path = PathBuf::from(&track.path);
        let _ = player.play(path.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let _ = db.increment_play_count(track.id, now);
        Self::refresh_jump_list_async(db);

        if let Some(w) = window.upgrade() {
            let rom = w.get_show_romanized();
            let rom_title = w.get_romanize_title();
            let rom_artist = w.get_romanize_artist();
            let rom_lyrics = w.get_romanize_lyrics();

            let title = Self::format_text_display(&track.title, rom && rom_title);
            let artist = Self::format_text_display(&track.artist, rom && rom_artist);
            let album = Self::format_text_display(&track.album, rom && rom_title);

            w.set_track_title(SharedString::from(title.clone()));
            w.set_track_artist(SharedString::from(artist.clone()));
            w.set_track_album(SharedString::from(album));
            w.set_audio_format(SharedString::from(track.format.clone()));
            w.set_bit_depth(track.bit_depth as i32);
            w.set_sample_rate(track.sample_rate as i32);
            w.set_duration_ms(track.duration_ms as i32);
            w.set_active_playing_id(track.id as i32);
            w.set_is_playing(true);

            // Update Window title so Taskbar hover thumbnail displays current playing track
            let win_title = if artist.trim().is_empty() || artist == "Unknown Artist" {
                format!("{} • ShrimpMusic Player", title)
            } else {
                format!("{} — {} • ShrimpMusic Player", title, artist)
            };
            let _ = w.window().with_winit_window(|win| {
                win.set_title(&win_title);
            });

            // Instrumental detection
            let genre_lower = track.genre.as_deref().unwrap_or("").to_lowercase();
            let title_lower = track.title.to_lowercase();
            let is_inst = genre_lower.contains("instrumental")
                || genre_lower.contains("classical")
                || genre_lower.contains("soundtrack")
                || genre_lower.contains("ost")
                || title_lower.contains("(instrumental)")
                || title_lower.contains("[instrumental]");
            w.set_is_instrumental(is_inst);

            // Lyrics setup
            if let Ok(Some(lyrics_rec)) = db.get_lyrics(track.id) {
                w.set_has_lyrics(true);
                let line_models: Vec<LyricLineModel> = lyrics_rec
                    .lines
                    .iter()
                    .map(|l| {
                        let text = Self::format_text_display(&l.text, rom && rom_lyrics);
                        LyricLineModel {
                            timestamp_ms: l.timestamp_ms as i32,
                            text: SharedString::from(text),
                            is_active: false,
                        }
                    })
                    .collect();
                w.set_lyrics_lines(ModelRc::new(VecModel::from(line_models)));
            } else {
                w.set_has_lyrics(false);
                w.set_lyrics_lines(ModelRc::new(VecModel::from(Vec::new())));
            }

            // Waveform peaks extraction & caching
            if let Ok(Some(peaks)) = db.get_track_waveform(track.id) {
                w.set_waveform_peaks(ModelRc::new(VecModel::from(peaks)));
            } else {
                let db_path = default_db_path();
                let path_w = path.clone();
                let track_id = track.id;
                let win_w = window.clone();
                std::thread::spawn(move || {
                    if let Ok(peaks) = generate_waveform_peaks(&path_w) {
                        if let Ok(worker_db) = open_database(&db_path) {
                            let _ = worker_db.update_track_waveform(track_id, &peaks);
                        }
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win_w.upgrade() {
                                if w.get_active_playing_id() == track_id as i32 {
                                    w.set_waveform_peaks(ModelRc::new(VecModel::from(peaks)));
                                }
                            }
                        });
                    }
                });
            }

            // Album Artwork with safety check and on-demand fallback
            let mut loaded_cover = false;
            if let Some(thumb) = &track.thumbnail_path {
                let p = std::path::Path::new(thumb);
                if p.is_file() {
                    if let Ok(img) = slint::Image::load_from_path(p) {
                        w.set_cover_image(img);
                        w.set_has_cover(true);
                        loaded_cover = true;
                    }
                }
            }

            // Fallback: If thumbnail file is missing on disk, regenerate on-demand from audio file
            if !loaded_cover {
                let audio_path = std::path::Path::new(&track.path);
                if audio_path.is_file() {
                    if let Ok(meta) = crate::library::extract_metadata(audio_path) {
                        if let Some(cover_data) = &meta.cover_art {
                            let key = format!("{}-{}-{}", meta.album, meta.artist, meta.title);
                            if let Some(new_thumb) = crate::library::artwork::generate_and_save_thumbnail(cover_data, &key) {
                                let new_path = std::path::Path::new(&new_thumb);
                                if new_path.is_file() {
                                    if let Ok(img) = slint::Image::load_from_path(new_path) {
                                        w.set_cover_image(img);
                                        w.set_has_cover(true);
                                        loaded_cover = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !loaded_cover {
                w.set_has_cover(false);
            }
        }
    }

    fn setup_event_loop(&self) {
        let player = self.player.clone();
        let win_weak = self.window.as_weak();
        let q_ref = self.queue.clone();
        let db = self.db.clone();
        let taskbar = self.taskbar.clone();
        let tray = self.tray.clone();
        let last_scroll = self.last_lyrics_scroll_time.clone();
        let vol_ctrl = self.win_volume.clone();
        let dev_watcher = self.device_watcher.clone();
        let app_scope_ev = self.active_scope.clone();
        let app_sort_ev = self.sort_criteria.clone();
        let app_asc_ev = self.sort_ascending.clone();
        let taskbar_attached = Rc::new(Cell::new(false));

        self._timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(w) = win_weak.upgrade() else { return };

                // -3. Attach Windows Taskbar Thumbnail Toolbar once native HWND is ready
                if !taskbar_attached.get() {
                    let mut found_hwnd: Option<isize> = None;
                    let _ = w.window().with_winit_window(|win| {
                        use i_slint_backend_winit::winit::raw_window_handle::{
                            HasWindowHandle, RawWindowHandle,
                        };
                        if let Ok(handle) = win.window_handle() {
                            if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                                found_hwnd = Some(win32.hwnd.get());
                            }
                        }
                    });
                    if let Some(hwnd) = found_hwnd {
                        taskbar.borrow_mut().attach_window(hwnd);
                        taskbar_attached.set(true);
                    }
                }

                // -2b. Poll Windows Taskbar Thumbnail Buttons & Jump List IPC
                while let Some(tb_action) = {
                    let tb = taskbar.borrow();
                    tb.poll_actions()
                } {
                    match tb_action {
                        TaskbarAction::Previous => {
                            let mut q = q_ref.borrow_mut();
                            if let Some(track) = q.previous().cloned() {
                                Self::sync_queue_to_ui(&q, &w);
                                drop(q);
                                Self::play_track_record(&player, &track, &win_weak, &db);
                            }
                        }
                        TaskbarAction::PlayPause => {
                            if w.get_is_playing() {
                                let _ = player.pause();
                            } else {
                                let _ = player.resume();
                            }
                        }
                        TaskbarAction::Next => {
                            let mut q = q_ref.borrow_mut();
                            if let Some(track) = q.next().cloned() {
                                Self::sync_queue_to_ui(&q, &w);
                                drop(q);
                                Self::play_track_record(&player, &track, &win_weak, &db);
                            }
                        }
                        TaskbarAction::PlayTrack(track_id) => {
                            let _ = w.window().with_winit_window(|win| {
                                win.set_visible(true);
                                win.set_minimized(false);
                                let _ = win.focus_window();
                            });
                            Self::play_track_from_jump_list(
                                track_id,
                                &db,
                                &q_ref,
                                &player,
                                &win_weak,
                                app_scope_ev.get(),
                                app_sort_ev.get(),
                                app_asc_ev.get(),
                            );
                        }
                    }
                }

                // -2. Real-Time Audio Device Hotplug Monitoring (USB DAC / Type-C converter plug/unplug)
                if let Some(devs) = dev_watcher.poll_devices_changed() {
                    Self::apply_device_models_to_ui(&w, devs, None);
                }

                // -1. Poll Windows System Tray Events
                if let Some(tray_ctrl) = tray.borrow().as_ref() {
                    while let Some(action) = tray_ctrl.poll_events() {
                        match action {
                            TrayAction::ShowWindow => {
                                let _ = w.window().with_winit_window(|win| {
                                    win.set_visible(true);
                                    win.set_minimized(false);
                                    let _ = win.focus_window();
                                });
                            }
                            TrayAction::TogglePlayPause => {
                                if w.get_is_playing() {
                                    let _ = player.pause();
                                } else {
                                    let _ = player.resume();
                                }
                            }
                            TrayAction::ExitApp => {
                                std::process::exit(0);
                            }
                        }
                    }
                }

                // 0. Continuous Windows Master Volume Sync for Hearing Safety Modal
                if w.get_is_safety_modal_visible() {
                    let win_vol = vol_ctrl.get_volume();
                    let cur_vol = w.get_windows_master_volume();
                    if (win_vol - cur_vol).abs() > 0.005 {
                        w.set_windows_master_volume(win_vol);
                    }
                }

                // 1. Kinetic Lyrics 4-Second Auto-Scroll Re-sync Check
                if let Some(scroll_time) = last_scroll.get() {
                    if scroll_time.elapsed() >= Duration::from_secs(4) {
                        last_scroll.set(None);
                        w.set_is_user_scrolling_lyrics(false);
                    }
                }

                // 2. Poll Audio Engine Events
                while let Some(event) = player.poll_event() {
                    match event {
                        AudioEvent::Position(pos) => {
                            let pos_ms = pos.as_millis() as i32;
                            w.set_position_ms(pos_ms);

                            let lines_model = w.get_lyrics_lines();
                            let count = lines_model.row_count();
                            let mut active_idx = -1;
                            for i in 0..count {
                                if let Some(line) = lines_model.row_data(i) {
                                    if line.timestamp_ms <= pos_ms {
                                        active_idx = i as i32;
                                    } else {
                                        break;
                                    }
                                }
                            }
                            for i in 0..count {
                                if let Some(mut line) = lines_model.row_data(i) {
                                    let should_active = (i as i32) == active_idx;
                                    if line.is_active != should_active {
                                        line.is_active = should_active;
                                        lines_model.set_row_data(i, line);
                                    }
                                }
                            }

                            // Kinetic Auto-scroll lyrics smoothly if user not scrolling manually
                            if !w.get_is_user_scrolling_lyrics() && active_idx >= 0 {
                                w.set_active_lyric_index(active_idx);
                                let target_y = -((active_idx * 52) as f32 - 180.0).max(0.0);
                                w.set_lyrics_scroll_offset_y(target_y);
                            }
                        }
                        AudioEvent::StateChanged(state) => {
                            let is_p = state == PlayerState::Playing;
                            w.set_is_playing(is_p);
                            if !is_p && w.get_is_testing_audio() {
                                w.set_is_testing_audio(false);
                            }
                            taskbar.borrow_mut().set_playback_state(is_p);
                        }
                        AudioEvent::TrackStarted { spec, is_exclusive, .. } => {
                            w.set_audio_format(SharedString::from(spec.codec_name.clone()));
                            w.set_bit_depth(spec.bit_depth.unwrap_or(16) as i32);
                            w.set_sample_rate(spec.sample_rate as i32);
                            if let Some(dur) = spec.duration {
                                w.set_duration_ms(dur.as_millis() as i32);
                            }
                            w.set_is_exclusive(is_exclusive);
                        }
                        AudioEvent::EndOfTrack => {
                            let mut q = q_ref.borrow_mut();
                            if let Some(track) = q.next().cloned() {
                                Self::sync_queue_to_ui(&q, &w);
                                drop(q);
                                Self::play_track_record(&player, &track, &win_weak, &db);
                            } else {
                                w.set_is_playing(false);
                            }
                        }
                        AudioEvent::Error(err) => {
                            eprintln!("[AudioError] {}", err);
                        }
                    }
                }
            },
        );
    }
}

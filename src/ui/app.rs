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
use super::taskbar::TaskbarController;
use super::tray::{TrayAction, TrayController};

slint::include_modules!();

pub struct ShrimpMusicApp {
    window: AppWindow,
    db: Rc<Database>,
    player: Rc<AudioPlayer>,
    queue: Rc<RefCell<PlaybackQueue>>,
    taskbar: Rc<RefCell<TaskbarController>>,
    tray: Rc<RefCell<Option<TrayController>>>,
    active_scope: Cell<Option<i64>>,
    sort_criteria: Cell<SortCriteria>,
    sort_ascending: Cell<bool>,
    show_romanized: Cell<bool>,
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

        window.set_library_sort_criteria(sort_criteria.to_i32());
        window.set_library_sort_ascending(sort_ascending);
        window.set_show_romanized(show_romanized);

        let app = Self {
            window,
            db,
            player,
            queue,
            taskbar,
            tray,
            active_scope: Cell::new(None),
            sort_criteria: Cell::new(sort_criteria),
            sort_ascending: Cell::new(sort_ascending),
            show_romanized: Cell::new(show_romanized),
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

        Ok(app)
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
        tracks
            .iter()
            .map(|t| {
                let duration_sec = t.duration_ms / 1000;
                let mins = duration_sec / 60;
                let secs = duration_sec % 60;

                let title = Self::format_text_display(&t.title, romanize);
                let artist = Self::format_text_display(&t.artist, romanize);
                let album = Self::format_text_display(&t.album, romanize);

                TrackItemModel {
                    id: t.id as i32,
                    title: SharedString::from(title),
                    artist: SharedString::from(artist),
                    album: SharedString::from(album),
                    duration_str: SharedString::from(format!("{}:{:02}", mins, secs)),
                    format: SharedString::from(t.format.clone()),
                    is_hi_res: t.sample_rate >= 88200 || t.bit_depth >= 24,
                    is_active: t.is_active,
                }
            })
            .collect()
    }

    fn refresh_tracks(&self) {
        let scope = self.active_scope.get();
        let sort = self.sort_criteria.get();
        let asc = self.sort_ascending.get();
        let rom = self.show_romanized.get();

        if let Ok(tracks) = self.db.list_tracks(scope, sort, asc) {
            let models = Self::convert_tracks_to_models_with_option(&tracks, rom);

            let mut q = self.queue.borrow_mut();
            if q.is_empty() {
                q.set_items(tracks, None);
            }
            Self::sync_queue_to_ui_with_option(&q, &self.window, rom);

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
        Self::sync_queue_to_ui_with_option(queue, window, window.get_show_romanized())
    }

    fn sync_queue_to_ui_with_option(queue: &PlaybackQueue, window: &AppWindow, romanize: bool) {
        let models = Self::convert_tracks_to_models_with_option(queue.items(), romanize);
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
        self.window.on_on_track_double_clicked(move |id| {
            if let Ok(Some(track)) = db9.get_track_by_id(id as i64) {
                let mut q = q9.borrow_mut();
                if q.is_empty() {
                    if let Ok(all_tracks) = db9.list_tracks(None, SortCriteria::Title, true) {
                        q.set_items(all_tracks, None);
                    }
                }
                if let Some(idx) = q.items().iter().position(|t| t.id == id as i64) {
                    let items = q.items().to_vec();
                    q.set_items(items, Some(idx));
                }
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
                    let models = Self::convert_tracks_to_models_with_option(&tracks, app_rom.get());
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
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
                    let models = Self::convert_tracks_to_models_with_option(&tracks, app_rom_sort.get());
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
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
                        let models = Self::convert_tracks_to_models_with_option(&tracks, app_rom_s.get());
                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                    }
                } else {
                    if let Ok(results) = SearchEngine::search(&db_search, &trimmed) {
                        let models = Self::convert_tracks_to_models_with_option(&results.tracks, app_rom_s.get());
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
        self.window.on_on_select_playlist(move |pl_id| {
            if let Some(w) = w_pl_sel.upgrade() {
                if let Ok(Some(pl)) = db_pl_sel.get_playlist(pl_id as i64) {
                    w.set_selected_playlist_id(pl_id);
                    w.set_selected_playlist_name(SharedString::from(pl.name));
                    if let Ok(items) = db_pl_sel.get_playlist_tracks(pl_id as i64) {
                        let tracks: Vec<TrackRecord> = items.into_iter().map(|item| item.track).collect();
                        let models = Self::convert_tracks_to_models(&tracks);
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
                    let models = Self::convert_tracks_to_models(&tracks);
                    w.set_playlist_tracks(ModelRc::new(VecModel::from(models)));
                }
                Self::sync_playlists_to_ui(&db_pl_rm, &w);
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
                    let models = Self::convert_tracks_to_models(&tracks);
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
            let cur_scope_ref = app_scope_rescan.clone();
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
                            let _ = scanner.scan_folder(
                                &worker_db,
                                folder.id,
                                &folder_path,
                                None,
                                Some(&prog_tx),
                            );

                            drop(prog_tx);
                            let _ = monitor_handle.join();

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_c.upgrade() {
                                    w.set_is_scanning(false);
                                    let cur_scope = cur_scope_ref.get();
                                    Self::sync_folders_and_scopes_to_ui(&worker_db, &w, cur_scope);
                                    if let Ok(tracks) = worker_db.list_tracks(cur_scope, cur_sort, cur_asc) {
                                        let models = Self::convert_tracks_to_models_with_option(&tracks, cur_rom);
                                        w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                                    }
                                }
                            });
                        }
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
        let app_scope_rom = self.active_scope.clone();
        let app_sort_rom = self.sort_criteria.clone();
        let app_asc_rom = self.sort_ascending.clone();
        let q_rom = q_ref.clone();
        self.window.on_on_toggle_romanized(move |en| {
            app_rom_toggle.set(en);
            let _ = db_rom.set_preference("show_romanized", if en { "1" } else { "0" });
            if let Some(w) = w_rom.upgrade() {
                w.set_show_romanized(en);
                // Refresh library tracks and queue tracks display
                if let Ok(tracks) = db_rom.list_tracks(app_scope_rom.get(), app_sort_rom.get(), app_asc_rom.get()) {
                    let models = Self::convert_tracks_to_models_with_option(&tracks, en);
                    w.set_library_tracks(ModelRc::new(VecModel::from(models)));
                }
                let q = q_rom.borrow();
                Self::sync_queue_to_ui_with_option(&q, &w, en);

                // Refresh currently playing track and lyrics display
                let playing_id = w.get_active_playing_id();
                if playing_id > 0 {
                    if let Ok(Some(track)) = db_rom.get_track_by_id(playing_id as i64) {
                        let title = Self::format_text_display(&track.title, en);
                        let artist = Self::format_text_display(&track.artist, en);
                        let album = Self::format_text_display(&track.album, en);
                        w.set_track_title(SharedString::from(title));
                        w.set_track_artist(SharedString::from(artist));
                        w.set_track_album(SharedString::from(album));

                        if let Ok(Some(lyrics_rec)) = db_rom.get_lyrics(track.id) {
                            let line_models: Vec<LyricLineModel> = lyrics_rec
                                .lines
                                .iter()
                                .map(|l| {
                                    let text = Self::format_text_display(&l.text, en);
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
        });

        // 21. Native Windows File Explorer - Add Music Folder Dialog
        let win_add = win_weak.clone();
        let app_scope_add = self.active_scope.clone();
        let app_sort_add = self.sort_criteria.clone();
        let app_asc_add = self.sort_ascending.clone();
        let app_rom_add = self.show_romanized.clone();

        self.window.on_on_add_folder_clicked(move || {
            let win_c = win_add.clone();
            let cur_scope_ref = app_scope_add.clone();
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
                                    let cur_scope = cur_scope_ref.get();
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
                                        let models = Self::convert_tracks_to_models_with_option(&tracks, cur_rom);
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

        // 23. Clear queue
        let q_clear = q_ref.clone();
        let w_clear = win_weak.clone();
        self.window.on_on_clear_queue(move || {
            let mut q = q_clear.borrow_mut();
            q.clear();
            if let Some(w) = w_clear.upgrade() {
                Self::sync_queue_to_ui(&q, &w);
            }
        });
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

        if let Some(w) = window.upgrade() {
            let rom = w.get_show_romanized();
            let title = Self::format_text_display(&track.title, rom);
            let artist = Self::format_text_display(&track.artist, rom);
            let album = Self::format_text_display(&track.album, rom);

            w.set_track_title(SharedString::from(title));
            w.set_track_artist(SharedString::from(artist));
            w.set_track_album(SharedString::from(album));
            w.set_audio_format(SharedString::from(track.format.clone()));
            w.set_bit_depth(track.bit_depth as i32);
            w.set_sample_rate(track.sample_rate as i32);
            w.set_duration_ms(track.duration_ms as i32);
            w.set_active_playing_id(track.id as i32);
            w.set_is_playing(true);

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
                        let text = Self::format_text_display(&l.text, rom);
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

        self._timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(w) = win_weak.upgrade() else { return };

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

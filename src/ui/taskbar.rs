//! Windows Taskbar Thumbnail Preview, Controls & Jump List
//!
//! Implements:
//! 1. ITaskbarList3 ThumbBarAddButtons / ThumbBarUpdateButtons for native
//!    Windows Taskbar thumbnail hover controls (Prev, Play/Pause, Next).
//! 2. Window subclassing to handle WM_COMMAND (THBN_CLICKED) and WM_COPYDATA.
//! 3. ICustomDestinationList for the "Recently Played" Taskbar Jump List.
//! 4. Single-instance IPC forwarding when clicking a Jump List item.

use crossbeam_channel::{unbounded, Receiver, Sender};

pub const THUMBBUTTON_PREV_ID: u32 = 101;
pub const THUMBBUTTON_PLAY_PAUSE_ID: u32 = 102;
pub const THUMBBUTTON_NEXT_ID: u32 = 103;

pub const APP_USER_MODEL_ID: &str = "ShrimpMusic.Player";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarAction {
    Previous,
    PlayPause,
    Next,
    PlayTrack(i64),
}

#[cfg(windows)]
mod win_impl {
    use super::*;
    use std::ffi::c_void;
    use std::sync::Mutex;
    use windows::core::{w, Interface, GUID, PCWSTR};
    use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, PROPERTYKEY, WPARAM};
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        ICustomDestinationList, IShellLinkW, ITaskbarList3, SetCurrentProcessExplicitAppUserModelID,
        ShellLink, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, CreateIcon, DestroyIcon, EnumWindows, GetPropW, IsIconic,
        RegisterWindowMessageW, SendMessageW, SetForegroundWindow, SetPropW, SetWindowLongPtrW,
        ShowWindow, GWLP_WNDPROC, HICON, SW_RESTORE, WM_COMMAND, WM_COPYDATA, WNDPROC,
    };

    const COPYDATA_PLAY_TRACK_MAGIC: usize = 0x5348_524D; // "SHRM"
    const PROP_MAIN_WINDOW: PCWSTR = w!("ShrimpMusicPlayer_MainWindow");

    // CLSIDs & PKEYs
    // CLSID_TaskbarList: {56FDF344-FD6D-11d0-958A-006097C9A090}
    const CLSID_TASKBAR_LIST: GUID = GUID::from_u128(0x56FDF344_FD6D_11d0_958A_006097C9A090);
    // CLSID_DestinationList: {77f10cf0-3db5-4966-b520-b7c54fd35ed6}
    const CLSID_DESTINATION_LIST: GUID = GUID::from_u128(0x77f10cf0_3db5_4966_b520_b7c54fd35ed6);
    // CLSID_EnumerableObjectCollection: {2d3468c1-36a7-43b6-ac24-d3f02fd9607a}
    const CLSID_ENUMERABLE_OBJECT_COLLECTION: GUID =
        GUID::from_u128(0x2d3468c1_36a7_43b6_ac24_d3f02fd9607a);
    // PKEY_Title: {F29F85E0-4FF9-1068-AB91-08002B27B3D9}, pid 2
    const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9),
        pid: 2,
    };

    struct WndProcState {
        orig_wndproc: isize,
        action_tx: Sender<TaskbarAction>,
        taskbar_created_msg: u32,
    }

    static WNDPROC_STATE: Mutex<Option<WndProcState>> = Mutex::new(None);

    pub fn init_app_user_model_id() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let _ = SetCurrentProcessExplicitAppUserModelID(w!("ShrimpMusic.Player"));
        }
    }

    /// Try to forward `--play-track <id>` to an already running ShrimpMusic Player window.
    /// Returns `true` if forwarded (meaning this process should exit immediately).
    pub fn try_forward_play_track(track_id: i64) -> bool {
        struct SearchCtx {
            found: Option<HWND>,
        }

        unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            unsafe {
                let ctx = &mut *(lparam.0 as *mut SearchCtx);
                let prop = GetPropW(hwnd, PROP_MAIN_WINDOW);
                if !prop.0.is_null() {
                    ctx.found = Some(hwnd);
                    return false.into();
                }
                true.into()
            }
        }

        unsafe {
            let mut ctx = SearchCtx { found: None };
            let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut _ as isize));

            if let Some(hwnd) = ctx.found {
                let mut track_id_bytes = track_id.to_le_bytes();
                let cds = COPYDATASTRUCT {
                    dwData: COPYDATA_PLAY_TRACK_MAGIC,
                    cbData: track_id_bytes.len() as u32,
                    lpData: track_id_bytes.as_mut_ptr() as *mut c_void,
                };
                SendMessageW(
                    hwnd,
                    WM_COPYDATA,
                    Some(WPARAM(0)),
                    Some(LPARAM(&cds as *const _ as isize)),
                );
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetForegroundWindow(hwnd);
                return true;
            }
        }
        false
    }

    unsafe extern "system" fn taskbar_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let mut orig: isize = 0;
        let mut tb_created_msg: u32 = 0;
        let mut tx_opt: Option<Sender<TaskbarAction>> = None;

        if let Ok(guard) = WNDPROC_STATE.lock() {
            if let Some(state) = guard.as_ref() {
                orig = state.orig_wndproc;
                tb_created_msg = state.taskbar_created_msg;
                tx_opt = Some(state.action_tx.clone());
            }
        }

        if msg == WM_COMMAND {
            let hiword = ((wparam.0 >> 16) & 0xFFFF) as u32;
            let loword = (wparam.0 & 0xFFFF) as u32;
            if hiword == THBN_CLICKED {
                if let Some(tx) = &tx_opt {
                    match loword {
                        THUMBBUTTON_PREV_ID => {
                            let _ = tx.send(TaskbarAction::Previous);
                            return LRESULT(0);
                        }
                        THUMBBUTTON_PLAY_PAUSE_ID => {
                            let _ = tx.send(TaskbarAction::PlayPause);
                            return LRESULT(0);
                        }
                        THUMBBUTTON_NEXT_ID => {
                            let _ = tx.send(TaskbarAction::Next);
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
        } else if msg == WM_COPYDATA {
            if lparam.0 != 0 {
                unsafe {
                    let cds = &*(lparam.0 as *const COPYDATASTRUCT);
                    if cds.dwData == COPYDATA_PLAY_TRACK_MAGIC
                        && cds.cbData == 8
                        && !cds.lpData.is_null()
                    {
                        let bytes = std::slice::from_raw_parts(cds.lpData as *const u8, 8);
                        let track_id = i64::from_le_bytes(bytes.try_into().unwrap_or([0; 8]));
                        if let Some(tx) = &tx_opt {
                            let _ = tx.send(TaskbarAction::PlayTrack(track_id));
                        }
                        return LRESULT(1);
                    }
                }
            }
        } else if tb_created_msg != 0 && msg == tb_created_msg {
            // Explorer restarted; buttons could be re-added if needed
        }

        if orig != 0 {
            unsafe {
                let proc_fn: WNDPROC = std::mem::transmute(orig);
                CallWindowProcW(proc_fn, hwnd, msg, wparam, lparam)
            }
        } else {
            LRESULT(0)
        }
    }

    pub struct WinTaskbarInner {
        taskbar: Option<ITaskbarList3>,
        hwnd: Option<isize>,
        icon_prev: HICON,
        icon_play: HICON,
        icon_pause: HICON,
        icon_next: HICON,
        buttons_added: bool,
    }

    impl WinTaskbarInner {
        pub fn new() -> Self {
            Self {
                taskbar: None,
                hwnd: None,
                icon_prev: create_transport_icon(IconKind::Prev),
                icon_play: create_transport_icon(IconKind::Play),
                icon_pause: create_transport_icon(IconKind::Pause),
                icon_next: create_transport_icon(IconKind::Next),
                buttons_added: false,
            }
        }

        pub fn attach_window(
            &mut self,
            hwnd_val: isize,
            is_playing: bool,
            action_tx: Sender<TaskbarAction>,
        ) {
            if self.hwnd == Some(hwnd_val) && self.buttons_added {
                return;
            }
            self.hwnd = Some(hwnd_val);

            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let hwnd = HWND(hwnd_val as *mut c_void);

                // Mark window with custom property for single-instance Jump List handoff
                let _ = SetPropW(hwnd, PROP_MAIN_WINDOW, Some(HANDLE(1 as *mut c_void)));

                // Subclass window if not already subclassed for this HWND
                let should_subclass = WNDPROC_STATE
                    .lock()
                    .map(|g| g.is_none())
                    .unwrap_or(false);

                if should_subclass {
                    let tb_created_msg = RegisterWindowMessageW(w!("TaskbarButtonCreated"));
                    let orig = SetWindowLongPtrW(
                        hwnd,
                        GWLP_WNDPROC,
                        taskbar_wndproc as *const () as isize,
                    );
                    if let Ok(mut guard) = WNDPROC_STATE.lock() {
                        *guard = Some(WndProcState {
                            orig_wndproc: orig,
                            action_tx,
                            taskbar_created_msg: tb_created_msg,
                        });
                    }
                }

                if self.taskbar.is_none() {
                    self.taskbar =
                        CoCreateInstance(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER).ok();
                }

                if let Some(tb) = &self.taskbar {
                    let _ = tb.HrInit();

                    let play_icon = if is_playing {
                        self.icon_pause
                    } else {
                        self.icon_play
                    };
                    let play_tip = if is_playing {
                        "Jeda (Pause)"
                    } else {
                        "Putar (Play)"
                    };

                    let buttons = [
                        THUMBBUTTON {
                            dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                            iId: THUMBBUTTON_PREV_ID,
                            iBitmap: 0,
                            hIcon: self.icon_prev,
                            szTip: to_u16_slice("Sebelumnya (Prev)"),
                            dwFlags: Default::default(),
                        },
                        THUMBBUTTON {
                            dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                            iId: THUMBBUTTON_PLAY_PAUSE_ID,
                            iBitmap: 0,
                            hIcon: play_icon,
                            szTip: to_u16_slice(play_tip),
                            dwFlags: Default::default(),
                        },
                        THUMBBUTTON {
                            dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                            iId: THUMBBUTTON_NEXT_ID,
                            iBitmap: 0,
                            hIcon: self.icon_next,
                            szTip: to_u16_slice("Berikutnya (Next)"),
                            dwFlags: Default::default(),
                        },
                    ];

                    if tb.ThumbBarAddButtons(hwnd, &buttons).is_ok() {
                        self.buttons_added = true;
                    }
                }
            }
        }

        pub fn set_playback_state(&mut self, is_playing: bool) {
            if let (Some(tb), Some(hwnd_val)) = (&self.taskbar, self.hwnd) {
                if !self.buttons_added {
                    return;
                }
                unsafe {
                    let hwnd = HWND(hwnd_val as *mut c_void);
                    let tip = if is_playing {
                        "Jeda (Pause)"
                    } else {
                        "Putar (Play)"
                    };
                    let icon = if is_playing {
                        self.icon_pause
                    } else {
                        self.icon_play
                    };

                    let button = THUMBBUTTON {
                        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                        iId: THUMBBUTTON_PLAY_PAUSE_ID,
                        iBitmap: 0,
                        hIcon: icon,
                        szTip: to_u16_slice(tip),
                        dwFlags: Default::default(),
                    };

                    let _ = tb.ThumbBarUpdateButtons(hwnd, &[button]);
                }
            }
        }

        /// Update the Windows Taskbar Jump List ("Recently Played" category)
        pub fn update_jump_list(&self, recent_tracks: &[(i64, String, String)]) {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

                let Ok(cdl) = CoCreateInstance::<_, ICustomDestinationList>(
                    &CLSID_DESTINATION_LIST,
                    None,
                    CLSCTX_INPROC_SERVER,
                ) else {
                    return;
                };

                let _ = cdl.SetAppID(w!("ShrimpMusic.Player"));

                let mut min_slots: u32 = 0;
                let Ok(_removed) = cdl.BeginList::<IObjectArray>(&mut min_slots) else {
                    return;
                };

                if !recent_tracks.is_empty() {
                    if let Ok(collection) = CoCreateInstance::<_, IObjectCollection>(
                        &CLSID_ENUMERABLE_OBJECT_COLLECTION,
                        None,
                        CLSCTX_INPROC_SERVER,
                    ) {
                        let exe_path = std::env::current_exe()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let exe_wide: Vec<u16> =
                            exe_path.encode_utf16().chain(std::iter::once(0)).collect();

                        for (track_id, title, artist) in recent_tracks.iter().take(5) {
                            if let Ok(link) = CoCreateInstance::<_, IShellLinkW>(
                                &ShellLink,
                                None,
                                CLSCTX_INPROC_SERVER,
                            ) {
                                let _ = link.SetPath(PCWSTR(exe_wide.as_ptr()));
                                let args = format!("--play-track {}", track_id);
                                let args_wide: Vec<u16> =
                                    args.encode_utf16().chain(std::iter::once(0)).collect();
                                let _ = link.SetArguments(PCWSTR(args_wide.as_ptr()));
                                let _ = link.SetIconLocation(PCWSTR(exe_wide.as_ptr()), 0);

                                let label = if artist.trim().is_empty()
                                    || artist.eq_ignore_ascii_case("Unknown Artist")
                                {
                                    title.clone()
                                } else {
                                    format!("{} — {}", title, artist)
                                };

                                if let Ok(prop_store) = link.cast::<IPropertyStore>() {
                                    let pv = make_bstr_propvariant(&label);
                                    let _ = prop_store.SetValue(&PKEY_TITLE, &pv);
                                    let _ = prop_store.Commit();
                                }

                                let _ = collection.AddObject(&link);
                            }
                        }

                        if let Ok(obj_array) = collection.cast::<IObjectArray>() {
                            let _ = cdl.AppendCategory(w!("Recently Played"), &obj_array);
                        }
                    }
                }

                let _ = cdl.CommitList();
            }
        }
    }

    impl Drop for WinTaskbarInner {
        fn drop(&mut self) {
            unsafe {
                if !self.icon_prev.is_invalid() {
                    let _ = DestroyIcon(self.icon_prev);
                }
                if !self.icon_play.is_invalid() {
                    let _ = DestroyIcon(self.icon_play);
                }
                if !self.icon_pause.is_invalid() {
                    let _ = DestroyIcon(self.icon_pause);
                }
                if !self.icon_next.is_invalid() {
                    let _ = DestroyIcon(self.icon_next);
                }
            }
        }
    }

    fn make_bstr_propvariant(text: &str) -> PROPVARIANT {
        // PROPVARIANT with VT_LPWSTR (31) or VT_BSTR (8)
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bstr = windows::core::BSTR::from_wide(&wide[..wide.len() - 1]);
        PROPVARIANT::from(bstr)
    }

    fn to_u16_slice(s: &str) -> [u16; 260] {
        let mut buf = [0u16; 260];
        for (i, c) in s.encode_utf16().take(259).enumerate() {
            buf[i] = c;
        }
        buf
    }

    #[derive(Clone, Copy)]
    enum IconKind {
        Prev,
        Play,
        Pause,
        Next,
    }

    /// Render a crisp 24x24 RGBA transport icon into a Win32 HICON
    fn create_transport_icon(kind: IconKind) -> HICON {
        const W: usize = 24;
        const H: usize = 24;
        let mut rgba = vec![0u8; W * H * 4]; // BGRA format for CreateIcon

        let set_px = |buf: &mut [u8], x: usize, y: usize, alpha: u8| {
            if x < W && y < H {
                let idx = (y * W + x) * 4;
                // White glyph (#FFFFFF) with given alpha (premultiplied or straight)
                buf[idx] = 255; // B
                buf[idx + 1] = 255; // G
                buf[idx + 2] = 255; // R
                buf[idx + 3] = alpha; // A
            }
        };

        let fill_rect = |buf: &mut [u8], x0: usize, y0: usize, x1: usize, y1: usize| {
            for y in y0..y1 {
                for x in x0..x1 {
                    set_px(buf, x, y, 255);
                }
            }
        };

        match kind {
            IconKind::Play => {
                // Right-pointing triangle centered in 24x24: x from 7..18, y from 5..19
                for y in 5..19 {
                    let dy = if y < 12 { y - 5 } else { 18 - y };
                    let width = ((dy as f32 / 6.5) * 10.0).round() as usize + 1;
                    for x in 7..(7 + width).min(19) {
                        set_px(&mut rgba, x, y, 255);
                    }
                }
            }
            IconKind::Pause => {
                // Two vertical bars
                fill_rect(&mut rgba, 6, 5, 10, 19);
                fill_rect(&mut rgba, 14, 5, 18, 19);
            }
            IconKind::Prev => {
                // Left vertical bar
                fill_rect(&mut rgba, 5, 5, 8, 19);
                // Left-pointing triangle: tip at x=9, base at x=18
                for y in 5..19 {
                    let dy = if y < 12 { y - 5 } else { 18 - y };
                    let width = ((dy as f32 / 6.5) * 9.0).round() as usize + 1;
                    let start_x = 18usize.saturating_sub(width).max(9);
                    for x in start_x..18 {
                        set_px(&mut rgba, x, y, 255);
                    }
                }
            }
            IconKind::Next => {
                // Right-pointing triangle: base at x=6, tip at x=15
                for y in 5..19 {
                    let dy = if y < 12 { y - 5 } else { 18 - y };
                    let width = ((dy as f32 / 6.5) * 9.0).round() as usize + 1;
                    for x in 6..(6 + width).min(16) {
                        set_px(&mut rgba, x, y, 255);
                    }
                }
                // Right vertical bar
                fill_rect(&mut rgba, 16, 5, 19, 19);
            }
        }

        // AND mask: 1 bit per pixel, padded to 16-bit word boundary per row
        // 24 pixels -> 3 bytes -> padded to 4 bytes per row
        let row_bytes = ((W + 15) / 16) * 2;
        let mut and_mask = vec![0u8; row_bytes * H];
        for y in 0..H {
            for x in 0..W {
                let alpha = rgba[(y * W + x) * 4 + 3];
                if alpha == 0 {
                    let byte_idx = y * row_bytes + (x / 8);
                    let bit = 7 - (x % 8);
                    and_mask[byte_idx] |= 1 << bit;
                }
            }
        }

        unsafe {
            CreateIcon(
                None,
                W as i32,
                H as i32,
                1,
                32,
                and_mask.as_ptr(),
                rgba.as_ptr(),
            )
            .unwrap_or_default()
        }
    }
}

pub fn init_app_user_model_id() {
    #[cfg(windows)]
    win_impl::init_app_user_model_id();
}

pub fn try_forward_play_track_to_existing(track_id: i64) -> bool {
    #[cfg(windows)]
    {
        win_impl::try_forward_play_track(track_id)
    }
    #[cfg(not(windows))]
    {
        let _ = track_id;
        false
    }
}

pub struct TaskbarController {
    #[cfg(windows)]
    inner: win_impl::WinTaskbarInner,
    is_playing: bool,
    action_tx: Sender<TaskbarAction>,
    action_rx: Receiver<TaskbarAction>,
}

impl TaskbarController {
    pub fn new() -> Self {
        let (action_tx, action_rx) = unbounded();
        Self {
            #[cfg(windows)]
            inner: win_impl::WinTaskbarInner::new(),
            is_playing: false,
            action_tx,
            action_rx,
        }
    }

    /// Attach to window HWND and initialize the 3 thumbnail buttons (Prev, Play/Pause, Next)
    pub fn attach_window(&mut self, hwnd_val: isize) {
        #[cfg(windows)]
        self.inner
            .attach_window(hwnd_val, self.is_playing, self.action_tx.clone());
        #[cfg(not(windows))]
        let _ = hwnd_val;
    }

    /// Update playback state in Taskbar thumbnail (Play vs Pause icon & tooltip)
    pub fn set_playback_state(&mut self, is_playing: bool) {
        self.is_playing = is_playing;
        #[cfg(windows)]
        self.inner.set_playback_state(is_playing);
    }

    /// Update the "Recently Played" category in the Windows Taskbar Jump List
    pub fn update_jump_list(&self, recent_tracks: &[(i64, String, String)]) {
        #[cfg(windows)]
        self.inner.update_jump_list(recent_tracks);
        #[cfg(not(windows))]
        let _ = recent_tracks;
    }

    /// Poll any pending button clicks or Jump List track launch events
    pub fn poll_actions(&self) -> Option<TaskbarAction> {
        self.action_rx.try_recv().ok()
    }
}

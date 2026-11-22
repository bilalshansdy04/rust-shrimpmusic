//! Windows Taskbar Thumbnail Preview & Controls
//!
//! Implements ITaskbarList3 ThumbBarAddButtons / ThumbBarUpdateButtons for
//! native Windows Taskbar thumbnail hover controls (Prev, Play/Pause, Next)
//! identical to Spotify/foobar2000.

#[cfg(windows)]
use windows::core::GUID;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    ITaskbarList3, THB_FLAGS, THB_TOOLTIP, THUMBBUTTON,
};

pub const THUMBBUTTON_PREV_ID: u32 = 101;
pub const THUMBBUTTON_PLAY_PAUSE_ID: u32 = 102;
pub const THUMBBUTTON_NEXT_ID: u32 = 103;

pub struct TaskbarController {
    #[cfg(windows)]
    taskbar: Option<ITaskbarList3>,
    #[cfg(windows)]
    hwnd: Option<isize>,
    is_playing: bool,
}

impl TaskbarController {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            Self {
                taskbar: None,
                hwnd: None,
                is_playing: false,
            }
        }

        #[cfg(not(windows))]
        {
            Self { is_playing: false }
        }
    }

    /// Attach to window HWND and initialize the 3 thumbnail buttons
    pub fn attach_window(&mut self, hwnd_val: isize) {
        #[cfg(windows)]
        {
            self.hwnd = Some(hwnd_val);
            if self.taskbar.is_none() {
                let clsid = GUID::from_u128(0x56FDF344_FD6D_11d0_958A_006097C9A090);
                self.taskbar = unsafe { CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER).ok() };
            }

            if let Some(tb) = &self.taskbar {
                unsafe {
                    let hwnd = HWND(hwnd_val as *mut _);
                    let _ = tb.HrInit();

                    let buttons = [
                        THUMBBUTTON {
                            dwMask: THB_FLAGS | THB_TOOLTIP,
                            iId: THUMBBUTTON_PREV_ID,
                            iBitmap: 0,
                            hIcon: Default::default(),
                            szTip: Self::to_u16_slice("Sebelumnya (Prev)"),
                            dwFlags: Default::default(),
                        },
                        THUMBBUTTON {
                            dwMask: THB_FLAGS | THB_TOOLTIP,
                            iId: THUMBBUTTON_PLAY_PAUSE_ID,
                            iBitmap: 0,
                            hIcon: Default::default(),
                            szTip: Self::to_u16_slice("Putar / Jeda (Play/Pause)"),
                            dwFlags: Default::default(),
                        },
                        THUMBBUTTON {
                            dwMask: THB_FLAGS | THB_TOOLTIP,
                            iId: THUMBBUTTON_NEXT_ID,
                            iBitmap: 0,
                            hIcon: Default::default(),
                            szTip: Self::to_u16_slice("Berikutnya (Next)"),
                            dwFlags: Default::default(),
                        },
                    ];

                    let _ = tb.ThumbBarAddButtons(hwnd, &buttons);
                }
            }
        }
    }

    /// Update playback state in Taskbar thumbnail
    pub fn set_playback_state(&mut self, is_playing: bool) {
        self.is_playing = is_playing;
        #[cfg(windows)]
        {
            if let (Some(tb), Some(hwnd_val)) = (&self.taskbar, self.hwnd) {
                unsafe {
                    let hwnd = HWND(hwnd_val as *mut _);
                    let tip = if is_playing {
                        "Jeda (Pause)"
                    } else {
                        "Putar (Play)"
                    };

                    let button = THUMBBUTTON {
                        dwMask: THB_FLAGS | THB_TOOLTIP,
                        iId: THUMBBUTTON_PLAY_PAUSE_ID,
                        iBitmap: 0,
                        hIcon: Default::default(),
                        szTip: Self::to_u16_slice(tip),
                        dwFlags: Default::default(),
                    };

                    let _ = tb.ThumbBarUpdateButtons(hwnd, &[button]);
                }
            }
        }
    }

    #[cfg(windows)]
    fn to_u16_slice(s: &str) -> [u16; 260] {
        let mut buf = [0u16; 260];
        for (i, c) in s.encode_utf16().take(259).enumerate() {
            buf[i] = c;
        }
        buf
    }
}

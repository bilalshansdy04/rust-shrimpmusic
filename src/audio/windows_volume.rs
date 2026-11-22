use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_channel::{unbounded, Receiver, Sender};

#[derive(Clone)]
pub struct WindowsVolumeController {
    current_volume: Arc<AtomicU32>,
    set_tx: Sender<f32>,
    is_running: Arc<AtomicBool>,
}

impl Default for WindowsVolumeController {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsVolumeController {
    pub fn new() -> Self {
        let current_volume = Arc::new(AtomicU32::new(0.12_f32.to_bits()));
        let (set_tx, set_rx) = unbounded::<f32>();
        let is_running = Arc::new(AtomicBool::new(true));

        #[cfg(windows)]
        {
            let vol_clone = current_volume.clone();
            let running_clone = is_running.clone();

            let _ = std::thread::Builder::new()
                .name("windows-volume-watcher".into())
                .spawn(move || {
                    worker_loop(vol_clone, set_rx, running_clone);
                });
        }

        Self {
            current_volume,
            set_tx,
            is_running,
        }
    }

    pub fn get_volume(&self) -> f32 {
        f32::from_bits(self.current_volume.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, level: f32) {
        let _ = self.set_tx.send(level.clamp(0.0, 1.0));
    }
}

impl Drop for WindowsVolumeController {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

#[cfg(windows)]
fn worker_loop(
    current_vol: Arc<AtomicU32>,
    set_rx: Receiver<f32>,
    is_running: Arc<AtomicBool>,
) {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::Media::Audio::{
        MMDeviceEnumerator, IMMDeviceEnumerator, eRender, eMultimedia,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

    // Isolate COM MTA initialization strictly to this background worker thread
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let init_endpoint = || -> Option<IAudioEndpointVolume> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
            device.Activate(CLSCTX_ALL, None).ok()
        }
    };

    let mut endpoint_volume = init_endpoint();

    // Read initial volume immediately on startup
    if let Some(ref ep) = endpoint_volume {
        if let Ok(vol) = unsafe { ep.GetMasterVolumeLevelScalar() } {
            current_vol.store(vol.to_bits(), Ordering::SeqCst);
        }
    }

    while is_running.load(Ordering::Relaxed) {
        // 1. Process any pending set_volume requests from UI
        while let Ok(target_vol) = set_rx.try_recv() {
            if let Some(ref ep) = endpoint_volume {
                let _ = unsafe { ep.SetMasterVolumeLevelScalar(target_vol, std::ptr::null()) };
                current_vol.store(target_vol.to_bits(), Ordering::SeqCst);
            }
        }

        // 2. Poll Windows Master Volume (updates when user presses Fn+F2 / Fn+F3 or adjusts Windows OSD)
        if let Some(ref ep) = endpoint_volume {
            match unsafe { ep.GetMasterVolumeLevelScalar() } {
                Ok(vol) => {
                    current_vol.store(vol.to_bits(), Ordering::Relaxed);
                }
                Err(_) => {
                    // Re-acquire endpoint if default audio device was changed/hotplugged
                    endpoint_volume = init_endpoint();
                }
            }
        } else {
            endpoint_volume = init_endpoint();
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

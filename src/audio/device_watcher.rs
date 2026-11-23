use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_channel::{unbounded, Receiver, Sender};
use super::device::{AudioDeviceInfo, DeviceManager};

#[derive(Clone)]
pub struct DeviceWatcher {
    event_rx: Receiver<Vec<AudioDeviceInfo>>,
    is_running: Arc<AtomicBool>,
}

impl Default for DeviceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceWatcher {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let is_running = Arc::new(AtomicBool::new(true));

        let running_clone = is_running.clone();
        let _ = std::thread::Builder::new()
            .name("audio-device-watcher".into())
            .spawn(move || {
                worker_loop(tx, running_clone);
            });

        Self {
            event_rx: rx,
            is_running,
        }
    }

    pub fn poll_devices_changed(&self) -> Option<Vec<AudioDeviceInfo>> {
        let mut latest = None;
        while let Ok(devices) = self.event_rx.try_recv() {
            latest = Some(devices);
        }
        latest
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

fn worker_loop(tx: Sender<Vec<AudioDeviceInfo>>, is_running: Arc<AtomicBool>) {
    // Keep thread in MTA COM mode for WASAPI device enumeration
    let _ = wasapi::initialize_mta();

    let mut last_device_snapshot: Vec<AudioDeviceInfo> = Vec::new();

    // Initial enumeration
    if let Ok(dm) = DeviceManager::new() {
        if let Ok(devs) = dm.list_output_devices() {
            last_device_snapshot = devs.clone();
            let _ = tx.send(devs);
        }
    }

    while is_running.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(1000));

        if let Ok(dm) = DeviceManager::new() {
            if let Ok(devs) = dm.list_output_devices() {
                // Check if device list changed (count, IDs, or default device switch)
                let changed = devs.len() != last_device_snapshot.len()
                    || devs.iter().zip(&last_device_snapshot).any(|(a, b)| a.id != b.id || a.is_default != b.is_default);

                if changed {
                    last_device_snapshot = devs.clone();
                    let _ = tx.send(devs);
                }
            }
        }
    }
}

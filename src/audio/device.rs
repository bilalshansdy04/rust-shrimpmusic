use wasapi::{Device, DeviceEnumerator, Direction};
use super::error::AudioResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub struct DeviceManager {
    enumerator: DeviceEnumerator,
}

impl DeviceManager {
    pub fn new() -> AudioResult<Self> {
        let _ = wasapi::initialize_mta();
        let enumerator = DeviceEnumerator::new()?;
        Ok(Self { enumerator })
    }

    /// List output devices on an isolated background thread to keep the main GUI thread in STA mode
    pub fn list_output_devices_isolated() -> AudioResult<Vec<AudioDeviceInfo>> {
        std::thread::spawn(|| {
            let dm = DeviceManager::new()?;
            dm.list_output_devices()
        })
        .join()
        .map_err(|_| super::error::AudioError::Device("Thread join error in device enumeration".to_string()))?
    }

    pub fn list_output_devices(&self) -> AudioResult<Vec<AudioDeviceInfo>> {
        let default_device = self.enumerator.get_default_device(Direction::Render)?;
        let default_id = default_device.get_id().unwrap_or_default();

        let collection = self.enumerator.get_device_collection(Direction::Render)?;
        let count = collection.get_nbr_devices()?;

        let mut devices = Vec::with_capacity(count as usize);
        for i in 0..count {
            if let Ok(dev) = collection.get_device_at_index(i) {
                let id = dev.get_id().unwrap_or_default();
                let name = dev.get_friendlyname().unwrap_or_else(|_| "Unknown Device".to_string());
                let is_default = id == default_id;

                devices.push(AudioDeviceInfo {
                    id,
                    name,
                    is_default,
                });
            }
        }

        Ok(devices)
    }

    pub fn get_device(&self, device_id: Option<&str>) -> AudioResult<Device> {
        match device_id {
            Some(id) => {
                let dev = self.enumerator.get_device(id)?;
                Ok(dev)
            }
            None => {
                let dev = self.enumerator.get_default_device(Direction::Render)?;
                Ok(dev)
            }
        }
    }
}

use wasapi::{Device, DeviceEnumerator, Direction};
use super::error::AudioResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClassification {
    /// Dedicated USB DAC (FiiO, iFi, Topping, Schiit, Moondrop, Chord, Sony, etc.)
    UsbDac,
    /// Type-C to 3.5mm Converter with embedded DAC chip (ALC5686, CX31993, JA11, CS43131, KT0210, etc.)
    TypeCDacConverter,
    /// General USB Audio / Interface (Mic/Headset dongle)
    UsbAudioGeneric,
    /// Standard 3.5mm passive analog audio jack or onboard codec
    OnboardAnalog,
    /// Internal / Built-in Speakers
    InternalSpeakers,
    /// Bluetooth Audio output
    Bluetooth,
    /// Software / Virtual Audio device (FxSound, Voicemeeter, VB-Cable)
    VirtualAudio,
    /// Other unclassified audio device
    Other,
}

impl DeviceClassification {
    pub fn is_external_dac(&self) -> bool {
        matches!(self, Self::UsbDac | Self::TypeCDacConverter | Self::UsbAudioGeneric)
    }

    pub fn display_badge(&self) -> &'static str {
        match self {
            Self::UsbDac => "USB DAC",
            Self::TypeCDacConverter => "Type-C DAC",
            Self::UsbAudioGeneric => "USB Audio",
            Self::OnboardAnalog => "Analog Jack",
            Self::InternalSpeakers => "Speaker",
            Self::Bluetooth => "Bluetooth",
            Self::VirtualAudio => "Virtual",
            Self::Other => "Audio",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub classification: DeviceClassification,
    pub supports_bit_perfect: bool,
}

impl AudioDeviceInfo {
    pub fn classify(name: &str, id: &str) -> (DeviceClassification, bool) {
        let name_lower = name.to_lowercase();
        let id_lower = id.to_lowercase();

        // 1. Virtual / Enhancers
        if name_lower.contains("fxsound")
            || name_lower.contains("voicemeeter")
            || name_lower.contains("vb-audio")
            || name_lower.contains("virtual")
            || id_lower.contains("root\\media")
        {
            return (DeviceClassification::VirtualAudio, false);
        }

        // 2. Bluetooth
        if name_lower.contains("bluetooth")
            || name_lower.contains("hands-free")
            || name_lower.contains("bthhfenum")
            || id_lower.contains("bthhfenum")
        {
            return (DeviceClassification::Bluetooth, false);
        }

        // 3. Dedicated USB DAC / Type-C Converters with DAC chip
        let is_usb = name_lower.contains("usb") || id_lower.contains("usb") || id_lower.contains("usbaudio");

        let known_dac_chip_or_brands = [
            "jadeaudio", "ja11", "fiio", "topping", "ifi", "moondrop", "ibasso",
            "shanling", "tempotec", "sonata", "alc5686", "cx31993", "cs43131",
            "es9038", "es9280", "sabre", "ak4493", "ak4499", "cirrus", "realtek usb",
            "type-c", "type c", "kt0210", "ab13x", "jm6", "jm20", "jcally",
            "dawn", "space", "shanghai", "questyle", "chord", "audioquest",
        ];

        let has_dac_chip_or_brand = known_dac_chip_or_brands.iter().any(|k| name_lower.contains(k) || id_lower.contains(k));

        if is_usb {
            if name_lower.contains("type-c") || name_lower.contains("type c") || name_lower.contains("converter") || name_lower.contains("dongle") || name_lower.contains("ja11") || name_lower.contains("cx31993") || name_lower.contains("alc5686") {
                return (DeviceClassification::TypeCDacConverter, true);
            }
            if has_dac_chip_or_brand || name_lower.contains("dac") {
                return (DeviceClassification::UsbDac, true);
            }
            // General USB Audio (Headphones / USB Sound Card)
            return (DeviceClassification::UsbAudioGeneric, true);
        }

        // 4. Onboard Analog vs Internal Speakers
        if name_lower.contains("speaker") || name_lower.contains("speaker/hp") {
            return (DeviceClassification::InternalSpeakers, false);
        }
        if name_lower.contains("headphone") || name_lower.contains("earphone") || name_lower.contains("line out") {
            return (DeviceClassification::OnboardAnalog, false);
        }

        (DeviceClassification::Other, false)
    }
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

                let (classification, supports_bit_perfect) = AudioDeviceInfo::classify(&name, &id);

                devices.push(AudioDeviceInfo {
                    id,
                    name,
                    is_default,
                    classification,
                    supports_bit_perfect,
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

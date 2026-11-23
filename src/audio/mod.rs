pub mod decoder;
pub mod device;
pub mod dsp;
pub mod error;
pub mod player;
pub mod queue;
pub mod safety;
pub mod wasapi_engine;
pub mod waveform;
pub mod windows_volume;
pub mod device_watcher;

pub use decoder::{SymphoniaDecoder, SymphoniaDecoder as AudioDecoder, TrackAudioSpec};
pub use device::{AudioDeviceInfo, DeviceClassification, DeviceManager};
pub use device_watcher::DeviceWatcher;
pub use dsp::{
    convert_f32_to_bytes, DspProcessor, EqPreset, Equalizer10Band, TargetSampleFormat,
    VolumeBalanceMode, EQ_FREQUENCIES,
};
pub use error::{AudioError, AudioResult};
pub use player::{AudioCommand, AudioEvent, AudioPlayer, PlayerState};
pub use queue::{LoopMode, PlaybackQueue, ShuffleStyle};
pub use safety::SafetyTestSignal;
pub use waveform::{generate_synthetic_waveform, generate_waveform_peaks, WAVEFORM_BAR_COUNT};
pub use wasapi_engine::WasapiStream;
pub use windows_volume::WindowsVolumeController;

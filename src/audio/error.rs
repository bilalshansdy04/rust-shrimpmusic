use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Symphonia error: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),

    #[error("No supported audio track found in file")]
    NoAudioTrack,

    #[error("WASAPI error: {0}")]
    Wasapi(#[from] wasapi::WasapiError),

    #[error("Format not supported by device: {0}")]
    FormatNotSupported(String),

    #[error("Resampling error: {0}")]
    Resample(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("Channel communication error")]
    ChannelClosed,
}

pub type AudioResult<T> = Result<T, AudioError>;

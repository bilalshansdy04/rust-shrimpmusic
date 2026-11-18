use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::{Audio, GenericAudioBufferRef};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

use super::error::{AudioError, AudioResult};

#[derive(Clone, Debug)]
pub struct TrackAudioSpec {
    pub sample_rate: u32,
    pub channels: usize,
    pub bit_depth: Option<u32>,
    pub duration: Option<Duration>,
    pub codec_name: String,
}

pub struct SymphoniaDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    spec: TrackAudioSpec,
}

impl SymphoniaDecoder {
    pub fn open<P: AsRef<Path>>(path: P) -> AudioResult<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let format = symphonia::default::get_probe()
            .probe(&hint, mss, format_opts, metadata_opts)?;

        let (track_id, audio_params, duration) = format
            .tracks()
            .iter()
            .find_map(|t| match t.codec_params.as_ref() {
                Some(symphonia::core::codecs::CodecParameters::Audio(ap)) => {
                    let dur = if let (Some(dur), Some(tb)) = (t.duration, t.time_base) {
                        tb.calc_duration(dur).map(|time| {
                            Duration::from_nanos(time.as_nanos().max(0) as u64)
                        })
                    } else {
                        None
                    };
                    Some((t.id, ap.clone(), dur))
                }
                _ => None,
            })
            .ok_or(AudioError::NoAudioTrack)?;

        let sample_rate = audio_params.sample_rate.unwrap_or(44100);
        let channels = audio_params.channels.as_ref().map(|c| c.count()).unwrap_or(2);
        let bit_depth = audio_params.bits_per_sample;
        let codec_name = format!("{:?}", audio_params.codec);

        let spec = TrackAudioSpec {
            sample_rate,
            channels,
            bit_depth,
            duration,
            codec_name,
        };

        let decoder_opts = AudioDecoderOptions::default();
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&audio_params, &decoder_opts)?;

        Ok(Self {
            format,
            decoder,
            track_id,
            spec,
        })
    }

    pub fn spec(&self) -> &TrackAudioSpec {
        &self.spec
    }

    pub fn next_packet_samples(&mut self) -> AudioResult<Option<Vec<f32>>> {
        loop {
            let maybe_packet = match self.format.next_packet() {
                Ok(pkg) => pkg,
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(err) => return Err(AudioError::Symphonia(err)),
            };

            let packet = match maybe_packet {
                Some(p) => p,
                None => return Ok(None),
            };

            if packet.track_id != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let samples = buffer_to_interleaved_f32(decoded);
                    if !samples.is_empty() {
                        return Ok(Some(samples));
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    continue;
                }
                Err(err) => return Err(AudioError::Symphonia(err)),
            }
        }
    }

    pub fn seek(&mut self, target: Duration) -> AudioResult<Duration> {
        let time = Time::from(target.as_secs() as u32);
        let seek_to = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };

        let res = self.format.seek(SeekMode::Accurate, seek_to)?;
        self.decoder.reset();

        let actual_time = Duration::from_secs_f64(res.actual_ts.get() as f64 / self.spec.sample_rate as f64);
        Ok(actual_time)
    }
}

fn buffer_to_interleaved_f32(buf: GenericAudioBufferRef<'_>) -> Vec<f32> {
    match buf {
        GenericAudioBufferRef::S16(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push(slice[ch][f] as f32 / 32768.0);
                }
            }
            out
        }
        GenericAudioBufferRef::S24(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push(slice[ch][f].0 as f32 / 8388608.0);
                }
            }
            out
        }
        GenericAudioBufferRef::S32(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push(slice[ch][f] as f32 / 2147483648.0);
                }
            }
            out
        }
        GenericAudioBufferRef::F32(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push(slice[ch][f]);
                }
            }
            out
        }
        GenericAudioBufferRef::F64(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push(slice[ch][f] as f32);
                }
            }
            out
        }
        GenericAudioBufferRef::U8(slice) => {
            let frames = slice.frames();
            let channels = slice.num_planes();
            let mut out = Vec::with_capacity(frames * channels);
            for f in 0..frames {
                for ch in 0..channels {
                    out.push((slice[ch][f] as f32 - 128.0) / 128.0);
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

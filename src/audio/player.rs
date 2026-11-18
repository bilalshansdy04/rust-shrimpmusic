use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};
use wasapi::initialize_mta;

use super::decoder::{SymphoniaDecoder, TrackAudioSpec};
use super::device::DeviceManager;
use super::dsp::{convert_f32_to_bytes, DspProcessor};
use super::error::{AudioError, AudioResult};
use super::wasapi_engine::WasapiStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug)]
pub enum AudioCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    SetExclusive(bool),
    SetDevice(Option<String>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    StateChanged(PlayerState),
    TrackStarted {
        spec: TrackAudioSpec,
        is_exclusive: bool,
        device_name: String,
    },
    Position(Duration),
    EndOfTrack,
    Error(String),
}

pub struct AudioPlayer {
    cmd_tx: Sender<AudioCommand>,
    event_rx: Receiver<AudioEvent>,
    worker_handle: Option<JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> AudioResult<Self> {
        let (cmd_tx, cmd_rx) = unbounded::<AudioCommand>();
        let (event_tx, event_rx) = unbounded::<AudioEvent>();
        let is_running = Arc::new(AtomicBool::new(true));

        let running_clone = is_running.clone();
        let worker_handle = thread::Builder::new()
            .name("shrimp-audio-thread".to_string())
            .spawn(move || {
                run_audio_worker(cmd_rx, event_tx, running_clone);
            })
            .map_err(|e| AudioError::Io(e))?;

        Ok(Self {
            cmd_tx,
            event_rx,
            worker_handle: Some(worker_handle),
            is_running,
        })
    }

    pub fn play<P: AsRef<Path>>(&self, path: P) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::Play(path.as_ref().to_path_buf()))
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn pause(&self) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::Pause)
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn resume(&self) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::Resume)
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn stop(&self) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::Stop)
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn seek(&self, target: Duration) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::Seek(target))
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn set_volume(&self, volume: f32) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::SetVolume(volume))
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn set_exclusive(&self, exclusive: bool) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::SetExclusive(exclusive))
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn set_device(&self, device_id: Option<String>) -> AudioResult<()> {
        self.cmd_tx
            .send(AudioCommand::SetDevice(device_id))
            .map_err(|_| AudioError::ChannelClosed)
    }

    pub fn poll_event(&self) -> Option<AudioEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn event_receiver(&self) -> &Receiver<AudioEvent> {
        &self.event_rx
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(AudioCommand::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerState {
    decoder: Option<SymphoniaDecoder>,
    stream: Option<WasapiStream>,
    dsp: DspProcessor,
    current_path: Option<PathBuf>,
    target_device_id: Option<String>,
    exclusive_mode: bool,
    player_state: PlayerState,
    current_position: Duration,
    last_position_update: Instant,
    sample_queue: Vec<u8>,
}

fn run_audio_worker(
    cmd_rx: Receiver<AudioCommand>,
    event_tx: Sender<AudioEvent>,
    is_running: Arc<AtomicBool>,
) {
    let _ = initialize_mta();

    let mut state = WorkerState {
        decoder: None,
        stream: None,
        dsp: DspProcessor::new(),
        current_path: None,
        target_device_id: None,
        exclusive_mode: false,
        player_state: PlayerState::Stopped,
        current_position: Duration::ZERO,
        last_position_update: Instant::now(),
        sample_queue: Vec::new(),
    };

    let device_manager = match DeviceManager::new() {
        Ok(dm) => dm,
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error(format!("Failed to init DeviceManager: {}", e)));
            return;
        }
    };

    while is_running.load(Ordering::SeqCst) {
        let timeout = if state.player_state == PlayerState::Playing {
            Duration::from_millis(0)
        } else {
            Duration::from_millis(10)
        };

        match cmd_rx.recv_timeout(timeout) {
            Ok(cmd) => match cmd {
                AudioCommand::Play(path) => {
                    handle_play_command(&path, &mut state, &device_manager, &event_tx);
                }
                AudioCommand::Pause => {
                    if state.player_state == PlayerState::Playing {
                        state.player_state = PlayerState::Paused;
                        let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Paused));
                    }
                }
                AudioCommand::Resume => {
                    if state.player_state == PlayerState::Paused {
                        state.player_state = PlayerState::Playing;
                        let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Playing));
                    }
                }
                AudioCommand::Stop => {
                    state.stream = None;
                    state.decoder = None;
                    state.sample_queue.clear();
                    state.current_position = Duration::ZERO;
                    state.player_state = PlayerState::Stopped;
                    let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
                }
                AudioCommand::Seek(target) => {
                    if let Some(ref mut decoder) = state.decoder {
                        if let Ok(actual) = decoder.seek(target) {
                            state.current_position = actual;
                            state.sample_queue.clear();
                            let _ = event_tx.send(AudioEvent::Position(actual));
                        }
                    }
                }
                AudioCommand::SetVolume(vol) => {
                    state.dsp.set_volume(vol);
                }
                AudioCommand::SetExclusive(excl) => {
                    if state.exclusive_mode != excl {
                        state.exclusive_mode = excl;
                        if state.player_state == PlayerState::Playing {
                            if let Some(path) = state.current_path.clone() {
                                let saved_pos = state.current_position;
                                handle_play_command(&path, &mut state, &device_manager, &event_tx);
                                if let Some(ref mut dec) = state.decoder {
                                    let _ = dec.seek(saved_pos);
                                    state.current_position = saved_pos;
                                }
                            }
                        }
                    }
                }
                AudioCommand::SetDevice(dev_id) => {
                    state.target_device_id = dev_id;
                    if state.player_state == PlayerState::Playing {
                        if let Some(path) = state.current_path.clone() {
                            let saved_pos = state.current_position;
                            handle_play_command(&path, &mut state, &device_manager, &event_tx);
                            if let Some(ref mut dec) = state.decoder {
                                let _ = dec.seek(saved_pos);
                                state.current_position = saved_pos;
                            }
                        }
                    }
                }
                AudioCommand::Shutdown => {
                    state.stream = None;
                    state.decoder = None;
                    break;
                }
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        if state.player_state == PlayerState::Playing {
            process_playback_tick(&mut state, &event_tx);
        }
    }
}

fn handle_play_command(
    path: &Path,
    state: &mut WorkerState,
    dm: &DeviceManager,
    event_tx: &Sender<AudioEvent>,
) {
    state.stream = None;
    state.decoder = None;
    state.sample_queue.clear();

    let decoder = match SymphoniaDecoder::open(path) {
        Ok(dec) => dec,
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error(format!("Failed to decode file: {}", e)));
            state.player_state = PlayerState::Stopped;
            let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
            return;
        }
    };

    let spec = decoder.spec().clone();

    let device = match dm.get_device(state.target_device_id.as_deref()) {
        Ok(dev) => dev,
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error(format!("Failed to get device: {}", e)));
            state.player_state = PlayerState::Stopped;
            let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
            return;
        }
    };

    let device_name = device
        .get_friendlyname()
        .unwrap_or_else(|_| "Audio Device".to_string());

    let stream = match WasapiStream::open(
        &device,
        spec.sample_rate,
        spec.channels,
        spec.bit_depth,
        state.exclusive_mode,
    ) {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error(format!("WASAPI stream open error: {}", e)));
            state.player_state = PlayerState::Stopped;
            let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
            return;
        }
    };

    if stream.sample_rate() != spec.sample_rate {
        let _ = state
            .dsp
            .setup_resampler(spec.sample_rate, stream.sample_rate(), spec.channels);
    } else {
        let _ = state.dsp.setup_resampler(spec.sample_rate, spec.sample_rate, spec.channels);
    }

    let is_exclusive = stream.is_exclusive();

    state.decoder = Some(decoder);
    state.stream = Some(stream);
    state.current_path = Some(path.to_path_buf());
    state.player_state = PlayerState::Playing;
    state.current_position = Duration::ZERO;
    state.last_position_update = Instant::now();

    let _ = event_tx.send(AudioEvent::TrackStarted {
        spec,
        is_exclusive,
        device_name,
    });
    let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Playing));
}

fn process_playback_tick(state: &mut WorkerState, event_tx: &Sender<AudioEvent>) {
    let (stream, decoder) = match (state.stream.as_mut(), state.decoder.as_mut()) {
        (Some(s), Some(d)) => (s, d),
        _ => return,
    };

    let bytes_per_frame = stream.bytes_per_frame();
    if bytes_per_frame == 0 {
        return;
    }

    while state.sample_queue.len() < bytes_per_frame * 2048 {
        match decoder.next_packet_samples() {
            Ok(Some(samples)) => {
                let frame_count = samples.len() / decoder.spec().channels;
                let frame_duration = Duration::from_secs_f64(frame_count as f64 / decoder.spec().sample_rate as f64);
                state.current_position += frame_duration;

                let processed = match state.dsp.process_samples(samples) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = event_tx.send(AudioEvent::Error(format!("DSP error: {}", e)));
                        break;
                    }
                };

                let raw_bytes = convert_f32_to_bytes(&processed, stream.target_sample_format());
                state.sample_queue.extend_from_slice(&raw_bytes);
            }
            Ok(None) => {
                if state.sample_queue.is_empty() {
                    state.player_state = PlayerState::Stopped;
                    state.stream = None;
                    state.decoder = None;
                    let _ = event_tx.send(AudioEvent::EndOfTrack);
                    let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
                    return;
                }
                break;
            }
            Err(e) => {
                let _ = event_tx.send(AudioEvent::Error(format!("Decode error: {}", e)));
                state.player_state = PlayerState::Stopped;
                let _ = event_tx.send(AudioEvent::StateChanged(PlayerState::Stopped));
                return;
            }
        }
    }

    if let Ok(_) = stream.wait_for_buffer(50) {
        if let Ok(avail_frames) = stream.available_frames() {
            let avail_bytes = avail_frames * bytes_per_frame;
            let bytes_to_write = avail_bytes.min(state.sample_queue.len());
            let full_frames_bytes = (bytes_to_write / bytes_per_frame) * bytes_per_frame;

            if full_frames_bytes > 0 {
                let chunk: Vec<u8> = state.sample_queue.drain(..full_frames_bytes).collect();
                let _ = stream.write_bytes(&chunk);
            }
        }
    }

    if state.last_position_update.elapsed() >= Duration::from_millis(50) {
        state.last_position_update = Instant::now();
        let _ = event_tx.send(AudioEvent::Position(state.current_position));
    }
}

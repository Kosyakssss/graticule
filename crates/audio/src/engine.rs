use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Host, SampleFormat, SizedSample, Stream, StreamConfig};
use kernel::{Feed, Watch};

use crate::{Deck, Mix, RATE, Sound, Transport, take_path, wav};

/// How often the engine tells the view where it is while it plays.
const FRAME: Duration = Duration::from_millis(16);

/// How long [`saved`] waits for a take to be written.
const SAVING: Duration = Duration::from_secs(10);

static RUNNING: AtomicBool = AtomicBool::new(false);
static SAVED: Mutex<Vec<(u64, Result<i64, String>)>> = Mutex::new(Vec::new());
static TOLD: Condvar = Condvar::new();

#[derive(Default)]
struct Shared {
    at: AtomicI64,
    level: AtomicU32,
}

/// Plays the mix `mix` holds, through the system's default output, for as long as the app runs, and records from its default input into a take while the mix says to; tells `deck` where it is about sixty times a second while it plays, and nothing while it is stopped. A recorded take is written to [`take_path`] when recording stops, and [`saved`] says when.
pub fn run(mix: Watch<Mix>, deck: Feed<Deck>) {
    RUNNING.store(true, Ordering::Release);
    let host = cpal::default_host();
    let shared = Arc::new(Shared::default());
    let opened = host.default_output_device().ok_or_else(|| "There is no audio output".to_string()).and_then(|d| play(&d, mix.clone(), shared.clone()).map(|s| (s, named(&d))));
    let device = opened.as_ref().map(|(_, name)| name.clone()).map_err(Clone::clone);
    let stream = opened.ok().map(|(stream, _)| stream);
    let tell = |playing: bool, cue: u64| {
        let level = f32::from_bits(shared.level.swap(0, Ordering::Relaxed));
        deck.publish(Deck { at: shared.at.load(Ordering::Relaxed), cue, level, playing, device: device.clone() });
    };
    tell(false, 0);
    let (mut seen, mut playing, mut taking) = (0, false, None::<Taking>);
    loop {
        seen = mix.wait(seen, playing.then_some(FRAME));
        let (transport, folder) = mix.read(|m| m.map(|m| (m.transport, m.folder.clone()))).unwrap_or_default();
        let (now, cue, recording) = match transport {
            Transport::Stopped => (false, 0, None),
            Transport::Playing { cue, .. } => (true, cue, None),
            Transport::Recording { cue, take, .. } => (true, cue, Some(take)),
        };
        if now != playing {
            if let Some(stream) = &stream {
                let _ = if now { stream.play() } else { stream.pause() };
            }
            playing = now;
        }
        if taking.as_ref().map(|t| t.take) != recording {
            if let Some(done) = taking.take() {
                done.finish(&folder);
            }
            taking = recording.map(|take| Taking::start(&host, take));
        }
        if let Some(t) = &mut taking {
            t.drain();
        }
        tell(playing, cue);
    }
}

/// How long the take numbered `take` is, in samples, once the engine has written it, waiting up to ten seconds; or why it wasn't written. With no engine running, it says so at once.
pub fn saved(take: u64) -> Result<i64, String> {
    if !RUNNING.load(Ordering::Acquire) {
        return Err("No audio engine is running".into());
    }
    let told = SAVED.lock().unwrap_or_else(PoisonError::into_inner);
    let (told, _) = TOLD.wait_timeout_while(told, SAVING, |s| !s.iter().any(|(t, _)| *t == take)).unwrap_or_else(PoisonError::into_inner);
    told.iter().find(|(t, _)| *t == take).map_or_else(|| Err("The take wasn't written in time".into()), |(_, r)| r.clone())
}

fn tell_saved(take: u64, result: Result<i64, String>) {
    SAVED.lock().unwrap_or_else(PoisonError::into_inner).push((take, result));
    TOLD.notify_all();
}

fn named(device: &Device) -> String {
    device.description().map_or_else(|_| "The system's audio output".to_string(), |d| d.name().to_string())
}

fn play(device: &Device, mix: Watch<Mix>, shared: Arc<Shared>) -> Result<Stream, String> {
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let (format, config) = (config.sample_format(), StreamConfig::from(config));
    let stream = match format {
        SampleFormat::F32 => output::<f32>(device, config, mix, shared),
        SampleFormat::I16 => output::<i16>(device, config, mix, shared),
        SampleFormat::I32 => output::<i32>(device, config, mix, shared),
        SampleFormat::U16 => output::<u16>(device, config, mix, shared),
        other => return Err(format!("The audio output takes samples as {other}, which the engine can't write")),
    }
    .map_err(|e| e.to_string())?;
    let _ = stream.pause();
    Ok(stream)
}

fn output<T: SizedSample + FromSample<f32>>(device: &Device, config: StreamConfig, mix: Watch<Mix>, shared: Arc<Shared>) -> Result<Stream, cpal::Error> {
    let channels = usize::from(config.channels).max(1);
    let step = f64::from(RATE) / f64::from(config.sample_rate.max(1));
    let (mut at, mut playing, mut scratch) = (0f64, u64::MAX, vec![0f32; 16_384]);
    let fill = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        if scratch.len() < out.len() {
            scratch.resize(out.len(), 0.0);
        }
        let mixed = &mut scratch[..out.len()];
        mixed.fill(0.0);
        mix.read(|m| {
            let Some(m) = m else { return };
            let (from, cue) = match m.transport {
                Transport::Stopped => return,
                Transport::Playing { from, cue } | Transport::Recording { from, cue, .. } => (from, cue),
            };
            if cue != playing {
                (playing, at) = (cue, from as f64);
            }
            m.add(at, step, mixed, channels);
            at += step * (out.len() / channels) as f64;
        });
        let mut peak = 0f32;
        for (o, s) in out.iter_mut().zip(mixed.iter()) {
            let s = s.clamp(-1.0, 1.0);
            peak = peak.max(s.abs());
            *o = T::from_sample(s);
        }
        shared.at.store(at as i64, Ordering::Relaxed);
        shared.level.fetch_max(peak.to_bits(), Ordering::Relaxed);
    };
    device.build_output_stream::<T, _, _>(config, fill, |_| {}, None)
}

/// A take being recorded: what the input has sent so far, at the input's own rate.
struct Taking {
    take: u64,
    stream: Result<Stream, String>,
    rate: u32,
    sent: Option<Receiver<Vec<f32>>>,
    samples: Vec<f32>,
}

impl Taking {
    fn start(host: &Host, take: u64) -> Taking {
        let (tx, rx) = channel();
        let opened = host.default_input_device().ok_or_else(|| "There is no audio input".to_string()).and_then(|d| listen(&d, tx));
        let (stream, rate) = match opened {
            Ok((stream, rate)) => (Ok(stream), rate),
            Err(why) => (Err(why), RATE),
        };
        if let Ok(stream) = &stream {
            let _ = stream.play();
        }
        Taking { take, stream, rate, sent: Some(rx), samples: Vec::new() }
    }

    fn drain(&mut self) {
        if let Some(rx) = &self.sent {
            self.samples.extend(rx.try_iter().flatten());
        }
    }

    fn finish(mut self, folder: &Path) {
        self.drain();
        let result = match std::mem::replace(&mut self.stream, Err(String::new())) {
            Err(why) => Err(why),
            Ok(stream) => {
                drop(stream);
                let sound = Sound::new(wav::resampled(&self.samples, self.rate));
                let path = take_path(folder, self.take);
                let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, sound.wav()));
                written.map(|()| sound.len()).map_err(|e| format!("The take couldn't be written: {e}"))
            }
        };
        tell_saved(self.take, result);
    }
}

fn listen(device: &Device, tx: Sender<Vec<f32>>) -> Result<(Stream, u32), String> {
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let (format, config) = (config.sample_format(), StreamConfig::from(config));
    let rate = config.sample_rate;
    let stream = match format {
        SampleFormat::F32 => input::<f32>(device, config, tx),
        SampleFormat::I16 => input::<i16>(device, config, tx),
        SampleFormat::I32 => input::<i32>(device, config, tx),
        SampleFormat::U16 => input::<u16>(device, config, tx),
        other => return Err(format!("The audio input sends samples as {other}, which the engine can't read")),
    }
    .map_err(|e| e.to_string())?;
    Ok((stream, rate))
}

fn input<T: SizedSample>(device: &Device, config: StreamConfig, tx: Sender<Vec<f32>>) -> Result<Stream, cpal::Error>
where
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels).max(1);
    let heard = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let mono = data.chunks(channels).map(|f| f.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32).collect();
        let _ = tx.send(mono);
    };
    device.build_input_stream::<T, _, _>(config, heard, |_| {}, None)
}

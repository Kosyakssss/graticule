//! The audio service: sounds read from and written to WAV files, a mix of them placed in time, and an engine that plays the mix and records on a real-time thread through the system's audio devices.
//!
//! An app works the mix out from its facts as a derived value and hands it to the engine with `Jobs::serve`, which tells the view where it is through a live value: `jobs.serve(t!("Audio"), audio::run, Song::mix, Song::deck)`.

mod engine;
mod wav;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use engine::{run, saved};

/// Samples a second, in every sound and in a mix's time.
pub const RATE: u32 = 48_000;

const BLOCK: usize = 256;

/// A sound: one channel of samples at [`RATE`], from -1 to 1, with the quietest and loudest sample of each block of 256 kept for drawing its waveform.
pub struct Sound {
    samples: Vec<f32>,
    peaks: Vec<(f32, f32)>,
}

impl Sound {
    pub fn new(samples: Vec<f32>) -> Sound {
        let peaks = samples.chunks(BLOCK).map(|b| b.iter().fold((0f32, 0f32), |(lo, hi), s| (lo.min(*s), hi.max(*s)))).collect();
        Sound { samples, peaks }
    }

    /// How many samples it has.
    pub fn len(&self) -> i64 {
        self.samples.len() as i64
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// The quietest and loudest sample from `from` to `to`, by blocks of 256, for a waveform drawn a column at a time.
    pub fn peaks(&self, from: i64, to: i64) -> (f32, f32) {
        let (a, b) = ((from.max(0) as usize) / BLOCK, (to.max(0) as usize).div_ceil(BLOCK).min(self.peaks.len()));
        self.peaks.get(a..b.max(a + 1).min(self.peaks.len())).unwrap_or_default().iter().fold((0f32, 0f32), |(lo, hi), (l, h)| (lo.min(*l), hi.max(*h)))
    }

    /// The sound a WAV file holds, its channels mixed to one and brought to [`RATE`]: 8, 16, 24 or 32-bit whole numbers, or 32-bit floating point.
    pub fn read_wav(bytes: &[u8]) -> Result<Sound, String> {
        wav::read(bytes).map(Sound::new)
    }

    /// The sound as a WAV file: one channel of 32-bit floating point at [`RATE`].
    pub fn wav(&self) -> Vec<u8> {
        wav::write(&self.samples)
    }
}

/// Where the engine keeps a recorded take, in `folder`.
pub fn take_path(folder: &Path, take: u64) -> PathBuf {
    folder.join("takes").join(format!("{take:016x}.wav"))
}

/// A sound placed in a mix: from sample `from` of the sound, `len` samples long, starting at `at` in the mix's time, at `gain`.
pub struct Voice {
    pub sound: Arc<Sound>,
    pub at: i64,
    pub from: i64,
    pub len: i64,
    pub gain: f32,
}

/// What the engine does with a mix. `cue` tells one press of Play from the next, so playing again from the same place starts again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transport {
    #[default]
    Stopped,
    Playing {
        from: i64,
        cue: u64,
    },
    /// Playing, and recording from the input into the take numbered `take`.
    Recording {
        from: i64,
        cue: u64,
        take: u64,
    },
}

/// Sounds placed in time, and what to do with them: what the engine plays.
#[derive(Default)]
pub struct Mix {
    pub voices: Vec<Voice>,
    pub transport: Transport,
    /// The folder recorded takes are written to.
    pub folder: PathBuf,
}

impl Mix {
    /// Adds the voices sounding at `at` in the mix's time to `out`, `step` of the mix's samples to each sample of `out`.
    fn add(&self, at: f64, step: f64, out: &mut [f32], channels: usize) {
        let end = at + step * (out.len() / channels.max(1)) as f64;
        for v in self.voices.iter().filter(|v| (v.at as f64) < end && ((v.at + v.len) as f64) > at) {
            let samples = v.sound.samples();
            for (k, frame) in out.chunks_mut(channels.max(1)).enumerate() {
                let t = (at + step * k as f64) as i64 - v.at;
                if t < 0 || t >= v.len {
                    continue;
                }
                let s = samples.get((v.from + t) as usize).copied().unwrap_or_default() * v.gain;
                frame.iter_mut().for_each(|o| *o += s);
            }
        }
    }

    /// The mix from `from`, `len` samples of it, as one channel at [`RATE`]: what bouncing it writes.
    pub fn render(&self, from: i64, len: i64) -> Sound {
        let mut out = vec![0f32; len.max(0) as usize];
        self.add(from as f64, 1.0, &mut out, 1);
        out.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
        Sound::new(out)
    }

    /// Where the last voice ends.
    pub fn end(&self) -> i64 {
        self.voices.iter().map(|v| v.at + v.len).max().unwrap_or(0)
    }
}

/// What the engine says of itself: where it is, the cue it is playing, how loud the output was lately, from 0 to 1, and the device it plays through, or why it can't.
#[derive(Clone, Debug, PartialEq)]
pub struct Deck {
    pub at: i64,
    pub cue: u64,
    pub level: f32,
    pub playing: bool,
    pub device: Result<String, String>,
}

impl Default for Deck {
    fn default() -> Self {
        Deck { at: 0, cue: 0, level: 0.0, playing: false, device: Err("The audio engine hasn't started".into()) }
    }
}

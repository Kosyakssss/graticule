use std::f32::consts::TAU;
use std::sync::{Arc, OnceLock};

use audio::{RATE, Sound};
use graticule::prelude::*;

/// Samples to a beat, at 120 beats a minute.
pub const BEAT: i64 = RATE as i64 / 2;

/// Samples to a bar of four beats.
pub const BAR: i64 = BEAT * 4;

/// How long each instrument's loop is: two bars.
pub const LOOP: i64 = BAR * 2;

/// The instruments the band plays, each a two-bar loop made in code.
#[derive(Facts, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Drums,
    Bass,
    Keys,
    Lead,
}

impl Part {
    pub fn name(self) -> Message {
        match self {
            Part::Drums => t!("Drums"),
            Part::Bass => t!("Bass"),
            Part::Keys => t!("Keys"),
            Part::Lead => t!("Lead"),
        }
    }

    /// The loop, made once however often it is played.
    pub fn sound(self) -> Arc<Sound> {
        static MADE: [OnceLock<Arc<Sound>>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
        MADE[self as usize].get_or_init(|| Arc::new(Sound::new(self.play()))).clone()
    }

    fn play(self) -> Vec<f32> {
        let mut out = vec![0f32; LOOP as usize];
        let mut noise = Noise(0x9e37_79b9_7f4a_7c15);
        let beat = BEAT as usize;
        match self {
            Part::Drums => {
                for b in 0..8 {
                    let at = b * beat;
                    if b % 2 == 0 {
                        hit(&mut out, at, 0.35, |t| (TAU * (50.0 + 90.0 * (-t * 30.0).exp()) * t).sin() * 0.8);
                    } else {
                        hit(&mut out, at, 0.2, |t| noise.next() * 0.45 * (-t * 18.0).exp() + (TAU * 180.0 * t).sin() * 0.2 * (-t * 25.0).exp());
                    }
                    for half in [0, beat / 2] {
                        hit(&mut out, at + half, 0.05, |t| noise.next() * 0.15 * (-t * 90.0).exp());
                    }
                }
            }
            Part::Bass => {
                let notes = [45, 45, 57, 45, 41, 41, 53, 43];
                for (b, n) in notes.iter().enumerate() {
                    let f = pitch(*n);
                    hit(&mut out, b * beat, 0.45, |t| ((TAU * f * t).sin() + 0.4 * (TAU * 2.0 * f * t).sin() + 0.2 * (TAU * 3.0 * f * t).sin()) * 0.35 * (-t * 4.0).exp());
                }
            }
            Part::Keys => {
                for (bar, chord) in [[57, 60, 64], [53, 57, 60]].iter().enumerate() {
                    for k in 0..4 {
                        let at = (bar * 4 + k) * beat;
                        hit(&mut out, at, 0.48, |t| chord.iter().map(|n| (TAU * pitch(*n) * t).sin()).sum::<f32>() * 0.08 * (t * 40.0).min(1.0) * (-t * 3.0).exp());
                    }
                }
            }
            Part::Lead => {
                let line = [(0, 76, 1), (1, 74, 1), (2, 72, 2), (4, 69, 1), (5, 72, 1), (6, 74, 2)];
                for (at, n, beats) in line {
                    let (f, long) = (pitch(n), beats as f32 * 0.48);
                    hit(&mut out, at * beat, long, |t| {
                        let wave = (TAU * f * t).sin() + (TAU * 3.0 * f * t).sin() / 3.0 + (TAU * 5.0 * f * t).sin() / 5.0;
                        wave * 0.12 * (t * 30.0).min(1.0) * (1.0 - t / long).max(0.0)
                    });
                }
            }
        }
        out
    }
}

/// Adds `voice`, a function of seconds since it began, to `out` from sample `at`, for `seconds`.
fn hit(out: &mut [f32], at: usize, seconds: f32, mut voice: impl FnMut(f32) -> f32) {
    let len = (seconds * RATE as f32) as usize;
    for (k, s) in out.iter_mut().skip(at).take(len).enumerate() {
        *s += voice(k as f32 / RATE as f32);
    }
}

/// The frequency of MIDI note `n`.
fn pitch(n: i32) -> f32 {
    440.0 * 2f32.powf((n - 69) as f32 / 12.0)
}

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}

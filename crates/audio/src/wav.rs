use crate::RATE;

struct Format {
    kind: u16,
    channels: usize,
    rate: u32,
    bits: u16,
}

pub fn read(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("That isn't a WAV file".into());
    }
    let (mut format, mut data, mut at) = (None, None, 12usize);
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        match &bytes[at..at + 4] {
            b"fmt " if body.len() >= 16 => {
                let word = |k: usize| u16::from_le_bytes([body[k], body[k + 1]]);
                let kind = match word(0) {
                    0xfffe if body.len() >= 26 => word(24),
                    kind => kind,
                };
                format = Some(Format { kind, channels: usize::from(word(2)), rate: u32::from_le_bytes([body[4], body[5], body[6], body[7]]), bits: word(14) });
            }
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + size + size % 2;
    }
    let (Some(format), Some(data)) = (format, data) else { return Err("The WAV file has no sound in it".into()) };
    if format.channels == 0 || format.rate == 0 {
        return Err("The WAV file's format can't be read".into());
    }
    let width = usize::from(format.bits / 8);
    let sample = |b: &[u8]| -> Option<f32> {
        Some(match (format.kind, format.bits) {
            (1, 8) => (f32::from(b[0]) - 128.0) / 128.0,
            (1, 16) => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
            (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            _ => return None,
        })
    };
    let frame = width * format.channels;
    if frame == 0 {
        return Err("The WAV file's format can't be read".into());
    }
    let mut mono = Vec::with_capacity(data.len() / frame);
    for f in data.chunks_exact(frame) {
        let mut sum = 0.0;
        for c in f.chunks_exact(width) {
            sum += sample(c).ok_or_else(|| format!("WAV files of {} bits in format {} can't be read", format.bits, format.kind))?;
        }
        mono.push(sum / format.channels as f32);
    }
    Ok(resampled(&mono, format.rate))
}

/// `samples` at `rate` brought to [`RATE`], by straight lines between neighbours.
pub fn resampled(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let step = f64::from(rate) / f64::from(RATE);
    let len = (samples.len() as f64 / step).floor() as usize;
    (0..len)
        .map(|k| {
            let t = k as f64 * step;
            let (i, f) = (t as usize, (t - t.floor()) as f32);
            let (a, b) = (samples[i], samples.get(i + 1).copied().unwrap_or(samples[i]));
            a + (b - a) * f
        })
        .collect()
}

pub fn write(samples: &[f32]) -> Vec<u8> {
    let data = (samples.len() * 4) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

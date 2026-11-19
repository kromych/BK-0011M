//! Speaker output and WAV files.
//!
//! The BK's speaker is one bit (bit 6 of the tape/speaker latch). [`Audio`]
//! averages that level over each output sample period (a box filter, which
//! keeps aliasing down) and removes the DC offset.

/// Output volume for a fully driven speaker.
const VOLUME: f32 = 0.3;

pub struct Audio {
    cpu_hz: u64,
    rate: u64,
    /// Progress through the current output sample, in CPU cycles × rate.
    fill: u64,
    acc: f64,
    dc_in: f32,
    dc_out: f32,
    samples: Vec<f32>,
}

impl Audio {
    pub fn new(cpu_hz: u32, rate: u32) -> Self {
        Audio {
            cpu_hz: u64::from(cpu_hz),
            rate: u64::from(rate),
            fill: 0,
            acc: 0.0,
            dc_in: 0.0,
            dc_out: 0.0,
            samples: Vec::new(),
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate as u32
    }

    /// `cycles` CPU clocks with the speaker at `level` (0.0 or 1.0).
    pub fn advance(&mut self, cycles: u32, level: f32) {
        let mut ticks = u64::from(cycles) * self.rate;
        while ticks > 0 {
            let take = ticks.min(self.cpu_hz - self.fill);
            self.acc += f64::from(level) * take as f64;
            self.fill += take;
            ticks -= take;
            if self.fill == self.cpu_hz {
                let x = (self.acc / self.cpu_hz as f64) as f32 * VOLUME;
                // One-pole DC blocker: y = x - x' + 0.995 y'.
                let y = x - self.dc_in + 0.995 * self.dc_out;
                self.dc_in = x;
                self.dc_out = y;
                self.samples.push(y);
                self.fill = 0;
                self.acc = 0.0;
            }
        }
    }

    /// Hand over the samples produced so far.
    pub fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }
}

/// The mean absolute deviation: how strong a signal is.
fn spread(samples: &[f32]) -> f32 {
    let n = samples.len().max(1) as f32;
    let mean = samples.iter().sum::<f32>() / n;
    samples.iter().map(|s| (s - mean).abs()).sum::<f32>() / n
}

/// How far `b` lags `a` (so that `b[i + lag]` matches `a[i]`), within
/// `max_lag` samples, and the correlation there (-1 to 1). Measured on up
/// to 16 stretches of 4096 samples spread through the recording.
fn best_lag(a: &[f32], b: &[f32], max_lag: isize) -> (isize, f32) {
    const WINDOW: usize = 4096;
    let len = a.len().min(b.len());
    let margin = max_lag.unsigned_abs();
    if len < WINDOW + 2 * margin {
        return (0, 0.0);
    }
    let mean = |s: &[f32]| s[..len].iter().sum::<f32>() / len as f32;
    let (ma, mb) = (mean(a), mean(b));
    let windows = 16.min((len - 2 * margin) / WINDOW);
    let step = (len - 2 * margin - WINDOW) / windows.max(1);
    let starts: Vec<usize> = (0..windows).map(|w| margin + w * step).collect();
    let energy = |s: &[f32], m: f32, shift: isize| -> f32 {
        starts
            .iter()
            .flat_map(|&st| (st..st + WINDOW).map(move |i| (i as isize + shift) as usize))
            .map(|i| (s[i] - m).powi(2))
            .sum()
    };
    let ea = energy(a, ma, 0);
    (-max_lag..=max_lag)
        .map(|lag| {
            let cross: f32 = starts
                .iter()
                .flat_map(|&st| st..st + WINDOW)
                .map(|i| (a[i] - ma) * (b[(i as isize + lag) as usize] - mb))
                .sum();
            let norm = (ea * energy(b, mb, lag)).sqrt();
            (lag, if norm > 0.0 { cross / norm } else { 0.0 })
        })
        .max_by(|x, y| x.1.abs().total_cmp(&y.1.abs()))
        .unwrap_or((0, 0.0))
}

/// Mono audio, samples in -1.0..=1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct Wav {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Wav {
    /// Read a PCM WAV file (8 or 16 bits, any number of channels, combined
    /// into one by [`Wav::from_channels`]).
    pub fn parse(bytes: &[u8]) -> Result<Wav, String> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err("not a WAV file".into());
        }
        let (mut format, mut data) = (None, None);
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            let body = bytes
                .get(pos + 8..pos + 8 + len)
                .ok_or("truncated WAV chunk")?;
            match id {
                b"fmt " => format = Some(body),
                b"data" => data = Some(body),
                _ => {}
            }
            pos += 8 + len + (len & 1);
        }
        let format = format.ok_or("WAV has no fmt chunk")?;
        let data = data.ok_or("WAV has no data chunk")?;
        if format.len() < 16 || u16::from_le_bytes([format[0], format[1]]) != 1 {
            return Err("only PCM WAV files are supported".into());
        }
        let channels = usize::from(u16::from_le_bytes([format[2], format[3]])).max(1);
        let rate = u32::from_le_bytes(format[4..8].try_into().unwrap());
        let bits = u16::from_le_bytes([format[14], format[15]]);
        let sample = |s: &[u8]| -> f32 {
            match bits {
                8 => (f32::from(s[0]) - 128.0) / 128.0,
                _ => f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0,
            }
        };
        let width = match bits {
            8 => 1,
            16 => 2,
            _ => return Err(format!("{bits}-bit WAV files aren't supported")),
        };
        let mut split = vec![Vec::with_capacity(data.len() / (width * channels)); channels];
        for frame in data.chunks_exact(width * channels) {
            for (ch, s) in split.iter_mut().zip(frame.chunks_exact(width)) {
                ch.push(sample(s));
            }
        }
        Ok(Wav::from_channels(rate, split))
    }

    /// Combine the channels of a tape recording into one. A cassette's
    /// track is mono, but a stereo head reads it twice, and the two copies
    /// are often a little apart in time (the head's azimuth: 0.05 ms on
    /// some of the archive's recordings, 2 samples at 44.1 kHz). Averaged as
    /// they are, every edge blurs; so each channel is first moved into line
    /// with the strongest one, by the lag (up to 1 ms) where they correlate
    /// best, and turned over if it is inverted. A channel that doesn't
    /// follow the strongest (silent, or a different track) is left out.
    pub fn from_channels(rate: u32, mut channels: Vec<Vec<f32>>) -> Wav {
        channels.retain(|c| !c.is_empty());
        let Some(strongest) = (0..channels.len())
            .max_by(|&a, &b| spread(&channels[a]).total_cmp(&spread(&channels[b])))
        else {
            return Wav {
                rate,
                samples: Vec::new(),
            };
        };
        let reference = &channels[strongest];
        let max_lag = (rate / 1000).max(1) as isize;
        // (channel, lag, sign) of each channel that follows the reference.
        let used: Vec<(&Vec<f32>, isize, f32)> = channels
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if i == strongest {
                    return Some((c, 0, 1.0));
                }
                let (lag, r) = best_lag(reference, c, max_lag);
                (r.abs() >= 0.5).then_some((c, lag, r.signum()))
            })
            .collect();
        let samples = (0..reference.len() as isize)
            .map(|i| {
                let (sum, n) =
                    used.iter()
                        .fold(
                            (0.0, 0),
                            |(sum, n), (c, lag, sign)| match usize::try_from(i + lag)
                                .ok()
                                .and_then(|j| c.get(j))
                            {
                                Some(s) => (sum + s * sign, n + 1),
                                None => (sum, n),
                            },
                        );
                sum / n.max(1) as f32
            })
            .collect();
        Wav { rate, samples }
    }

    /// Encode as a 16-bit mono PCM WAV file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let data_len = self.samples.len() * 2;
        let mut out = Vec::with_capacity(44 + data_len);
        out.extend(b"RIFF");
        out.extend((36 + data_len as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes()); // PCM
        out.extend(1u16.to_le_bytes()); // mono
        out.extend(self.rate.to_le_bytes());
        out.extend((self.rate * 2).to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend((data_len as u32).to_le_bytes());
        for s in &self.samples {
            out.extend(((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tape-like square wave with pulses of 8 and 16 samples, from a
    /// fixed pseudo-random bit pattern.
    fn tape_like(n: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(n);
        let (mut seed, mut level) = (12345u32, 0.5f32);
        while out.len() < n {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let width = if seed >> 16 & 1 == 0 { 8 } else { 16 };
            out.extend(std::iter::repeat_n(level, width));
            level = -level;
        }
        out.truncate(n);
        out
    }

    #[test]
    fn channels_are_lined_up_before_mixing() {
        let left = tape_like(100_000);
        // The right channel is the same track 2 samples later, quieter.
        let right: Vec<f32> = [0.0, 0.0]
            .iter()
            .chain(&left[..left.len() - 2])
            .map(|s| s * 0.8)
            .collect();
        assert_eq!(best_lag(&left, &right, 44).0, 2);
        let wav = Wav::from_channels(44_100, vec![left.clone(), right]);
        // Lined up, the mix has the same edges as the left channel: each
        // sample has the left one's sign.
        let same = wav
            .samples
            .iter()
            .zip(&left)
            .skip(10)
            .filter(|(m, l)| m.signum() == l.signum())
            .count();
        assert_eq!(same, left.len() - 10);

        // An inverted channel is turned over to match the strongest; a
        // silent one is left out.
        let inverted: Vec<f32> = left.iter().map(|s| -0.5 * s).collect();
        let wav = Wav::from_channels(44_100, vec![left.clone(), inverted]);
        let expected: Vec<f32> = left.iter().map(|s| 0.75 * s).collect();
        assert_eq!(wav.samples, expected);
        let wav = Wav::from_channels(44_100, vec![vec![0.0; left.len()], left.clone()]);
        assert_eq!(wav.samples, left);
        // One channel, or none, passes through.
        assert_eq!(Wav::from_channels(8000, vec![left.clone()]).samples, left);
        assert!(Wav::from_channels(8000, vec![]).samples.is_empty());
    }

    #[test]
    fn box_filter_and_dc_blocker() {
        // 1 MHz CPU, 1 kHz output: 1000 cycles per sample.
        let mut a = Audio::new(1_000_000, 1000);
        a.advance(1500, 1.0);
        a.advance(500, 0.0);
        let s = a.take();
        assert_eq!(s.len(), 2);
        assert!((s[0] - VOLUME).abs() < 1e-6, "full sample");
        // Second sample averages half high, half low, minus the DC blocker's memory.
        assert!((s[1] - (VOLUME * 0.5 - VOLUME + 0.995 * VOLUME)).abs() < 1e-6);
    }

    #[test]
    fn wav_round_trip() {
        let wav = Wav {
            rate: 22050,
            samples: vec![0.0, 0.5, -0.5, 1.0],
        };
        let back = Wav::parse(&wav.to_bytes()).unwrap();
        assert_eq!(back.rate, 22050);
        for (a, b) in wav.samples.iter().zip(&back.samples) {
            assert!((a - b).abs() < 1e-3);
        }
    }
}

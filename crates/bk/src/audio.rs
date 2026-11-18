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

/// Mono audio, samples in -1.0..=1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct Wav {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Wav {
    /// Read a PCM WAV file (8 or 16 bits, any number of channels, mixed
    /// down to mono).
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
        let samples = data
            .chunks_exact(width * channels)
            .map(|frame| frame.chunks_exact(width).map(sample).sum::<f32>() / channels as f32)
            .collect();
        Ok(Wav { rate, samples })
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

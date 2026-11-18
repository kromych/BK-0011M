//! The cassette interface.
//!
//! Two ways in and out:
//! - The signal: a recorded tape plays into SEL1 bit 5 while the motor runs
//!   (tape latch bit 7 = 0), and what the BK writes (latch bits 6 and 5) can
//!   be recorded. This goes through the ROM's own tape code, as on the real
//!   machine.
//! - Fast tape: the machine intercepts the ROM's tape routine and reads or
//!   writes `.bin` files in a [`TapeFiles`] store instead (see `lib.rs`).

use std::path::PathBuf;

use crate::audio::Wav;
use crate::bin::BinFile;

/// Sample rate of recordings.
pub const RECORD_RATE: u32 = 44_100;

/// A tape signal: one level per sample.
#[derive(Clone, Debug, Default)]
pub struct TapeSignal {
    rate: u32,
    levels: Vec<bool>,
    /// CPU clocks the tape has moved under the head.
    clocks: u64,
}

impl TapeSignal {
    /// Square up a recording with a Schmitt trigger: the level only flips
    /// once the signal crosses the mean by a margin, so noise near the zero
    /// crossings of a real tape doesn't add false edges. The margin is a
    /// fraction of the mean absolute deviation, so it follows the recording
    /// level.
    pub fn from_wav(wav: &Wav) -> TapeSignal {
        let n = wav.samples.len().max(1) as f32;
        let mean = wav.samples.iter().sum::<f32>() / n;
        let deviation = wav.samples.iter().map(|s| (s - mean).abs()).sum::<f32>() / n;
        let margin = deviation * 0.2;
        let mut high = false;
        let levels = wav
            .samples
            .iter()
            .map(|&s| {
                if high && s < mean - margin {
                    high = false;
                } else if !high && s > mean + margin {
                    high = true;
                }
                high
            })
            .collect();
        TapeSignal {
            rate: wav.rate,
            levels,
            clocks: 0,
        }
    }

    pub fn to_wav(&self) -> Wav {
        Wav {
            rate: self.rate,
            samples: self
                .levels
                .iter()
                .map(|&l| if l { 0.7 } else { -0.7 })
                .collect(),
        }
    }

    pub fn len_seconds(&self) -> f64 {
        self.levels.len() as f64 / f64::from(self.rate.max(1))
    }

    /// Move the tape by `cycles` CPU clocks; returns the level under the head,
    /// or `None` past the end.
    pub fn play(&mut self, cycles: u32, cpu_hz: u32) -> Option<bool> {
        self.clocks += u64::from(cycles);
        let index = self.clocks * u64::from(self.rate) / u64::from(cpu_hz);
        self.levels.get(index as usize).copied()
    }

    /// Append `cycles` CPU clocks of `level`.
    pub fn record(&mut self, cycles: u32, cpu_hz: u32, level: bool) {
        if self.rate == 0 {
            self.rate = RECORD_RATE;
        }
        self.clocks += u64::from(cycles);
        let want = (self.clocks * u64::from(self.rate) / u64::from(cpu_hz)) as usize;
        self.levels.resize(want.max(self.levels.len()), level);
    }

    /// Rewind to the start, for playing back a recording.
    pub fn rewind(&mut self) {
        self.clocks = 0;
    }

    /// How far the tape has moved, in CPU clocks (for save states).
    pub fn position(&self) -> u64 {
        self.clocks
    }

    pub fn seek(&mut self, clocks: u64) {
        self.clocks = clocks;
    }

    /// The tape has played to its end (for a CPU clocked at `cpu_hz`).
    pub fn ended(&self, cpu_hz: u32) -> bool {
        self.clocks * u64::from(self.rate.max(1)) / u64::from(cpu_hz) >= self.levels.len() as u64
    }
}

/// Where fast tape reads and writes files.
pub trait TapeFiles {
    /// The file a program asked for by its 16-character tape name (trailing
    /// spaces removed).
    fn read(&mut self, name: &str) -> Option<BinFile>;
    fn write(&mut self, name: &str, file: &BinFile) -> Result<(), String>;
}

/// `.bin` files in a directory. A tape name `NAME` matches `NAME` or
/// `NAME.bin`, ignoring case and spaces (BASIC pads names before their
/// extension, e.g. `PROG  .COD`); writes go to `NAME.bin` without the
/// spaces.
pub struct DirFiles {
    pub dir: PathBuf,
}

/// A name as files are matched: no spaces, lower case.
fn file_key(name: &str) -> String {
    name.chars()
        .filter(|c| *c != ' ')
        .collect::<String>()
        .to_lowercase()
}

impl TapeFiles for DirFiles {
    fn read(&mut self, name: &str) -> Option<BinFile> {
        let key = file_key(name);
        let wanted = [key.clone(), format!("{key}.bin")];
        let entries = std::fs::read_dir(&self.dir).ok()?;
        for entry in entries.flatten() {
            let file_name = file_key(&entry.file_name().to_string_lossy());
            if wanted.contains(&file_name) {
                let bytes = std::fs::read(entry.path()).ok()?;
                return BinFile::parse(&bytes).ok();
            }
        }
        None
    }

    fn write(&mut self, name: &str, file: &BinFile) -> Result<(), String> {
        let name: String = name.chars().filter(|c| *c != ' ').collect();
        let path = self.dir.join(format!("{name}.bin"));
        std::fs::write(&path, file.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Files kept in memory, for tests.
#[derive(Default)]
pub struct MemFiles {
    pub files: Vec<(String, BinFile)>,
}

impl TapeFiles for MemFiles {
    fn read(&mut self, name: &str) -> Option<BinFile> {
        self.files
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| f.clone())
    }

    fn write(&mut self, name: &str, file: &BinFile) -> Result<(), String> {
        self.files.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self.files.push((name.to_string(), file.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schmitt_trigger_ignores_noise_near_the_crossings() {
        // Four 60-sample half-periods joined by slow ramps, like a real
        // recording, with ±0.05 noise that makes a bare threshold flicker.
        let mut samples = Vec::new();
        let mut level = -0.5f32;
        for half in 0..4 {
            let target = if half % 2 == 0 { 0.5 } else { -0.5 };
            for i in 0..60 {
                let ramp = (i as f32 / 40.0).min(1.0);
                let noise = if i % 2 == 0 { 0.05 } else { -0.05 };
                samples.push(level + (target - level) * ramp + noise);
            }
            level = target;
        }
        let edges = |levels: &[bool]| levels.windows(2).filter(|w| w[0] != w[1]).count();
        let bare: Vec<bool> = samples.iter().map(|&s| s > 0.0).collect();
        assert!(
            edges(&bare) > 4,
            "the test signal does make a bare threshold flicker"
        );
        let t = TapeSignal::from_wav(&Wav {
            rate: 1000,
            samples,
        });
        // The signal starts low and each half begins with a ramp: up, down, up, down.
        assert_eq!(edges(&t.levels), 4, "exactly one edge per ramp");
    }

    #[test]
    fn dir_files_match_names_without_spaces_or_case() {
        let dir = std::env::temp_dir().join(format!("bk-dirfiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut files = DirFiles { dir: dir.clone() };
        let file = BinFile {
            address: 0o1000,
            data: vec![1, 2],
        };
        files.write("PROG  .COD", &file).unwrap();
        assert!(dir.join("PROG.COD.bin").exists(), "spaces dropped");
        assert_eq!(files.read("prog.cod"), Some(file.clone()));
        assert_eq!(files.read("PROG  .COD"), Some(file.clone()));
        std::fs::write(dir.join("Xonix.bin"), file.to_bytes()).unwrap();
        assert_eq!(
            files.read("XONIX .BIN"),
            Some(file.clone()),
            "padded BASIC name"
        );
        assert_eq!(files.read("XONIX"), Some(file));
        assert_eq!(files.read("OTHER"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn record_then_play() {
        let mut t = TapeSignal::default();
        t.record(1000, 44_100, true); // 1000 clocks at 44.1 kHz CPU = 1000 samples
        t.record(500, 44_100, false);
        t.rewind();
        assert_eq!(t.play(10, 44_100), Some(true));
        assert_eq!(t.play(995, 44_100), Some(false));
        assert_eq!(t.play(1000, 44_100), None);
        let wav = t.to_wav();
        let back = TapeSignal::from_wav(&wav);
        assert_eq!(back.levels, t.levels);
    }
}

//! AY-3-8910 sound chip, the common BK add-on on the parallel port.
//!
//! Three square-wave tone channels, a noise generator, a mixer and an
//! envelope generator, clocked at 1.714 MHz (12 MHz / 7). The BK selects a
//! register with a word write to 177714 and writes it with a byte write;
//! both are inverted (`value ^ 0xFF`).

/// AY clock on the BK.
pub const AY_HZ: u64 = 12_000_000 / 7;

/// Output level for each 4-bit amplitude, as measured on a real AY-3-8910
/// (the values published with the Ayumi emulator): roughly logarithmic.
const VOLUME: [f32; 16] = [
    0.0,
    0.009_994_66,
    0.014_450_29,
    0.021_057_45,
    0.030_701_15,
    0.045_548_18,
    0.064_499_89,
    0.107_362_48,
    0.126_588_85,
    0.204_989_7,
    0.292_210_27,
    0.372_838_94,
    0.492_530_7,
    0.635_324_64,
    0.805_584_8,
    1.0,
];

/// Bits each register has: tone periods are 12 bits, noise 5, and so on.
const REG_MASK: [u8; 16] = [
    0xff, 0x0f, 0xff, 0x0f, 0xff, 0x0f, 0x1f, 0xff, 0x1f, 0x1f, 0x1f, 0xff, 0xff, 0x0f, 0xff, 0xff,
];

fn volume(level: u8) -> f32 {
    VOLUME[usize::from(level & 15)]
}

#[derive(Clone, Debug, Default)]
pub struct Ay {
    regs: [u8; 16],
    selected: usize,
    tone_count: [u16; 3],
    tone_out: [bool; 3],
    noise_count: u8,
    noise_lfsr: u32,
    /// Divides the /8 tick by 2 for noise and envelope (which step every 16
    /// AY clocks).
    half: bool,
    env_count: u32,
    env_pos: u8,
    env_attack: bool,
    env_holding: bool,
    env_level: u8,
    /// CPU clocks × AY_HZ not yet turned into ticks.
    acc: u64,
}

impl Ay {
    pub fn new() -> Self {
        let mut ay = Ay {
            noise_lfsr: 1,
            ..Default::default()
        };
        ay.reset();
        ay
    }

    pub fn reset(&mut self) {
        let acc = self.acc;
        *self = Ay {
            noise_lfsr: 1,
            acc,
            ..Default::default()
        };
        self.regs[7] = 0xff; // everything off
    }

    pub fn select(&mut self, register: u8) {
        self.selected = usize::from(register & 15);
    }

    pub fn write(&mut self, value: u8) {
        let r = self.selected;
        self.regs[r] = value & REG_MASK[r];
        if r == 13 {
            // A new envelope shape restarts the envelope.
            self.env_count = 0;
            self.env_pos = 0;
            self.env_attack = value & 4 != 0;
            self.env_holding = false;
            self.env_level = if self.env_attack { 0 } else { 15 };
        }
    }

    fn tone_period(&self, ch: usize) -> u16 {
        (u16::from(self.regs[2 * ch + 1]) << 8 | u16::from(self.regs[2 * ch])).max(1)
    }

    /// Advance by `cycles` CPU clocks of a CPU running at `cpu_hz`.
    pub fn run(&mut self, cycles: u32, cpu_hz: u32) {
        self.acc += u64::from(cycles) * AY_HZ;
        let per_tick = u64::from(cpu_hz) * 8;
        while self.acc >= per_tick {
            self.acc -= per_tick;
            self.tick();
        }
    }

    /// One tick of the tone generators (every 8 AY clocks).
    fn tick(&mut self) {
        for ch in 0..3 {
            self.tone_count[ch] += 1;
            if self.tone_count[ch] >= self.tone_period(ch) {
                self.tone_count[ch] = 0;
                self.tone_out[ch] = !self.tone_out[ch];
            }
        }
        self.half = !self.half;
        if !self.half {
            return;
        }
        self.noise_count += 1;
        if self.noise_count >= self.regs[6].max(1) {
            self.noise_count = 0;
            let bit = (self.noise_lfsr ^ (self.noise_lfsr >> 3)) & 1;
            self.noise_lfsr = (self.noise_lfsr >> 1) | (bit << 16);
        }
        let env_period = (u32::from(self.regs[12]) << 8 | u32::from(self.regs[11])).max(1);
        self.env_count += 1;
        if self.env_count >= env_period {
            self.env_count = 0;
            self.step_envelope();
        }
    }

    fn step_envelope(&mut self) {
        if self.env_holding {
            return;
        }
        self.env_pos = (self.env_pos + 1).min(16);
        if self.env_pos > 15 {
            let shape = self.regs[13];
            let (cont, att_flip, hold) = (shape & 8 != 0, shape & 2 != 0, shape & 1 != 0);
            if !cont {
                self.env_holding = true;
                self.env_level = 0;
                return;
            }
            if hold {
                self.env_holding = true;
                self.env_level = if self.env_attack ^ att_flip { 15 } else { 0 };
                return;
            }
            self.env_pos = 0;
            if att_flip {
                self.env_attack = !self.env_attack;
            }
        }
        self.env_level = if self.env_attack {
            self.env_pos
        } else {
            15 - self.env_pos
        };
    }

    /// Current output, 0.0..=1.0: the three channels averaged.
    pub fn output(&self) -> f32 {
        let mixer = self.regs[7];
        let noise = self.noise_lfsr & 1 != 0;
        let mut sum = 0.0;
        for ch in 0..3 {
            let tone_on = self.tone_out[ch] || mixer & (1 << ch) != 0;
            let noise_on = noise || mixer & (8 << ch) != 0;
            if tone_on && noise_on {
                let amp = self.regs[8 + ch];
                sum += volume(if amp & 0x10 != 0 {
                    self.env_level
                } else {
                    amp & 15
                });
            }
        }
        sum / 3.0
    }
}

impl Ay {
    pub fn save(&self, w: &mut crate::state::Writer) {
        self.regs.iter().for_each(|&v| w.u8(v));
        w.u8(self.selected as u8);
        self.tone_count.iter().for_each(|&v| w.u16(v));
        self.tone_out.iter().for_each(|&v| w.bool(v));
        w.u8(self.noise_count);
        w.u32(self.noise_lfsr);
        w.bool(self.half);
        w.u32(self.env_count);
        w.u8(self.env_pos);
        w.bool(self.env_attack);
        w.bool(self.env_holding);
        w.u8(self.env_level);
        w.u64(self.acc);
    }

    /// Counters out of range are rejected: they would overflow, or (the
    /// clock accumulator) take ages to catch up.
    pub fn load(&mut self, r: &mut crate::state::Reader) -> crate::state::Result<()> {
        let bad = |what: &str| Err(format!("save state is corrupt (AY {what})"));
        for (v, mask) in self.regs.iter_mut().zip(REG_MASK) {
            *v = r.u8()? & mask;
        }
        self.selected = usize::from(r.u8()? & 15);
        for v in &mut self.tone_count {
            *v = r.u16()?;
            if *v > 0xfff {
                return bad("tone counter");
            }
        }
        for v in &mut self.tone_out {
            *v = r.bool()?;
        }
        self.noise_count = r.u8()?;
        self.noise_lfsr = r.u32()?;
        self.half = r.bool()?;
        self.env_count = r.u32()?;
        self.env_pos = r.u8()?; // 16: a finished envelope
        self.env_attack = r.bool()?;
        self.env_holding = r.bool()?;
        self.env_level = r.u8()?;
        self.acc = r.u64()?;
        if self.noise_count > 0x1f || self.noise_lfsr > 0x1_ffff {
            return bad("noise generator");
        }
        if self.env_count > 0xffff || self.env_pos > 16 || self.env_level > 15 {
            return bad("envelope");
        }
        // Below one tick at the fastest CPU clock (8 AY clocks × 4 MHz).
        if self.acc >= 8 * 4_000_000 {
            return bad("clock");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ay: &mut Ay, reg: u8, value: u8) {
        ay.select(reg);
        ay.write(value);
    }

    /// A tone of period P toggles every P ticks: frequency AY_HZ / (16 P).
    #[test]
    fn tone_frequency() {
        let mut ay = Ay::new();
        set(&mut ay, 0, 100);
        set(&mut ay, 7, 0b111_110); // tone A only
        set(&mut ay, 8, 15);
        let cpu_hz = 3_000_000;
        let mut rising = 0;
        let mut last = ay.output();
        for _ in 0..cpu_hz / 100 {
            ay.run(100, cpu_hz); // one second in 100-clock steps
            let now = ay.output();
            if now > last {
                rising += 1;
            }
            last = now;
        }
        let expected = AY_HZ as f64 / (16.0 * 100.0);
        assert!(
            (f64::from(rising) - expected).abs() <= 2.0,
            "{rising} Hz, expected {expected:.1}"
        );
    }

    #[test]
    fn envelope_shapes() {
        // Shape 13 (/‾‾): rises, then holds at the top.
        let mut ay = Ay::new();
        set(&mut ay, 11, 1);
        set(&mut ay, 13, 13);
        let mut levels = Vec::new();
        for _ in 0..20 {
            for _ in 0..2 {
                ay.tick();
            }
            levels.push(ay.env_level);
        }
        assert_eq!(
            &levels[..16],
            &(1..=15).chain([15]).collect::<Vec<u8>>()[..]
        );
        assert!(levels[16..].iter().all(|&l| l == 15));

        // Shape 0 (\___): falls, then holds at zero.
        set(&mut ay, 13, 0);
        for _ in 0..40 {
            ay.tick();
        }
        assert_eq!(ay.env_level, 0);
        assert!(ay.env_holding);
    }

    #[test]
    fn mixer_and_volume() {
        let mut ay = Ay::new();
        assert_eq!(ay.output(), 0.0, "silent after reset");
        set(&mut ay, 7, 0b111_111); // tone and noise off on all: output is the level
        set(&mut ay, 8, 15);
        assert!((ay.output() - 1.0 / 3.0).abs() < 1e-6);
        set(&mut ay, 8, 13);
        assert!((ay.output() - volume(13) / 3.0).abs() < 1e-6);
        assert!(VOLUME.windows(2).all(|w| w[0] < w[1]), "rising");
    }

    /// Random register writes at random intervals, on both CPU clocks: the
    /// output stays in range, the counters stay within their registers,
    /// and the state always saves and loads back exactly.
    #[test]
    fn random_writes_stay_in_range() {
        let mut x: u64 = 0x2545_f491;
        let mut ay = Ay::new();
        for i in 0..100_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ay.select(x as u8);
            ay.write((x >> 8) as u8);
            ay.run(
                (x >> 16) as u32 % 3000,
                if i % 2 == 0 { 3_000_000 } else { 4_000_000 },
            );
            let out = ay.output();
            assert!((0.0..=1.0).contains(&out), "output {out}");
            assert!(ay.tone_count.iter().all(|&c| c <= 0xfff));
            assert!(ay.noise_count <= 0x1f && ay.env_pos <= 16 && ay.env_level <= 15);
            if i % 1000 == 0 {
                let mut w = crate::state::Writer::default();
                ay.save(&mut w);
                let mut back = Ay::new();
                back.load(&mut crate::state::Reader::new(&w.bytes)).unwrap();
                let mut again = crate::state::Writer::default();
                back.save(&mut again);
                assert_eq!(w.bytes, again.bytes);
            }
        }
    }
}

//! Keyboard controller and the VM1's programmable timer.

/// Keyboard: status register 177660, data register 177662 (read).
#[derive(Clone)]
pub struct Keyboard {
    /// Bit 7: a key code is ready. Bit 6: interrupt mask (1 = masked).
    status: u16,
    data: u16,
    vector: u16,
    /// Interrupt requested for the key in `data` and not yet taken.
    request: bool,
}

pub const KBD_VECTOR: u16 = 0o60;
pub const KBD_VECTOR_AR2: u16 = 0o274;

impl Keyboard {
    pub fn new() -> Self {
        Keyboard {
            status: 0o100,
            data: 0,
            vector: KBD_VECTOR,
            request: false,
        }
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    #[cfg(test)]
    pub fn data(&self) -> u16 {
        self.data
    }

    /// Reading the data register consumes the key.
    pub fn read_data(&mut self) -> u16 {
        self.status &= !0o200;
        self.request = false;
        self.data
    }

    /// Only the interrupt mask (bit 6) is writable.
    pub fn write_status(&mut self, value: u16) {
        self.status = (self.status & !0o100) | (value & 0o100);
    }

    /// A key code arrives. It's dropped if the previous one hasn't been read.
    /// Codes with bit 7 set (function keys) and AR2 combinations use vector 274.
    pub fn press(&mut self, code: u8, ar2: bool) {
        if self.status & 0o200 != 0 {
            return;
        }
        self.data = u16::from(code & 0o177);
        self.status |= 0o200;
        self.vector = if ar2 || code & 0o200 != 0 {
            KBD_VECTOR_AR2
        } else {
            KBD_VECTOR
        };
        self.request = true;
    }

    /// The request is held while masked and is taken once unmasked, unless
    /// the key has been read in the meantime.
    pub fn acknowledge(&mut self) -> Option<u16> {
        if self.request && self.status & 0o100 == 0 {
            self.request = false;
            Some(self.vector)
        } else {
            None
        }
    }

    /// INIT (bus reset): mask interrupts, clear "ready" (BOS manual §1.2.1).
    pub fn reset(&mut self) {
        self.status = 0o100;
        self.request = false;
    }
}

/// The K1801VM1's timer: reload 177706, counter 177710, control 177712.
/// It counts down at CPU clock / 128, further divided by the prescaler.
#[derive(Clone)]
pub struct Timer {
    pub reload: u16,
    pub counter: u16,
    flags: u16,
    divider: u16,
}

mod tf {
    pub const STOP: u16 = 0o1;
    pub const WRAPAROUND: u16 = 0o2;
    pub const EXPENABLE: u16 = 0o4;
    pub const ONESHOT: u16 = 0o10;
    pub const RUN: u16 = 0o20;
    pub const DIV16: u16 = 0o40;
    pub const DIV4: u16 = 0o100;
    pub const EXPIRY: u16 = 0o200;
}

/// CPU clocks per timer tick.
pub const TIMER_CLOCK_DIVIDER: u32 = 128;

impl Timer {
    pub fn new() -> Self {
        let mut t = Timer {
            reload: 0,
            counter: 0,
            flags: 0,
            divider: 0,
        };
        t.reset();
        t
    }

    pub fn reset(&mut self) {
        self.reload = 0o11000;
        self.counter = 0o177777;
        self.flags = 0o177400;
        self.divider = 0;
    }

    pub fn control(&self) -> u16 {
        self.flags
    }

    /// Writing the control register also reloads the counter.
    pub fn write_control(&mut self, value: u16) {
        self.counter = self.reload;
        self.divider = 0;
        self.flags = 0o177400 | value;
    }

    /// One tick of the timer's input clock (every 128 CPU clocks).
    pub fn tick(&mut self) {
        if self.flags & tf::STOP != 0 {
            self.counter = self.reload;
            return;
        }
        if self.flags & tf::RUN == 0 {
            return;
        }
        self.divider += 1;
        let period = match self.flags & (tf::DIV16 | tf::DIV4) {
            0 => 1,
            tf::DIV16 => 16,
            tf::DIV4 => 4,
            _ => 64,
        };
        if self.divider < period {
            return;
        }
        self.divider = 0;
        self.counter = self.counter.wrapping_sub(1);
        if self.counter == 0 && self.flags & tf::WRAPAROUND == 0 {
            if self.flags & tf::ONESHOT != 0 {
                self.flags &= !tf::RUN;
            }
            self.counter = self.reload;
            if self.flags & tf::EXPENABLE != 0 {
                self.flags |= tf::EXPIRY;
            }
        }
    }
}

use crate::state::{Reader, Result, Writer};

impl Keyboard {
    pub fn save(&self, w: &mut Writer) {
        w.u16(self.status);
        w.u16(self.data);
        w.u16(self.vector);
        w.bool(self.request);
    }

    pub fn load(&mut self, r: &mut Reader) -> Result<()> {
        self.status = r.u16()?;
        self.data = r.u16()?;
        self.vector = r.u16()?;
        self.request = r.bool()?;
        Ok(())
    }
}

impl Timer {
    pub fn save(&self, w: &mut Writer) {
        w.u16(self.reload);
        w.u16(self.counter);
        w.u16(self.flags);
        w.u16(self.divider);
    }

    pub fn load(&mut self, r: &mut Reader) -> Result<()> {
        self.reload = r.u16()?;
        self.counter = r.u16()?;
        self.flags = r.u16()?;
        self.divider = r.u16()?;
        if self.divider > 64 {
            return Err("save state is corrupt (timer prescaler)".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_interrupt_waits_for_unmask_and_is_cancelled_by_reading() {
        let mut k = Keyboard::new();
        k.press(0o101, false);
        assert_eq!(k.status() & 0o200, 0o200);
        assert_eq!(k.acknowledge(), None, "masked at power-on");
        k.write_status(0);
        assert_eq!(k.acknowledge(), Some(KBD_VECTOR));
        assert_eq!(k.acknowledge(), None, "taken once");
        k.press(0o102, false);
        assert_eq!(
            k.read_data(),
            0o101,
            "second key dropped until the first is read"
        );
        k.press(0o102, true);
        k.write_status(0o100);
        k.write_status(0);
        assert_eq!(k.read_data(), 0o102);
        assert_eq!(k.acknowledge(), None, "reading cancels the request");
        k.press(0o203, false);
        assert_eq!((k.acknowledge(), k.data()), (Some(KBD_VECTOR_AR2), 0o003));
    }

    #[test]
    fn timer_counts_and_expires() {
        let mut t = Timer::new();
        t.reload = 3;
        t.write_control(tf::RUN | tf::EXPENABLE);
        t.tick();
        t.tick();
        assert_eq!((t.counter, t.control() & tf::EXPIRY), (1, 0));
        t.tick();
        assert_eq!((t.counter, t.control() & tf::EXPIRY), (3, tf::EXPIRY));

        t.write_control(tf::RUN | tf::DIV4);
        for _ in 0..4 {
            t.tick();
        }
        assert_eq!(t.counter, 2);

        t.write_control(tf::RUN | tf::ONESHOT);
        for _ in 0..3 {
            t.tick();
        }
        assert_eq!(t.control() & tf::RUN, 0, "one-shot stops");

        t.write_control(tf::RUN | tf::WRAPAROUND);
        for _ in 0..4 {
            t.tick();
        }
        assert_eq!(t.counter, 0o177777, "wraps past zero");
    }
}

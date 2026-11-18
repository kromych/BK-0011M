//! The BK-0010's serial line (the monitor's "telegraph channel", EMT 40–50,
//! monitor manual 00001-01.32.03 §4.1.5 and §10.1).
//!
//! There is no UART: the system register's bits carry the line, and the
//! monitor drives it in software. Writing 177716: bit 4 is the transmit
//! line (1 when idle), bit 5 tells the other end the BK is ready to
//! receive. Reading 177716: bit 4 is the receive line, bit 7 the other
//! end's ready signal. A byte is a start bit (0), eight data bits, least
//! significant first, and stop bits (1).
//!
//! The emulated far end keeps in step with the BK's driver rather than
//! with a clock: it takes one bit from each write to 177716 and presents
//! one bit to each read, as the monitor's routines do (they write the line
//! once per bit sent and read it once per bit received). So it works at
//! any of the monitor's speeds, and the host side needs no baud rate.
//! (In the emulator's timing the monitor's "9600" is about 6700 baud.)

use std::collections::VecDeque;

/// Bits of 177716.
const LINE: u16 = 0o20;
const READY: u16 = 0o40;
/// The tape motor's control bit: 1 stops the motor. Writes with the motor
/// running are the tape's, not the line's.
const MOTOR_OFF: u16 = 0o200;

#[derive(Clone, Debug, Default)]
pub struct Serial {
    /// Sending (BK to host): data bits received so far, and how many; `None`
    /// while the line is idle.
    tx: Option<(u8, u8)>,
    /// Receiving (host to BK): the byte being presented and how many of its
    /// bits the BK has read (0 = the start bit is next).
    rx: Option<(u8, u8)>,
    /// The BK is telling the other end it is ready to receive.
    ready: bool,
    /// Bytes for the BK to receive.
    pub input: VecDeque<u8>,
    /// Bytes the BK sent.
    pub output: Vec<u8>,
}

impl Serial {
    /// A write to the system register.
    pub fn write(&mut self, value: u16) {
        self.ready = value & (MOTOR_OFF | READY) == MOTOR_OFF | READY;
        if value & MOTOR_OFF == 0 {
            return;
        }
        let bit = value & LINE != 0;
        match self.tx {
            None if !bit => self.tx = Some((0, 0)),
            None => {}
            Some((byte, 8)) => {
                // The stop bit; a 0 here is a framing error and the byte is
                // dropped.
                if bit {
                    self.output.push(byte);
                }
                self.tx = None;
            }
            Some((byte, n)) => self.tx = Some((byte | u8::from(bit) << n, n + 1)),
        }
    }

    /// The receive line's level for a read of the system register (which
    /// moves the line on by one bit while a byte is going).
    pub fn read(&mut self) -> u16 {
        if self.rx.is_none()
            && self.ready
            && let Some(byte) = self.input.pop_front()
        {
            self.rx = Some((byte, 0));
        }
        let bit = match self.rx {
            None => true,
            Some((_, 0)) => false,
            Some((byte, n)) => byte >> (n - 1) & 1 != 0,
        };
        self.rx = match self.rx {
            Some((_, 8)) | None => None,
            Some((byte, n)) => Some((byte, n + 1)),
        };
        if bit { LINE } else { 0 }
    }

    pub fn save(&self, w: &mut crate::state::Writer) {
        for part in [self.tx, self.rx] {
            let (byte, n) = part.unwrap_or((0, 0xff));
            w.u8(byte);
            w.u8(n);
        }
        w.bool(self.ready);
    }

    pub fn load(&mut self, r: &mut crate::state::Reader) -> crate::state::Result<()> {
        let mut part = || -> crate::state::Result<Option<(u8, u8)>> {
            let (byte, n) = (r.u8()?, r.u8()?);
            match n {
                0xff => Ok(None),
                0..=8 => Ok(Some((byte, n))),
                _ => Err("save state is corrupt (serial line)".into()),
            }
        };
        self.tx = part()?;
        self.rx = part()?;
        self.ready = r.bool()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_each_way() {
        let mut s = Serial::default();
        // 0x41: start, 1 0 0 0 0 0 1 0, two stop bits, as EMT 42 writes them.
        for bit in [0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 1] {
            s.write(MOTOR_OFF | if bit == 1 { LINE } else { 0 });
        }
        assert_eq!(s.output, b"A");
        // Tape writes (motor running) don't reach the line.
        s.write(0);
        s.write(LINE);
        assert!(s.tx.is_none());

        s.input.extend(b"\x81");
        assert_eq!(s.read(), LINE, "idle until the BK is ready");
        s.write(MOTOR_OFF | READY | LINE);
        let bits: Vec<u16> = (0..10).map(|_| s.read() / LINE).collect();
        assert_eq!(bits, [0, 1, 0, 0, 0, 0, 0, 0, 1, 1]);
    }
}

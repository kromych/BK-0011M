//! The motherboard as the CPU sees it: memory plus I/O registers.
//! Differences between the BK-0010 and BK-0011M are noted where they apply.

use vm1::{Bus, BusError};

use crate::ay::Ay;
use crate::devices::{Keyboard, TIMER_CLOCK_DIVIDER, Timer};
use crate::fdc::Fdc;
use crate::memory::{Memory, Region};
use crate::roms::Model;

/// SEL1 (177716 read) bits.
pub mod sel1 {
    /// Start address in the high byte: the CPU begins at 140000 (BK-0011M).
    pub const START_BK0011M: u16 = 0o140000;
    /// The BK-0010 starts in its monitor ROM at 100000.
    pub const START_BK0010: u16 = 0o100000;
    /// Set by any write to 177716 and by keyboard/tape events; cleared by reading.
    pub const CHANGED: u16 = 0o4;
    /// Tape input, inverted: 0 while the signal is high.
    pub const TAPE_IN: u16 = 0o40;
    /// 1 while no key is held down.
    pub const KEY_UP: u16 = 0o100;
    /// The serial line's receive data (BK-0010).
    pub const SERIAL_IN: u16 = 0o20;
}

/// 177662 write-register bits (BK-0011M).
pub mod ctl {
    /// Bits 11–8: palette number.
    pub const PALETTE_SHIFT: u16 = 8;
    /// Disables the 50 Hz EVNT interrupt.
    pub const EVNT_OFF: u16 = 0o40000;
    /// Shows the second screen page.
    pub const SCREEN_2: u16 = 0o100000;
}

#[derive(Clone)]
pub struct Board {
    pub mem: Memory,
    pub kbd: Keyboard,
    pub timer: Timer,
    timer_clock: u32,
    /// SEL1 as read from 177716.
    pub sel1: u16,
    /// Tape/speaker latch: the last 177716 write with bit 11 clear.
    pub tape_latch: u16,
    /// 177662 write register: palette, EVNT disable, screen page.
    pub control: u16,
    /// 177664.
    pub scroll: u16,
    /// 177714 input (joystick) and output (printer, Covox, AY).
    pub port_in: u16,
    pub port_out: u16,
    /// A printer on the parallel port, when attached.
    pub printer: Option<Printer>,
    /// Something on the serial line, when attached.
    pub serial: Option<crate::serial::Serial>,
    /// An AY-3-8910 on the parallel port, when fitted: a word write to
    /// 177714 selects a register, a byte write writes it, both inverted.
    pub ay: Option<Ay>,
    /// The floppy controller, when fitted.
    pub fdc: Option<Fdc>,
    /// Memory wait clocks owed by the instruction being executed.
    waits: u32,
    /// BK-0010: the frame pulse is wired to IRQ2, giving a 50 Hz interrupt
    /// on vector 100 as the BK-0011M has.
    pub irq2_50hz: bool,
    /// Debugger watchpoints: word addresses whose writes stop the machine.
    pub watch: Vec<u16>,
    /// A write to a watched address: (address, value).
    pub watch_hit: Option<(u16, u16)>,
}

/// A printer on the parallel port. The ROMs' printer drivers wait for a
/// ready line, put the character on the output lines with a strobe, wait
/// for the printer to go busy, and drop the strobe: BASIC on the BK-0010.01
/// with bit 8 (000400) for both the strobe and ready, BOS on the BK-0011M
/// with bit 14 (040000). The printer takes a byte on a rising strobe on
/// either bit, reports busy on both until the strobe falls, and ready
/// otherwise.
#[derive(Clone, Debug, Default)]
pub struct Printer {
    strobe: bool,
    busy: bool,
    /// Bytes printed and not yet taken.
    pub printed: Vec<u8>,
}

impl Printer {
    const LINES: u16 = 0o40400;

    fn status(&self) -> u16 {
        if self.busy { 0 } else { Self::LINES }
    }

    fn write(&mut self, value: u16) {
        let strobe = value & Self::LINES != 0;
        if strobe && !self.strobe {
            self.printed.push(value as u8);
            self.busy = true;
        }
        if !strobe {
            self.busy = false;
        }
        self.strobe = strobe;
    }
}

/// Extra CPU clocks per RAM/ROM access on the BK-0011M, on top of the
/// instruction timing tables. The tables match the BK-0010 (3 MHz), where
/// the 037 memory controller's 6 MHz slots come every 4 CPU clocks; at
/// 4 MHz they come every 5.33, and writes are held 3 clocks instead of 1.
/// Fitted to the MiST core's chip-accurate CPU and 037 model running from
/// RAM (tools/vm1-timing): 664 clocks against MiST's 674 over 23 test
/// instructions, worst case 6.
pub const BK0011M_ACCESS_WAIT: u32 = 2;

impl Board {
    pub fn new(mem: Memory) -> Self {
        let mut board = Board {
            mem,
            kbd: Keyboard::new(),
            timer: Timer::new(),
            timer_clock: 0,
            sel1: 0,
            tape_latch: 0,
            control: 0,
            scroll: 0,
            port_in: 0,
            port_out: 0,
            printer: None,
            serial: None,
            ay: None,
            fdc: None,
            waits: 0,
            irq2_50hz: false,
            watch: Vec::new(),
            watch_hit: None,
        };
        board.power_on();
        board
    }

    pub fn power_on(&mut self) {
        self.mem.reset();
        if let Some(fdc) = &mut self.fdc {
            fdc.reset();
        }
        if let Some(ay) = &mut self.ay {
            ay.reset();
        }
        self.kbd = Keyboard::new();
        self.timer.reset();
        self.timer_clock = 0;
        let start = match self.mem.model() {
            Model::Bk0010 => sel1::START_BK0010,
            Model::Bk0011m => sel1::START_BK0011M,
        };
        self.sel1 = start | 0o300;
        self.tape_latch = 0o200;
        self.control = 0o047400;
        self.scroll = 0o1330;
        self.port_in = 0;
        self.port_out = 0;
        self.waits = 0;
        self.watch_hit = None;
    }

    /// Advance devices by `cycles` CPU clocks.
    pub fn tick(&mut self, cycles: u32) {
        self.timer_clock += cycles;
        while self.timer_clock >= TIMER_CLOCK_DIVIDER {
            self.timer_clock -= TIMER_CLOCK_DIVIDER;
            self.timer.tick();
        }
        if let Some(fdc) = &mut self.fdc {
            fdc.tick(cycles);
        }
        if let Some(ay) = &mut self.ay {
            ay.run(cycles, crate::cpu_hz(self.mem.model()));
        }
    }

    fn is_bk0011m(&self) -> bool {
        self.mem.model() == Model::Bk0011m
    }

    /// Memory wait clocks owed since the last call.
    pub fn take_waits(&mut self) -> u32 {
        std::mem::take(&mut self.waits)
    }

    /// Count a memory (RAM or ROM) access for wait states.
    fn memory_access(&mut self) {
        if self.is_bk0011m() {
            self.waits += BK0011M_ACCESS_WAIT;
        }
    }

    /// The 50 Hz interrupt: the BK-0011M's, unless switched off; on a
    /// BK-0010 only with `irq2_50hz`, and only once vector 100 holds a
    /// handler. (With the line live from power-on, the interrupt would
    /// come as soon as the monitor enables interrupts, before anything has
    /// set the vector, and the machine would crash; so a BK-0010 can't have
    /// had it wired permanently.)
    pub fn evnt_enabled(&self) -> bool {
        if self.is_bk0011m() {
            self.control & ctl::EVNT_OFF == 0
        } else {
            self.irq2_50hz && self.mem.peek_word(0o100).is_some_and(|v| v != 0)
        }
    }

    /// The BK-0010's colors are those of BK-0011M palette 0.
    pub fn palette(&self) -> usize {
        if self.is_bk0011m() {
            usize::from(self.control >> ctl::PALETTE_SHIFT) & 15
        } else {
            0
        }
    }

    pub fn screen_page(&self) -> &[u8] {
        self.mem.screen(self.control & ctl::SCREEN_2 != 0)
    }

    /// Tape motor: latch bit 7, 0 = running.
    pub fn motor_on(&self) -> bool {
        self.tape_latch & 0o200 == 0
    }

    /// Tape output, as recorded: latch bit 6.
    pub fn speaker(&self) -> bool {
        self.tape_latch & 0o100 != 0
    }

    /// The speaker's level. Bits 6, 5 and (BK-0011M) 2 of the latch all
    /// carry tape data (BOS manual §1.2.6.3) and drive the speaker as a
    /// 3-bit DAC weighted 4:2:1, as in the MiST core; bit 6 alone is 1.0.
    pub fn speaker_level(&self) -> f32 {
        let bit = |b: u16| if self.tape_latch & b != 0 { 1.0 } else { 0.0 };
        bit(0o100)
            + 0.5 * bit(0o40)
            + if self.is_bk0011m() {
                0.25 * bit(0o4)
            } else {
                0.0
            }
    }

    /// The tape signal under the head.
    pub fn set_tape_input(&mut self, high: bool) {
        let bit = if high { 0 } else { sel1::TAPE_IN };
        if self.sel1 & sel1::TAPE_IN != bit {
            self.sel1 = (self.sel1 & !sel1::TAPE_IN) | bit | sel1::CHANGED;
        }
    }

    pub fn set_key_held(&mut self, held: bool) {
        if held {
            self.sel1 &= !sel1::KEY_UP;
        } else {
            self.sel1 |= sel1::KEY_UP;
        }
        self.sel1 |= sel1::CHANGED;
    }

    fn io_read(&mut self, addr: u16) -> Result<u16, BusError> {
        Ok(match addr {
            0o177660 => self.kbd.status(),
            0o177662 => self.kbd.read_data(),
            0o177664 => self.scroll,
            // VM1 internal registers; fixed values as in bkbtl.
            0o177700 => 0o177740,
            0o177702 => 0o177777,
            0o177704 => 0o177440,
            0o177706 => self.timer.reload,
            0o177710 => self.timer.counter,
            0o177712 => self.timer.control(),
            0o177714 => match &self.printer {
                Some(p) => p.status(),
                None => self.port_in,
            },
            0o177130 => self.fdc.as_mut().ok_or(BusError)?.read_status(),
            0o177132 => self.fdc.as_mut().ok_or(BusError)?.read_data(),
            0o177716 => {
                let mut value = self.sel1;
                self.sel1 &= !sel1::CHANGED;
                if let Some(s) = &mut self.serial {
                    value = (value & !sel1::SERIAL_IN) | s.read();
                }
                value
            }
            _ => return Err(BusError),
        })
    }

    fn io_write(&mut self, addr: u16, value: u16) -> Result<(), BusError> {
        match addr {
            0o177660 => self.kbd.write_status(value),
            // Read-only on the BK-0010.
            0o177662 if !self.is_bk0011m() => return Err(BusError),
            0o177662 => self.control = value,
            0o177664 => self.scroll = value & 0o1377,
            0o177700..=0o177704 => {}
            0o177706 => self.timer.reload = value,
            0o177710 => {} // counter is read-only
            0o177712 => self.timer.write_control(value),
            0o177714 => {
                self.port_out = value;
                if let Some(ay) = &mut self.ay {
                    ay.select(!value as u8);
                }
                if let Some(p) = &mut self.printer {
                    p.write(value);
                }
            }
            0o177130 => {
                let fdc = self.fdc.as_mut().ok_or(BusError)?;
                // On a BK-0010, bits 2-3 choose what 120000-157777 shows;
                // they don't reach the controller.
                let value = if self.mem.model() == Model::Bk0010 {
                    self.mem.window = match value & 0o14 {
                        0o14 => crate::memory::Bk0010Window::Basic,
                        0o10 => crate::memory::Bk0010Window::Nothing,
                        _ => crate::memory::Bk0010Window::ExtraRam,
                    };
                    value & !0o14
                } else {
                    value
                };
                fdc.write_command(value)
            }
            0o177132 => self.fdc.as_mut().ok_or(BusError)?.write_data(value),
            0o177716 => {
                self.sel1 |= sel1::CHANGED;
                if value & 0o4000 != 0 && self.is_bk0011m() {
                    self.mem.paging = value;
                } else {
                    self.tape_latch = value;
                    if let Some(s) = &mut self.serial {
                        s.write(value);
                    }
                }
            }
            _ => return Err(BusError),
        }
        Ok(())
    }

    fn io_write_byte(&mut self, addr: u16, value: u8) -> Result<(), BusError> {
        // Byte writes to the scroll register zero-extend into the whole word.
        if addr & !1 == 0o177664 {
            return self.io_write(0o177664, u16::from(value));
        }
        // The paging register only takes word writes (BOS manual §1.2.6.2);
        // a byte write reaches the tape/speaker latch.
        if addr & !1 == 0o177716 {
            self.sel1 |= sel1::CHANGED;
            if addr & 1 == 0 {
                self.tape_latch = (self.tape_latch & 0xff00) | u16::from(value);
                if let Some(s) = &mut self.serial {
                    s.write(self.tape_latch);
                }
            }
            return Ok(());
        }
        // A byte write to the parallel port is an AY data write.
        if addr & !1 == 0o177714 {
            if addr & 1 == 0 {
                self.port_out = (self.port_out & 0xff00) | u16::from(value);
                if let Some(ay) = &mut self.ay {
                    ay.write(!value);
                }
            } else {
                self.port_out = (self.port_out & 0x00ff) | (u16::from(value) << 8);
            }
            if let Some(p) = &mut self.printer {
                p.write(self.port_out);
            }
            return Ok(());
        }
        // Rejected before the merge below, whose read would consume a key.
        if addr & !1 == 0o177662 && !self.is_bk0011m() {
            return Err(BusError);
        }
        // Otherwise merge with the register's current value (as bkbtl does).
        let word = self.io_read(addr & !1)?;
        let word = if addr & 1 != 0 {
            (word & 0x00ff) | (u16::from(value) << 8)
        } else {
            (word & 0xff00) | u16::from(value)
        };
        self.io_write(addr & !1, word)
    }
}

impl Bus for Board {
    fn read_word(&mut self, addr: u16) -> Result<u16, BusError> {
        match self.mem.translate(addr) {
            Region::Ram(o) => {
                self.memory_access();
                Ok(self.mem.ram_word(o))
            }
            Region::Rom(..) => {
                self.memory_access();
                Ok(self.mem.peek_word(addr).expect("ROM is readable"))
            }
            Region::Io => self.io_read(addr),
            Region::None => Err(BusError),
        }
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), BusError> {
        if !self.watch.is_empty() && self.watch.contains(&(addr & !1)) {
            self.watch_hit = Some((addr, value));
        }
        match self.mem.translate(addr) {
            Region::Ram(o) => {
                self.memory_access();
                self.mem.set_ram_word(o, value);
                Ok(())
            }
            Region::Io => self.io_write(addr, value),
            Region::Rom(..) | Region::None => Err(BusError),
        }
    }

    fn read_byte(&mut self, addr: u16) -> Result<u8, BusError> {
        match self.mem.translate(addr) {
            Region::Ram(o) => {
                self.memory_access();
                Ok(self.mem.ram[o])
            }
            Region::Rom(r, o) => {
                self.memory_access();
                Ok(self.mem.rom_byte(r, o))
            }
            Region::Io => {
                let word = self.io_read(addr & !1)?;
                Ok(if addr & 1 != 0 {
                    (word >> 8) as u8
                } else {
                    word as u8
                })
            }
            Region::None => Err(BusError),
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), BusError> {
        if !self.watch.is_empty() && self.watch.contains(&(addr & !1)) {
            self.watch_hit = Some((addr, u16::from(value)));
        }
        match self.mem.translate(addr) {
            Region::Ram(o) => {
                self.memory_access();
                self.mem.ram[o] = value;
                Ok(())
            }
            Region::Io => self.io_write_byte(addr, value),
            Region::Rom(..) | Region::None => Err(BusError),
        }
    }

    fn reset_devices(&mut self) {
        self.timer.reset();
        self.kbd.reset();
        if let Some(fdc) = &mut self.fdc {
            fdc.reset();
        }
    }

    fn acknowledge_interrupt(&mut self) -> Option<u16> {
        self.kbd.acknowledge()
    }
}

impl Board {
    pub fn save(&self, w: &mut crate::state::Writer) {
        self.mem.save(w);
        self.kbd.save(w);
        self.timer.save(w);
        w.u32(self.timer_clock);
        for v in [
            self.sel1,
            self.tape_latch,
            self.control,
            self.scroll,
            self.port_in,
            self.port_out,
        ] {
            w.u16(v);
        }
        if let Some(ay) = &self.ay {
            ay.save(w);
        }
        if let Some(p) = &self.printer {
            w.bool(p.strobe);
            w.bool(p.busy);
        }
        if let Some(s) = &self.serial {
            s.save(w);
        }
        if let Some(fdc) = &self.fdc {
            fdc.save(w);
        }
    }

    /// Which devices are fitted comes from the machine, not the state; the
    /// header has already been checked to match.
    pub fn load(&mut self, r: &mut crate::state::Reader) -> crate::state::Result<()> {
        self.mem.load(r)?;
        self.kbd.load(r)?;
        self.timer.load(r)?;
        self.timer_clock = r.u32()?;
        if self.timer_clock >= TIMER_CLOCK_DIVIDER {
            return Err("save state is corrupt (timer clock)".into());
        }
        for v in [
            &mut self.sel1,
            &mut self.tape_latch,
            &mut self.control,
            &mut self.scroll,
            &mut self.port_in,
            &mut self.port_out,
        ] {
            *v = r.u16()?;
        }
        if let Some(ay) = &mut self.ay {
            ay.load(r)?;
        }
        if let Some(p) = &mut self.printer {
            p.strobe = r.bool()?;
            p.busy = r.bool()?;
        }
        if let Some(s) = &mut self.serial {
            s.load(r)?;
        }
        if let Some(fdc) = &mut self.fdc {
            fdc.load(r)?;
        }
        self.waits = 0;
        Ok(())
    }
}

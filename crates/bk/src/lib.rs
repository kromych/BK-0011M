//! The Elektronika BK-0011M, and the BK-0010 it grew from: memory map, I/O
//! devices and video, around the `vm1` CPU core. Hardware details are in
//! `docs/hardware-reference.md`.
//!
//! The machine runs a frame (20 ms) at a time. A front end calls
//! [`Machine::run_frame`] 50 times a second, then [`Machine::render`] and
//! [`Machine::take_audio`].

mod audio;
mod ay;
mod bin;
mod board;
pub mod debugger;
mod devices;
mod fdc;
mod memory;
mod recording;
mod roms;
mod serial;
mod state;
mod tape;
mod tapeio;
mod typist;
mod video;

pub use audio::Wav;
pub use bin::BinFile;
pub use debugger::Debugger;
pub use fdc::{DiskImage, IMAGE_BYTES};
pub use recording::parse_recording;
pub use roms::{Bk0010Rom, Model, Roms};
pub use tape::{DirFiles, MemFiles, TapeFiles, TapeSignal};
pub use tapeio::{TapeFile, checksum, encode_tape, read_tape_files};
pub use typist::{Typist, russian_key};
pub use video::{Monitor, PALETTES, SCREEN_HEIGHT, SCREEN_WIDTH};
pub use vm1;
pub use vm1::CpuState;

use audio::Audio;
use board::Board;
use memory::{Memory, Region};
use vm1::Cpu;

/// Default audio sample rate.
pub const AUDIO_RATE: u32 = 44_100;

/// CPU clocks charged for a fast-tape transfer.
const FAST_TAPE_CYCLES: u32 = 1000;

/// Paging field for each physical RAM page (inverse of the paging table).
const PAGE_FIELD: [u16; 8] = [6, 0, 2, 3, 4, 1, 7, 5];

/// CPU clock: 3 MHz on the BK-0010, 4 MHz on the BK-0011M.
pub fn cpu_hz(model: Model) -> u32 {
    match model {
        Model::Bk0010 => 3_000_000,
        Model::Bk0011m => 4_000_000,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    /// Floppy controller fitted.
    pub fdd: bool,
    pub monitor: Monitor,
    /// An AY-3-8910 sound chip on the parallel port (177714).
    pub ay: bool,
    /// A Covox: an 8-bit DAC on the low byte of the parallel port output.
    pub covox: bool,
    /// A printer on the parallel port (instead of a joystick); what it
    /// prints comes from [`Machine::take_printed`].
    pub printer: bool,
    /// Something on the serial line (BK-0010): [`Machine::serial_send`]
    /// gives it bytes for the BK, [`Machine::take_serial`] what the BK sent.
    pub serial: bool,
    /// BK-0010: a 50 Hz interrupt on vector 100, as when the frame pulse is
    /// connected to IRQ2, once a program has set the vector. The manual
    /// lists the vector ("Сигнал IRQ2") but not what drives it; FOCAL's
    /// FCLK() counts these interrupts.
    pub irq2_50hz: bool,
}

/// Why [`Machine::run_frame`] stopped before the end of the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Break {
    /// About to execute the instruction at a breakpoint.
    Breakpoint(u16),
    /// About to execute, at `pc`, an instruction whose opcode was asked for
    /// ([`Machine::set_opcode_breaks`]; an EMT or TRAP, say).
    Opcode(u16),
    /// The instruction at `pc` wrote `value` to the watched address `addr`.
    Watch { pc: u16, addr: u16, value: u16 },
}

/// One executed instruction, recorded while tracing: the CPU before it and
/// the instruction's words.
#[derive(Clone, Copy, Debug)]
pub struct TraceEntry {
    pub cpu: vm1::CpuState,
    pub words: [u16; 3],
}

/// The level of a high tape signal in the sound with [`Machine::set_tape_sound`]
/// (the speaker's tape-out bit is 1.0).
const TAPE_SOUND_LEVEL: f32 = 0.5;

pub struct Machine {
    cpu: Cpu,
    board: Board,
    config: Config,
    /// CPU clocks per 50 Hz frame.
    frame_cycles: u32,
    /// CPU clocks already run into the current frame.
    frame_clock: u32,
    /// The current frame has started (its 50 Hz interrupt is raised) but
    /// not finished: a breakpoint stopped it.
    in_frame: bool,
    /// Debugger breakpoints.
    breakpoints: Vec<u16>,
    /// Debugger: opcodes to stop before.
    opcode_breaks: Vec<u16>,
    /// Debugger: the instructions executed, while tracing.
    trace: Option<Vec<TraceEntry>>,
    frames: u64,
    held_keys: Vec<u8>,
    cpu_hz: u32,
    audio: Audio,
    files: Option<Box<dyn TapeFiles>>,
    /// Entry of the ROM's tape routine, where fast tape steps in.
    tape_hook: Option<u16>,
    /// Mix the tape being played into the sound, as the cassette
    /// recorder's own speaker would play it.
    tape_sound: bool,
}

impl Machine {
    /// The model follows from the ROM set.
    pub fn new(roms: Roms, config: Config) -> Self {
        let hz = cpu_hz(roms.model());
        let frame_cycles = hz / 50;
        let mut machine = Machine {
            cpu_hz: hz,
            audio: Audio::new(hz, AUDIO_RATE),
            files: None,
            tape_hook: None,
            tape_sound: false,
            frame_cycles,
            cpu: Cpu::new(),
            board: Board::new(Memory::new(roms, config.fdd)),
            config,
            frame_clock: 0,
            in_frame: false,
            breakpoints: Vec::new(),
            opcode_breaks: Vec::new(),
            trace: None,
            frames: 0,
            held_keys: Vec::new(),
        };
        if config.fdd {
            machine.board.fdc = Some(fdc::Fdc::new(hz));
        }
        // The BK-0011M holds write strobes 2 clocks longer (as the MiST
        // core's chip-level CPU shows with its clocking).
        if machine.model() == Model::Bk0011m {
            machine.cpu.write_delay = 2;
        }
        if config.ay {
            machine.board.ay = Some(ay::Ay::new());
        }
        machine.board.irq2_50hz = config.irq2_50hz;
        if config.printer {
            machine.board.printer = Some(board::Printer::default());
        }
        if config.serial {
            machine.board.serial = Some(serial::Serial::default());
        }
        machine.power_on();
        // The tape routine's address, from the ROM's own call table: the
        // BK-0010 monitor's EMT 36 slot, or the BK-0011M BOS .BMB10 slot.
        let slot = match machine.model() {
            Model::Bk0010 => 0o100036,
            Model::Bk0011m => 0o140242,
        };
        machine.tape_hook = machine.peek_word(slot).filter(|&a| a >= 0o100000);
        machine
    }

    /// Power-cycle: a cold start, exactly like a newly built machine. RAM is
    /// cleared, the devices and the CPU reset, the frame count restarts.
    /// Disks stay in their drives and a tape in the recorder, as they would.
    pub fn power_on(&mut self) {
        self.board.power_on();
        self.cpu.reset(self.board.sel1 & 0o177400);
        self.frame_clock = 0;
        self.in_frame = false;
        self.frames = 0;
        self.held_keys.clear();
    }

    /// Run one 20 ms frame: raise the 50 Hz interrupt, then run the CPU for
    /// a frame's worth of clocks. With breakpoints or watchpoints set, it can
    /// stop early; the next call finishes the frame. To go on from a
    /// breakpoint, step past it with [`Machine::step_into`] first.
    pub fn run_frame(&mut self) -> Option<Break> {
        if !self.in_frame {
            self.begin_frame();
        }
        while self.frame_clock < self.frame_cycles {
            if !self.breakpoints.is_empty() && self.breakpoints.contains(&self.cpu.pc()) {
                return Some(Break::Breakpoint(self.cpu.pc()));
            }
            if !self.opcode_breaks.is_empty()
                && !self.cpu.is_waiting()
                && self
                    .peek_word(self.cpu.pc())
                    .is_some_and(|op| self.opcode_breaks.contains(&op))
            {
                return Some(Break::Opcode(self.cpu.pc()));
            }
            let pc = self.cpu.pc();
            self.step();
            if let Some((addr, value)) = self.board.watch_hit.take() {
                if self.frame_clock >= self.frame_cycles {
                    self.end_frame();
                }
                return Some(Break::Watch { pc, addr, value });
            }
        }
        self.end_frame();
        None
    }

    fn begin_frame(&mut self) {
        if self.board.evnt_enabled() {
            self.cpu.request_evnt();
        }
        self.in_frame = true;
    }

    fn end_frame(&mut self) {
        self.frame_clock -= self.frame_cycles;
        self.frames += 1;
        self.in_frame = false;
    }

    /// Execute one instruction for a debugger, keeping frames (and their
    /// 50 Hz interrupt) in step. Breakpoints don't apply; watchpoints do.
    pub fn step_into(&mut self) -> Option<Break> {
        if !self.in_frame {
            self.begin_frame();
        }
        let pc = self.cpu.pc();
        self.step();
        if self.frame_clock >= self.frame_cycles {
            self.end_frame();
        }
        let (addr, value) = self.board.watch_hit.take()?;
        Some(Break::Watch { pc, addr, value })
    }

    pub fn set_breakpoints(&mut self, addrs: &[u16]) {
        self.breakpoints = addrs.to_vec();
    }

    /// Stop before executing any instruction whose first word is one of
    /// these (for example 104016, EMT 16).
    pub fn set_opcode_breaks(&mut self, opcodes: &[u16]) {
        self.opcode_breaks = opcodes.to_vec();
    }

    /// Record every instruction executed, for [`Machine::take_trace`].
    pub fn set_tracing(&mut self, on: bool) {
        self.trace = on.then(Vec::new);
    }

    /// The instructions executed since the last call, while tracing.
    pub fn take_trace(&mut self) -> Vec<TraceEntry> {
        self.trace.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// A word of physical RAM: `page` (16 KB each: 0-7 on the BK-0011M,
    /// whose screens are 5 and 6 and system page 7; 0-1 the BK-0010's RAM)
    /// at `offset` (0-37777), whatever is mapped in.
    pub fn peek_page_word(&self, page: usize, offset: u16) -> Option<u16> {
        let at = page.checked_mul(memory::PAGE_SIZE)? + usize::from(offset & !1);
        (usize::from(offset) < memory::PAGE_SIZE && at + 1 < self.board.mem.ram.len())
            .then(|| self.board.mem.ram_word(at))
    }

    /// Change a word of physical RAM (see [`Machine::peek_page_word`]).
    pub fn poke_page_word(&mut self, page: usize, offset: u16, value: u16) -> bool {
        if self.peek_page_word(page, offset).is_none() {
            return false;
        }
        let at = page * memory::PAGE_SIZE + usize::from(offset & !1);
        self.board.mem.set_ram_word(at, value);
        true
    }

    /// Stop when the CPU writes any of these addresses (RAM or I/O; word
    /// addresses, so a byte write to either half counts).
    pub fn set_watchpoints(&mut self, addrs: &[u16]) {
        self.board.watch = addrs.iter().map(|a| a & !1).collect();
        self.board.watch_hit = None;
    }

    /// Execute one CPU instruction (plus interrupts) and advance devices.
    /// Returns the CPU clocks used.
    pub fn step(&mut self) -> u32 {
        if self.trace.is_some() && !self.cpu.is_waiting() {
            let pc = self.cpu.pc();
            let word = |i: u16| self.peek_word(pc.wrapping_add(2 * i)).unwrap_or(0);
            let entry = TraceEntry {
                cpu: self.cpu.state(),
                words: [word(0), word(1), word(2)],
            };
            if let Some(trace) = &mut self.trace {
                trace.push(entry);
            }
        }
        let speaker_level = self.board.speaker_level();
        let tape = if self.tape_sound && self.board.tape_input() && self.tape_busy() {
            TAPE_SOUND_LEVEL
        } else {
            0.0
        };
        let covox = if self.config.covox {
            f32::from(self.board.port_out as u8) / 255.0
        } else {
            0.0
        };
        // The CPU and every device run on one clock: the CPU reports each
        // bus cycle's time within the step (Bus::cycle), and the board brings
        // its devices, the tape among them, up to that moment first.
        let start = self.board.clock;
        self.board.step_start = start;
        let cycles = if self.files.is_some()
            && Some(self.cpu.pc()) == self.tape_hook
            && !self.cpu.is_waiting()
            && self.fast_tape()
        {
            FAST_TAPE_CYCLES
        } else {
            self.cpu.step(&mut self.board)
        };
        self.board.catch_up(start + u64::from(cycles));
        self.frame_clock += cycles;
        // The sound level during the instruction is the one before it; a
        // change takes effect from the next instruction (a fraction of an
        // audio sample).
        let ay = self.board.ay.as_ref().map_or(0.0, ay::Ay::output);
        self.audio
            .advance(cycles, speaker_level + covox + ay + tape);
        cycles
    }

    /// Audio produced since the last call, mono, at [`Machine::audio_rate`].
    pub fn take_audio(&mut self) -> Vec<f32> {
        self.audio.take()
    }

    pub fn audio_rate(&self) -> u32 {
        self.audio.rate()
    }

    /// Change the audio sample rate (discarding samples not yet taken).
    pub fn set_audio_rate(&mut self, rate: u32) {
        self.audio = Audio::new(self.cpu_hz, rate);
    }

    /// Put a tape in the recorder; it plays while the BK runs the motor.
    pub fn insert_tape(&mut self, tape: Option<TapeSignal>) {
        self.board.tape_in = tape;
    }

    /// Play the tape being read through the sound as well (off at first),
    /// as the cassette recorder's speaker does. What the BK writes to tape
    /// is heard anyway: it drives the BK's own speaker.
    pub fn set_tape_sound(&mut self, on: bool) {
        self.tape_sound = on;
    }

    pub fn tape_sound(&self) -> bool {
        self.tape_sound
    }

    /// Start recording what the BK writes to tape (while the motor runs).
    pub fn start_recording(&mut self) {
        self.board.recording = Some(TapeSignal::default());
    }

    pub fn take_recording(&mut self) -> Option<TapeSignal> {
        self.board.recording.take()
    }

    /// Fast tape: when a program reads or writes a named file, use this
    /// store instead of the tape signal. Files it doesn't have still come
    /// from the tape.
    pub fn set_tape_files(&mut self, files: Option<Box<dyn TapeFiles>>) {
        self.files = files;
    }

    /// The whole machine state (CPU, RAM, devices, disks in the drives), to
    /// restore later with [`Machine::load_state`]. ROMs, tapes and the
    /// fast-tape file store aren't included; the tape position is.
    pub fn save_state(&self) -> Vec<u8> {
        let mut w = state::Writer::default();
        w.bytes.extend(state::MAGIC);
        w.u16(state::VERSION);
        self.save_header(&mut w);
        let c = self.cpu.state();
        c.r.iter().for_each(|&v| w.u16(v));
        w.u16(c.psw);
        for flag in [
            c.waiting,
            c.halted,
            c.irq1,
            c.evnt,
            c.overlap,
            c.sign_extend,
        ] {
            w.bool(flag);
        }
        self.board.save(&mut w);
        w.u32(self.frame_clock);
        w.bool(self.in_frame);
        w.u64(self.frames);
        w.bytes(&self.held_keys);
        w.bool(self.board.tape_in.is_some());
        w.u64(self.board.tape_in.as_ref().map_or(0, TapeSignal::position));
        w.bytes
    }

    /// The model, fitted devices and ROMs: what a state needs to match.
    fn save_header(&self, w: &mut state::Writer) {
        w.u8(match self.model() {
            Model::Bk0010 => 0,
            Model::Bk0011m => 1,
        });
        w.bool(self.config.fdd);
        w.bool(self.config.ay);
        w.bool(self.config.covox);
        w.bool(self.config.printer);
        w.bool(self.config.serial);
        w.u64(self.board.mem.rom_checksum());
    }

    /// Restore a state from [`Machine::save_state`]. It must come from a
    /// machine of the same model, with the same ROMs and devices. On error
    /// nothing changes.
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.get(..8) != Some(&state::MAGIC[..]) {
            return Err("not a BK save state".into());
        }
        let mut r = state::Reader::new(&bytes[8..]);
        let version = r.u16()?;
        if version != state::VERSION {
            return Err(format!(
                "save state version {version}; this emulator reads version {}",
                state::VERSION
            ));
        }
        let mut expected = state::Writer::default();
        self.save_header(&mut expected);
        let mut theirs = state::Writer::default();
        let model = r.u8()?;
        theirs.u8(model);
        let (fdd, ay, covox, printer, serial) =
            (r.bool()?, r.bool()?, r.bool()?, r.bool()?, r.bool()?);
        theirs.bool(fdd);
        theirs.bool(ay);
        theirs.bool(covox);
        theirs.bool(printer);
        theirs.bool(serial);
        let roms = r.u64()?;
        theirs.u64(roms);
        if theirs.bytes != expected.bytes {
            let name = |m| if m == 0 { "BK-0010" } else { "BK-0011M" };
            let devices = |fitted: [bool; 5]| {
                let names = ["floppy", "AY", "Covox", "printer", "serial line"];
                let list: Vec<&str> = names
                    .iter()
                    .zip(fitted)
                    .filter(|d| d.1)
                    .map(|d| *d.0)
                    .collect();
                if list.is_empty() {
                    "no extras".to_string()
                } else {
                    list.join(", ")
                }
            };
            return Err(format!(
                "save state is for a {} with {}{}; this machine is a {} with {}",
                name(model),
                devices([fdd, ay, covox, printer, serial]),
                if roms != self.board.mem.rom_checksum() {
                    " and other ROMs"
                } else {
                    ""
                },
                name(expected.bytes[0]),
                devices([
                    self.config.fdd,
                    self.config.ay,
                    self.config.covox,
                    self.config.printer,
                    self.config.serial,
                ]),
            ));
        }
        let mut regs = [0; 8];
        for v in &mut regs {
            *v = r.u16()?;
        }
        let cpu = vm1::CpuState {
            r: regs,
            psw: r.u16()?,
            waiting: r.bool()?,
            halted: r.bool()?,
            irq1: r.bool()?,
            evnt: r.bool()?,
            overlap: r.bool()?,
            sign_extend: r.bool()?,
        };
        let mut board = self.board.clone();
        board.load(&mut r)?;
        let frame_clock = r.u32()?;
        let in_frame = r.bool()?;
        let frames = r.u64()?;
        let held_keys = r.bytes()?;
        let had_tape = r.bool()?;
        let tape_position = r.u64()?;
        r.finish()?;
        // Out-of-range clocks would overflow, or stall the machine.
        if frame_clock >= 16 * self.frame_cycles || frames >= 1 << 56 || tape_position >= 1 << 40 {
            return Err("save state is corrupt (frame or tape clock)".into());
        }

        self.cpu.set_state(cpu);
        self.board = board;
        self.frame_clock = frame_clock;
        self.in_frame = in_frame;
        self.frames = frames;
        self.held_keys = held_keys;
        if had_tape && let Some(tape) = &mut self.board.tape_in {
            tape.seek(tape_position);
        }
        Ok(())
    }

    /// What the printer has printed since the last call (KOI-8 bytes, as the
    /// BK sends them).
    pub fn take_printed(&mut self) -> Vec<u8> {
        self.board
            .printer
            .as_mut()
            .map(|p| std::mem::take(&mut p.printed))
            .unwrap_or_default()
    }

    /// Bytes for the BK to receive on the serial line (with
    /// [`Config::serial`]).
    pub fn serial_send(&mut self, bytes: &[u8]) {
        if let Some(s) = &mut self.board.serial {
            s.input.extend(bytes);
        }
    }

    /// Bytes waiting for the BK to receive them.
    pub fn serial_pending(&self) -> usize {
        self.board.serial.as_ref().map_or(0, |s| s.input.len())
    }

    /// What the BK has sent on the serial line since the last call.
    pub fn take_serial(&mut self) -> Vec<u8> {
        self.board
            .serial
            .as_mut()
            .map(|s| std::mem::take(&mut s.output))
            .unwrap_or_default()
    }

    /// What the parallel port's input lines show (177714 read): a joystick,
    /// for one (bit 0 up, 1 right, 2 down, 3 left, 4–7 buttons, 1 = held).
    pub fn set_port_input(&mut self, value: u16) {
        self.board.port_in = value;
    }

    /// The tape/speaker latch: the last 177716 write with bit 11 clear.
    pub fn tape_latch(&self) -> u16 {
        self.board.tape_latch
    }

    /// Put a disk in drive 0-3. Needs a floppy controller (`Config::fdd`).
    pub fn insert_disk(&mut self, drive: usize, image: DiskImage) -> Result<(), String> {
        let fdc = self
            .board
            .fdc
            .as_mut()
            .ok_or("no floppy controller fitted")?;
        fdc.insert(drive, image);
        Ok(())
    }

    /// Take a disk out, with any changes written into the image.
    pub fn eject_disk(&mut self, drive: usize) -> Option<DiskImage> {
        self.board.fdc.as_mut()?.eject(drive)
    }

    /// The disk in a drive, with changes so far written into the image.
    pub fn disk(&mut self, drive: usize) -> Option<&DiskImage> {
        self.board.fdc.as_mut()?.image(drive)
    }

    /// Start or stop logging floppy controller register traffic; returns
    /// what was logged so far.
    pub fn trace_disk(&mut self, on: bool) -> Vec<String> {
        let Some(fdc) = self.board.fdc.as_mut() else {
            return Vec::new();
        };
        let old = fdc.trace.take().unwrap_or_default();
        fdc.trace = on.then(Vec::new);
        old
    }

    pub fn disk_motor_on(&self) -> bool {
        self.board.fdc.as_ref().is_some_and(|f| f.motor_on())
    }

    pub fn tape_motor_on(&self) -> bool {
        self.board.motor_on()
    }

    /// How far the inserted tape has played, in CPU clocks.
    pub fn tape_position(&self) -> Option<u64> {
        self.board.tape_in.as_ref().map(TapeSignal::position)
    }

    /// The inserted tape's position and length, in seconds of recording.
    pub fn tape_time(&self) -> Option<(f64, f64)> {
        let tape = self.board.tape_in.as_ref()?;
        let len = tape.len_seconds();
        let at = tape.position() as f64 / f64::from(self.cpu_hz);
        Some((at.min(len), len))
    }

    /// Wind the inserted tape to a position from [`Machine::tape_position`].
    pub fn seek_tape(&mut self, clocks: u64) {
        if let Some(tape) = &mut self.board.tape_in {
            tape.seek(clocks);
        }
    }

    /// No tape, or the inserted one has played to its end.
    pub fn tape_ended(&self) -> bool {
        self.board
            .tape_in
            .as_ref()
            .is_none_or(|t| t.ended(self.cpu_hz))
    }

    /// The tape is moving with something to play or record: the motor runs
    /// and a recording is inserted that hasn't run out, or recording is on.
    pub fn tape_busy(&self) -> bool {
        self.board.motor_on() && (self.board.recording.is_some() || !self.tape_ended())
    }

    // ----- fast tape -----

    fn mem_byte(&self, addr: u16) -> Option<u8> {
        match self.board.mem.translate(addr) {
            Region::Ram(o) => Some(self.board.mem.ram[o]),
            Region::Rom(r, o) => Some(self.board.mem.rom_byte(r, o)),
            _ => None,
        }
    }

    fn set_mem_byte(&mut self, addr: u16, b: u8) -> bool {
        match self.board.mem.translate(addr) {
            Region::Ram(o) => {
                self.board.mem.ram[o] = b;
                true
            }
            _ => false,
        }
    }

    fn mem_word(&self, addr: u16) -> u16 {
        self.peek_word(addr).unwrap_or(0)
    }

    fn set_mem_word(&mut self, addr: u16, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.set_mem_byte(addr, lo);
        self.set_mem_byte(addr.wrapping_add(1), hi);
    }

    /// A 16-character tape name, trailing spaces and NULs removed.
    fn tape_name(&self, addr: u16) -> String {
        let name: String = (0..16)
            .map(|i| self.mem_byte(addr + i).unwrap_or(0))
            .map(|b| vm1::koi8::decode(b).unwrap_or(' '))
            .collect();
        name.trim_end().to_string()
    }

    fn copy_name(&mut self, from: u16, to: u16) {
        for i in 0..16 {
            let b = self.mem_byte(from + i).unwrap_or(0);
            self.set_mem_byte(to + i, b);
        }
    }

    /// Copy `data` to `addr`; false if any byte would miss RAM.
    fn copy_in(&mut self, addr: u16, data: &[u8]) -> bool {
        data.iter()
            .enumerate()
            .all(|(i, &b)| self.set_mem_byte(addr.wrapping_add(i as u16), b))
    }

    fn copy_out(&self, addr: u16, len: u16) -> Vec<u8> {
        (0..len)
            .map(|i| self.mem_byte(addr.wrapping_add(i)).unwrap_or(0))
            .collect()
    }

    /// Return from the tape routine to its caller (RTS PC).
    fn return_from_routine(&mut self) {
        let sp = self.cpu.r[6];
        self.cpu.r[7] = self.mem_word(sp);
        self.cpu.r[6] = sp.wrapping_add(2);
    }

    /// Handle the tape request the CPU is about to make, if the file store
    /// can. Returns false to let the ROM do it (with the tape signal).
    fn fast_tape(&mut self) -> bool {
        match self.model() {
            Model::Bk0010 => self.fast_tape_bk0010(),
            Model::Bk0011m => self.fast_tape_bk0011m(),
        }
    }

    /// BK-0010 monitor EMT 36. R1 points at the block: byte 0 operation
    /// (0 motor off, 1 motor on, 2 write, 3 read, 4 dummy read), byte 1
    /// result, 2 address, 4 length, 6 name; on reading, 26 the file's
    /// address, 30 its length, 32 its name.
    fn fast_tape_bk0010(&mut self) -> bool {
        let block = self.cpu.r[1];
        let Some(code) = self.mem_byte(block) else {
            return false;
        };
        let name = self.tape_name(block + 6);
        if name.is_empty() {
            return false;
        }
        let result = match code {
            3 | 4 => {
                let Some(file) = self.files.as_mut().and_then(|f| f.read(&name)) else {
                    return false;
                };
                let mut result = 0;
                let addr = self.mem_word(block + 2);
                let dest = if addr == 0 { file.address } else { addr };
                if code == 3 && !self.copy_in(dest, &file.data) {
                    result = 3;
                }
                // As the ROM does (117124): the address the file goes to,
                // and its length, in the monitor's cells 264 and 266.
                self.set_mem_word(0o264, dest);
                self.set_mem_word(0o266, file.data.len() as u16);
                self.set_mem_word(block + 0o26, file.address);
                self.set_mem_word(block + 0o30, file.data.len() as u16);
                self.copy_name(block + 6, block + 0o32);
                result
            }
            2 => {
                let addr = self.mem_word(block + 2);
                let data = self.copy_out(addr, self.mem_word(block + 4));
                let file = BinFile {
                    address: addr,
                    data,
                };
                if self
                    .files
                    .as_mut()
                    .is_none_or(|f| f.write(&name, &file).is_err())
                {
                    return false;
                }
                0
            }
            _ => return false,
        };
        self.set_mem_byte(block + 1, result);
        self.return_from_routine();
        true
    }

    /// BK-0011M BOS .BMB10. R0 points at the area: byte 0 operation (0
    /// write, 1 read, 2 read the header, 3 dummy read), 2 address, 4
    /// length, 6 name, 26 the pages to map at 040000 and 100000 during the
    /// transfer (negative: leave as is); on reading, 30 the file's address,
    /// 32 its length, 34 its name. Errors set C and leave a code in byte 52.
    fn fast_tape_bk0011m(&mut self) -> bool {
        let area = self.cpu.r[0];
        let Some(code) = self.mem_byte(area) else {
            return false;
        };
        let name = self.tape_name(area + 6);
        if name.is_empty() {
            return false;
        }
        // Pages for the transfer.
        let saved = self.board.mem.paging;
        let [page0, page1] = self.mem_word(area + 0o26).to_le_bytes();
        let mut transfer = saved | 0o4000;
        if (page0 as i8) >= 0 {
            transfer = (transfer & !0o70000) | (PAGE_FIELD[usize::from(page0 & 7)] << 12);
        }
        if (page1 as i8) >= 0 {
            transfer = (transfer & !0o3433) | (PAGE_FIELD[usize::from(page1 & 7)] << 8);
        }

        let error = match code {
            1..=3 => {
                let Some(file) = self.files.as_mut().and_then(|f| f.read(&name)) else {
                    return false;
                };
                let mut error = 0;
                if code == 1 {
                    let addr = self.mem_word(area + 2);
                    let dest = if addr == 0 { file.address } else { addr };
                    self.board.mem.paging = transfer;
                    if !self.copy_in(dest, &file.data) {
                        error = 3;
                    }
                    self.board.mem.paging = saved;
                }
                self.set_mem_word(area + 0o30, file.address);
                self.set_mem_word(area + 0o32, file.data.len() as u16);
                self.copy_name(area + 6, area + 0o34);
                error
            }
            0 => {
                let addr = self.mem_word(area + 2);
                let len = self.mem_word(area + 4);
                self.board.mem.paging = transfer;
                let data = self.copy_out(addr, len);
                self.board.mem.paging = saved;
                let file = BinFile {
                    address: addr,
                    data,
                };
                if self
                    .files
                    .as_mut()
                    .is_none_or(|f| f.write(&name, &file).is_err())
                {
                    return false;
                }
                0
            }
            _ => return false,
        };
        // The return code in byte 52: the ROM clears it first (154646), so
        // a success leaves 0 there; callers such as the monitor's L command
        // check it.
        self.set_mem_byte(0o52, error);
        if error == 0 {
            self.cpu.psw &= !vm1::psw::C;
        } else {
            self.cpu.psw |= vm1::psw::C;
        }
        self.return_from_routine();
        true
    }

    /// Render the displayed screen page into `out`
    /// (`SCREEN_WIDTH × SCREEN_HEIGHT`, 0x00RRGGBB).
    pub fn render(&self, out: &mut [u32]) {
        video::render(
            self.board.screen_page(),
            self.board.scroll,
            self.board.palette(),
            self.config.monitor,
            out,
        );
    }

    pub fn monitor(&self) -> Monitor {
        self.config.monitor
    }

    pub fn set_monitor(&mut self, monitor: Monitor) {
        self.config.monitor = monitor;
    }

    /// A key goes down. `code` is the BK key code (KOI-7; codes with bit 7
    /// set are function keys). `ar2` means the AR2 modifier is held.
    pub fn key_down(&mut self, code: u8, ar2: bool) {
        if !self.held_keys.contains(&code) {
            self.held_keys.push(code);
        }
        self.board.set_key_held(true);
        self.board.kbd.press(code, ar2);
    }

    pub fn key_up(&mut self, code: u8) {
        self.held_keys.retain(|&k| k != code);
        if self.held_keys.is_empty() {
            self.board.set_key_held(false);
        }
    }

    /// Type `text` as a patient typist would, running frames as needed.
    /// `\n` is Enter; other control characters are sent as their codes
    /// (e.g. `\x0f` is ЛАТ), and U+0080-U+00FF are the function-key codes
    /// 200-377 (e.g. `\u{81}` is ПОВТ). Other characters are skipped.
    ///
    /// The BK has no keyboard buffer: a key that arrives before the previous
    /// one is read is lost, and BASIC reads (and discards) keys while a
    /// command runs. So this waits for each key to be taken, and gives each
    /// Enter time for the command to finish.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            // BK codes are 7-bit KOI-7, plus function-key codes 200-377
            // (as U+0080-U+00FF); '\n' (012) is the BK's Enter.
            let code = match c {
                '\0'..='\u{ff}' => c as u8,
                _ => continue,
            };
            for _ in 0..500 {
                if !self.key_pending() {
                    break;
                }
                self.run_frame();
            }
            self.key_down(code, false);
            for _ in 0..3 {
                self.run_frame();
            }
            self.key_up(code);
            // Enter gives the program time for the line; after a function
            // key the driver can be busy too (FOCAL loses a key typed
            // within 50 ms of ТАБ).
            let pause = match code {
                0o012 => 100,
                0o200.. => 10,
                _ => 3,
            };
            for _ in 0..pause {
                self.run_frame();
            }
        }
    }

    /// A key code is waiting to be read by the program (177660 bit 7).
    pub fn key_pending(&self) -> bool {
        self.board.kbd.status() & 0o200 != 0
    }

    /// The STOP key: not a normal key, it interrupts the CPU (IRQ1).
    /// On the BK-0011M it's ignored while software has set the STOP-lock bit
    /// (bit 12 of the tape register, BOS manual §1.2.6.3).
    pub fn press_stop(&mut self) {
        let locked = self.model() == Model::Bk0011m && self.board.tape_latch & 0o10000 != 0;
        if !locked {
            self.cpu.request_irq1();
        }
    }

    pub fn model(&self) -> Model {
        self.board.mem.model()
    }

    /// Copy a `.bin` file into memory as the CPU currently sees it. Every
    /// byte must land in RAM.
    pub fn load_bin(&mut self, bin: &BinFile) -> Result<(), String> {
        for (i, &b) in bin.data.iter().enumerate() {
            let addr = bin.address.wrapping_add(i as u16);
            match self.board.mem.translate(addr) {
                Region::Ram(o) => self.board.mem.ram[o] = b,
                _ => return Err(format!("{addr:06o} is not RAM")),
            }
        }
        Ok(())
    }

    /// Start a loaded program at `pc` with a fresh stack below 1000 and
    /// interrupts enabled, as the monitor's start command does.
    pub fn start_at(&mut self, pc: u16) {
        self.cpu.start_at(pc);
        self.cpu.r[6] = 0o1000;
        self.cpu.psw = 0;
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Read a word of RAM or ROM as the CPU currently sees it, without side
    /// effects. `None` for I/O registers and unmapped addresses.
    pub fn peek_word(&self, addr: u16) -> Option<u16> {
        self.board.mem.peek_word(addr)
    }

    /// Change a word of RAM (as the CPU at this moment sees it). False if
    /// the address isn't RAM.
    pub fn poke_word(&mut self, addr: u16, value: u16) -> bool {
        let addr = addr & !1;
        if !matches!(self.board.mem.translate(addr), Region::Ram(_)) {
            return false;
        }
        self.set_mem_word(addr, value);
        true
    }

    /// The CPU, to change registers from a debugger.
    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }
}

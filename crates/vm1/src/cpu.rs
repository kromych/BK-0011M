use crate::bus::{Bus, BusError};
use crate::decode::{Cond, DoubleOp, Instr, Operand, SingleOp, decode};
use crate::timing as t;

/// PSW bits.
pub mod psw {
    pub const C: u16 = 0o1;
    pub const V: u16 = 0o2;
    pub const Z: u16 = 0o4;
    pub const N: u16 = 0o10;
    /// Trace trap after each instruction.
    pub const T: u16 = 0o20;
    /// Priority: masks EVNT (IRQ2) and VIRQ. The VM1 has no other priority levels.
    pub const P: u16 = 0o200;
    /// Halt mode.
    pub const HALT: u16 = 0o400;
    /// Masks ACLO, IRQ1, EVNT and VIRQ. Can only be set in halt mode.
    pub const MASK_10: u16 = 0o2000;
    /// Masks IRQ1. Can only be set in halt mode.
    pub const MASK_11: u16 = 0o4000;
}

/// Trap and interrupt vectors handled inside the CPU.
pub mod vector {
    /// Bus timeout, illegal JMP/JSR, HALT, and the STOP key (IRQ1) on the BK.
    pub const BUS_ERROR: u16 = 0o4;
    pub const RESERVED: u16 = 0o10;
    /// Trace trap and BPT.
    pub const TRACE: u16 = 0o14;
    pub const IOT: u16 = 0o20;
    pub const EMT: u16 = 0o30;
    pub const TRAP: u16 = 0o34;
    /// EVNT (IRQ2): the 50 Hz frame interrupt on the BK-0011M.
    pub const EVNT: u16 = 0o100;
}

/// SEL1, the VM1's own system register; halt-mode entry writes to it.
const SEL1: u16 = 0o177716;

const SP: usize = 6;
const PC: usize = 7;

/// A trap raised by the instruction just executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    BusError,
    Reserved,
    /// HALT, START or STEP: halt-mode entry.
    Halt,
    Bpt,
    Iot,
    Emt,
    Trap,
}

#[derive(Clone, Copy, Debug)]
enum Loc {
    Reg(usize),
    Mem(u16),
}

type Exec = Result<(u32, Option<Fault>), BusError>;

/// Everything about the CPU, for save states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuState {
    pub r: [u16; 8],
    pub psw: u16,
    pub waiting: bool,
    pub halted: bool,
    /// Pending STOP (IRQ1) and 50 Hz (EVNT) requests.
    pub irq1: bool,
    pub evnt: bool,
}

/// K1801VM1 processor state.
#[derive(Clone, Debug)]
pub struct Cpu {
    /// R0–R5, R6 = SP, R7 = PC.
    pub r: [u16; 8],
    pub psw: u16,
    waiting: bool,
    halted: bool,
    irq1: bool,
    evnt: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            r: [0; 8],
            psw: 0o340,
            waiting: false,
            halted: false,
            irq1: false,
            evnt: false,
        }
    }

    /// Power-on or reset. The VM1 starts at the address in the high byte of
    /// SEL1, so the machine passes `sel1 & 0o177400` (0o140000 on the BK-0011M).
    pub fn reset(&mut self, start: u16) {
        *self = Self::new();
        self.r[PC] = start;
    }

    pub fn pc(&self) -> u16 {
        self.r[PC]
    }

    pub fn state(&self) -> CpuState {
        CpuState {
            r: self.r,
            psw: self.psw,
            waiting: self.waiting,
            halted: self.halted,
            irq1: self.irq1,
            evnt: self.evnt,
        }
    }

    pub fn set_state(&mut self, s: CpuState) {
        self.r = s.r;
        self.psw = s.psw;
        self.waiting = s.waiting;
        self.halted = s.halted;
        self.irq1 = s.irq1;
        self.evnt = s.evnt;
    }

    /// Inside `WAIT`, waiting for an interrupt.
    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    /// Stopped by a bus error while entering the bus-error trap itself (for
    /// example SP pointing at non-existent memory). Only `reset` restarts it.
    /// The real BK hangs in this state too.
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Continue execution at `pc` (as a monitor's "go" command would), leaving
    /// WAIT or a halted state.
    pub fn start_at(&mut self, pc: u16) {
        self.r[PC] = pc;
        self.waiting = false;
        self.halted = false;
    }

    /// Raise EVNT (IRQ2, vector 100). Latched until the CPU takes it.
    pub fn request_evnt(&mut self) {
        self.evnt = true;
    }

    /// Raise IRQ1: the STOP key on the BK. Latched until the CPU takes it.
    pub fn request_irq1(&mut self) {
        self.irq1 = true;
    }

    /// Execute one instruction, then take any traps and interrupts that are
    /// due. Returns the CPU clock cycles used.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        if self.halted {
            return t::WAIT_POLL;
        }
        let (cycles, fault, skip_interrupts) = if self.waiting {
            (t::WAIT_POLL, None, false)
        } else {
            self.execute_next(bus)
        };
        if skip_interrupts {
            cycles
        } else {
            cycles + self.service_interrupts(bus, fault)
        }
    }

    fn execute_next<B: Bus>(&mut self, bus: &mut B) -> (u32, Option<Fault>, bool) {
        let pc = self.r[PC];
        self.r[PC] = pc.wrapping_add(2);
        let Ok(opcode) = bus.read_word(pc & !1) else {
            return (t::ILLEGAL, Some(Fault::BusError), false);
        };
        let instr = decode(opcode);
        match self.execute(bus, instr) {
            Ok((cycles, fault)) => {
                // RTT that sets T delays the trace trap by one instruction; the
                // VM1 checks no interrupts at all after it.
                let skip = instr == Instr::Rtt && self.psw & psw::T != 0;
                (cycles, fault, skip)
            }
            // The instruction was aborted at the failing bus cycle.
            Err(BusError) => (t::ILLEGAL, Some(Fault::BusError), false),
        }
    }

    /// Take pending traps and interrupts in VM1 priority order, repeating
    /// until nothing unmasked is left (an interrupt can be taken before the
    /// first instruction of a trap handler).
    ///
    /// BPT, IOT, EMT and TRAP come first: the VM1's microcode runs their trap
    /// sequence as part of the instruction (their vectors don't go through
    /// the interrupt priority matrix), so interrupts and the trace trap are
    /// then judged against the handler's PSW. Checked on the MiST core's
    /// chip-level VM1 model: an EMT executed with EVNT pending enters the EMT
    /// handler first, and one executed with T set takes no trace trap until
    /// the handler's RTI restores T (tools/vm1-timing, `EVNT_AT`).
    fn service_interrupts<B: Bus>(&mut self, bus: &mut B, mut fault: Option<Fault>) -> u32 {
        let mut cycles = 0;
        // No trace trap is taken while in WAIT, and at most one per step.
        let mut trace_done = self.waiting;
        let instruction_trap = match fault {
            Some(Fault::Bpt) => Some(vector::TRACE),
            Some(Fault::Iot) => Some(vector::IOT),
            Some(Fault::Emt) => Some(vector::EMT),
            Some(Fault::Trap) => Some(vector::TRAP),
            _ => None,
        };
        if let Some(vector) = instruction_trap {
            // The trap sequence is already counted in the instruction time.
            fault = self.enter_trap(bus, vector).err().map(|_| Fault::BusError);
        }
        loop {
            let p = self.psw;
            let interruptible = p & (psw::P | psw::MASK_10) == 0;
            let mut is_bus_error = false;

            let vector = if let Some(f @ (Fault::BusError | Fault::Reserved)) = fault {
                fault = None;
                is_bus_error = f == Fault::BusError;
                if is_bus_error {
                    vector::BUS_ERROR
                } else {
                    vector::RESERVED
                }
            } else if !trace_done && p & psw::T != 0 {
                trace_done = true;
                vector::TRACE
            } else if self.irq1 && p & (psw::MASK_10 | psw::MASK_11) == 0 {
                self.irq1 = false;
                // Halt-mode entry writes SEL1, setting its "written" flag (bit 2)
                // so the handler can tell STOP from a bus hang. The BK has no
                // halt-mode memory, so the CPU ends up at vector 4 in user mode.
                let _ = bus.write_word(SEL1, 0o210);
                vector::BUS_ERROR
            } else if self.evnt && interruptible {
                self.evnt = false;
                vector::EVNT
            } else if let Some(v) = interruptible.then(|| bus.acknowledge_interrupt()).flatten() {
                v
            } else if fault == Some(Fault::Halt) {
                // Halt-mode entry writes SEL1 and saves PC/PSW at
                // 177674/177676, which do not exist on the BK, so it becomes
                // a bus error. The BOS manual (§1.3) confirms HALT sets the
                // SEL1 write flag, which is how BOS tells it from a bus error.
                let _ = bus.write_word(SEL1, 0o210);
                fault = Some(Fault::BusError);
                continue;
            } else {
                break;
            };

            self.waiting = false;
            cycles += t::INTERRUPT_ENTRY;
            if self.enter_trap(bus, vector).is_err() {
                if is_bus_error {
                    self.halted = true; // double bus error
                    break;
                }
                fault = Some(Fault::BusError);
            }
        }
        cycles
    }

    fn enter_trap<B: Bus>(&mut self, bus: &mut B, vector: u16) -> Result<(), BusError> {
        let (old_pc, old_psw) = (self.r[PC], self.psw);
        self.push(bus, old_psw)?;
        self.push(bus, old_pc)?;
        let pc = self.read_w(bus, vector)?;
        let new_psw = self.read_w(bus, vector.wrapping_add(2))?;
        self.r[PC] = pc;
        self.psw = new_psw & 0o377; // only the low byte is loaded; leaves halt mode
        Ok(())
    }

    // ----- memory helpers -----

    fn read_w<B: Bus>(&mut self, bus: &mut B, addr: u16) -> Result<u16, BusError> {
        bus.read_word(addr & !1)
    }

    fn write_w<B: Bus>(&mut self, bus: &mut B, addr: u16, value: u16) -> Result<(), BusError> {
        bus.write_word(addr & !1, value)
    }

    fn fetch<B: Bus>(&mut self, bus: &mut B) -> Result<u16, BusError> {
        let pc = self.r[PC];
        self.r[PC] = pc.wrapping_add(2);
        self.read_w(bus, pc)
    }

    fn push<B: Bus>(&mut self, bus: &mut B, value: u16) -> Result<(), BusError> {
        self.r[SP] = self.r[SP].wrapping_sub(2);
        let sp = self.r[SP];
        self.write_w(bus, sp, value)
    }

    fn pop<B: Bus>(&mut self, bus: &mut B) -> Result<u16, BusError> {
        let sp = self.r[SP];
        self.r[SP] = sp.wrapping_add(2);
        self.read_w(bus, sp)
    }

    /// Resolve an operand to a register or memory address, applying
    /// autoincrement/decrement and fetching index words.
    fn ea<B: Bus>(&mut self, bus: &mut B, o: Operand, byte: bool) -> Result<Loc, BusError> {
        let r = o.reg();
        // Byte autoincrement/decrement steps by 1, except on SP and PC.
        let step = if byte && r < SP { 1 } else { 2 };
        Ok(match o.mode() {
            0 => Loc::Reg(r),
            1 => Loc::Mem(self.r[r]),
            2 => {
                let a = self.r[r];
                self.r[r] = a.wrapping_add(step);
                Loc::Mem(a)
            }
            3 => {
                let a = self.r[r];
                self.r[r] = a.wrapping_add(2);
                Loc::Mem(self.read_w(bus, a)?)
            }
            4 => {
                self.r[r] = self.r[r].wrapping_sub(step);
                Loc::Mem(self.r[r])
            }
            5 => {
                self.r[r] = self.r[r].wrapping_sub(2);
                let a = self.r[r];
                Loc::Mem(self.read_w(bus, a)?)
            }
            6 => {
                let x = self.fetch(bus)?;
                Loc::Mem(self.r[r].wrapping_add(x))
            }
            _ => {
                let x = self.fetch(bus)?;
                let a = self.r[r].wrapping_add(x);
                Loc::Mem(self.read_w(bus, a)?)
            }
        })
    }

    /// Address of a memory operand (mode != 0).
    fn address<B: Bus>(&mut self, bus: &mut B, o: Operand) -> Result<u16, BusError> {
        match self.ea(bus, o, false)? {
            Loc::Mem(a) => Ok(a),
            Loc::Reg(_) => unreachable!("caller checks mode != 0"),
        }
    }

    fn load<B: Bus>(&mut self, bus: &mut B, loc: Loc, byte: bool) -> Result<u16, BusError> {
        match (loc, byte) {
            (Loc::Reg(r), false) => Ok(self.r[r]),
            (Loc::Reg(r), true) => Ok(self.r[r] & 0xff),
            (Loc::Mem(a), false) => self.read_w(bus, a),
            (Loc::Mem(a), true) => bus.read_byte(a).map(u16::from),
        }
    }

    fn store<B: Bus>(&mut self, bus: &mut B, loc: Loc, byte: bool, v: u16) -> Result<(), BusError> {
        match (loc, byte) {
            (Loc::Reg(r), false) => self.r[r] = v,
            (Loc::Reg(r), true) => self.r[r] = (self.r[r] & 0xff00) | (v & 0xff),
            (Loc::Mem(a), false) => self.write_w(bus, a, v)?,
            (Loc::Mem(a), true) => bus.write_byte(a, v as u8)?,
        }
        Ok(())
    }

    /// The VM1 writes bytes with a read-modify-write bus cycle, so a byte
    /// write that doesn't otherwise read its destination still reads it
    /// first. This matters for registers with read side effects.
    fn byte_write_read<B: Bus>(&mut self, bus: &mut B, loc: Loc) -> Result<(), BusError> {
        if let Loc::Mem(a) = loc {
            bus.read_byte(a)?;
        }
        Ok(())
    }

    // ----- flags -----

    fn flag(&self, f: u16) -> bool {
        self.psw & f != 0
    }

    fn set_nzvc(&mut self, n: bool, z: bool, v: bool, c: bool) {
        let bit = |set: bool, mask: u16| if set { mask } else { 0 };
        let flags = bit(n, psw::N) | bit(z, psw::Z) | bit(v, psw::V) | bit(c, psw::C);
        self.psw = (self.psw & !0o17) | flags;
    }

    fn cond(&self, c: Cond) -> bool {
        let (n, z, v, cy) = (
            self.flag(psw::N),
            self.flag(psw::Z),
            self.flag(psw::V),
            self.flag(psw::C),
        );
        match c {
            Cond::Br => true,
            Cond::Bne => !z,
            Cond::Beq => z,
            Cond::Bge => n == v,
            Cond::Blt => n != v,
            Cond::Bgt => !z && n == v,
            Cond::Ble => z || n != v,
            Cond::Bpl => !n,
            Cond::Bmi => n,
            Cond::Bhi => !cy && !z,
            Cond::Blos => cy || z,
            Cond::Bvc => !v,
            Cond::Bvs => v,
            Cond::Bcc => !cy,
            Cond::Bcs => cy,
        }
    }

    // ----- instructions -----

    fn execute<B: Bus>(&mut self, bus: &mut B, instr: Instr) -> Exec {
        let done = |cycles| Ok((cycles, None));
        match instr {
            Instr::Halt | Instr::Start | Instr::Step => Ok((t::ILLEGAL, Some(Fault::Halt))),
            Instr::Reserved => Ok((t::ILLEGAL, Some(Fault::Reserved))),
            Instr::Bpt => Ok((t::EMT, Some(Fault::Bpt))),
            Instr::Iot => Ok((t::EMT, Some(Fault::Iot))),
            Instr::Emt(_) => Ok((t::EMT, Some(Fault::Emt))),
            Instr::Trap(_) => Ok((t::EMT, Some(Fault::Trap))),
            Instr::Wait => {
                self.waiting = true;
                done(t::WAIT)
            }
            Instr::Reset => {
                bus.reset_devices();
                done(t::RESET)
            }
            Instr::Rti | Instr::Rtt => {
                let pc = self.pop(bus)?;
                let new_psw = self.pop(bus)?;
                self.r[PC] = pc;
                // Outside the halt-mode ROM area only the low byte is loaded
                // and the halt-mode bit is kept.
                self.psw = if pc < 0o160000 {
                    (new_psw & 0o377) | (self.psw & psw::HALT)
                } else {
                    new_psw & 0o777
                };
                done(t::RTI)
            }
            Instr::Jmp(dst) => {
                if dst.mode() == 0 {
                    return Ok((t::EMT, Some(Fault::BusError)));
                }
                self.r[PC] = self.address(bus, dst)?;
                done(t::DST_READ[dst.mode() as usize])
            }
            Instr::Jsr(reg, dst) => {
                if dst.mode() == 0 {
                    return Ok((t::EMT, Some(Fault::BusError)));
                }
                let target = self.address(bus, dst)?;
                let reg = reg as usize;
                self.r[SP] = self.r[SP].wrapping_sub(2);
                let (sp, linkage) = (self.r[SP], self.r[reg]);
                self.write_w(bus, sp, linkage)?;
                self.r[reg] = self.r[PC];
                self.r[PC] = target;
                done(t::JSR[dst.mode() as usize])
            }
            Instr::Rts(reg) => {
                let reg = reg as usize;
                let target = self.r[reg];
                let saved = self.pop(bus)?;
                self.r[PC] = target;
                self.r[reg] = saved;
                done(t::RTS)
            }
            Instr::Mark(n) => {
                self.r[SP] = self.r[PC].wrapping_add(2 * n as u16);
                self.r[PC] = self.r[5];
                self.r[5] = self.pop(bus)?;
                done(t::MARK)
            }
            Instr::Sob(reg, n) => {
                let reg = reg as usize;
                self.r[reg] = self.r[reg].wrapping_sub(1);
                if self.r[reg] != 0 {
                    self.r[PC] = self.r[PC].wrapping_sub(2 * n as u16);
                }
                done(t::SOB)
            }
            Instr::Branch(c, offset) => {
                if self.cond(c) {
                    self.r[PC] = self.r[PC].wrapping_add((offset as i16 as u16).wrapping_mul(2));
                }
                done(t::BRANCH)
            }
            Instr::ClearFlags(mask) => {
                self.psw &= !(mask as u16);
                done(if mask == 0 { t::NOP } else { t::REGREG })
            }
            Instr::SetFlags(mask) => {
                self.psw |= mask as u16;
                done(if mask == 0 { t::NOP } else { t::REGREG })
            }
            Instr::Swab(dst) => {
                let loc = self.ea(bus, dst, false)?;
                let v = self.load(bus, loc, false)?.swap_bytes();
                self.store(bus, loc, false, v)?;
                self.set_nzvc(v & 0x80 != 0, v & 0xff == 0, false, false);
                done(t::REGREG + t::DST[dst.mode() as usize])
            }
            Instr::Sxt(dst) => {
                let n = self.flag(psw::N);
                let loc = self.ea(bus, dst, false)?;
                self.store(bus, loc, false, if n { 0xffff } else { 0 })?;
                self.set_nzvc(n, !n, false, self.flag(psw::C));
                done(t::REGREG + t::DST[dst.mode() as usize])
            }
            Instr::Mtps(src) => {
                let loc = self.ea(bus, src, true)?;
                let v = self.load(bus, loc, true)?;
                // The T bit can't be changed by MTPS.
                self.psw = (self.psw & !0o357) | (v & 0o357);
                done(t::REGREG + t::DST[src.mode() as usize])
            }
            Instr::Mfps(dst) => {
                let v = self.psw & 0xff;
                let loc = self.ea(bus, dst, true)?;
                match loc {
                    Loc::Reg(r) => self.r[r] = v as u8 as i8 as u16,
                    Loc::Mem(_) => {
                        self.byte_write_read(bus, loc)?;
                        self.store(bus, loc, true, v)?;
                    }
                }
                self.set_nzvc(v & 0x80 != 0, v == 0, false, self.flag(psw::C));
                done(t::REGREG + t::DST[dst.mode() as usize])
            }
            Instr::Xor(reg, dst) => {
                let loc = self.ea(bus, dst, false)?;
                let d = self.load(bus, loc, false)?;
                let v = d ^ self.r[reg as usize];
                self.store(bus, loc, false, v)?;
                self.set_nzvc(v & 0x8000 != 0, v == 0, false, self.flag(psw::C));
                done(t::REGREG + t::DST_READ[dst.mode() as usize])
            }
            Instr::Single(op, byte, dst) => self.single(bus, op, byte, dst),
            Instr::Double(op, byte, src, dst) => self.double(bus, op, byte, src, dst),
        }
    }

    fn single<B: Bus>(&mut self, bus: &mut B, op: SingleOp, byte: bool, dst: Operand) -> Exec {
        let (sign, mask) = if byte {
            (0x80u16, 0xffu16)
        } else {
            (0x8000, 0xffff)
        };
        let m = dst.mode() as usize;
        let loc = self.ea(bus, dst, byte)?;

        if op == SingleOp::Clr {
            if byte {
                self.byte_write_read(bus, loc)?;
            }
            self.store(bus, loc, byte, 0)?;
            self.set_nzvc(false, true, false, false);
            return Ok((t::REGREG + t::DST[m], None));
        }

        let d = self.load(bus, loc, byte)?;
        let c = self.flag(psw::C);
        let (r, v, c) = match op {
            SingleOp::Tst => {
                self.set_nzvc(d & sign != 0, d == 0, false, false);
                return Ok((t::REGREG + t::SRC[m], None));
            }
            SingleOp::Clr => unreachable!("handled above"),
            SingleOp::Com => (!d & mask, false, true),
            SingleOp::Inc => {
                let r = d.wrapping_add(1) & mask;
                (r, r == sign, c)
            }
            SingleOp::Dec => (d.wrapping_sub(1) & mask, d == sign, c),
            SingleOp::Neg => {
                let r = d.wrapping_neg() & mask;
                (r, r == sign, r != 0)
            }
            SingleOp::Adc => {
                let r = d.wrapping_add(c as u16) & mask;
                (r, c && r == sign, c && r == 0)
            }
            SingleOp::Sbc => {
                let r = d.wrapping_sub(c as u16) & mask;
                (r, c && d == sign, c && r == mask)
            }
            SingleOp::Ror | SingleOp::Rol | SingleOp::Asr | SingleOp::Asl => {
                let (r, carry) = match op {
                    SingleOp::Ror => ((d >> 1) | if c { sign } else { 0 }, d & 1 != 0),
                    SingleOp::Rol => (((d << 1) | c as u16) & mask, d & sign != 0),
                    SingleOp::Asr => ((d >> 1) | (d & sign), d & 1 != 0),
                    _ => ((d << 1) & mask, d & sign != 0),
                };
                (r, (r & sign != 0) != carry, carry)
            }
        };
        self.store(bus, loc, byte, r)?;
        self.set_nzvc(r & sign != 0, r == 0, v, c);
        Ok((t::REGREG + t::DST[m], None))
    }

    fn double<B: Bus>(
        &mut self,
        bus: &mut B,
        op: DoubleOp,
        byte: bool,
        src: Operand,
        dst: Operand,
    ) -> Exec {
        let (sign, mask) = if byte {
            (0x80u16, 0xffu16)
        } else {
            (0x8000, 0xffff)
        };
        let (sm, dm) = (src.mode() as usize, dst.mode() as usize);

        // The source is fully read before the destination address is formed.
        let sloc = self.ea(bus, src, byte)?;
        let s = self.load(bus, sloc, byte)?;
        let dloc = self.ea(bus, dst, byte)?;

        let write_time = t::REGREG
            + t::SRC[sm]
            + if sm != 0 {
                t::DST_AFTER_MEM_SRC[dm]
            } else {
                t::DST[dm]
            };
        let c = self.flag(psw::C);

        match op {
            DoubleOp::Mov => {
                match (dloc, byte) {
                    // MOVB to a register sign-extends into the whole register.
                    (Loc::Reg(r), true) => self.r[r] = s as u8 as i8 as u16,
                    (_, true) => {
                        self.byte_write_read(bus, dloc)?;
                        self.store(bus, dloc, true, s)?;
                    }
                    (_, false) => self.store(bus, dloc, false, s)?,
                }
                self.set_nzvc(s & sign != 0, s == 0, false, c);
                Ok((write_time, None))
            }
            DoubleOp::Cmp | DoubleOp::Bit => {
                let d = self.load(bus, dloc, byte)?;
                if op == DoubleOp::Cmp {
                    let r = s.wrapping_sub(d) & mask;
                    let v = (s ^ d) & (s ^ r) & sign != 0;
                    self.set_nzvc(r & sign != 0, r == 0, v, s < d);
                } else {
                    let r = s & d;
                    self.set_nzvc(r & sign != 0, r == 0, false, c);
                }
                let read_time = if sm != 0 { t::SRC[dm] } else { t::DST_READ[dm] };
                Ok((t::REGREG + t::SRC[sm] + read_time, None))
            }
            DoubleOp::Bic | DoubleOp::Bis | DoubleOp::Add | DoubleOp::Sub => {
                let d = self.load(bus, dloc, byte)?;
                let (r, v, c) = match op {
                    DoubleOp::Bic => (d & !s, false, c),
                    DoubleOp::Bis => (d | s, false, c),
                    DoubleOp::Add => {
                        let r = d.wrapping_add(s);
                        (r, !(s ^ d) & (s ^ r) & sign != 0, r < d)
                    }
                    _ => {
                        let r = d.wrapping_sub(s);
                        (r, (d ^ s) & (d ^ r) & sign != 0, d < s)
                    }
                };
                self.store(bus, dloc, byte, r)?;
                self.set_nzvc(r & sign != 0, r & mask == 0, v, c);
                Ok((write_time, None))
            }
        }
    }
}

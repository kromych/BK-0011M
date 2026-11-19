use crate::bus::{Bus, BusError, Cycle};
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

type Exec = Result<Option<Fault>, BusError>;

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
    /// The next opcode fetch overlaps the end of the last instruction
    /// (see [`crate::timing::OVERLAP`]).
    pub overlap: bool,
    /// The last instruction is still sign-extending a byte into a register
    /// (see [`crate::timing::SIGN_EXTEND`]).
    pub sign_extend: bool,
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
    overlap: bool,
    sign_extend: bool,
    /// CPU clocks into the current step: the bus-cycle schedule's clock.
    t: u32,
    /// Extra clocks before every write strobe: 0 on the BK-0010; the
    /// BK-0011M holds writes longer.
    pub write_delay: u32,
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
            overlap: false,
            sign_extend: false,
            t: 0,
            write_delay: 0,
        }
    }

    /// Power-on or reset. The VM1 starts at the address in the high byte of
    /// SEL1, so the machine passes `sel1 & 0o177400` (0o140000 on the BK-0011M).
    pub fn reset(&mut self, start: u16) {
        let write_delay = self.write_delay;
        *self = Self::new();
        self.write_delay = write_delay;
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
            overlap: self.overlap,
            sign_extend: self.sign_extend,
        }
    }

    pub fn set_state(&mut self, s: CpuState) {
        self.r = s.r;
        self.psw = s.psw;
        self.waiting = s.waiting;
        self.halted = s.halted;
        self.irq1 = s.irq1;
        self.evnt = s.evnt;
        self.overlap = s.overlap;
        self.sign_extend = s.sign_extend;
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
    /// due. Returns the CPU clock cycles used: the bus-cycle schedule of
    /// [`crate::timing`], with each cycle's reply delay from [`Bus::cycle`].
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.t = 0;
        if self.halted {
            return t::WAIT_POLL;
        }
        let (fault, skip_interrupts) = if self.waiting {
            self.t = t::WAIT_POLL;
            (None, false)
        } else {
            self.execute_next(bus)
        };
        if !skip_interrupts {
            self.service_interrupts(bus, fault);
        }
        self.t
    }

    fn execute_next<B: Bus>(&mut self, bus: &mut B) -> (Option<Fault>, bool) {
        let Ok(opcode) = self.fetch(bus) else {
            return (Some(Fault::BusError), false);
        };
        if std::mem::take(&mut self.overlap) {
            self.t -= t::OVERLAP;
        }
        if std::mem::take(&mut self.sign_extend) {
            self.t = self.t.max(t::SIGN_EXTEND);
        }
        let instr = decode(opcode);
        self.overlap = !ends_on_register(instr);
        self.sign_extend = matches!(instr, Instr::Double(DoubleOp::Mov, true, _, d) | Instr::Mfps(d) if d.mode() == 0);
        match self.execute(bus, instr) {
            Ok(fault) => {
                // RTT that sets T delays the trace trap by one instruction; the
                // VM1 checks no interrupts at all after it.
                let skip = instr == Instr::Rtt && self.psw & psw::T != 0;
                (fault, skip)
            }
            // The instruction was aborted at the failing bus cycle.
            Err(BusError) => (Some(Fault::BusError), false),
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
    fn service_interrupts<B: Bus>(&mut self, bus: &mut B, mut fault: Option<Fault>) {
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
            self.gap(t::TRAP_INSTR);
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
                    self.gap(t::AFTER_TIMEOUT);
                    vector::BUS_ERROR
                } else {
                    self.gap(t::TRAP_INSTR);
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
                let _ = self.wr(bus, SEL1, 0o210, false, t::TRAP_PUSH_STROBE);
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
                self.gap(t::TRAP_INSTR);
                let _ = self.wr(bus, SEL1, 0o210, false, t::TRAP_PUSH_STROBE);
                fault = Some(Fault::BusError);
                continue;
            } else {
                break;
            };

            self.waiting = false;
            if self.enter_trap(bus, vector).is_err() {
                if is_bus_error {
                    self.halted = true; // double bus error
                    break;
                }
                fault = Some(Fault::BusError);
            }
        }
    }

    /// The trap sequence: push PSW and PC, read the new PC and PSW.
    fn enter_trap<B: Bus>(&mut self, bus: &mut B, vector: u16) -> Result<(), BusError> {
        let (old_pc, old_psw) = (self.r[PC], self.psw);
        let [g1, g2, g3, g4] = t::TRAP_GAPS;
        self.r[SP] = self.r[SP].wrapping_sub(2);
        self.wr(bus, self.r[SP], old_psw, false, t::TRAP_PUSH_STROBE)?;
        self.gap(g1);
        self.r[SP] = self.r[SP].wrapping_sub(2);
        self.wr(bus, self.r[SP], old_pc, false, t::TRAP_PUSH_STROBE)?;
        self.gap(g2);
        let pc = self.rd(bus, vector, false)?;
        self.gap(g3);
        let new_psw = self.rd(bus, vector.wrapping_add(2), false)?;
        self.gap(g4);
        self.r[PC] = pc;
        self.psw = new_psw & 0o377; // only the low byte is loaded; leaves halt mode
        self.overlap = true;
        Ok(())
    }

    // ----- bus cycles, on the schedule's clock -----

    fn gap(&mut self, clocks: u32) {
        self.t += clocks;
    }

    /// A read cycle with its strobe now (a word read ignores address bit 0).
    fn rd<B: Bus>(&mut self, bus: &mut B, addr: u16, byte: bool) -> Result<u16, BusError> {
        let wait = bus.cycle(self.t, addr, Cycle::Read);
        let r = if byte {
            bus.read_byte(addr).map(u16::from)
        } else {
            bus.read_word(addr & !1)
        };
        self.t += if r.is_ok() {
            wait + t::AFTER_REPLY
        } else {
            t::TIMEOUT
        };
        r
    }

    /// A write cycle whose strobe comes `strobe` clocks from now.
    fn wr<B: Bus>(
        &mut self,
        bus: &mut B,
        addr: u16,
        value: u16,
        byte: bool,
        strobe: u32,
    ) -> Result<(), BusError> {
        self.t += strobe + self.write_delay;
        let wait = bus.cycle(self.t, addr, Cycle::Write);
        let r = if byte {
            bus.write_byte(addr, value as u8)
        } else {
            bus.write_word(addr & !1, value)
        };
        self.t += if r.is_ok() {
            wait + t::AFTER_REPLY
        } else {
            // Timed from the CPU's own strobe, before the board's delay.
            t::TIMEOUT - self.write_delay
        };
        r
    }

    /// A read-modify-write (DATIO) cycle: read now, write `f`'s result.
    fn rmw<B: Bus>(
        &mut self,
        bus: &mut B,
        addr: u16,
        byte: bool,
        f: impl FnOnce(&mut Self, u16) -> u16,
    ) -> Result<(), BusError> {
        let wait = bus.cycle(self.t, addr, Cycle::Read);
        let d = if byte {
            bus.read_byte(addr).map(u16::from)
        } else {
            bus.read_word(addr & !1)
        };
        let d = match d {
            Ok(d) => d,
            Err(e) => {
                self.t += t::TIMEOUT;
                return Err(e);
            }
        };
        self.t += wait + t::RMW_TURN + self.write_delay;
        let v = f(self, d);
        let wait = bus.cycle(self.t, addr, Cycle::Write);
        let r = if byte {
            bus.write_byte(addr, v as u8)
        } else {
            bus.write_word(addr & !1, v)
        };
        self.t += if r.is_ok() {
            wait + t::AFTER_REPLY
        } else {
            t::TIMEOUT - self.write_delay
        };
        r
    }

    fn fetch<B: Bus>(&mut self, bus: &mut B) -> Result<u16, BusError> {
        let pc = self.r[PC];
        self.r[PC] = pc.wrapping_add(2);
        self.rd(bus, pc, false)
    }

    fn pop<B: Bus>(&mut self, bus: &mut B) -> Result<u16, BusError> {
        let sp = self.r[SP];
        self.r[SP] = sp.wrapping_add(2);
        self.rd(bus, sp, false)
    }

    /// Resolve an operand to a register or memory address, applying
    /// autoincrement/decrement and doing the bus cycles its mode needs
    /// (index words, pointers): the first now, each followed by
    /// [`t::CHAIN`]. Returns whether it made any.
    fn ea<B: Bus>(&mut self, bus: &mut B, o: Operand, byte: bool) -> Result<(Loc, bool), BusError> {
        let r = o.reg();
        // Byte autoincrement/decrement steps by 1, except on SP and PC.
        let step = if byte && r < SP { 1 } else { 2 };
        let chain = |cpu: &mut Self, v: u16| {
            cpu.gap(t::CHAIN);
            v
        };
        Ok(match o.mode() {
            0 => (Loc::Reg(r), false),
            1 => (Loc::Mem(self.r[r]), false),
            2 => {
                let a = self.r[r];
                self.r[r] = a.wrapping_add(step);
                (Loc::Mem(a), false)
            }
            3 => {
                let a = self.r[r];
                self.r[r] = a.wrapping_add(2);
                let p = self.rd(bus, a, false)?;
                (Loc::Mem(chain(self, p)), true)
            }
            4 => {
                self.r[r] = self.r[r].wrapping_sub(step);
                (Loc::Mem(self.r[r]), false)
            }
            5 => {
                self.r[r] = self.r[r].wrapping_sub(2);
                let a = self.r[r];
                let p = self.rd(bus, a, false)?;
                (Loc::Mem(chain(self, p)), true)
            }
            6 => {
                let x = self.fetch(bus)?;
                let x = chain(self, x);
                (Loc::Mem(self.r[r].wrapping_add(x)), true)
            }
            _ => {
                let x = self.fetch(bus)?;
                let x = chain(self, x);
                let a = self.r[r].wrapping_add(x);
                let p = self.rd(bus, a, false)?;
                (Loc::Mem(chain(self, p)), true)
            }
        })
    }

    fn reg_load(&self, r: usize, byte: bool) -> u16 {
        if byte { self.r[r] & 0xff } else { self.r[r] }
    }

    fn reg_store(&mut self, r: usize, byte: bool, v: u16) {
        self.r[r] = if byte {
            (self.r[r] & 0xff00) | (v & 0xff)
        } else {
            v
        };
    }

    /// Read an operand: a register, or a read cycle now.
    fn load<B: Bus>(&mut self, bus: &mut B, loc: Loc, byte: bool) -> Result<u16, BusError> {
        match loc {
            Loc::Reg(r) => Ok(self.reg_load(r, byte)),
            Loc::Mem(a) => self.rd(bus, a, byte),
        }
    }

    /// Modify an operand in place: a register, or a read-modify-write cycle.
    fn modify<B: Bus>(
        &mut self,
        bus: &mut B,
        loc: Loc,
        byte: bool,
        f: impl FnOnce(&mut Self, u16) -> u16,
    ) -> Result<(), BusError> {
        match loc {
            Loc::Reg(r) => {
                let d = self.reg_load(r, byte);
                let v = f(self, d);
                self.reg_store(r, byte, v);
                Ok(())
            }
            Loc::Mem(a) => self.rmw(bus, a, byte, f),
        }
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

    /// The address JMP and JSR jump to, with their timing up to the jump
    /// (JMP) or the push (JSR).
    fn jump_target<B: Bus>(
        &mut self,
        bus: &mut B,
        dst: Operand,
        jsr: bool,
    ) -> Result<u16, BusError> {
        let m = dst.mode();
        if matches!(m, 1 | 2 | 4) {
            self.gap(if jsr {
                t::jsr_direct(m)
            } else {
                t::jmp_direct(m)
            });
        } else {
            self.gap(t::first(m));
        }
        let (loc, read) = self.ea(bus, dst, false)?;
        if read {
            // ea left CHAIN after the last read.
            self.gap(
                if jsr {
                    t::JSR_AFTER_READ
                } else {
                    t::JMP_AFTER_READ
                } - t::CHAIN,
            );
        }
        match loc {
            Loc::Mem(a) => Ok(a),
            Loc::Reg(_) => unreachable!("caller checks mode != 0"),
        }
    }

    fn execute<B: Bus>(&mut self, bus: &mut B, instr: Instr) -> Exec {
        match instr {
            Instr::Halt | Instr::Start | Instr::Step => Ok(Some(Fault::Halt)),
            Instr::Reserved => Ok(Some(Fault::Reserved)),
            Instr::Bpt => Ok(Some(Fault::Bpt)),
            Instr::Iot => Ok(Some(Fault::Iot)),
            Instr::Emt(_) => Ok(Some(Fault::Emt)),
            Instr::Trap(_) => Ok(Some(Fault::Trap)),
            Instr::Wait => {
                self.gap(t::TAIL);
                self.waiting = true;
                Ok(None)
            }
            Instr::Reset => {
                bus.reset_devices();
                self.gap(t::RESET);
                Ok(None)
            }
            Instr::Rti | Instr::Rtt => {
                let [g1, g2, g3] = t::RTI;
                self.gap(g1);
                let pc = self.pop(bus)?;
                self.gap(g2);
                let new_psw = self.pop(bus)?;
                self.gap(g3);
                self.r[PC] = pc;
                // Outside the halt-mode ROM area only the low byte is loaded
                // and the halt-mode bit is kept.
                self.psw = if pc < 0o160000 {
                    (new_psw & 0o377) | (self.psw & psw::HALT)
                } else {
                    new_psw & 0o777
                };
                Ok(None)
            }
            Instr::Jmp(dst) => {
                if dst.mode() == 0 {
                    self.gap(t::TRAP_INSTR);
                    return Ok(Some(Fault::BusError));
                }
                self.r[PC] = self.jump_target(bus, dst, false)?;
                Ok(None)
            }
            Instr::Jsr(reg, dst) => {
                if dst.mode() == 0 {
                    self.gap(t::TRAP_INSTR);
                    return Ok(Some(Fault::BusError));
                }
                let target = self.jump_target(bus, dst, true)?;
                let reg = reg as usize;
                self.r[SP] = self.r[SP].wrapping_sub(2);
                let (sp, linkage) = (self.r[SP], self.r[reg]);
                let strobe = self.t + t::JSR_PUSH_STROBE;
                self.wr(bus, sp, linkage, false, t::JSR_PUSH_STROBE)?;
                self.t = (self.t + 1).max(strobe + t::JSR_NEXT_FETCH);
                self.r[reg] = self.r[PC];
                self.r[PC] = target;
                Ok(None)
            }
            Instr::Rts(reg) => {
                let reg = reg as usize;
                self.gap(t::RTS);
                let target = self.r[reg];
                let saved = self.pop(bus)?;
                self.gap(t::TAIL);
                self.r[PC] = target;
                self.r[reg] = saved;
                Ok(None)
            }
            Instr::Mark(n) => {
                self.gap(t::MARK);
                self.r[SP] = self.r[PC].wrapping_add(2 * n as u16);
                self.r[PC] = self.r[5];
                self.r[5] = self.pop(bus)?;
                self.gap(t::TAIL);
                Ok(None)
            }
            Instr::Sob(reg, n) => {
                self.gap(t::SOB);
                let reg = reg as usize;
                self.r[reg] = self.r[reg].wrapping_sub(1);
                if self.r[reg] != 0 {
                    self.r[PC] = self.r[PC].wrapping_sub(2 * n as u16);
                }
                Ok(None)
            }
            Instr::Branch(c, offset) => {
                self.gap(t::BRANCH);
                if self.cond(c) {
                    self.r[PC] = self.r[PC].wrapping_add((offset as i16 as u16).wrapping_mul(2));
                }
                Ok(None)
            }
            Instr::ClearFlags(mask) => {
                self.gap(t::TAIL);
                self.psw &= !(mask as u16);
                Ok(None)
            }
            Instr::SetFlags(mask) => {
                self.gap(t::TAIL);
                self.psw |= mask as u16;
                Ok(None)
            }
            Instr::Swab(dst) => self.modify_operand(bus, dst, false, 0, |cpu, d| {
                let v = d.swap_bytes();
                cpu.set_nzvc(v & 0x80 != 0, v & 0xff == 0, false, false);
                v
            }),
            Instr::Sxt(dst) => self.modify_operand(bus, dst, false, 0, |cpu, _| {
                let n = cpu.flag(psw::N);
                cpu.set_nzvc(n, !n, false, cpu.flag(psw::C));
                if n { 0xffff } else { 0 }
            }),
            Instr::Mtps(src) => {
                let m = src.mode();
                let v = if m == 0 {
                    // From a register: the next opcode is fetched and thrown
                    // away, and fetched again a fixed time after.
                    self.gap(t::TAIL);
                    let fetch = self.t;
                    self.rd(bus, self.r[PC], false)?;
                    let refetch = fetch + t::MTPS_REFETCH;
                    self.gap(refetch.saturating_sub(self.t).max(1));
                    self.reg_load(src.reg(), true)
                } else {
                    self.gap(t::first(m));
                    let (loc, _) = self.ea(bus, src, true)?;
                    let v = self.load(bus, loc, true)?;
                    self.gap(t::TAIL_MTPS);
                    v
                };
                // The T bit can't be changed by MTPS.
                self.psw = (self.psw & !0o357) | (v & 0o357);
                Ok(None)
            }
            Instr::Mfps(dst) => {
                let v = self.psw & 0xff;
                if dst.mode() == 0 {
                    // MFPS to a register sign-extends into the whole register.
                    self.gap(t::TAIL);
                    self.r[dst.reg()] = v as u8 as i8 as u16;
                    self.set_nzvc(v & 0x80 != 0, v == 0, false, self.flag(psw::C));
                    return Ok(None);
                }
                self.modify_operand(bus, dst, true, 0, |cpu, _| {
                    cpu.set_nzvc(v & 0x80 != 0, v == 0, false, cpu.flag(psw::C));
                    v
                })
            }
            Instr::Xor(reg, dst) => {
                let s = self.r[reg as usize];
                self.modify_operand(bus, dst, false, t::XOR_EXTRA, |cpu, d| {
                    let v = d ^ s;
                    cpu.set_nzvc(v & 0x8000 != 0, v == 0, false, cpu.flag(psw::C));
                    v
                })
            }
            Instr::Single(op, byte, dst) => self.single(bus, op, byte, dst),
            Instr::Double(op, byte, src, dst) => self.double(bus, op, byte, src, dst),
        }
    }

    /// A single-operand instruction that changes its operand: a register
    /// operation, or the operand's address cycles and a read-modify-write
    /// cycle (even CLR and SXT read first, as the chip does).
    fn modify_operand<B: Bus>(
        &mut self,
        bus: &mut B,
        dst: Operand,
        byte: bool,
        extra: u32,
        f: impl FnOnce(&mut Self, u16) -> u16,
    ) -> Exec {
        let m = dst.mode();
        if m == 0 {
            self.gap(t::TAIL);
        } else {
            self.gap(t::first(m) + extra);
        }
        let (loc, _) = self.ea(bus, dst, byte)?;
        self.modify(bus, loc, byte, f)?;
        if m != 0 {
            self.gap(t::TAIL_RMW);
        }
        Ok(None)
    }

    fn single<B: Bus>(&mut self, bus: &mut B, op: SingleOp, byte: bool, dst: Operand) -> Exec {
        let (sign, mask) = if byte {
            (0x80u16, 0xffu16)
        } else {
            (0x8000, 0xffff)
        };
        if op == SingleOp::Tst {
            let m = dst.mode();
            if m == 0 {
                self.gap(t::TAIL);
            } else {
                self.gap(t::first(m));
            }
            let (loc, _) = self.ea(bus, dst, byte)?;
            let d = self.load(bus, loc, byte)?;
            if m != 0 {
                self.gap(t::TAIL);
            }
            self.set_nzvc(d & sign != 0, d == 0, false, false);
            return Ok(None);
        }
        self.modify_operand(bus, dst, byte, 0, |cpu, d| {
            let c = cpu.flag(psw::C);
            let (r, v, c) = match op {
                SingleOp::Clr => (0, false, false),
                SingleOp::Tst => unreachable!("handled above"),
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
            cpu.set_nzvc(r & sign != 0, r == 0, v, c);
            r
        })
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
        let (sm, dm) = (src.mode(), dst.mode());

        // The source is fully read before the destination address is formed.
        let s = if sm != 0 {
            self.gap(t::first(sm));
            let (sloc, _) = self.ea(bus, src, byte)?;
            let s = self.load(bus, sloc, byte)?;
            self.gap(if dm == 0 { t::TAIL } else { t::after_src(dm) });
            s
        } else {
            let s = self.reg_load(src.reg(), byte);
            self.gap(if dm == 0 {
                t::TAIL
            } else {
                t::first(dm) + t::AFTER_REG_SRC
            });
            s
        };
        let (dloc, read) = self.ea(bus, dst, byte)?;
        let c = self.flag(psw::C);

        match op {
            DoubleOp::Mov => {
                match (dloc, byte) {
                    // MOVB to a register sign-extends into the whole register.
                    (Loc::Reg(r), true) => self.r[r] = s as u8 as i8 as u16,
                    (Loc::Reg(r), false) => self.r[r] = s,
                    // A plain write cycle, byte or word (MOVB doesn't read its
                    // destination: checked on the chip).
                    (Loc::Mem(a), _) => {
                        let strobe = if read {
                            t::WRITE_AFTER_READ
                        } else {
                            t::write_strobe(dm)
                        };
                        self.wr(bus, a, s, byte, strobe)?;
                        self.gap(t::TAIL);
                    }
                }
                self.set_nzvc(s & sign != 0, s & mask == 0, false, c);
                Ok(None)
            }
            DoubleOp::Cmp | DoubleOp::Bit => {
                let d = self.load(bus, dloc, byte)?;
                if dm != 0 {
                    self.gap(t::TAIL);
                }
                if op == DoubleOp::Cmp {
                    let r = s.wrapping_sub(d) & mask;
                    let v = (s ^ d) & (s ^ r) & sign != 0;
                    self.set_nzvc(r & sign != 0, r == 0, v, s < d);
                } else {
                    let r = s & d;
                    self.set_nzvc(r & sign != 0, r == 0, false, c);
                }
                Ok(None)
            }
            DoubleOp::Bic | DoubleOp::Bis | DoubleOp::Add | DoubleOp::Sub => {
                self.modify(bus, dloc, byte, |cpu, d| {
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
                    cpu.set_nzvc(r & sign != 0, r & mask == 0, v, c);
                    r
                })?;
                if dm != 0 {
                    self.gap(t::TAIL_RMW);
                }
                Ok(None)
            }
        }
    }
}

/// An instruction that ends with the plain register-operation tail: a
/// register destination, or no operand at all (NOP and the flag
/// instructions). After any other the VM1 overlaps the next opcode fetch
/// with the end of the instruction.
fn ends_on_register(instr: Instr) -> bool {
    match instr {
        Instr::ClearFlags(_) | Instr::SetFlags(_) => true,
        Instr::Single(_, _, d)
        | Instr::Swab(d)
        | Instr::Sxt(d)
        | Instr::Mfps(d)
        | Instr::Xor(_, d)
        | Instr::Double(_, _, _, d) => d.mode() == 0,
        _ => false,
    }
}

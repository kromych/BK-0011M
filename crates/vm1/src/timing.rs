//! Instruction timings in CPU clock cycles.
//!
//! These figures are the measured values used by the bkbtl emulator
//! (https://github.com/nzeemin/bkbtl), so that cycle counts can be compared
//! against it directly. Tables are indexed by addressing mode 0–7.

/// Base cost of every operate instruction (register to register).
pub const REGREG: u32 = 12;
pub const NOP: u32 = 12;
pub const BRANCH: u32 = 16;
pub const SOB: u32 = 20;
pub const RTS: u32 = 32;
pub const MARK: u32 = 36;
pub const RTI: u32 = 40;
/// EMT, TRAP, IOT, BPT, and JMP/JSR with an illegal register operand.
/// Includes the trap sequence.
pub const EMT: u32 = 68;
/// Reserved opcodes, HALT/START/STEP, and any instruction aborted by a bus
/// error. Includes the trap sequence.
pub const ILLEGAL: u32 = 144;
pub const WAIT: u32 = 1140;
pub const RESET: u32 = 1140;

/// Extra cost of a hardware interrupt or trap taken between instructions.
/// bkbtl charges nothing here; the real figure has not been measured.
pub const INTERRUPT_ENTRY: u32 = 0;

/// Clocks consumed per `Cpu::step` while the CPU sits in `WAIT`. Sets the
/// granularity of interrupt latency during `WAIT`.
pub const WAIT_POLL: u32 = 4;

/// Source operand.
pub const SRC: [u32; 8] = [0, 12, 12, 24, 16, 20, 20, 28];
/// Destination operand that is read and written, or written after a
/// register source.
pub const DST: [u32; 8] = [0, 20, 20, 32, 24, 36, 32, 40];
/// Destination operand written after a memory source.
pub const DST_AFTER_MEM_SRC: [u32; 8] = [4, 16, 20, 24, 16, 24, 24, 36];
/// Destination that is only read (CMP/BIT with a register source), XOR, JMP.
pub const DST_READ: [u32; 8] = [0, 20, 20, 32, 20, 28, 32, 36];
/// JSR.
pub const JSR: [u32; 8] = [0, 32, 32, 40, 32, 40, 40, 48];

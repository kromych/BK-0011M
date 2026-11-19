//! K1801VM1 CPU core: the PDP-11-compatible processor of the Elektronika BK-0010/0011M.
//!
//! The VM1 implements the PDP-11 base instruction set without EIS/FIS, plus
//! `XOR`, `SOB`, `SXT`, `MARK`, `RTT`, `MTPS`, `MFPS` and the halt-mode opcodes
//! `START`/`STEP`. See `docs/hardware-reference.md` in the repository for the
//! machine-level details.
//!
//! The crate has no I/O of its own. A machine implements [`Bus`] and calls
//! [`Cpu::step`], which executes one instruction (plus any interrupts that are
//! due) and returns the number of CPU clock cycles it took.
//!
//! All numbers in comments and docs are octal unless marked otherwise.

pub mod asm;
mod bus;
mod cpu;
pub mod decode;
mod disasm;
pub mod koi8;
pub mod timing;

pub use bus::{Bus, BusError, Cycle};
pub use cpu::{Cpu, CpuState, psw, vector};
pub use disasm::{Disassembly, Ref, disassemble, disassemble_with};

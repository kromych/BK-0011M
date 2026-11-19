//! The VM1's bus-cycle schedule: the CPU clocks between bus cycles, where in
//! a cycle its data strobe comes, and how long it runs after the reply.
//! Measured on the chip-level VM1 model of the MiST core
//! (tools/vm1-timing), for every addressing mode of every instruction class;
//! the executor in `cpu.rs` follows it cycle by cycle. Replaying the
//! measurement programs (`examples/replay.rs`) reproduces every bus cycle of
//! the chip's (strobe, reply, end) with replies of 1 to 5 clocks; an instant
//! reply makes some gaps a clock longer on the chip, which isn't modelled
//! (nothing on the BK replies at once).

/// A cycle runs this long after its reply.
pub const AFTER_REPLY: u32 = 4;
/// In a read-modify-write (DATIO) cycle: from the read's reply to the write.
pub const RMW_TURN: u32 = 5;
/// A cycle nobody answers ends this long after its strobe (the bus timeout);
/// the bus-error trap follows after [`AFTER_TIMEOUT`].
pub const TIMEOUT: u32 = 62;
pub const AFTER_TIMEOUT: u32 = 7;
/// Between an operand's index or pointer reads and its next cycle.
pub const CHAIN: u32 = 2;

/// From the opcode fetch to an operand's first cycle.
pub fn first(mode: u8) -> u32 {
    if mode == 4 || mode == 5 { 7 } else { 6 }
}
/// After a register source, to the destination's first cycle.
pub const AFTER_REG_SRC: u32 = 6;
/// After a memory source operand, to the destination's first cycle.
pub fn after_src(dst_mode: u8) -> u32 {
    if dst_mode == 4 || dst_mode == 5 { 5 } else { 4 }
}
/// Write strobe offset for a destination written without an address read
/// first: (Rn), (Rn)+, -(Rn). After an index or pointer read it is
/// [`WRITE_AFTER_READ`].
pub fn write_strobe(dst_mode: u8) -> u32 {
    match dst_mode {
        2 => 6,
        4 => 5,
        _ => 3,
    }
}
pub const WRITE_AFTER_READ: u32 = 2;
/// The tail before the next fetch: after a register operation or a read, a
/// write, a read-modify-write, MTPS.
pub const TAIL: u32 = 4;
pub const TAIL_RMW: u32 = 1;
pub const TAIL_MTPS: u32 = 10;
/// MTPS from a register fetches the next opcode, throws it away, and
/// fetches it again this long after the thrown-away fetch's strobe.
pub const MTPS_REFETCH: u32 = 12;
/// The opcode fetch after an instruction that doesn't end with the plain
/// register-operation tail (one with a memory destination, a jump, branch,
/// return or trap) overlaps the end of that instruction: the first gap
/// after it is this much shorter, whatever the instruction and the reply
/// delay. (The gaps here are those after a register operation.)
pub const OVERLAP: u32 = 1;
/// After MOVB or MFPS into a register, which sign-extend, the next
/// instruction's first gap starts no sooner than this long after its fetch's
/// strobe: as if the fetch's reply took at least 3 clocks.
pub const SIGN_EXTEND: u32 = 7;
/// XOR's first gap is longer than other single-operand instructions'.
pub const XOR_EXTRA: u32 = 3;

pub const BRANCH: u32 = 9;
pub const SOB: u32 = 14;
/// JMP to (Rn), (Rn)+, -(Rn): from the fetch to the target's fetch.
pub fn jmp_direct(mode: u8) -> u32 {
    match mode {
        2 => 18,
        4 => 16,
        _ => 15,
    }
}
/// JMP through an index or pointer: after the last read.
pub const JMP_AFTER_READ: u32 = 10;
/// JSR to (Rn), (Rn)+, -(Rn): from the fetch to the push.
pub fn jsr_direct(mode: u8) -> u32 {
    match mode {
        2 => 16,
        4 => 15,
        _ => 13,
    }
}
/// JSR through an index or pointer: after the last read, to the push.
pub const JSR_AFTER_READ: u32 = 8;
pub const JSR_PUSH_STROBE: u32 = 2;
/// After the push the next fetch comes at least 1 clock after the push
/// ends and at least this long after the CPU raised the push's strobe
/// (before any write delay of the board).
pub const JSR_NEXT_FETCH: u32 = 9;
/// RTS: to the pop.
pub const RTS: u32 = 12;
pub const MARK: u32 = 13;
/// RTI and RTT: to the first pop, between the pops, after the second.
pub const RTI: [u32; 3] = [4, 4, 9];
/// EMT, TRAP, IOT, BPT: from the fetch to the trap sequence.
pub const TRAP_INSTR: u32 = 7;
/// The trap sequence: push PSW (strobe 1), gap, push PC (strobe 1), gap,
/// read the vector's PC, gap, read its PSW, gap to the handler's fetch.
pub const TRAP_PUSH_STROBE: u32 = 1;
pub const TRAP_GAPS: [u32; 4] = [1, 5, 5, 9];

/// Not measured on the chip: kept from bkbtl's tables.
pub const WAIT: u32 = 1140;
pub const RESET: u32 = 1140;

/// Clocks consumed per `Cpu::step` while the CPU sits in `WAIT`. Sets the
/// granularity of interrupt latency during `WAIT`.
pub const WAIT_POLL: u32 = 4;

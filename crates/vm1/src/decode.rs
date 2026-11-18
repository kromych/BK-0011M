//! Instruction decoding, shared by the CPU, the disassembler and the assembler
//! so that all three agree on which opcodes exist on the VM1.

/// A 6-bit operand specifier: addressing mode in bits 5–3, register in bits 2–0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operand(pub u8);

impl Operand {
    pub fn new(mode: u8, reg: u8) -> Self {
        Operand(((mode & 7) << 3) | (reg & 7))
    }
    pub fn mode(self) -> u8 {
        self.0 >> 3
    }
    pub fn reg(self) -> usize {
        (self.0 & 7) as usize
    }
    /// Extra instruction words this operand takes: an index for modes 6/7,
    /// or an immediate/absolute value for modes 2/3 on PC.
    pub fn extra_words(self) -> u16 {
        match (self.mode(), self.reg()) {
            (6 | 7, _) | (2 | 3, 7) => 1,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingleOp {
    Clr,
    Com,
    Inc,
    Dec,
    Neg,
    Adc,
    Sbc,
    Tst,
    Ror,
    Rol,
    Asr,
    Asl,
}

impl SingleOp {
    pub const ALL: [SingleOp; 12] = [
        SingleOp::Clr,
        SingleOp::Com,
        SingleOp::Inc,
        SingleOp::Dec,
        SingleOp::Neg,
        SingleOp::Adc,
        SingleOp::Sbc,
        SingleOp::Tst,
        SingleOp::Ror,
        SingleOp::Rol,
        SingleOp::Asr,
        SingleOp::Asl,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SingleOp::Clr => "CLR",
            SingleOp::Com => "COM",
            SingleOp::Inc => "INC",
            SingleOp::Dec => "DEC",
            SingleOp::Neg => "NEG",
            SingleOp::Adc => "ADC",
            SingleOp::Sbc => "SBC",
            SingleOp::Tst => "TST",
            SingleOp::Ror => "ROR",
            SingleOp::Rol => "ROL",
            SingleOp::Asr => "ASR",
            SingleOp::Asl => "ASL",
        }
    }

    /// Word opcode with a zero operand field (005000–006300).
    pub fn opcode(self) -> u16 {
        0o005000 + 0o100 * self as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoubleOp {
    Mov,
    Cmp,
    Bit,
    Bic,
    Bis,
    Add,
    Sub,
}

impl DoubleOp {
    pub const ALL: [DoubleOp; 7] = [
        DoubleOp::Mov,
        DoubleOp::Cmp,
        DoubleOp::Bit,
        DoubleOp::Bic,
        DoubleOp::Bis,
        DoubleOp::Add,
        DoubleOp::Sub,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DoubleOp::Mov => "MOV",
            DoubleOp::Cmp => "CMP",
            DoubleOp::Bit => "BIT",
            DoubleOp::Bic => "BIC",
            DoubleOp::Bis => "BIS",
            DoubleOp::Add => "ADD",
            DoubleOp::Sub => "SUB",
        }
    }

    /// ADD and SUB have no byte form (SUB occupies the byte slot of ADD).
    pub fn has_byte_form(self) -> bool {
        !matches!(self, DoubleOp::Add | DoubleOp::Sub)
    }

    /// Word opcode with zero operand fields.
    pub fn opcode(self) -> u16 {
        match self {
            DoubleOp::Mov => 0o010000,
            DoubleOp::Cmp => 0o020000,
            DoubleOp::Bit => 0o030000,
            DoubleOp::Bic => 0o040000,
            DoubleOp::Bis => 0o050000,
            DoubleOp::Add => 0o060000,
            DoubleOp::Sub => 0o160000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Br,
    Bne,
    Beq,
    Bge,
    Blt,
    Bgt,
    Ble,
    Bpl,
    Bmi,
    Bhi,
    Blos,
    Bvc,
    Bvs,
    Bcc,
    Bcs,
}

impl Cond {
    pub const ALL: [Cond; 15] = [
        Cond::Br,
        Cond::Bne,
        Cond::Beq,
        Cond::Bge,
        Cond::Blt,
        Cond::Bgt,
        Cond::Ble,
        Cond::Bpl,
        Cond::Bmi,
        Cond::Bhi,
        Cond::Blos,
        Cond::Bvc,
        Cond::Bvs,
        Cond::Bcc,
        Cond::Bcs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cond::Br => "BR",
            Cond::Bne => "BNE",
            Cond::Beq => "BEQ",
            Cond::Bge => "BGE",
            Cond::Blt => "BLT",
            Cond::Bgt => "BGT",
            Cond::Ble => "BLE",
            Cond::Bpl => "BPL",
            Cond::Bmi => "BMI",
            Cond::Bhi => "BHI",
            Cond::Blos => "BLOS",
            Cond::Bvc => "BVC",
            Cond::Bvs => "BVS",
            Cond::Bcc => "BCC",
            Cond::Bcs => "BCS",
        }
    }

    /// Opcode with a zero offset.
    pub fn opcode(self) -> u16 {
        match self {
            Cond::Br => 0o000400,
            Cond::Bne => 0o001000,
            Cond::Beq => 0o001400,
            Cond::Bge => 0o002000,
            Cond::Blt => 0o002400,
            Cond::Bgt => 0o003000,
            Cond::Ble => 0o003400,
            Cond::Bpl => 0o100000,
            Cond::Bmi => 0o100400,
            Cond::Bhi => 0o101000,
            Cond::Blos => 0o101400,
            Cond::Bvc => 0o102000,
            Cond::Bvs => 0o102400,
            Cond::Bcc => 0o103000,
            Cond::Bcs => 0o103400,
        }
    }
}

/// Condition-code bits as used in the low 4 bits of the PSW and of the
/// CLx/SEx opcodes.
pub const FLAG_C: u8 = 0o1;
pub const FLAG_V: u8 = 0o2;
pub const FLAG_Z: u8 = 0o4;
pub const FLAG_N: u8 = 0o10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Halt,
    Wait,
    Rti,
    Bpt,
    Iot,
    Reset,
    Rtt,
    /// 000010–000013: halt-mode START. Low 2 bits are ignored.
    Start,
    /// 000014–000017: halt-mode STEP. Low 2 bits are ignored.
    Step,
    Jmp(Operand),
    Rts(u8),
    /// 000240–000257: clear the flags in the mask. 000240 is NOP.
    ClearFlags(u8),
    /// 000260–000277: set the flags in the mask. 000260 does nothing.
    SetFlags(u8),
    Swab(Operand),
    Branch(Cond, i8),
    Jsr(u8, Operand),
    /// Single-operand op; `true` = byte form.
    Single(SingleOp, bool, Operand),
    Mark(u8),
    Sxt(Operand),
    Mtps(Operand),
    Mfps(Operand),
    /// Double-operand op; `true` = byte form.
    Double(DoubleOp, bool, Operand, Operand),
    Xor(u8, Operand),
    Sob(u8, u8),
    Emt(u8),
    Trap(u8),
    /// Not implemented on the VM1 (EIS, FIS, MFPx/MTPx, SPL, MFPT, ...):
    /// traps to vector 10.
    Reserved,
}

pub fn decode(op: u16) -> Instr {
    let dst = Operand((op & 0o77) as u8);
    let src = Operand(((op >> 6) & 0o77) as u8);
    let reg = ((op >> 6) & 7) as u8;
    let byte = op & 0o100000 != 0;

    match (op >> 12) & 7 {
        1 => return Instr::Double(DoubleOp::Mov, byte, src, dst),
        2 => return Instr::Double(DoubleOp::Cmp, byte, src, dst),
        3 => return Instr::Double(DoubleOp::Bit, byte, src, dst),
        4 => return Instr::Double(DoubleOp::Bic, byte, src, dst),
        5 => return Instr::Double(DoubleOp::Bis, byte, src, dst),
        6 if byte => return Instr::Double(DoubleOp::Sub, false, src, dst),
        6 => return Instr::Double(DoubleOp::Add, false, src, dst),
        7 if byte => return Instr::Reserved, // 17xxxx: FPP
        7 => {
            return match (op >> 9) & 7 {
                4 => Instr::Xor(reg, dst),
                7 => Instr::Sob(reg, (op & 0o77) as u8),
                _ => Instr::Reserved, // EIS (MUL/DIV/ASH/ASHC), FIS, 076xxx
            };
        }
        _ => {}
    }

    // Opcodes 000000–007777 and 100000–107777.
    let is_branch = if byte {
        op < 0o104000
    } else {
        (0o000400..=0o003777).contains(&op)
    };
    if is_branch {
        const WORD: [Cond; 8] = [
            Cond::Br, // index 0 is 000000-000377, which is not a branch
            Cond::Br,
            Cond::Bne,
            Cond::Beq,
            Cond::Bge,
            Cond::Blt,
            Cond::Bgt,
            Cond::Ble,
        ];
        const BYTE: [Cond; 8] = [
            Cond::Bpl,
            Cond::Bmi,
            Cond::Bhi,
            Cond::Blos,
            Cond::Bvc,
            Cond::Bvs,
            Cond::Bcc,
            Cond::Bcs,
        ];
        let idx = ((op >> 8) & 7) as usize;
        let cond = if byte { BYTE[idx] } else { WORD[idx] };
        return Instr::Branch(cond, op as u8 as i8);
    }

    if byte {
        return match op {
            0o104000..=0o104377 => Instr::Emt(op as u8),
            0o104400..=0o104777 => Instr::Trap(op as u8),
            0o105000..=0o106377 => {
                Instr::Single(SingleOp::ALL[((op >> 6) & 0o77) as usize - 0o50], true, dst)
            }
            0o106400..=0o106477 => Instr::Mtps(dst),
            0o106700..=0o106777 => Instr::Mfps(dst),
            _ => Instr::Reserved, // MFPD, MTPD, 107xxx
        };
    }

    match op {
        0 => Instr::Halt,
        1 => Instr::Wait,
        2 => Instr::Rti,
        3 => Instr::Bpt,
        4 => Instr::Iot,
        5 => Instr::Reset,
        6 => Instr::Rtt,
        0o10..=0o13 => Instr::Start,
        0o14..=0o17 => Instr::Step,
        0o100..=0o177 => Instr::Jmp(dst),
        0o200..=0o207 => Instr::Rts((op & 7) as u8),
        0o240..=0o257 => Instr::ClearFlags((op & 0o17) as u8),
        0o260..=0o277 => Instr::SetFlags((op & 0o17) as u8),
        0o300..=0o377 => Instr::Swab(dst),
        0o004000..=0o004777 => Instr::Jsr(reg, dst),
        0o005000..=0o006377 => Instr::Single(
            SingleOp::ALL[((op >> 6) & 0o77) as usize - 0o50],
            false,
            dst,
        ),
        0o006400..=0o006477 => Instr::Mark((op & 0o77) as u8),
        0o006700..=0o006777 => Instr::Sxt(dst),
        _ => Instr::Reserved, // 7, 20-77, 210-237, MFPI, MTPI, 007xxx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_tables_agree_with_decode() {
        let d = Operand(0o27);
        for op in SingleOp::ALL {
            assert_eq!(decode(op.opcode() | 0o27), Instr::Single(op, false, d));
            assert_eq!(decode(op.opcode() | 0o100027), Instr::Single(op, true, d));
        }
        let s = Operand(0o12);
        for op in DoubleOp::ALL {
            assert_eq!(decode(op.opcode() | 0o1227), Instr::Double(op, false, s, d));
            if op.has_byte_form() {
                assert_eq!(
                    decode(op.opcode() | 0o101227),
                    Instr::Double(op, true, s, d)
                );
            }
        }
        for c in Cond::ALL {
            assert_eq!(decode(c.opcode() | 0o377), Instr::Branch(c, -1));
        }
    }

    #[test]
    fn reserved_opcodes() {
        for op in [
            0o7, 0o20, 0o77, 0o210, 0o237, 0o006500, 0o006600, 0o007000, 0o070000, 0o071234,
            0o073777, 0o075000, 0o076000, 0o106500, 0o106600, 0o107000, 0o170000, 0o177777,
        ] {
            assert_eq!(decode(op), Instr::Reserved, "{op:06o}");
        }
    }
}

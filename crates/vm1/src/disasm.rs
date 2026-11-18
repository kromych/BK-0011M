use crate::decode::{FLAG_C, FLAG_N, FLAG_V, FLAG_Z, Instr, Operand, decode};

/// One disassembled instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disassembly {
    /// MACRO-11 style text, e.g. `MOV    #001000, R0`. Numbers are octal.
    /// PC-relative operands and branch targets are shown as absolute addresses.
    pub text: String,
    /// Instruction length in words (1–3).
    pub words: u16,
}

const REG_NAMES: [&str; 8] = ["R0", "R1", "R2", "R3", "R4", "R5", "SP", "PC"];

/// How an instruction uses a value, for symbol lookup in
/// [`disassemble_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ref {
    /// Branch or SOB target, or the address a PC-relative operand refers to.
    Target,
    /// Absolute address: `@#addr`.
    Absolute,
    /// Immediate value: `#n`.
    Immediate,
    /// Index of an indexed operand: `X(Rn)`.
    Index,
}

/// Disassemble the instruction at `addr`. `read` returns the word at an
/// (even) address; it is called for the opcode and any extra words.
pub fn disassemble(addr: u16, read: impl FnMut(u16) -> u16) -> Disassembly {
    disassemble_with(addr, read, |_, _| None)
}

/// Like [`disassemble`], but `symbol` may name any address or value the
/// instruction refers to; `None` prints it as an octal number.
pub fn disassemble_with(
    addr: u16,
    mut read: impl FnMut(u16) -> u16,
    mut symbol: impl FnMut(u16, Ref) -> Option<String>,
) -> Disassembly {
    let mut pc = addr.wrapping_add(2);
    let opcode = read(addr);
    let instr = decode(opcode);
    let mut name = |v: u16, kind: Ref| symbol(v, kind).unwrap_or_else(|| format!("{v:06o}"));
    let mut operand = |o: Operand, pc: &mut u16, name: &mut dyn FnMut(u16, Ref) -> String| {
        format_operand(o, pc, &mut read, name)
    };

    let (mnemonic, operands): (String, String) = match instr {
        Instr::Halt => ("HALT".into(), String::new()),
        Instr::Wait => ("WAIT".into(), String::new()),
        Instr::Rti => ("RTI".into(), String::new()),
        Instr::Bpt => ("BPT".into(), String::new()),
        Instr::Iot => ("IOT".into(), String::new()),
        Instr::Reset => ("RESET".into(), String::new()),
        Instr::Rtt => ("RTT".into(), String::new()),
        // START/STEP ignore their low 2 bits and 000260 sets no flags; only
        // the canonical encodings get a mnemonic, so text always assembles back.
        Instr::Start if opcode == 0o10 => ("START".into(), String::new()),
        Instr::Step if opcode == 0o14 => ("STEP".into(), String::new()),
        Instr::SetFlags(0) => (".WORD".into(), format!("{opcode:06o}")),
        Instr::Start | Instr::Step => (".WORD".into(), format!("{opcode:06o}")),
        Instr::ClearFlags(mask) => (flag_mnemonic(mask, "CL", "CCC"), String::new()),
        Instr::SetFlags(mask) => (flag_mnemonic(mask, "SE", "SCC"), String::new()),
        Instr::Jmp(d) => ("JMP".into(), operand(d, &mut pc, &mut name)),
        Instr::Rts(r) => ("RTS".into(), REG_NAMES[r as usize].into()),
        Instr::Swab(d) => ("SWAB".into(), operand(d, &mut pc, &mut name)),
        Instr::Sxt(d) => ("SXT".into(), operand(d, &mut pc, &mut name)),
        Instr::Mtps(d) => ("MTPS".into(), operand(d, &mut pc, &mut name)),
        Instr::Mfps(d) => ("MFPS".into(), operand(d, &mut pc, &mut name)),
        Instr::Mark(n) => ("MARK".into(), format!("{n:02o}")),
        Instr::Emt(n) => ("EMT".into(), format!("{n:03o}")),
        Instr::Trap(n) => ("TRAP".into(), format!("{n:03o}")),
        Instr::Branch(c, off) => {
            let target = pc.wrapping_add((off as i16 as u16).wrapping_mul(2));
            (c.name().into(), name(target, Ref::Target))
        }
        Instr::Sob(r, n) => {
            let target = pc.wrapping_sub(2 * n as u16);
            let target = name(target, Ref::Target);
            ("SOB".into(), format!("{}, {target}", REG_NAMES[r as usize]))
        }
        Instr::Jsr(r, d) => (
            "JSR".into(),
            format!(
                "{}, {}",
                REG_NAMES[r as usize],
                operand(d, &mut pc, &mut name)
            ),
        ),
        Instr::Xor(r, d) => (
            "XOR".into(),
            format!(
                "{}, {}",
                REG_NAMES[r as usize],
                operand(d, &mut pc, &mut name)
            ),
        ),
        Instr::Single(op, byte, d) => (with_b(op.name(), byte), operand(d, &mut pc, &mut name)),
        Instr::Double(op, byte, s, d) => {
            let s = operand(s, &mut pc, &mut name);
            let d = operand(d, &mut pc, &mut name);
            (with_b(op.name(), byte), format!("{s}, {d}"))
        }
        Instr::Reserved => (".WORD".into(), format!("{opcode:06o}")),
    };

    let text = if operands.is_empty() {
        mnemonic
    } else {
        format!("{mnemonic:<6} {operands}")
    };
    Disassembly {
        text,
        words: pc.wrapping_sub(addr) / 2,
    }
}

fn with_b(name: &str, byte: bool) -> String {
    if byte {
        format!("{name}B")
    } else {
        name.to_string()
    }
}

/// `CLC`, `SEN!SEZ`, `CCC`, ... An empty mask is a no-op and shows as NOP.
fn flag_mnemonic(mask: u8, prefix: &str, all: &str) -> String {
    match mask {
        0 => "NOP".into(),
        0o17 => all.into(),
        _ => [(FLAG_N, 'N'), (FLAG_Z, 'Z'), (FLAG_V, 'V'), (FLAG_C, 'C')]
            .iter()
            .filter(|(bit, _)| mask & bit != 0)
            .map(|(_, l)| format!("{prefix}{l}"))
            .collect::<Vec<_>>()
            .join("!"),
    }
}

fn format_operand(
    o: Operand,
    pc: &mut u16,
    read: &mut impl FnMut(u16) -> u16,
    name: &mut dyn FnMut(u16, Ref) -> String,
) -> String {
    let r = REG_NAMES[o.reg()];
    let mut next_word = || {
        let w = read(*pc);
        *pc = pc.wrapping_add(2);
        w
    };
    match (o.mode(), o.reg()) {
        (0, _) => r.to_string(),
        (1, _) => format!("({r})"),
        (2, 7) => format!("#{}", name(next_word(), Ref::Immediate)),
        (2, _) => format!("({r})+"),
        (3, 7) => format!("@#{}", name(next_word(), Ref::Absolute)),
        (3, _) => format!("@({r})+"),
        (4, _) => format!("-({r})"),
        (5, _) => format!("@-({r})"),
        (mode, reg) => {
            let x = next_word();
            let at = if mode == 7 { "@" } else { "" };
            if reg == 7 {
                // PC-relative: show the absolute target.
                format!("{at}{}", name(pc.wrapping_add(x), Ref::Target))
            } else {
                format!("{at}{}({r})", name(x, Ref::Index))
            }
        }
    }
}

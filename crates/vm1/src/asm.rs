//! A small two-pass PDP-11 assembler in MACRO-11 syntax.
//!
//! It exists for writing CPU tests as source code and for checking the
//! disassembler: disassembling an opcode, or a whole ROM, and assembling the
//! result must give back the same bytes.
//!
//! - Numbers are octal; a trailing `.` makes them decimal (`10.`).
//! - Expressions: numbers, symbols, `.` (current address), joined by `+`/`-`.
//! - One statement per line; `;` starts a comment; `NAME:` defines a label.
//! - `NAME = expr` defines a symbol; `. = expr` moves forward to an address,
//!   filling with zero bytes.
//! - Directives: `.WORD`, `.BYTE`, `.ASCII`, `.EVEN`, `.END` (stops assembly).
//! - `.ASCII` takes delimited strings and `<expr>` byte values, e.g.
//!   `.ASCII /Ok/<12>`. Cyrillic letters are encoded in KOI-8R, the BK's
//!   character set.
//! - Input is case-insensitive, except inside `.ASCII` strings.

use std::collections::HashMap;
use std::fmt;

use crate::decode::{Cond, DoubleOp, FLAG_C, FLAG_N, FLAG_V, FLAG_Z, Operand, SingleOp};
use crate::koi8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    /// 1-based source line.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for AsmError {}

#[derive(Clone, Debug)]
pub struct Program {
    pub origin: u16,
    /// The assembled bytes, to be loaded at `origin`.
    pub bytes: Vec<u8>,
    /// Labels and `=` symbols, keyed by upper-case name.
    pub symbols: HashMap<String, u16>,
}

impl Program {
    pub fn symbol(&self, name: &str) -> Option<u16> {
        self.symbols.get(&name.to_ascii_uppercase()).copied()
    }

    /// The program as words; an odd trailing byte is padded with 0.
    pub fn words(&self) -> Vec<u16> {
        self.bytes
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
            .collect()
    }
}

/// Assemble `source` to run at address `origin`.
pub fn assemble(source: &str, origin: u16) -> Result<Program, AsmError> {
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols = HashMap::new();
    // Pass 1 defines labels; unknown symbols evaluate to 0. Pass 2 emits.
    pass(&lines, origin, &mut symbols, false)?;
    let bytes = pass(&lines, origin, &mut symbols, true)?;
    Ok(Program {
        origin,
        bytes,
        symbols,
    })
}

fn pass(
    lines: &[&str],
    origin: u16,
    symbols: &mut HashMap<String, u16>,
    resolve: bool,
) -> Result<Vec<u8>, AsmError> {
    let mut out = Vec::new();
    let mut loc = origin;
    for (i, line) in lines.iter().enumerate() {
        let err = |message: String| AsmError {
            line: i + 1,
            message,
        };
        let (labels, stmt) = parse_line(line).map_err(err)?;
        if !resolve {
            for label in labels {
                if symbols.insert(label.clone(), loc).is_some() {
                    return Err(err(format!("duplicate symbol {label}")));
                }
            }
        }
        let Some(stmt) = stmt else { continue };
        let ctx = Ctx {
            symbols: &*symbols,
            resolve,
        };
        let bytes: Vec<u8> = match stmt {
            Stmt::Assign(name, expr) => {
                let value = eval(expr, loc, &ctx).map_err(err)?;
                if name != "." {
                    symbols.insert(name, value);
                    continue;
                }
                if value < loc {
                    return Err(err(format!("can't move . back to {value:06o}")));
                }
                vec![0; usize::from(value - loc)]
            }
            Stmt::Ascii(parts) => encode_ascii(&parts, loc, &ctx).map_err(err)?,
            Stmt::Op { mnemonic, operands } => {
                let ops: Vec<&str> = operands.iter().map(String::as_str).collect();
                match mnemonic.as_str() {
                    ".END" => break,
                    ".EVEN" => vec![0; usize::from(loc & 1)],
                    ".BYTE" => ops
                        .iter()
                        .map(|e| {
                            let v = eval(e, loc, &ctx)?;
                            if v <= 0xff || v >= 0xff80 {
                                Ok(v as u8)
                            } else {
                                Err(format!(".BYTE value {v:o} doesn't fit in a byte"))
                            }
                        })
                        .collect::<Result<_, String>>()
                        .map_err(err)?,
                    m => {
                        if loc & 1 != 0 {
                            return Err(err(format!("{m} at odd address {loc:06o}")));
                        }
                        let words = encode(m, &ops, loc, &ctx).map_err(err)?;
                        words.iter().flat_map(|w| w.to_le_bytes()).collect()
                    }
                }
            }
        };
        loc = loc.wrapping_add(bytes.len() as u16);
        out.extend(bytes);
    }
    Ok(out)
}

struct Ctx<'a> {
    symbols: &'a HashMap<String, u16>,
    /// false in pass 1: unknown symbols are 0 and range checks are skipped.
    resolve: bool,
}

enum Stmt<'a> {
    /// `NAME = expr`, or `. = expr`.
    Assign(String, &'a str),
    Ascii(Vec<AsciiPart<'a>>),
    /// An instruction or directive; mnemonic and operands upper-cased.
    Op {
        mnemonic: String,
        operands: Vec<String>,
    },
}

enum AsciiPart<'a> {
    Text(&'a str),
    /// `<expr>`: one byte.
    Byte(&'a str),
}

fn identifier_len(s: &str) -> usize {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return 0,
    }
    chars
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .map_or(s.len(), |(i, _)| i)
}

fn strip_comment(s: &str) -> &str {
    s.split(';').next().unwrap_or("")
}

/// Split a line into its labels and its statement (if any).
fn parse_line(line: &str) -> Result<(Vec<String>, Option<Stmt<'_>>), String> {
    let mut rest = line.trim_start();
    let mut labels = Vec::new();
    loop {
        let n = identifier_len(rest);
        if n == 0 || !rest[n..].starts_with(':') {
            break;
        }
        labels.push(rest[..n].to_ascii_uppercase());
        rest = rest[n + 1..].trim_start();
    }
    if rest.is_empty() || rest.starts_with(';') {
        return Ok((labels, None));
    }

    let name_len = if rest.starts_with('.') && rest[1..].trim_start().starts_with('=') {
        1
    } else {
        identifier_len(rest)
    };
    if name_len > 0
        && let Some(expr) = rest[name_len..].trim_start().strip_prefix('=')
    {
        let name = rest[..name_len].to_ascii_uppercase();
        return Ok((labels, Some(Stmt::Assign(name, strip_comment(expr).trim()))));
    }

    let end = rest
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(rest.len());
    let mnemonic = rest[..end].to_ascii_uppercase();
    let args = &rest[end..];
    if mnemonic == ".ASCII" {
        return Ok((labels, Some(Stmt::Ascii(parse_ascii(args)?))));
    }
    let args = strip_comment(args).trim();
    let operands: Vec<String> = if args.is_empty() {
        Vec::new()
    } else {
        args.split(',')
            .map(|o| o.trim().to_ascii_uppercase())
            .collect()
    };
    if operands.iter().any(|o| o.is_empty()) {
        return Err("empty operand".into());
    }
    Ok((labels, Some(Stmt::Op { mnemonic, operands })))
}

/// `.ASCII` arguments: delimited strings and `<expr>` bytes, then a comment.
fn parse_ascii(mut s: &str) -> Result<Vec<AsciiPart<'_>>, String> {
    let mut parts = Vec::new();
    loop {
        s = s.trim_start();
        if s.is_empty() || s.starts_with(';') {
            break;
        }
        if let Some(r) = s.strip_prefix('<') {
            let close = r.find('>').ok_or("missing > in .ASCII")?;
            parts.push(AsciiPart::Byte(&r[..close]));
            s = &r[close + 1..];
        } else {
            let delim = s.chars().next().unwrap();
            let body = &s[delim.len_utf8()..];
            let close = body.find(delim).ok_or("unterminated .ASCII string")?;
            parts.push(AsciiPart::Text(&body[..close]));
            s = &body[close + delim.len_utf8()..];
        }
    }
    if parts.is_empty() {
        return Err(".ASCII needs a string".into());
    }
    Ok(parts)
}

fn encode_ascii(parts: &[AsciiPart], loc: u16, ctx: &Ctx) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for part in parts {
        match part {
            AsciiPart::Text(text) => {
                for c in text.chars() {
                    out.push(koi8::encode(c).ok_or(format!("{c:?} has no KOI-8 code"))?);
                }
            }
            AsciiPart::Byte(expr) => {
                let v = eval(&expr.to_ascii_uppercase(), loc, ctx)?;
                if v > 0xff {
                    return Err(format!("<{expr}> doesn't fit in a byte"));
                }
                out.push(v as u8);
            }
        }
    }
    Ok(out)
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn register(s: &str) -> Option<u8> {
    match s {
        "SP" => Some(6),
        "PC" => Some(7),
        _ => {
            let n = s.strip_prefix('R')?;
            (n.len() == 1)
                .then(|| n.parse::<u8>().ok())
                .flatten()
                .filter(|&r| r < 8)
        }
    }
}

fn eval(expr: &str, loc: u16, ctx: &Ctx) -> Result<u16, String> {
    let expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if expr.is_empty() {
        return Err("missing expression".into());
    }
    let mut total: u16 = 0;
    let mut sign_negative = false;
    let mut term = String::new();
    let apply = |term: &str, negative: bool, total: &mut u16| -> Result<(), String> {
        if term.is_empty() {
            return Err(format!("bad expression {expr}"));
        }
        let v = term_value(term, loc, ctx)?;
        *total = if negative {
            total.wrapping_sub(v)
        } else {
            total.wrapping_add(v)
        };
        Ok(())
    };
    for (i, ch) in expr.char_indices() {
        if (ch == '+' || ch == '-') && !(i > 0 && term.is_empty()) {
            if i > 0 {
                apply(&term, sign_negative, &mut total)?;
                term.clear();
            }
            sign_negative = ch == '-';
        } else {
            term.push(ch);
        }
    }
    apply(&term, sign_negative, &mut total)?;
    Ok(total)
}

fn term_value(term: &str, loc: u16, ctx: &Ctx) -> Result<u16, String> {
    if term == "." {
        return Ok(loc);
    }
    if term.starts_with(|c: char| c.is_ascii_digit()) {
        let (digits, radix) = match term.strip_suffix('.') {
            Some(d) => (d, 10),
            None => (term, 8),
        };
        return u32::from_str_radix(digits, radix)
            .ok()
            .filter(|&v| v <= 0xffff)
            .map(|v| v as u16)
            .ok_or_else(|| format!("bad number {term}"));
    }
    if !is_identifier(term) || register(term).is_some() {
        return Err(format!("bad expression term {term}"));
    }
    match ctx.symbols.get(term) {
        Some(&v) => Ok(v),
        None if !ctx.resolve => Ok(0),
        None => Err(format!("undefined symbol {term}")),
    }
}

/// The extra word an operand needs.
enum Extra<'a> {
    None,
    /// The value itself (immediate, absolute, index).
    Value(&'a str),
    /// PC-relative: the word holds target − (address of the word + 2).
    Relative(&'a str),
}

fn parse_operand(s: &str) -> Result<(Operand, Extra<'_>), String> {
    if let Some(rest) = s.strip_prefix("@#") {
        return Ok((Operand::new(3, 7), Extra::Value(rest)));
    }
    if let Some(rest) = s.strip_prefix('#') {
        return Ok((Operand::new(2, 7), Extra::Value(rest)));
    }
    let (deferred, s) = match s.strip_prefix('@') {
        Some(rest) => (1, rest.trim()),
        None => (0, s),
    };
    let inner_reg =
        |inner: &str| register(inner.trim()).ok_or_else(|| format!("bad register {inner}"));

    if let Some(r) = register(s) {
        // @Rn is the same as (Rn).
        return Ok((Operand::new(deferred, r), Extra::None));
    }
    if let Some(inner) = s.strip_prefix('(').and_then(|x| x.strip_suffix(")+")) {
        return Ok((Operand::new(2 + deferred, inner_reg(inner)?), Extra::None));
    }
    if let Some(inner) = s.strip_prefix("-(").and_then(|x| x.strip_suffix(')')) {
        return Ok((Operand::new(4 + deferred, inner_reg(inner)?), Extra::None));
    }
    if let Some(body) = s.strip_suffix(')') {
        let open = body.rfind('(').ok_or_else(|| format!("bad operand {s}"))?;
        let (index, r) = (body[..open].trim(), inner_reg(&body[open + 1..])?);
        return Ok(match (index.is_empty(), deferred) {
            (true, 0) => (Operand::new(1, r), Extra::None),
            (true, _) => (Operand::new(7, r), Extra::Value("0")), // @(Rn) = @0(Rn)
            (false, _) => (Operand::new(6 + deferred, r), Extra::Value(index)),
        });
    }
    Ok((Operand::new(6 + deferred, 7), Extra::Relative(s)))
}

fn expect_count(m: &str, ops: &[&str], n: usize) -> Result<(), String> {
    if ops.len() == n {
        Ok(())
    } else {
        Err(format!("{m} takes {n} operand(s), got {}", ops.len()))
    }
}

fn parse_register(s: &str) -> Result<u16, String> {
    register(s)
        .map(u16::from)
        .ok_or_else(|| format!("expected a register, got {s}"))
}

fn encode(m: &str, ops: &[&str], loc: u16, ctx: &Ctx) -> Result<Vec<u16>, String> {
    if m == ".WORD" {
        return ops.iter().map(|e| eval(e, loc, ctx)).collect();
    }

    let no_operand = match m {
        "HALT" => Some(0),
        "WAIT" => Some(1),
        "RTI" => Some(2),
        "BPT" => Some(3),
        "IOT" => Some(4),
        "RESET" => Some(5),
        "RTT" => Some(6),
        "START" => Some(0o10),
        "STEP" => Some(0o14),
        "NOP" => Some(0o240),
        _ => flag_opcode(m),
    };
    if let Some(op) = no_operand {
        expect_count(m, ops, 0)?;
        return Ok(vec![op]);
    }

    let small_number = |max: u16| -> Result<u16, String> {
        let v = match ops {
            [] => 0,
            [e] => eval(e, loc, ctx)?,
            _ => return Err(format!("{m} takes at most one operand")),
        };
        if v > max {
            return Err(format!("{m} operand {v:o} exceeds {max:o}"));
        }
        Ok(v)
    };
    match m {
        "EMT" => return Ok(vec![0o104000 | small_number(0o377)?]),
        "TRAP" => return Ok(vec![0o104400 | small_number(0o377)?]),
        "MARK" => return Ok(vec![0o006400 | small_number(0o77)?]),
        "RTS" => {
            expect_count(m, ops, 1)?;
            return Ok(vec![0o000200 | parse_register(ops[0])?]);
        }
        "SOB" => {
            expect_count(m, ops, 2)?;
            let r = parse_register(ops[0])?;
            let target = eval(ops[1], loc, ctx)?;
            let back = loc.wrapping_add(2).wrapping_sub(target) as i16 as i32;
            if (ctx.resolve && !(0..=126).contains(&back)) || back % 2 != 0 {
                return Err(format!("SOB target {target:06o} out of range"));
            }
            return Ok(vec![0o077000 | (r << 6) | ((back / 2) as u16 & 0o77)]);
        }
        _ => {}
    }

    let cond = match m {
        "BHIS" => Some(Cond::Bcc),
        "BLO" => Some(Cond::Bcs),
        _ => Cond::ALL.into_iter().find(|c| c.name() == m),
    };
    if let Some(c) = cond {
        expect_count(m, ops, 1)?;
        let target = eval(ops[0], loc, ctx)?;
        let diff = target.wrapping_sub(loc.wrapping_add(2)) as i16 as i32;
        if (ctx.resolve && !(-256..=254).contains(&diff)) || diff % 2 != 0 {
            return Err(format!("branch target {target:06o} out of range"));
        }
        return Ok(vec![c.opcode() | ((diff / 2) as u16 & 0xff)]);
    }

    // Everything else takes general operands.
    let (base, reg_operand, n_general): (u16, bool, usize) =
        if let Some((op, byte)) = lookup_double(m) {
            (op.opcode() | if byte { 0o100000 } else { 0 }, false, 2)
        } else if let Some((op, byte)) = lookup_single(m) {
            (op.opcode() | if byte { 0o100000 } else { 0 }, false, 1)
        } else {
            match m {
                "JMP" => (0o000100, false, 1),
                "SWAB" => (0o000300, false, 1),
                "SXT" => (0o006700, false, 1),
                "MTPS" => (0o106400, false, 1),
                "MFPS" => (0o106700, false, 1),
                "JSR" => (0o004000, true, 1),
                "XOR" => (0o074000, true, 1),
                _ => return Err(format!("unknown instruction {m}")),
            }
        };

    expect_count(m, ops, n_general + reg_operand as usize)?;
    let mut opcode = base;
    let general = if reg_operand {
        opcode |= parse_register(ops[0])? << 6;
        &ops[1..]
    } else {
        ops
    };

    let mut extras = Vec::new();
    for (i, text) in general.iter().enumerate() {
        let (operand, extra) = parse_operand(text)?;
        // The last general operand is the destination (bits 5-0).
        let shift = if i + 1 == general.len() { 0 } else { 6 };
        opcode |= (operand.0 as u16) << shift;
        extras.push(extra);
    }

    let mut words = vec![opcode];
    for extra in extras {
        let word_loc = loc.wrapping_add(2 * words.len() as u16);
        match extra {
            Extra::None => {}
            Extra::Value(e) => words.push(eval(e, loc, ctx)?),
            Extra::Relative(e) => {
                words.push(eval(e, loc, ctx)?.wrapping_sub(word_loc.wrapping_add(2)))
            }
        }
    }
    Ok(words)
}

fn lookup_double(m: &str) -> Option<(DoubleOp, bool)> {
    DoubleOp::ALL.into_iter().find_map(|op| {
        if m == op.name() {
            Some((op, false))
        } else if op.has_byte_form() && m.strip_suffix('B') == Some(op.name()) {
            Some((op, true))
        } else {
            None
        }
    })
}

fn lookup_single(m: &str) -> Option<(SingleOp, bool)> {
    SingleOp::ALL.into_iter().find_map(|op| {
        if m == op.name() {
            Some((op, false))
        } else if m.strip_suffix('B') == Some(op.name()) {
            Some((op, true))
        } else {
            None
        }
    })
}

/// CLC, SEV, CCC, SCC, and combinations such as `CLN!CLZ`.
fn flag_opcode(m: &str) -> Option<u16> {
    let mut base = None;
    let mut mask = 0u16;
    for part in m.split('!') {
        let (b, bits) = match part {
            "CCC" => (0o240, 0o17),
            "SCC" => (0o260, 0o17),
            _ => {
                let b = match part.get(..2)? {
                    "CL" => 0o240,
                    "SE" => 0o260,
                    _ => return None,
                };
                let bit = match &part[2..] {
                    "C" => FLAG_C,
                    "V" => FLAG_V,
                    "Z" => FLAG_Z,
                    "N" => FLAG_N,
                    _ => return None,
                };
                (b, bit as u16)
            }
        };
        if base.is_some_and(|x| x != b) {
            return None; // mixing CLx and SEx in one opcode isn't possible
        }
        base = Some(b);
        mask |= bits;
    }
    Some(base? | mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<u16> {
        assemble(src, 0o1000).unwrap().words()
    }

    #[test]
    fn operand_modes() {
        assert_eq!(words("MOV R1, R2"), [0o010102]);
        assert_eq!(words("MOV (R1)+, -(SP)"), [0o012146]);
        assert_eq!(words("MOV @(R1)+, @-(R2)"), [0o013152]);
        assert_eq!(words("MOV #1234, @#177716"), [0o012737, 0o1234, 0o177716]);
        assert_eq!(words("MOV 10(R1), @-2(R2)"), [0o016172, 0o10, 0o177776]);
        assert_eq!(words("MOV @R1, @(R2)"), [0o011172, 0]);
        // PC-relative: offset from the address after the index word.
        assert_eq!(words("CLR 1010"), [0o005067, 0o4]);
        assert_eq!(words("JMP @2000"), [0o000177, 0o774]);
    }

    #[test]
    fn labels_and_branches() {
        let p = assemble("START: INC R0\n  BR START\nEND: .WORD END, 10.", 0o1000).unwrap();
        assert_eq!(p.words(), [0o005200, 0o000776, 0o1004, 10]);
        assert_eq!(p.symbol("end"), Some(0o1004));
    }

    #[test]
    fn misc_instructions() {
        assert_eq!(words("SOB R3, ."), [0o077301]);
        assert_eq!(words("JSR PC, @#100"), [0o004737, 0o100]);
        assert_eq!(words("RTS PC"), [0o000207]);
        assert_eq!(words("EMT 16"), [0o104016]);
        assert_eq!(words("TRAP"), [0o104400]);
        assert_eq!(words("CLN!CLC"), [0o000251]);
        assert_eq!(words("SCC"), [0o000277]);
        assert_eq!(words("MOVB R0, R1"), [0o110001]);
        assert_eq!(words("XOR R2, (R3)"), [0o074213]);
    }

    #[test]
    fn data_directives_and_symbols() {
        let p = assemble(
            "SEL1 = 177716\n\
             START: MOV #1, @#SEL1 ; comment\n\
             MSG: .ASCII /Ok; Да/<12>\n\
             .BYTE 1, -1\n\
             .EVEN\n\
             . = START + 20\n\
             END: .WORD MSG",
            0o1000,
        )
        .unwrap();
        assert_eq!(p.symbol("SEL1"), Some(0o177716));
        assert_eq!(p.symbol("msg"), Some(0o1006));
        assert_eq!(
            &p.bytes[6..16],
            &[b'O', b'k', b';', b' ', 0o344, 0o301, 0o12, 1, 0o377, 0]
        );
        assert_eq!(p.symbol("END"), Some(0o1020));
        assert_eq!(p.words().last(), Some(&0o1006));
        assert!(
            assemble(".BYTE 1\nNOP", 0).is_err(),
            "instruction at odd address"
        );
        assert!(assemble("NOP\n. = 0", 0).is_err(), ". moves backward");
    }

    #[test]
    fn errors_name_the_line() {
        let e = assemble("NOP\nBR NOWHERE\n.WORD 0", 0).unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.message.contains("NOWHERE"));
        assert!(assemble("ADDB R0, R1", 0).is_err());
        assert!(assemble("MOV R0", 0).is_err());
    }
}

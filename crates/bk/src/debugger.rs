//! A command-line debugger, independent of any user interface: a front end
//! passes it command lines and shows the text it returns. Numbers are
//! octal, as on the BK.
//!
//! While the machine runs (after `c` or `n`), the front end calls
//! [`Machine::run_frame`] as usual and hands any [`Break`] it returns to
//! [`Debugger::stopped`]; while tracing it calls [`Debugger::flush_trace`]
//! now and then.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;

use crate::{Break, Machine, TraceEntry};
use vm1::{Ref, disassemble_with, koi8, psw};

pub const HELP: &str = "\
Debugger commands. Numbers and addresses are octal; a trailing dot makes
a number decimal (`m 1000 8.`), as in MACRO-11. An address can also be a
register name (`u pc`, `m sp`) or a symbol (`b KEYIN`), and for m, e and f
a physical RAM page and offset, `pN:OFF` (`m p7:0`: page 7, whatever is
mapped in).
  r                 registers and the next instruction
  s [N]             step N instructions (default 1), into calls and traps
  n                 step over: run a JSR, EMT, TRAP, IOT or BPT to its return
  c                 continue until a breakpoint or watchpoint
  b [ADDR [COND]]   set a breakpoint at ADDR, or list them. COND is
                    REG=VALUE or ADDR=VALUE (a memory word): stop only then
  be N, bt N        stop before any EMT N, or TRAP N
  bc ADDR | all     clear a breakpoint, or all of them (bc emt N, bc trap N)
  w [ADDR]          stop when the CPU writes ADDR (RAM or I/O), or list
  wc ADDR | all     clear watchpoints
  u [ADDR] [N]      disassemble N instructions (default 16) from ADDR
  m [ADDR] [N]      show N words of memory (default 64) from ADDR
  e ADDR WORD...    store words in RAM from ADDR
  f VALUE [FROM [TO]]  find the word VALUE in memory (FROM-TO, default all)
  sym [NAME [ADDR]] list the number of symbols, show one, or define one
  t FILE | t off    trace every instruction executed to FILE, or stop
  r0..r5, sp, pc, psw VALUE   set a register
An empty line repeats s, n, u or m (u and m carry on from where they
stopped).";

/// What the front end should do after a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Stay stopped and read another command.
    Stay,
    /// Run the machine until [`Machine::run_frame`] returns a break.
    Run,
}

/// What a breakpoint's condition looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operand {
    Reg(usize),
    Mem(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Condition {
    what: Operand,
    value: u16,
}

impl Condition {
    fn holds(&self, m: &Machine) -> bool {
        let v = match self.what {
            Operand::Reg(r) => Some(m.cpu().r[r]),
            Operand::Mem(a) => m.peek_word(a),
        };
        v == Some(self.value)
    }
}

/// An address for the memory commands: as the CPU sees it, or in a
/// physical RAM page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Cpu(u16),
    Page(usize, u16),
}

impl Place {
    fn offset(self, by: u16) -> Place {
        match self {
            Place::Cpu(a) => Place::Cpu(a.wrapping_add(by)),
            Place::Page(p, o) => Place::Page(p, o.wrapping_add(by) & 0o37777),
        }
    }
    fn peek(self, m: &Machine) -> Option<u16> {
        match self {
            Place::Cpu(a) => m.peek_word(a),
            Place::Page(p, o) => m.peek_page_word(p, o),
        }
    }
    fn poke(self, m: &mut Machine, value: u16) -> bool {
        match self {
            Place::Cpu(a) => m.poke_word(a, value),
            Place::Page(p, o) => m.poke_page_word(p, o, value),
        }
    }
    fn label(self) -> String {
        match self {
            Place::Cpu(a) => format!("{a:06o}"),
            Place::Page(p, o) => format!("p{p}:{o:06o}"),
        }
    }
}

#[derive(Default)]
pub struct Debugger {
    breakpoints: Vec<(u16, Option<Condition>)>,
    /// Opcodes to stop before (`be`, `bt`).
    opcodes: Vec<u16>,
    watches: Vec<u16>,
    /// A one-off breakpoint for `n`.
    temporary: Option<u16>,
    last: String,
    next_list: Option<u16>,
    next_dump: Option<Place>,
    /// Names by address, and addresses by lower-case name.
    symbols: BTreeMap<u16, String>,
    by_name: HashMap<String, u16>,
    /// Names of EMT calls by number, for the disassembly.
    emts: HashMap<u8, String>,
    trace: Option<std::io::BufWriter<std::fs::File>>,
}

const REGS: [&str; 8] = ["R0", "R1", "R2", "R3", "R4", "R5", "SP", "PC"];

impl Debugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add symbols from text in the annotation format (`name ADDR NAME`,
    /// `emt N NAME`, with `;` comments; see `annotations/`) or MACRO-11's
    /// `NAME = ADDR`. Returns how many were added.
    pub fn load_symbols(&mut self, text: &str) -> usize {
        let mut added = 0;
        for line in text.lines() {
            let line = line.split(';').next().unwrap_or("").trim();
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["name", addr, name] => {
                    if let Ok(a) = u16::from_str_radix(addr, 8) {
                        self.define(name, a);
                        added += 1;
                    }
                }
                ["emt", n, name] => {
                    if let Ok(n) = u8::from_str_radix(n, 8) {
                        self.emts.insert(n, name.to_string());
                        added += 1;
                    }
                }
                [name, "=", addr] => {
                    if let Ok(a) = u16::from_str_radix(addr.trim_end_matches('.'), 8) {
                        self.define(name, a);
                        added += 1;
                    }
                }
                _ => {}
            }
        }
        added
    }

    fn define(&mut self, name: &str, addr: u16) {
        self.symbols.insert(addr, name.to_string());
        self.by_name.insert(name.to_ascii_lowercase(), addr);
    }

    /// Registers, flags and the next instruction.
    pub fn status(&self, m: &Machine) -> String {
        let cpu = m.cpu();
        let mut out = String::new();
        let regs: Vec<String> = REGS
            .iter()
            .zip(cpu.r)
            .map(|(name, v)| format!("{name} {v:06o}"))
            .collect();
        out += &regs.join("  ");
        let p = cpu.psw;
        let flag = |bit: u16, c: char| if p & bit != 0 { c } else { '-' };
        let _ = write!(
            out,
            "\nPSW {p:06o} ({}{}{}{}{}{}){}{}  frame {}\n",
            flag(psw::P, 'P'),
            flag(psw::T, 'T'),
            flag(psw::N, 'N'),
            flag(psw::Z, 'Z'),
            flag(psw::V, 'V'),
            flag(psw::C, 'C'),
            if cpu.is_waiting() { "  WAIT" } else { "" },
            if cpu.is_halted() { "  HALTED" } else { "" },
            m.frames(),
        );
        out += &self.line(m, cpu.pc()).0;
        out
    }

    /// An instruction's text, with symbols for the addresses it uses and
    /// the name of an EMT call.
    fn text(&self, addr: u16, read: impl FnMut(u16) -> u16) -> (String, u16) {
        let d = disassemble_with(addr, read, |v, r| match r {
            Ref::Target | Ref::Absolute => self.symbols.get(&v).cloned(),
            Ref::Immediate | Ref::Index => None,
        });
        let mut text = d.text;
        if let Some(n) = text
            .strip_prefix("EMT")
            .and_then(|n| u8::from_str_radix(n.trim(), 8).ok())
            && let Some(name) = self.emts.get(&n)
        {
            let _ = write!(text, "    ; {name}");
        }
        (text, d.words)
    }

    /// One disassembled line (after a label line if the address has a
    /// name) and the address after it.
    fn line(&self, m: &Machine, addr: u16) -> (String, u16) {
        if m.peek_word(addr).is_none() {
            return (
                format!("  {addr:06o}  (not memory)\n"),
                addr.wrapping_add(2),
            );
        }
        let (text, n) = self.text(addr, |a| m.peek_word(a).unwrap_or(0));
        let words: Vec<String> = (0..n)
            .map(|i| format!("{:06o}", m.peek_word(addr.wrapping_add(2 * i)).unwrap_or(0)))
            .collect();
        let mark = match (
            addr == m.cpu().pc(),
            self.breakpoints.iter().any(|b| b.0 == addr),
        ) {
            (true, true) => ">*",
            (true, false) => "> ",
            (false, true) => " *",
            (false, false) => "  ",
        };
        let label = self
            .symbols
            .get(&addr)
            .map(|s| format!("{s}:\n"))
            .unwrap_or_default();
        (
            format!("{label}{mark}{addr:06o}  {:<20} {text}\n", words.join(" ")),
            addr.wrapping_add(2 * n),
        )
    }

    /// Run a command line. Returns the text to show and what to do next.
    pub fn command(&mut self, m: &mut Machine, line: &str) -> (String, Action) {
        let line = line.trim();
        let line = if line.is_empty() {
            self.last.clone()
        } else {
            line.to_string()
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = words.first() else {
            return (String::new(), Action::Stay);
        };
        let args = &words[1..];
        // Only these repeat on an empty line; u and m carry on from where
        // they stopped.
        self.last = match cmd {
            "u" | "m" => cmd.to_string(),
            "s" | "n" => line.clone(),
            _ => String::new(),
        };
        let result = self.run_command(m, cmd, args);
        self.flush_trace(m);
        match result {
            Ok(result) => result,
            Err(e) => (format!("{e}\n"), Action::Stay),
        }
    }

    /// An octal number, a decimal one with a trailing dot, a register's
    /// value, or a symbol.
    fn value(&self, m: &Machine, s: &str) -> Result<u16, String> {
        if let Some(r) = register(s) {
            return Ok(m.cpu().r[r]);
        }
        if let Some(&a) = self.by_name.get(&s.to_ascii_lowercase()) {
            return Ok(a);
        }
        match s.strip_suffix('.') {
            Some(decimal) => decimal.parse(),
            None => u16::from_str_radix(s, 8),
        }
        .map_err(|_| {
            format!(
                "{s:?} isn't an octal number, a decimal one (with a dot), a register or a symbol"
            )
        })
    }

    /// An address for m, e and f: anything [`Debugger::value`] takes, or
    /// `pN:OFF`.
    fn place(&self, m: &Machine, s: &str) -> Result<Place, String> {
        if let Some((page, offset)) = s.strip_prefix(['p', 'P']).and_then(|r| r.split_once(':')) {
            let page: usize = page
                .parse()
                .map_err(|_| format!("{s:?}: bad page number"))?;
            let offset =
                u16::from_str_radix(offset, 8).map_err(|_| format!("{s:?}: bad offset"))?;
            if m.peek_page_word(page, offset).is_none() {
                return Err(format!("{s}: no such RAM page or offset"));
            }
            return Ok(Place::Page(page, offset & !1));
        }
        Ok(Place::Cpu(self.value(m, s)?))
    }

    fn run_command(
        &mut self,
        m: &mut Machine,
        cmd: &str,
        args: &[&str],
    ) -> Result<(String, Action), String> {
        let stay = |s: String| Ok((s, Action::Stay));
        let num = |i: usize| -> Result<Option<u16>, String> {
            args.get(i).map(|a| self.value(m, a)).transpose()
        };
        match cmd {
            "h" | "?" | "help" => stay(format!("{HELP}\n")),
            "r" => stay(self.status(m)),
            "s" => {
                let n = num(0)?.unwrap_or(1).max(1);
                let mut out = String::new();
                for i in 0..n {
                    if n <= 20 {
                        out += &self.line(m, m.cpu().pc()).0;
                    }
                    if let Some(b) = m.step_into() {
                        out += &self.describe(b);
                        break;
                    }
                    if m.cpu().is_halted() {
                        let _ = writeln!(out, "the CPU halted after {} instructions", i + 1);
                        break;
                    }
                }
                stay(out + &self.status(m))
            }
            "n" => {
                let pc = m.cpu().pc();
                let op = m.peek_word(pc).unwrap_or(0);
                let calls = op & 0o177000 == 0o004000 // JSR
                    || op & 0o177000 == 0o104000 // EMT, TRAP
                    || op == 0o000003 // BPT
                    || op == 0o000004; // IOT
                if !calls {
                    return self.run_command(m, "s", &[]);
                }
                let (_, words) = self.text(pc, |a| m.peek_word(a).unwrap_or(0));
                self.temporary = Some(pc.wrapping_add(2 * words));
                self.resume(m)
            }
            "c" => self.resume(m),
            "b" => match num(0)? {
                None if self.breakpoints.is_empty() && self.opcodes.is_empty() => {
                    stay("no breakpoints\n".into())
                }
                None => {
                    let mut out = String::new();
                    for (a, cond) in &self.breakpoints {
                        let line = self.line(m, *a).0;
                        match cond {
                            Some(c) => {
                                let what = match c.what {
                                    Operand::Reg(r) => REGS[r].to_string(),
                                    Operand::Mem(a) => format!("{a:06o}"),
                                };
                                let _ =
                                    writeln!(out, "{}  if {what}={:06o}", line.trim_end(), c.value);
                            }
                            None => out += &line,
                        }
                    }
                    for op in &self.opcodes {
                        let kind = if op & 0o400 == 0 { "EMT" } else { "TRAP" };
                        let _ = writeln!(out, "  before any {kind} {:03o}", op & 0o377);
                    }
                    stay(out)
                }
                Some(a) => {
                    let cond = args.get(1).map(|c| self.condition(m, c)).transpose()?;
                    self.breakpoints.retain(|b| b.0 != a);
                    self.breakpoints.push((a, cond));
                    self.sync(m);
                    stay(format!(
                        "breakpoint at {a:06o}{}\n",
                        if cond.is_some() {
                            " (with a condition)"
                        } else {
                            ""
                        }
                    ))
                }
            },
            "be" | "bt" => {
                let n = num(0)?.ok_or(format!("{cmd} needs a number"))?;
                if n > 0o377 {
                    return Err("EMT and TRAP numbers go up to 377".into());
                }
                let op = if cmd == "be" { 0o104000 } else { 0o104400 } | n;
                if !self.opcodes.contains(&op) {
                    self.opcodes.push(op);
                }
                self.sync(m);
                let kind = if cmd == "be" { "EMT" } else { "TRAP" };
                stay(format!("stopping before {kind} {n:03o}\n"))
            }
            "bc" => {
                match args {
                    ["all"] => {
                        self.breakpoints.clear();
                        self.opcodes.clear();
                    }
                    [kind @ ("emt" | "trap"), n] => {
                        let n = self.value(m, n)?;
                        let op = if *kind == "emt" { 0o104000 } else { 0o104400 } | (n & 0o377);
                        if !self.opcodes.contains(&op) {
                            return Err(format!(
                                "not stopping before {} {n:03o}",
                                kind.to_uppercase()
                            ));
                        }
                        self.opcodes.retain(|&o| o != op);
                    }
                    [a] => {
                        let a = self.value(m, a)?;
                        if !self.breakpoints.iter().any(|b| b.0 == a) {
                            return Err(format!("nothing set at {a:06o}"));
                        }
                        self.breakpoints.retain(|b| b.0 != a);
                    }
                    _ => return Err("which address? (or all, emt N, trap N)".into()),
                }
                self.sync(m);
                stay(String::new())
            }
            "w" => match num(0)? {
                None if self.watches.is_empty() => stay("no watchpoints\n".into()),
                None => stay(
                    self.watches
                        .iter()
                        .map(|a| format!("watching {a:06o}\n"))
                        .collect(),
                ),
                Some(a) => {
                    let a = a & !1;
                    if !self.watches.contains(&a) {
                        self.watches.push(a);
                    }
                    self.sync(m);
                    stay(format!("watching writes to {a:06o}\n"))
                }
            },
            "wc" => {
                match args.first().copied() {
                    Some("all") => self.watches.clear(),
                    Some(a) => {
                        let a = self.value(m, a)? & !1;
                        if !self.watches.contains(&a) {
                            return Err(format!("nothing set at {a:06o}"));
                        }
                        self.watches.retain(|&x| x != a);
                    }
                    None => return Err("which address? (or all)".into()),
                }
                self.sync(m);
                stay(String::new())
            }
            "u" => {
                let mut addr = num(0)?
                    .or(self.next_list.filter(|_| args.is_empty()))
                    .unwrap_or(m.cpu().pc())
                    & !1;
                let n = num(1)?.unwrap_or(16);
                let mut out = String::new();
                for _ in 0..n {
                    let (text, next) = self.line(m, addr);
                    out += &text;
                    addr = next;
                }
                self.next_list = Some(addr);
                stay(out)
            }
            "m" => {
                let start = match args.first() {
                    Some(a) => self.place(m, a)?,
                    None => self.next_dump.unwrap_or(Place::Cpu(m.cpu().pc())),
                };
                let mut place = match start {
                    Place::Cpu(a) => Place::Cpu(a & !1),
                    p => p,
                };
                let n = num(1)?.unwrap_or(64);
                let mut out = String::new();
                for row in 0..n.div_ceil(8) {
                    let count = (n - 8 * row).min(8);
                    let words: Vec<Option<u16>> =
                        (0..count).map(|i| place.offset(2 * i).peek(m)).collect();
                    let _ = write!(out, "{} ", place.label());
                    for w in &words {
                        match w {
                            Some(w) => {
                                let _ = write!(out, " {w:06o}");
                            }
                            None => out += " ------",
                        }
                    }
                    out += &"       ".repeat(8 - usize::from(count));
                    out += "  ";
                    for b in words
                        .iter()
                        .flat_map(|w| w.map_or([0, 0], u16::to_le_bytes))
                    {
                        out.push(match koi8::decode(b) {
                            Some(c) if !c.is_control() => c,
                            _ => '.',
                        });
                    }
                    out.push('\n');
                    place = place.offset(2 * count);
                }
                self.next_dump = Some(place);
                stay(out)
            }
            "e" => {
                let place = self.place(m, args.first().ok_or("e needs an address and words")?)?;
                if args.len() < 2 {
                    return Err("e needs an address and words".into());
                }
                for (i, a) in args[1..].iter().enumerate() {
                    let at = place.offset(2 * i as u16);
                    let v = self.value(m, a)?;
                    if !at.poke(m, v) {
                        return Err(format!("{} isn't RAM", at.label()));
                    }
                }
                stay(String::new())
            }
            "f" => {
                let want = num(0)?.ok_or("f needs a value")?;
                let from = args
                    .get(1)
                    .map(|a| self.place(m, a))
                    .transpose()?
                    .unwrap_or(Place::Cpu(0));
                let to = num(2)?.unwrap_or(0o177776);
                let mut out = String::new();
                let mut found = 0;
                let mut place = match from {
                    Place::Cpu(a) => Place::Cpu(a & !1),
                    p => p,
                };
                loop {
                    let offset = match place {
                        Place::Cpu(a) => a,
                        Place::Page(_, o) => o,
                    };
                    if offset > to {
                        break;
                    }
                    if place.peek(m) == Some(want) {
                        found += 1;
                        if found <= 64 {
                            let _ = writeln!(out, "{}", place.label());
                        }
                    }
                    let next = place.offset(2);
                    let wrapped = match (place, next) {
                        (Place::Cpu(a), Place::Cpu(b)) => b < a,
                        (Place::Page(_, a), Place::Page(_, b)) => b < a,
                        _ => true,
                    };
                    if wrapped {
                        break;
                    }
                    place = next;
                }
                let _ = writeln!(
                    out,
                    "{found} found{}",
                    if found > 64 { " (64 shown)" } else { "" }
                );
                stay(out)
            }
            "sym" => match args {
                [] => stay(format!(
                    "{} symbols, {} EMT names\n",
                    self.symbols.len(),
                    self.emts.len()
                )),
                [name] => match self.by_name.get(&name.to_ascii_lowercase()) {
                    Some(a) => stay(format!("{name} = {a:06o}\n")),
                    None => Err(format!("no symbol {name}")),
                },
                [name, addr, ..] => {
                    let a = self.value(m, addr)?;
                    self.define(name, a);
                    stay(format!("{name} = {a:06o}\n"))
                }
            },
            "t" => match args.first().copied() {
                None => stay(
                    if self.trace.is_some() {
                        "tracing\n"
                    } else {
                        "not tracing\n"
                    }
                    .into(),
                ),
                Some("off") => {
                    self.flush_trace(m);
                    self.trace = None;
                    m.set_tracing(false);
                    stay("tracing off\n".into())
                }
                Some(path) => {
                    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
                    self.trace = Some(std::io::BufWriter::new(file));
                    m.set_tracing(true);
                    stay(format!("tracing to {path}\n"))
                }
            },
            "psw" => {
                let v = num(0)?.ok_or("psw needs a value")?;
                m.cpu_mut().psw = v & 0o377;
                stay(self.status(m))
            }
            _ => {
                let Some(reg) = register(cmd) else {
                    return Err(format!("unknown command {cmd:?}; h for help"));
                };
                let v = num(0)?.ok_or(format!("{cmd} needs a value"))?;
                if reg == 7 {
                    m.cpu_mut().start_at(v);
                } else {
                    m.cpu_mut().r[reg] = v;
                }
                stay(self.status(m))
            }
        }
    }

    /// `REG=VALUE` or `ADDR=VALUE`.
    fn condition(&self, m: &Machine, text: &str) -> Result<Condition, String> {
        let (lhs, rhs) = text
            .split_once('=')
            .ok_or(format!("{text:?}: a condition is REG=VALUE or ADDR=VALUE"))?;
        let what = match register(lhs) {
            Some(r) => Operand::Reg(r),
            None => Operand::Mem(self.value(m, lhs)? & !1),
        };
        Ok(Condition {
            what,
            value: self.value(m, rhs)?,
        })
    }

    /// Step off the current instruction (so a breakpoint there doesn't stop
    /// us at once), then let the front end run.
    fn resume(&mut self, m: &mut Machine) -> Result<(String, Action), String> {
        self.sync(m);
        if let Some(b) = m.step_into()
            && let Some(text) = self.stopped(m, b)
        {
            return Ok((text, Action::Stay));
        }
        if self.temporary == Some(m.cpu().pc()) {
            self.temporary = None;
            self.sync(m);
            return Ok((self.status(m), Action::Stay));
        }
        Ok((String::new(), Action::Run))
    }

    /// The machine stopped at a break while running. Returns the text to
    /// show; `None` when it was a breakpoint whose condition doesn't hold,
    /// in which case the debugger has stepped past it and the front end
    /// should keep running.
    pub fn stopped(&mut self, m: &mut Machine, why: Break) -> Option<String> {
        if let Break::Breakpoint(pc) = why
            && Some(pc) != self.temporary
            && let Some((_, Some(cond))) = self.breakpoints.iter().find(|b| b.0 == pc)
            && !cond.holds(m)
        {
            return match m.step_into() {
                Some(b) => self.stopped(m, b),
                None => None,
            };
        }
        let out = match why {
            Break::Breakpoint(pc)
                if Some(pc) == self.temporary && !self.breakpoints.iter().any(|b| b.0 == pc) =>
            {
                String::new()
            }
            b => self.describe(b),
        };
        self.temporary = None;
        self.sync(m);
        self.flush_trace(m);
        Some(out + &self.status(m))
    }

    /// Run until a break, at most `max_frames` frames: for scripts and
    /// tests. Returns the text to show, or `None` if nothing stopped it.
    pub fn run(&mut self, m: &mut Machine, max_frames: u32) -> Option<String> {
        for _ in 0..max_frames {
            if let Some(b) = m.run_frame()
                && let Some(text) = self.stopped(m, b)
            {
                return Some(text);
            }
            self.flush_trace(m);
        }
        None
    }

    /// Whether `t` is writing a trace (the front end should call
    /// [`Debugger::flush_trace`] after running frames).
    pub fn tracing(&self) -> bool {
        self.trace.is_some()
    }

    /// Write the instructions executed since the last call to the trace
    /// file.
    pub fn flush_trace(&mut self, m: &mut Machine) {
        let entries = m.take_trace();
        let Some(file) = self.trace.as_mut() else {
            return;
        };
        let mut out = String::new();
        for TraceEntry { cpu, words } in entries {
            let pc = cpu.r[7];
            let (text, n) = {
                let read = |a: u16| {
                    words
                        .get(usize::from(a.wrapping_sub(pc) / 2))
                        .copied()
                        .unwrap_or(0)
                };
                // Symbols from `self`, read from the recorded words.
                let d = disassemble_with(pc, read, |v, r| match r {
                    Ref::Target | Ref::Absolute => self.symbols.get(&v).cloned(),
                    _ => None,
                });
                (d.text, d.words)
            };
            let shown: Vec<String> = words[..usize::from(n)]
                .iter()
                .map(|w| format!("{w:06o}"))
                .collect();
            let regs: Vec<String> = cpu.r[..7]
                .iter()
                .zip(REGS)
                .map(|(v, n)| format!("{n}={v:06o}"))
                .collect();
            let _ = writeln!(
                out,
                "{pc:06o}  {:<20} {text:<28} {} PSW={:03o}",
                shown.join(" "),
                regs.join(" "),
                cpu.psw
            );
        }
        let _ = file.write_all(out.as_bytes());
        let _ = file.flush();
    }

    /// Clear every breakpoint and watchpoint, so the machine runs freely
    /// (when nobody is left to continue it).
    pub fn detach(&mut self, m: &mut Machine) {
        self.breakpoints.clear();
        self.opcodes.clear();
        self.watches.clear();
        self.temporary = None;
        self.sync(m);
    }

    /// Give the machine the current breakpoints and watchpoints.
    fn sync(&self, m: &mut Machine) {
        let mut bps: Vec<u16> = self.breakpoints.iter().map(|b| b.0).collect();
        bps.extend(self.temporary);
        m.set_breakpoints(&bps);
        m.set_opcode_breaks(&self.opcodes);
        m.set_watchpoints(&self.watches);
    }

    fn describe(&self, b: Break) -> String {
        match b {
            Break::Breakpoint(pc) => format!("breakpoint at {pc:06o}\n"),
            Break::Opcode(pc) => format!("stopped before the call at {pc:06o}\n"),
            Break::Watch { pc, addr, value } => {
                format!("watchpoint: the instruction at {pc:06o} wrote {value:06o} to {addr:06o}\n")
            }
        }
    }
}

fn register(name: &str) -> Option<usize> {
    match name.to_ascii_lowercase().as_str() {
        "sp" => Some(6),
        "pc" => Some(7),
        r => r.strip_prefix('r')?.parse().ok().filter(|&n| n < 8),
    }
}

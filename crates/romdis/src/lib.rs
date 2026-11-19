//! Commented disassembly of the BK-0011M ROM images.
//!
//! Code is found by following the program from its entry points (the ROM
//! start, annotated entries, interrupt handlers the code installs, and jump
//! tables). Remaining gaps get a cautious linear scan; whatever is still
//! unclaimed is data. The listing is MACRO-11 source that `vm1::asm`
//! assembles back into the identical ROM image.

pub mod annotations;
pub mod disk;
pub mod inventory;

use std::path::Path;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use annotations::{Annotations, Layout};
use vm1::decode::{Cond, DoubleOp, Instr, Operand, decode};
use vm1::{Ref, disassemble, disassemble_with, koi8};

/// Which machine a ROM or program belongs to. It decides the shared
/// annotations: BOS calls on the BK-0011M, monitor calls on the BK-0010.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Family {
    #[default]
    Bk0010,
    Bk0011m,
}

impl Family {
    /// Shared annotation files, in `annotations/`, applied before a ROM's
    /// or program's own file.
    pub fn common_annotations(self) -> &'static [&'static str] {
        match self {
            Family::Bk0010 => &["devices.ann", "bk0010.ann"],
            Family::Bk0011m => &["devices.ann", "bk0011m.ann"],
        }
    }
}

/// One stock ROM image.
pub struct RomSpec {
    pub file: &'static str,
    pub origin: u16,
    pub title: &'static str,
    pub family: Family,
    /// ROM-specific annotation file in `annotations/`.
    pub annotations: &'static str,
}

pub const ROMS: &[RomSpec] = &[
    RomSpec {
        file: "b11m_bos.rom",
        origin: 0o140000,
        title: "BK-0011M BOS (base operating system), 140000-157777",
        family: Family::Bk0011m,
        annotations: "b11m_bos.ann",
    },
    RomSpec {
        file: "b11m_ext.rom",
        origin: 0o120000,
        title: "BK-0011M BOS, second part: ROM page 1, 120000-137777",
        family: Family::Bk0011m,
        annotations: "b11m_ext.ann",
    },
    RomSpec {
        file: "basic11m_0.rom",
        origin: 0o100000,
        title: "BK-0011M BASIC, ROM page 0, 100000-137777",
        family: Family::Bk0011m,
        annotations: "basic11m_0.ann",
    },
    RomSpec {
        file: "basic11m_1.rom",
        origin: 0o100000,
        title: "BK-0011M BASIC, ROM page 1, 100000-117777",
        family: Family::Bk0011m,
        annotations: "basic11m_1.ann",
    },
    RomSpec {
        file: "b11m_mstd.rom",
        origin: 0o160000,
        title: "BK-0011M MSTD test and diagnostics ROM, 160000-177577",
        family: Family::Bk0011m,
        annotations: "b11m_mstd.ann",
    },
    RomSpec {
        file: "disk_327.rom",
        origin: 0o160000,
        title: "Floppy controller (KNGMD) ROM 327: disk driver and boot loader, 160000-167777",
        family: Family::Bk0011m,
        annotations: "disk_327.ann",
    },
    RomSpec {
        file: "disk_326.rom",
        origin: 0o160000,
        title: "Floppy controller (KNGMD) ROM 326 for the BK-0010.01: disk driver and boot loader, 160000-167777",
        family: Family::Bk0010,
        annotations: "disk_326.ann",
    },
    RomSpec {
        file: "disk_253.rom",
        origin: 0o160000,
        title: "Floppy controller (KNGMD) ROM 253, another version (bk doesn't use it), 160000-167777",
        family: Family::Bk0010,
        annotations: "disk_253.ann",
    },
    RomSpec {
        file: "monit10.rom",
        origin: 0o100000,
        title: "BK-0010 monitor (driver-monitor system), 100000-117777",
        family: Family::Bk0010,
        annotations: "monit10.ann",
    },
    RomSpec {
        file: "basic10_1.rom",
        origin: 0o120000,
        title: "BK-0010.01 Vilnius BASIC, part 1, 120000-137777",
        family: Family::Bk0010,
        annotations: "basic10.ann",
    },
    RomSpec {
        file: "basic10_2.rom",
        origin: 0o140000,
        title: "BK-0010.01 Vilnius BASIC, part 2, 140000-157777",
        family: Family::Bk0010,
        annotations: "basic10.ann",
    },
    RomSpec {
        file: "basic10_3.rom",
        origin: 0o160000,
        title: "BK-0010.01 Vilnius BASIC, part 3, 160000-177577",
        family: Family::Bk0010,
        annotations: "basic10.ann",
    },
    RomSpec {
        file: "focal.rom",
        origin: 0o120000,
        title: "BK-0010 FOCAL, 120000-137777",
        family: Family::Bk0010,
        annotations: "focal.ann",
    },
    RomSpec {
        file: "tests.rom",
        origin: 0o160000,
        title: "BK-0010 test ROM, 160000-177577",
        family: Family::Bk0010,
        annotations: "tests10.ann",
    },
];

/// Split a `.bin` file into its load address and data (a word with the
/// address, a word with the length, then the data; anything after is ignored).
pub fn parse_bin(bytes: &[u8]) -> Result<(u16, &[u8]), String> {
    if bytes.len() < 4 {
        return Err("too short for a .bin header".into());
    }
    let address = u16::from_le_bytes([bytes[0], bytes[1]]);
    let len = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let data = bytes
        .get(4..4 + len)
        .ok_or(format!("header says {len} bytes, file is shorter"))?;
    Ok((address, data))
}

/// A program's start address: its load address, or 1000 for programs loaded
/// below 1000 (they overwrite the stack to start themselves from tape).
pub fn start_address(load: u16) -> u16 {
    load.max(0o1000)
}

/// Load the shared annotations for `family`, then `own` (a file in `dir`,
/// if it exists). Missing shared files are an error; a missing own file
/// isn't.
pub fn load_annotations(
    dir: &Path,
    family: Family,
    own: Option<&str>,
) -> Result<Annotations, String> {
    let mut ann = Annotations::default();
    for name in family.common_annotations() {
        let path = dir.join(name);
        let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        ann.add(&src)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(name) = own {
        let path = dir.join(name);
        if let Ok(src) = std::fs::read_to_string(&path) {
            ann.add(&src)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(ann)
}

/// Like [`load_annotations`], for a program loaded in RAM at `origin`,
/// `len` bytes long. The shared annotations describe the ROMs and the
/// system's cells; inside the program's own image they don't apply (the
/// program overwrites the cells, and a ROM there belongs to another machine
/// or another memory page), so what they say about those addresses is left
/// out. The program's own file applies in full.
pub fn load_program_annotations(
    dir: &Path,
    family: Family,
    own: Option<&str>,
    origin: u16,
    len: usize,
) -> Result<Annotations, String> {
    let mut ann = load_annotations(dir, family, None)?;
    let inside =
        |a: u16| (usize::from(origin)..usize::from(origin) + len).contains(&usize::from(a));
    ann.names.retain(|&a, _| !inside(a));
    ann.entries.retain(|&a| !inside(a));
    ann.layout.retain(|&(a, _, _)| !inside(a));
    ann.notes.retain(|&a, _| !inside(a));
    ann.heads.retain(|&a, _| !inside(a));
    if let Some(name) = own {
        let path = dir.join(name);
        if let Ok(src) = std::fs::read_to_string(&path) {
            ann.add(&src)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(ann)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unknown,
    /// First byte of an instruction.
    Code,
    /// Later bytes of an instruction.
    CodeCont,
    /// Word holding a code address.
    Ptr,
    Data,
    Text,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub code: usize,
    /// Part of `code` found by the linear scan.
    pub scanned: usize,
    pub pointers: usize,
    pub data: usize,
}

pub struct Listing {
    pub text: String,
    pub warnings: Vec<String>,
    pub stats: Stats,
}

struct Rom<'a> {
    data: &'a [u8],
    origin: u16,
    kind: Vec<Kind>,
    /// Offsets of instructions found by the linear scan.
    scanned: BTreeSet<usize>,
    /// Addresses used as branch, jump or call targets.
    code_refs: BTreeSet<u16>,
    warnings: Vec<String>,
}

fn is_unconditional(instr: Instr) -> bool {
    matches!(
        instr,
        Instr::Branch(Cond::Br, _)
            | Instr::Jmp(_)
            | Instr::Rts(_)
            | Instr::Rti
            | Instr::Rtt
            | Instr::Halt
    )
}

impl<'a> Rom<'a> {
    fn new(data: &'a [u8], origin: u16) -> Self {
        Rom {
            data,
            origin,
            kind: vec![Kind::Unknown; data.len()],
            scanned: BTreeSet::new(),
            code_refs: BTreeSet::new(),
            warnings: Vec::new(),
        }
    }

    fn contains(&self, addr: u16) -> bool {
        addr >= self.origin && usize::from(addr - self.origin) < self.data.len()
    }

    fn off(&self, addr: u16) -> usize {
        usize::from(addr - self.origin)
    }

    fn addr(&self, off: usize) -> u16 {
        self.origin + off as u16
    }

    /// Word at `addr`, or 0 outside the ROM.
    fn word(&self, addr: u16) -> u16 {
        if self.contains(addr) && self.contains(addr.wrapping_add(1)) {
            let o = self.off(addr);
            u16::from_le_bytes([self.data[o], self.data[o + 1]])
        } else {
            0
        }
    }

    fn insn_len(&self, addr: u16) -> usize {
        2 * usize::from(disassemble(addr, |a| self.word(a)).words)
    }

    fn mark(&mut self, off: usize, len: usize, kind: Kind) {
        for k in &mut self.kind[off..off + len] {
            *k = kind;
        }
    }

    /// Follow the program from `start`, marking instructions as code.
    fn trace(&mut self, start: u16, scanned: bool) {
        let mut stack = vec![start];
        while let Some(mut pc) = stack.pop() {
            loop {
                if !self.contains(pc) || pc & 1 != 0 {
                    break;
                }
                let off = self.off(pc);
                if self.kind[off] != Kind::Unknown {
                    break;
                }
                let instr = decode(self.word(pc));
                let len = self.insn_len(pc);
                if instr == Instr::Reserved
                    || off + len > self.data.len()
                    || self.kind[off..off + len]
                        .iter()
                        .any(|&k| k != Kind::Unknown)
                {
                    break;
                }
                self.kind[off] = Kind::Code;
                self.mark(off + 1, len - 1, Kind::CodeCont);
                if scanned {
                    self.scanned.insert(off);
                }

                let next = pc.wrapping_add(len as u16);
                let ext = pc.wrapping_add(2); // first extra word
                let mut falls_through = true;
                match instr {
                    Instr::Branch(c, o) => {
                        let t = next.wrapping_add((o as i16 as u16).wrapping_mul(2));
                        self.code_refs.insert(t);
                        stack.push(t);
                        falls_through = c != Cond::Br;
                    }
                    Instr::Sob(_, n) => {
                        let t = next.wrapping_sub(2 * u16::from(n));
                        self.code_refs.insert(t);
                        stack.push(t);
                    }
                    Instr::Jmp(d) | Instr::Jsr(_, d) => {
                        if let Some(t) = self.jump_target(ext, d, true) {
                            self.code_refs.insert(t);
                            stack.push(t);
                        }
                        // JSR with a link register other than PC is often
                        // followed by inline arguments, so don't assume code.
                        falls_through = matches!(instr, Instr::Jsr(7, _));
                    }
                    Instr::Rts(_)
                    | Instr::Rti
                    | Instr::Rtt
                    | Instr::Halt
                    | Instr::Start
                    | Instr::Step => falls_through = false,
                    // MOV #handler, @#vector: an interrupt handler.
                    Instr::Double(DoubleOp::Mov, false, s, d)
                        if s == Operand(0o27) && d == Operand(0o37) =>
                    {
                        let (value, vector) = (self.word(ext), self.word(ext + 2));
                        if vector < 0o400
                            && vector & 3 == 0
                            && self.contains(value)
                            && value & 1 == 0
                        {
                            self.code_refs.insert(value);
                            stack.push(value);
                        }
                    }
                    _ => {}
                }
                if !falls_through {
                    break;
                }
                pc = next;
            }
        }
    }

    /// The static destination of a JMP/JSR operand whose extra word is at
    /// `ext`. With `mark`, a pointer cell used by `@rel` becomes `Ptr`.
    fn jump_target(&mut self, ext: u16, d: Operand, mark: bool) -> Option<u16> {
        let x = self.word(ext);
        match (d.mode(), d.reg()) {
            (3, 7) => Some(x),
            (6, 7) => Some(ext.wrapping_add(2).wrapping_add(x)),
            (7, 7) => {
                let cell = ext.wrapping_add(2).wrapping_add(x);
                if !(self.contains(cell) && cell & 1 == 0 && self.contains(cell + 1)) {
                    return None;
                }
                let off = self.off(cell);
                if mark && self.kind[off..off + 2] == [Kind::Unknown; 2] {
                    self.mark(off, 2, Kind::Ptr);
                }
                (self.kind[off] == Kind::Ptr).then(|| self.word(cell))
            }
            _ => None,
        }
    }

    fn plausible_code_ptr(&self, v: u16) -> bool {
        if v & 1 != 0 || !self.contains(v) {
            return false;
        }
        let o = self.off(v);
        // Text decodes as instructions too easily; don't jump into it.
        let window = &self.data[o..(o + 16).min(self.data.len())];
        !looks_like_text(window)
            && match self.kind[o] {
                Kind::Code => true,
                Kind::Unknown => decode(self.word(v)) != Instr::Reserved,
                _ => false,
            }
    }

    /// Runs of 3 or more unclaimed words that all point at plausible code
    /// are jump tables. Returns the new code addresses.
    fn find_pointer_tables(&mut self) -> Vec<u16> {
        let mut found = Vec::new();
        let mut off = 0;
        while off + 1 < self.data.len() {
            let start = off;
            while off + 1 < self.data.len()
                && self.kind[off..off + 2] == [Kind::Unknown; 2]
                && self.plausible_code_ptr(self.word(self.addr(off)))
            {
                off += 2;
            }
            if (off - start) / 2 >= 3 {
                self.mark(start, off - start, Kind::Ptr);
                found.extend((start..off).step_by(2).map(|o| self.word(self.addr(o))));
            }
            if off == start {
                off += 2;
            }
        }
        found
    }

    /// Unclaimed runs, as word-aligned [start, end) offsets.
    fn unknown_runs(&self) -> Vec<(usize, usize)> {
        let mut runs = Vec::new();
        let mut off = 0;
        while off < self.data.len() {
            if self.kind[off] != Kind::Unknown {
                off += 1;
                continue;
            }
            let start = off;
            while off < self.data.len() && self.kind[off] == Kind::Unknown {
                off += 1;
            }
            let (s, e) = ((start + 1) & !1, off & !1);
            if e > s {
                runs.push((s, e));
            }
        }
        runs
    }

    /// Gaps that decode as at least two valid instructions ending in an
    /// unconditional transfer are probably code nobody jumps to directly
    /// (reached through computed jumps).
    fn scan_gaps(&self) -> Vec<u16> {
        let mut entries = Vec::new();
        for (start, end) in self.unknown_runs() {
            if looks_like_text(&self.data[start..end]) {
                continue;
            }
            let (mut off, mut n) = (start, 0);
            while off < end {
                let addr = self.addr(off);
                let instr = decode(self.word(addr));
                let len = self.insn_len(addr);
                if instr == Instr::Reserved || off + len > end {
                    break;
                }
                n += 1;
                off += len;
                if is_unconditional(instr) {
                    if n >= 2 && instr != Instr::Halt {
                        entries.push(self.addr(start));
                    }
                    break;
                }
            }
        }
        entries
    }
}

fn looks_like_text(bytes: &[u8]) -> bool {
    let printable = bytes.iter().filter(|&&b| koi8::decode(b).is_some()).count();
    bytes.len() >= 4 && printable * 4 >= bytes.len() * 3
}

/// Length of the text run starting at `i` (printable bytes, at least 4 of
/// them and at least 3 letters), or 0.
fn text_run(bytes: &[u8]) -> usize {
    let n = bytes
        .iter()
        .take_while(|&&b| koi8::decode(b).is_some())
        .count();
    let letters = bytes[..n]
        .iter()
        .filter(|&&b| koi8::decode(b).is_some_and(char::is_alphabetic))
        .count();
    if n >= 4 && letters >= 3 { n } else { 0 }
}

/// `.ASCII` operand for printable bytes, picking a delimiter not in the text.
fn ascii_operand(bytes: &[u8]) -> Option<String> {
    let text: String = bytes.iter().map(|&b| koi8::decode(b).unwrap()).collect();
    let delim = ['/', '"', '|', '\'', '!', '^', '~']
        .into_iter()
        .find(|d| !text.contains(*d))?;
    Some(format!("{delim}{text}{delim}"))
}

/// `.ASCII` operand for any bytes: printable runs delimited, others `<ooo>`.
fn ascii_with_escapes(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let n = bytes[i..]
            .iter()
            .take_while(|&&b| koi8::decode(b).is_some())
            .count();
        match (n > 0).then(|| ascii_operand(&bytes[i..i + n])).flatten() {
            Some(s) => {
                out.push_str(&s);
                i += n;
            }
            None => {
                out.push_str(&format!("<{:03o}>", bytes[i]));
                i += 1;
            }
        }
    }
    out
}

/// Interrupt vectors on the BK-0011M (BOS manual §1.4).
fn vector_name(v: u16) -> Option<&'static str> {
    Some(match v {
        0o4 => "bus error, HALT, STOP key",
        0o10 => "reserved instruction",
        0o14 => "BPT and T-bit",
        0o20 => "IOT",
        0o24 => "power fail",
        0o30 => "EMT",
        0o34 => "TRAP",
        0o60 => "keyboard",
        0o100 => "50 Hz timer",
        0o270 => "parallel port",
        0o274 => "keyboard: AR2 and function keys",
        _ => return None,
    })
}

const PAGE_MAP: [u16; 8] = [1, 5, 2, 3, 4, 7, 0, 6];

fn sel1_note(v: u16) -> String {
    if v & 0o4000 != 0 {
        let w1 = PAGE_MAP[usize::from(v >> 12) & 7];
        let w2 = match v & 0o33 {
            0 => format!("RAM page {}", PAGE_MAP[usize::from(v >> 8) & 7]),
            r if r & 1 != 0 => "ROM page 0".into(),
            r if r & 2 != 0 => "ROM page 1".into(),
            r if r & 0o10 != 0 => "ROM page 2".into(),
            _ => "ROM page 3".into(),
        };
        format!("paging: 040000 = RAM page {w1}, 100000 = {w2}")
    } else {
        let mut s = format!(
            "tape latch: motor {}",
            if v & 0o200 == 0 { "on" } else { "off" }
        );
        if v & 0o10000 != 0 {
            s.push_str(", STOP key locked");
        }
        s
    }
}

fn control_note(v: u16) -> String {
    format!(
        "palette {}, 50 Hz timer {}, show screen buffer {}",
        (v >> 8) & 15,
        if v & 0o40000 == 0 { "on" } else { "off" },
        v >> 15
    )
}

/// Disassemble one ROM image, starting at its first address.
pub fn disassemble_rom(
    data: &[u8],
    origin: u16,
    title: &str,
    source: &str,
    ann: &Annotations,
) -> Listing {
    disassemble_image(data, origin, &[origin], title, source, ann)
}

/// Disassemble a memory image loaded at `origin` into an annotated MACRO-11
/// listing, following the code from `starts` (plus annotated entries).
pub fn disassemble_image(
    data: &[u8],
    origin: u16,
    starts: &[u16],
    title: &str,
    source: &str,
    ann: &Annotations,
) -> Listing {
    let mut rom = Rom::new(data, origin);

    // Annotated layout first, so tracing respects it.
    let mut entries = starts.to_vec();
    for &(addr, layout, count) in &ann.layout {
        let bytes = usize::from(count) * if layout == Layout::Text { 1 } else { 2 };
        if !rom.contains(addr) || rom.off(addr) + bytes > data.len() {
            rom.warnings
                .push(format!("{addr:06o}: annotated range is outside the ROM"));
            continue;
        }
        let off = rom.off(addr);
        let kind = match layout {
            Layout::Ptrs => Kind::Ptr,
            Layout::Data => Kind::Data,
            Layout::Text => Kind::Text,
        };
        rom.mark(off, bytes, kind);
        if layout == Layout::Ptrs {
            entries.extend((0..count).map(|i| rom.word(addr + 2 * i)));
        }
    }
    entries.extend(ann.entries.iter().copied().filter(|&a| rom.contains(a)));

    // Follow the program, then jump tables, until nothing new turns up.
    loop {
        for e in entries.drain(..) {
            rom.code_refs.insert(e);
            rom.trace(e, false);
        }
        entries = rom.find_pointer_tables();
        if entries.is_empty() {
            break;
        }
    }
    // Then the linear scan of what's left.
    loop {
        let found = rom.scan_gaps();
        if found.is_empty() {
            break;
        }
        for e in found {
            rom.trace(e, true);
        }
        for e in rom.find_pointer_tables() {
            rom.code_refs.insert(e);
            rom.trace(e, true);
        }
    }
    for k in &mut rom.kind {
        if *k == Kind::Unknown {
            *k = Kind::Data;
        }
    }
    for &e in &ann.entries {
        if rom.contains(e) && rom.kind[rom.off(e)] != Kind::Code {
            rom.warnings.push(format!(
                "{e:06o}: annotated code entry is not an instruction"
            ));
        }
    }

    let labels = make_labels(&mut rom, ann);
    let externals: BTreeMap<u16, String> = ann
        .names
        .iter()
        .filter(|(a, _)| !rom.contains(**a))
        .map(|(a, n)| (*a, n.clone()))
        .collect();

    let text = emit(&rom, ann, title, source, &labels, &externals);
    let stats = Stats {
        code: rom
            .kind
            .iter()
            .filter(|k| matches!(k, Kind::Code | Kind::CodeCont))
            .count(),
        scanned: rom.scanned.iter().map(|&o| rom.insn_len(rom.addr(o))).sum(),
        pointers: rom.kind.iter().filter(|&&k| k == Kind::Ptr).count(),
        data: rom
            .kind
            .iter()
            .filter(|k| matches!(k, Kind::Data | Kind::Text))
            .count(),
    };
    Listing {
        text,
        warnings: rom.warnings,
        stats,
    }
}

/// Decide which addresses get labels. Labels sit only where a listing line
/// can start: an instruction, a pointer word, or any data byte.
fn make_labels(rom: &mut Rom, ann: &Annotations) -> BTreeMap<u16, String> {
    let line_start = |rom: &Rom, a: u16| {
        rom.contains(a)
            && match rom.kind[rom.off(a)] {
                Kind::Code | Kind::Data | Kind::Text => true,
                Kind::Ptr => (a - rom.origin).is_multiple_of(2),
                _ => false,
            }
    };
    let mut labels = BTreeMap::new();
    for (&a, name) in &ann.names {
        if rom.contains(a) {
            if line_start(rom, a) {
                labels.insert(a, name.clone());
            } else {
                rom.warnings.push(format!(
                    "{a:06o}: can't place label {name} inside an instruction"
                ));
            }
        }
    }
    for &t in &rom.code_refs {
        if rom.contains(t) && rom.kind[rom.off(t)] == Kind::Code {
            labels.entry(t).or_insert_with(|| format!("L{t:06o}"));
        }
    }
    // Jump tables.
    let mut prev = Kind::Unknown;
    for off in 0..rom.data.len() {
        let k = rom.kind[off];
        if k == Kind::Ptr && prev != Kind::Ptr {
            let a = rom.addr(off);
            labels.entry(a).or_insert_with(|| format!("T{a:06o}"));
        }
        prev = k;
    }
    // Data the code refers to.
    let mut data_refs = Vec::new();
    for off in 0..rom.data.len() {
        if rom.kind[off] == Kind::Code {
            disassemble_with(
                rom.addr(off),
                |a| rom.word(a),
                |v, r| {
                    if r != Ref::Immediate || v != rom.origin {
                        data_refs.push(v);
                    }
                    None
                },
            );
        }
    }
    for v in data_refs {
        if rom.contains(v) && matches!(rom.kind[rom.off(v)], Kind::Data | Kind::Text) {
            let o = rom.off(v);
            let prefix = if text_run(&rom.data[o..]) > 0 {
                'S'
            } else {
                'D'
            };
            labels
                .entry(v)
                .or_insert_with(|| format!("{prefix}{v:06o}"));
        }
    }
    labels
}

fn emit(
    rom: &Rom,
    ann: &Annotations,
    title: &str,
    source: &str,
    labels: &BTreeMap<u16, String>,
    externals: &BTreeMap<u16, String>,
) -> String {
    let mut out = String::new();
    let o = &mut out;
    let end = rom.origin as u32 + rom.data.len() as u32 - 1;
    let _ = writeln!(o, "; {title}");
    let _ = writeln!(
        o,
        "; {source}: {} bytes at {:06o}-{end:06o}.",
        rom.data.len(),
        rom.origin
    );
    let _ = writeln!(o, ";");
    let _ = writeln!(
        o,
        "; Generated by romdis. Assembling this file at {:06o} with the",
        rom.origin
    );
    let _ = writeln!(
        o,
        "; project's assembler (vm1::asm) reproduces the ROM byte for byte."
    );
    let _ = writeln!(
        o,
        "; Labels: Lnnnnnn code, Tnnnnnn jump table, Dnnnnnn data, Snnnnnn text,"
    );
    let _ = writeln!(
        o,
        "; where nnnnnn is the address. Other names come from annotations/*.ann."
    );
    let _ = writeln!(
        o,
        "; \"(scan)\" marks code found by a linear scan rather than by following"
    );
    let _ = writeln!(o, "; the program; it may really be data.");
    let _ = writeln!(o);

    let symbol = |v: u16, r: Ref| -> Option<String> {
        if let Some(name) = labels.get(&v)
            && (matches!(r, Ref::Target | Ref::Absolute) || v != rom.origin)
        {
            return Some(name.clone());
        }
        if !matches!(r, Ref::Target | Ref::Absolute) {
            return None;
        }
        externals.get(&v).cloned().or_else(|| {
            let base = externals.get(&(v.wrapping_sub(1)))?;
            (v & 1 == 1).then(|| format!("{base}+1"))
        })
    };

    if !externals.is_empty() {
        let _ = writeln!(o, "; Symbols outside this ROM");
        for (v, name) in externals {
            let note = ann.notes.get(v).and_then(|n| n.first());
            let line = format!("{name:<8}= {v:06o}");
            match note {
                Some(n) => {
                    let _ = writeln!(o, "{line:<41}; {n}");
                }
                None => {
                    let _ = writeln!(o, "{line}");
                }
            }
        }
        let _ = writeln!(o);
    }
    let _ = writeln!(o, "\t. = {:06o}", rom.origin);

    let line = |o: &mut String, addr: u16, label: Option<&String>, body: &str, notes: &[String]| {
        if let Some(h) = ann.heads.get(&addr) {
            let _ = writeln!(o);
            for h in h {
                let _ = writeln!(o, "; {h}");
            }
        }
        let label = match label {
            Some(l) if l.len() < 8 => format!("{l}:"),
            Some(l) => {
                let _ = writeln!(o, "{l}:");
                String::new()
            }
            None => String::new(),
        };
        let mut comment = format!("{addr:06o}");
        for n in notes {
            comment.push_str("  ");
            comment.push_str(n);
        }
        let _ = writeln!(o, "{label:<9}{body:<32}; {comment}");
    };

    let mut off = 0;
    while off < rom.data.len() {
        let addr = rom.addr(off);
        let label = labels.get(&addr);
        let mut notes: Vec<String> = Vec::new();
        match rom.kind[off] {
            Kind::Code => {
                let d = disassemble_with(addr, |a| rom.word(a), symbol);
                if rom.scanned.contains(&off) {
                    notes.push("(scan)".into());
                }
                notes.extend(auto_notes(rom, ann, addr));
                notes.extend(ann.notes.get(&addr).cloned().unwrap_or_default());
                line(&mut out, addr, label, &d.text, &notes);
                off += 2 * usize::from(d.words);
            }
            Kind::Ptr => {
                let target = rom.word(addr);
                let name = labels
                    .get(&target)
                    .cloned()
                    .unwrap_or_else(|| format!("{target:06o}"));
                // The slot's own note if it has one, else the target's.
                match ann.notes.get(&addr) {
                    Some(own) => notes.extend(own.iter().cloned()),
                    None => notes.extend(ann.notes.get(&target).and_then(|n| n.first()).cloned()),
                }
                line(&mut out, addr, label, &format!(".WORD  {name}"), &notes);
                off += 2;
            }
            _ => {
                // A data run ends at code, a pointer, a label or a head comment.
                let mut end = off + 1;
                while end < rom.data.len()
                    && matches!(rom.kind[end], Kind::Data | Kind::Text)
                    && !labels.contains_key(&rom.addr(end))
                    && !ann.heads.contains_key(&rom.addr(end))
                    && !ann.notes.contains_key(&rom.addr(end))
                {
                    end += 1;
                }
                let notes = ann.notes.get(&addr).cloned().unwrap_or_default();
                emit_data(rom, off, end, label, &notes, &mut |a, l, b, n| {
                    line(&mut out, a, l, b, n)
                });
                off = end;
            }
        }
    }
    let _ = writeln!(out, "\t.END");
    out
}

/// Writes one listing line: address, label, body, notes.
type LineSink<'a> = dyn FnMut(u16, Option<&String>, &str, &[String]) + 'a;

/// Data lines for [start, end): text as .ASCII, the rest as .WORD/.BYTE.
fn emit_data(
    rom: &Rom,
    start: usize,
    end: usize,
    label: Option<&String>,
    notes: &[String],
    line: &mut LineSink,
) {
    let mut i = start;
    let mut first = true;
    let mut emit = |addr: u16, body: &str| {
        let (l, n): (Option<&String>, &[String]) = if first { (label, notes) } else { (None, &[]) };
        first = false;
        line(addr, l, body, n);
    };
    while i < end {
        if rom.kind[i] == Kind::Text {
            let mut j = i;
            while j < end && rom.kind[j] == Kind::Text {
                j += 1;
            }
            for chunk in rom.data[i..j].chunks(32) {
                emit(
                    rom.addr(i),
                    &format!(".ASCII {}", ascii_with_escapes(chunk)),
                );
                i += chunk.len();
            }
            continue;
        }
        let run = text_run(&rom.data[i..end]);
        if run > 0 {
            for chunk in rom.data[i..i + run].chunks(40) {
                let body = match ascii_operand(chunk) {
                    Some(s) => format!(".ASCII {s}"),
                    None => bytes_line(chunk),
                };
                emit(rom.addr(i), &body);
                i += chunk.len();
            }
            continue;
        }
        // Up to the next text run.
        let mut j = i + 1;
        while j < end && text_run(&rom.data[j..end]) == 0 {
            j += 1;
        }
        let mut k = i;
        if k % 2 == 1 {
            emit(rom.addr(k), &bytes_line(&rom.data[k..k + 1]));
            k += 1;
        }
        while k + 1 < j {
            let n = ((j - k) / 2).min(4);
            let words: Vec<String> = (0..n)
                .map(|w| format!("{:06o}", rom.word(rom.addr(k + 2 * w))))
                .collect();
            emit(rom.addr(k), &format!(".WORD  {}", words.join(", ")));
            k += 2 * n;
        }
        if k < j {
            emit(rom.addr(k), &bytes_line(&rom.data[k..j]));
        }
        i = j;
    }
}

fn bytes_line(bytes: &[u8]) -> String {
    let b: Vec<String> = bytes.iter().map(|b| format!("{b:03o}")).collect();
    format!(".BYTE  {}", b.join(", "))
}

/// Comments worked out from the instruction itself.
fn auto_notes(rom: &Rom, ann: &Annotations, addr: u16) -> Vec<String> {
    let mut notes = Vec::new();
    let ext = addr.wrapping_add(2);
    match decode(rom.word(addr)) {
        Instr::Emt(n) => {
            if let Some((name, desc)) = ann.emts.get(&n) {
                notes.push(if desc.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {desc}")
                });
            }
        }
        Instr::Jmp(d) | Instr::Jsr(_, d) => {
            // Describe the destination (or the pointer slot) if annotated.
            let x = rom.word(ext);
            let slot = match (d.mode(), d.reg()) {
                (3, 7) => Some(x),
                (6, 7) | (7, 7) => Some(ext.wrapping_add(2).wrapping_add(x)),
                _ => None,
            };
            if let Some(n) = slot.and_then(|s| ann.notes.get(&s)).and_then(|n| n.first()) {
                notes.push(n.clone());
            }
        }
        Instr::Double(DoubleOp::Mov, false, s, d) if s == Operand(0o27) && d == Operand(0o37) => {
            let (value, dst) = (rom.word(ext), rom.word(ext + 2));
            match dst {
                0o177716 => notes.push(sel1_note(value)),
                0o177662 => notes.push(control_note(value)),
                v if v < 0o400 => {
                    if let Some(name) = vector_name(v) {
                        notes.push(format!("vector {v:o}: {name}"));
                    } else if let Some(name) = vector_name(v.wrapping_sub(2)) {
                        notes.push(format!("vector {:o} PSW ({name})", v - 2));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    notes
}

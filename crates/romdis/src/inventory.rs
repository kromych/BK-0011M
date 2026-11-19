//! Everything `romdis` lists, in one place for the tool and the tests:
//!
//! - the ROMs in `roms/` ([`ROMS`]), and the extra ROMs inside the MiST
//!   core's combined image ([`MIST_ROMS`]);
//! - the programs under `software/`, in folders too: `.bin` files, the
//!   programs on disk images (ANDOS and CSIDOS file systems, see
//!   [`crate::disk`]) and their boot blocks, and every file the BK-0010
//!   monitor ROM reads from a tape recording (`.wav` or `.mp3`).

use std::path::{Path, PathBuf};

use crate::annotations::Annotations;
use crate::disk::{self, System};
use crate::{Family, ROMS, parse_bin, start_address};

/// One image to list.
#[derive(Clone, Debug)]
pub struct Item {
    /// The listing's path under the output directory (ending in `.mac`).
    pub out: PathBuf,
    pub title: String,
    /// What the listing was made from, for its header.
    pub source: String,
    pub origin: u16,
    pub data: Vec<u8>,
    /// Where execution begins.
    pub starts: Vec<u16>,
    pub family: Family,
    /// The item's own annotation file, relative to the annotation directory.
    pub annotations: Option<String>,
    /// A program in RAM rather than a ROM (see
    /// [`crate::load_program_annotations`]).
    pub program: bool,
}

impl Item {
    /// The annotations to list the item with.
    pub fn load_annotations(&self, dir: &Path) -> Result<Annotations, String> {
        if self.program {
            crate::load_program_annotations(
                dir,
                self.family,
                self.annotations.as_deref(),
                self.origin,
                self.data.len(),
            )
        } else {
            crate::load_annotations(dir, self.family, self.annotations.as_deref())
        }
    }
}

/// What [`roms`] and [`software`] found.
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: Vec<Item>,
    /// Things skipped, and why.
    pub notes: Vec<String>,
    /// Text files to write beside the listings (a disk's or tape's
    /// contents), as (path under the output directory, text).
    pub indexes: Vec<(PathBuf, String)>,
}

/// A piece of a combined ROM image.
pub struct RomSlice {
    pub file: &'static str,
    pub offset: usize,
    pub len: usize,
    pub origin: u16,
    pub title: &'static str,
    pub family: Family,
    /// The listing's name.
    pub name: &'static str,
}

/// The MiST FPGA core's ROMs that aren't among the stock ones (where they
/// sit is in the core's `memory.sv`: the image is its memory from
/// 0xE0000). The rest of the image is the stock ROMs.
pub const MIST_ROMS: &[RomSlice] = &[
    RomSlice {
        file: "mist/bk0011m.rom",
        offset: 0xc000,
        len: 8248,
        origin: 0o100000,
        title: "MiST core: debugger, BK-0011M ROM page 13 at 100000 (started by 13;1C 100000G)",
        family: Family::Bk0011m,
        name: "mist_debugger",
    },
    RomSlice {
        file: "mist/bk0011m.rom",
        offset: 0x12000,
        len: 216,
        origin: 0o160000,
        title: "MiST core: its default disk ROM at 160000",
        family: Family::Bk0011m,
        name: "mist_disk",
    },
    RomSlice {
        file: "mist/bk0011m.rom",
        offset: 0x1c000,
        len: 4096,
        origin: 0o170000,
        title: "MiST core: the A16M extension's ROM (BK-0010), 170000-177777",
        family: Family::Bk0010,
        name: "mist_a16m",
    },
    RomSlice {
        file: "mist/bk0011m.rom",
        offset: 0x1d000,
        len: 4096,
        origin: 0o170000,
        title: "MiST core: the SMK512 extension's ROM-BIOS (BK-0011M), 170000-177777",
        family: Family::Bk0011m,
        name: "mist_smk512",
    },
];

/// The stock ROMs and the MiST core's extra ones found in `dir`.
pub fn roms(dir: &Path) -> Inventory {
    let mut inv = Inventory::default();
    for spec in ROMS {
        let Ok(data) = std::fs::read(dir.join(spec.file)) else {
            inv.notes.push(format!("{}: not found, skipped", spec.file));
            continue;
        };
        inv.items.push(Item {
            out: Path::new(spec.file).with_extension("mac"),
            title: spec.title.into(),
            source: spec.file.into(),
            origin: spec.origin,
            data,
            starts: vec![spec.origin],
            family: spec.family,
            annotations: Some(spec.annotations.into()),
            program: false,
        });
    }
    for slice in MIST_ROMS {
        let data = std::fs::read(dir.join(slice.file)).ok();
        let Some(data) = data
            .as_ref()
            .and_then(|d| d.get(slice.offset..slice.offset + slice.len))
        else {
            inv.notes
                .push(format!("{}: not found, {} skipped", slice.file, slice.name));
            continue;
        };
        inv.items.push(Item {
            out: PathBuf::from(format!("{}.mac", slice.name)),
            title: slice.title.into(),
            source: format!("{} at {:#x}", slice.file, slice.offset),
            origin: slice.origin,
            data: data.to_vec(),
            starts: vec![slice.origin],
            family: slice.family,
            annotations: Some(format!("{}.ann", slice.name)),
            program: false,
        });
    }
    inv
}

/// The programs under `dir` (see the module notes). Tapes are read with the
/// BK-0010 ROMs from `roms_dir`; `ann_dir` gives each program's machine
/// (`machine bk0011m` in its own `.ann` file, or its folder's or disk's).
pub fn software(dir: &Path, roms_dir: &Path, ann_dir: &Path) -> Inventory {
    let mut inv = Inventory::default();
    let mut files = Vec::new();
    walk(dir, &mut files);
    files.sort();
    for path in files {
        let rel = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
        let ext = rel
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let result = match ext.as_str() {
            "bin" => bin_program(&path, &rel, ann_dir, &mut inv),
            "dsk" | "img" | "bkd" => disk_programs(&path, &rel, ann_dir, &mut inv),
            "wav" | "mp3" => tape_programs(&path, &rel, roms_dir, ann_dir, &mut inv),
            _ => Ok(()),
        };
        if let Err(e) = result {
            inv.notes.push(format!("{}: {e}", rel.display()));
        }
    }
    unpacked(dir, roms_dir, ann_dir, &mut inv);
    inv
}

/// A program packed on its disk: the emulator boots the disk, starts the
/// program with `keys` (frames to run, then text to type; the last entry
/// is typed with a breakpoint at `entry`), stops when the unpacker jumps to
/// `entry`, and lists the memory it filled: `low` (from 000000) and RAM
/// `pages` at the addresses the program maps them to.
struct Unpacked {
    disk: &'static str,
    name: &'static str,
    keys: &'static [(u32, &'static str)],
    entry: u16,
    low: usize,
    pages: &'static [(usize, u16)],
}

const UNPACKED: &[Unpacked] = &[Unpacked {
    disk: "sabot2-v44.dsk",
    name: "SABOT2 unpacked",
    // The ANDOS title, its file manager, down to SABOT2, Enter.
    keys: &[(900, " "), (600, "\x1b"), (50, "\n")],
    entry: 0o1600,
    // Its loader unpacks into low memory, page 3 (at 040000) and pages
    // 1, 2 and 7 (each at 100000). Pages 2 and 7 hold no code (graphics,
    // by the look of them), page 4 is ANDOS, 5 and 6 the screens.
    low: 0o40000,
    pages: &[(3, 0o40000), (1, 0o100000)],
}];

/// Run a packed program to its entry and list what it unpacked.
fn unpacked(dir: &Path, roms_dir: &Path, ann_dir: &Path, inv: &mut Inventory) {
    for u in UNPACKED {
        let Ok(image) = std::fs::read(dir.join(u.disk)) else {
            continue;
        };
        let roms = match bk::Roms::load_bk0011m(roms_dir, true) {
            Ok(roms) => roms,
            Err(e) => {
                inv.notes
                    .push(format!("{}: needs the BK-0011M ROMs: {e}", u.disk));
                continue;
            }
        };
        let config = bk::Config {
            fdd: true,
            ..Default::default()
        };
        let mut m = bk::Machine::new(roms, config);
        if let Err(e) = m.insert_disk(0, bk::DiskImage::new(image, true)) {
            inv.notes.push(format!("{}: {e}", u.disk));
            continue;
        }
        for (i, &(frames, keys)) in u.keys.iter().enumerate() {
            for _ in 0..frames {
                m.run_frame();
            }
            if i + 1 == u.keys.len() {
                m.set_breakpoints(&[u.entry]);
            }
            m.type_text(keys);
        }
        let reached = (0..3000).any(|_| m.run_frame().is_some()) && m.cpu().pc() == u.entry;
        if !reached {
            inv.notes.push(format!(
                "{}: {} never reached {:06o}",
                u.disk, u.name, u.entry
            ));
            continue;
        }
        let stem = stem(Path::new(u.disk));
        let family = machine(ann_dir, &candidates(&stem)).unwrap_or(Family::Bk0011m);
        let out_dir = Path::new("software").join(&stem).join("unpacked");
        let word_bytes = |w: Option<u16>| w.unwrap_or(0).to_le_bytes();
        let low: Vec<u8> = (0..u.low as u16)
            .step_by(2)
            .flat_map(|a| word_bytes(m.peek_page_word(0, a)))
            .collect();
        // Low memory from 1000: below are the vectors and the system's cells.
        let mut parts = vec![("low".to_string(), 0o1000u16, low[0o1000..].to_vec())];
        for &(page, at) in u.pages {
            let data: Vec<u8> = (0..0o40000u16)
                .step_by(2)
                .flat_map(|o| word_bytes(m.peek_page_word(page, o)))
                .collect();
            parts.push((format!("page{page}"), at, data));
        }
        for (part, origin, data) in parts {
            inv.items.push(Item {
                out: out_dir.join(format!("{part}.mac")),
                title: format!(
                    "{}: {part} as it is when the unpacker jumps to {:06o} (at {origin:06o})",
                    u.name, u.entry
                ),
                source: format!("{}, run in the emulator to {:06o}", u.disk, u.entry),
                origin,
                starts: if part == "low" {
                    vec![u.entry]
                } else {
                    Vec::new()
                },
                data,
                family,
                annotations: Some(format!("software/{stem}/unpacked/{part}.ann")),
                program: true,
            });
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// `rel` without its extension, `/`-separated.
fn stem(rel: &Path) -> String {
    rel.with_extension("").to_string_lossy().replace('\\', "/")
}

/// A name made safe for a file name on any system.
fn safe(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"\:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() { "_".into() } else { s }
}

/// The machine named by the first of `candidates` (annotation files) that
/// names one. A file that doesn't parse is reported and passed over.
fn machine(ann_dir: &Path, candidates: &[String]) -> Option<Family> {
    candidates.iter().find_map(|name| {
        let src = std::fs::read_to_string(ann_dir.join(name)).ok()?;
        match Annotations::parse(&src) {
            Ok(ann) => ann.machine,
            Err(e) => {
                eprintln!("romdis: {name}: {e}");
                None
            }
        }
    })
}

/// `software/<stem>.ann` for a path and for each folder above it.
fn candidates(stem: &str) -> Vec<String> {
    let mut out = vec![format!("software/{stem}.ann")];
    let mut parts: Vec<&str> = stem.split('/').collect();
    while parts.len() > 1 {
        parts.pop();
        out.push(format!("software/{}.ann", parts.join("/")));
    }
    out
}

fn bin_program(path: &Path, rel: &Path, ann_dir: &Path, inv: &mut Inventory) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (address, data) = parse_bin(&bytes)?;
    let stem = stem(rel);
    let family = machine(ann_dir, &candidates(&stem)).unwrap_or(Family::Bk0010);
    let name = rel.file_name().unwrap().to_string_lossy().into_owned();
    inv.items.push(Item {
        out: Path::new("software").join(rel).with_extension("mac"),
        title: format!("{name}: program for the {family:?}"),
        source: rel.to_string_lossy().replace('\\', "/"),
        origin: address,
        data: data.to_vec(),
        starts: vec![start_address(address)],
        family,
        annotations: Some(format!("software/{stem}.ann")),
        program: true,
    });
    Ok(())
}

fn disk_programs(
    path: &Path,
    rel: &Path,
    ann_dir: &Path,
    inv: &mut Inventory,
) -> Result<(), String> {
    let image = std::fs::read(path).map_err(|e| e.to_string())?;
    let stem = stem(rel);
    let source = rel.to_string_lossy().replace('\\', "/");
    let Some((system, files)) = disk::read(&image) else {
        return Err("no ANDOS or CSIDOS file system found".into());
    };
    let default = match system {
        System::Andos => Family::Bk0010,
        System::Csidos => Family::Bk0011m,
    };
    let disk_family = machine(ann_dir, &candidates(&stem)).unwrap_or(default);
    let out_dir = Path::new("software").join(&stem);
    if let Some(boot) = disk::boot_block(&image) {
        inv.items.push(Item {
            out: out_dir.join("boot.mac"),
            title: format!("{source}: the {system:?} boot block, loaded at 1000 by the floppy ROM"),
            source: format!("{source}, block 0"),
            origin: 0o1000,
            data: boot.to_vec(),
            starts: vec![0o1000],
            family: disk_family,
            annotations: Some(format!("software/{stem}/boot.ann")),
            program: true,
        });
    }
    let mut index = format!("{source}: {system:?} disk, {} files\n\n", files.len());
    for f in &files {
        let fits = usize::from(f.address) + f.data.len() <= 0x10000;
        let why = match f.not_code {
            Some(why) => Some(why),
            None if f.data.is_empty() => Some("empty"),
            None if !fits => Some("runs past 177777 at its load address"),
            None => None,
        };
        index += &format!(
            "{:<40} {:06o} {:>6} bytes  {}{}\n",
            f.path,
            f.address,
            f.size,
            why.unwrap_or("listed"),
            f.note.map(|n| format!(" ({n})")).unwrap_or_default()
        );
        if why.is_some() {
            continue;
        }
        let own = format!("{stem}/{}", f.path);
        let family = machine(ann_dir, &candidates(&own)).unwrap_or(disk_family);
        let out = f.path.split('/').map(safe).collect::<Vec<_>>().join("/");
        inv.items.push(Item {
            out: out_dir.join(format!("{out}.mac")),
            title: format!(
                "{}: program on {source} ({system:?}), for the {family:?}",
                f.path
            ),
            source: format!("{source}: {}", f.path),
            origin: f.address,
            data: f.data.clone(),
            starts: vec![f.entry.unwrap_or(start_address(f.address))],
            family,
            annotations: Some(format!("software/{own}.ann")),
            program: true,
        });
    }
    inv.indexes.push((out_dir.join("files.txt"), index));
    Ok(())
}

fn tape_programs(
    path: &Path,
    rel: &Path,
    roms_dir: &Path,
    ann_dir: &Path,
    inv: &mut Inventory,
) -> Result<(), String> {
    let roms = bk::Roms::load_bk0010(roms_dir, bk::Bk0010Rom::Basic)
        .map_err(|e| format!("needs the BK-0010 ROMs to read the tape: {e}"))?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let wav = bk::parse_recording(bytes)?;
    let files = bk::read_tape_files(roms, bk::TapeSignal::from_wav(&wav))?;
    let stem = stem(rel);
    let source = rel.to_string_lossy().replace('\\', "/");
    let out_dir = Path::new("software").join(&stem);
    let mut index = format!(
        "{source}: {} files read by the BK-0010 monitor ROM\n\n",
        files.len()
    );
    let mut used: Vec<String> = Vec::new();
    for f in files {
        let mut name = safe(&f.name);
        let base = name.clone();
        let mut n = 1;
        while used.contains(&name) {
            n += 1;
            name = format!("{base}~{n}");
        }
        used.push(name.clone());
        let state = if !f.complete {
            format!(
                ", the recording ends inside it: the first {} of {} bytes",
                f.file.data.len(),
                f.length
            )
        } else if f.checksum_ok {
            String::new()
        } else {
            ", checksum failed: as the ROM read it".into()
        };
        index += &format!(
            "{:<20} {:06o} {:>6} bytes  {}\n",
            name,
            f.file.address,
            f.file.data.len(),
            if !f.complete {
                format!("cut off ({} bytes in the header)", f.length)
            } else if f.checksum_ok {
                "checksum OK".into()
            } else {
                "checksum failed".into()
            }
        );
        if f.file.data.is_empty() || usize::from(f.file.address) + f.file.data.len() > 0x10000 {
            continue;
        }
        let own = format!("{stem}/{name}");
        let family = machine(ann_dir, &candidates(&own)).unwrap_or(Family::Bk0010);
        inv.items.push(Item {
            out: out_dir.join(format!("{name}.mac")),
            title: format!("{}: from the tape {source}{state}", f.name),
            source: format!("{source}: {}", f.name),
            origin: f.file.address,
            starts: vec![start_address(f.file.address)],
            data: f.file.data,
            family,
            annotations: Some(format!("software/{own}.ann")),
            program: true,
        });
    }
    inv.indexes.push((out_dir.join("files.txt"), index));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_annotation_lookup() {
        assert_eq!(safe("R&O-REKL1"), "R&O-REKL1");
        assert_eq!(safe("a?b:c."), "a_b_c");
        assert_eq!(safe(""), "_");
        assert_eq!(
            candidates("ANDOS330/Утилиты/ANSETUP"),
            [
                "software/ANDOS330/Утилиты/ANSETUP.ann",
                "software/ANDOS330/Утилиты.ann",
                "software/ANDOS330.ann"
            ]
        );
    }
}

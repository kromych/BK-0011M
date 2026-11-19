//! The files on BK floppy disk images (800 KB, 512-byte blocks), for
//! listing the programs on them.
//!
//! - **ANDOS**: an MS-DOS FAT12 file system. The BK load address is kept in
//!   the directory entry's time field (177777: not loaded as it is). ANDOS
//!   3.x groups files into folders with volume-label entries: a label's
//!   byte 20 is its folder number and byte 21 its parent's; a file's byte 21
//!   is its folder (0 and 1, the volume's own label, are the top).
//! - **CSIDOS**: its own file system. Directory blocks follow from block 2,
//!   each a 12-byte header (word 0: the block's own number) and 25 entries of
//!   20 bytes: the parent folder (word), the name (8 bytes) and extension
//!   (3), a byte (a folder's own number), then the start block, load address
//!   and length in bytes. A folder has neither start nor length.
//!
//! Both systems' boot blocks are loaded at 1000 by the floppy ROM.

use std::collections::HashMap;

const BLOCK: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Andos,
    Csidos,
}

/// A file on a disk.
#[derive(Clone, Debug)]
pub struct DiskFile {
    /// Folders and name, `/`-separated (`Утилиты/ANSETUP`, `TOOLS/cs.exe`).
    pub path: String,
    /// Load address.
    pub address: u16,
    /// Where execution begins, if it isn't the load address.
    pub entry: Option<u16>,
    /// What is listed: the file, or the part of it loaded at `address`.
    pub data: Vec<u8>,
    /// The file's size.
    pub size: usize,
    /// Why the file isn't listed as a program (`None`: it is).
    pub not_code: Option<&'static str>,
    /// Anything else to say about it.
    pub note: Option<&'static str>,
}

/// The file system and files of a disk image, if it has one of the known
/// file systems.
pub fn read(image: &[u8]) -> Option<(System, Vec<DiskFile>)> {
    if let Some(files) = andos(image) {
        return Some((System::Andos, files));
    }
    csidos(image).map(|files| (System::Csidos, files))
}

/// The boot block, loaded at 1000 and started there when its first word is
/// 000240 (NOP).
pub fn boot_block(image: &[u8]) -> Option<&[u8]> {
    let block = image.get(..BLOCK)?;
    (block[..2] == [0o240, 0]).then_some(block)
}

fn word(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

/// KOI-8 text with trailing spaces and NULs removed.
fn koi8(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .rposition(|&b| b != 0 && b != b' ')
        .map_or(0, |i| i + 1);
    bytes[..end]
        .iter()
        .map(|&b| vm1::koi8::decode(b).unwrap_or('?'))
        .collect()
}

fn file_name(name: &[u8], ext: &[u8]) -> String {
    let (name, ext) = (koi8(name), koi8(ext));
    let name = name.trim_end_matches('.').to_string();
    if ext.is_empty() {
        name
    } else {
        format!("{name}.{ext}")
    }
}

/// Extensions that ANDOS disks use for text, fonts, archives, compressed
/// data and PC programs.
const ANDOS_NOT_CODE: &[&str] = &[
    "VXT", "DOC", "EDP", "TXT", "HLP", "BKF", "ARJ", "LZS", "COM", "EXE",
];

fn andos(image: &[u8]) -> Option<Vec<DiskFile>> {
    let b = image.get(..BLOCK)?;
    let (bps, spc, reserved, fats) = (
        usize::from(word(b, 11)),
        usize::from(b[13]),
        usize::from(word(b, 14)),
        usize::from(b[16]),
    );
    let (root_entries, fat_blocks) = (usize::from(word(b, 17)), usize::from(word(b, 22)));
    if bps != BLOCK || spc == 0 || fats == 0 || fat_blocks == 0 || b[21] != 0xf9 && b[21] != 0xf8 {
        return None;
    }
    let fat = image.get(reserved * BLOCK..(reserved + fat_blocks) * BLOCK)?;
    let root = (reserved + fats * fat_blocks) * BLOCK;
    let data_start = root + (root_entries * 32).div_ceil(BLOCK) * BLOCK;
    let cluster = spc * BLOCK;
    let next = |c: usize| -> usize {
        let v = usize::from(word(fat, c * 3 / 2));
        if c & 1 == 1 { v >> 4 } else { v & 0xfff }
    };
    let read_file = |first: usize, size: usize| -> Vec<u8> {
        let mut out = Vec::with_capacity(size);
        let mut c = first;
        while (2..0xff0).contains(&c) && out.len() < size && c * 3 / 2 + 1 < fat.len() {
            let at = data_start + (c - 2) * cluster;
            out.extend_from_slice(image.get(at..at + cluster).unwrap_or(&[]));
            c = next(c);
        }
        out.truncate(size);
        out
    };
    let entries: Vec<&[u8]> = image
        .get(root..root + root_entries * 32)?
        .chunks(32)
        .take_while(|e| e[0] != 0)
        .filter(|e| e[0] != 0xe5)
        .collect();
    // Folders: number -> (name, parent).
    let folders: HashMap<u8, (String, u8)> = entries
        .iter()
        .filter(|e| e[11] & 0x08 != 0 && e[20] > 1)
        .map(|e| (e[20], (koi8(&e[..11]).replace("  ", " "), e[21])))
        .collect();
    let folder_path = |mut n: u8| -> String {
        let mut parts = Vec::new();
        while let Some((name, parent)) = folders.get(&n) {
            parts.push(name.clone());
            if parts.len() > 8 || *parent == n {
                break;
            }
            n = *parent;
        }
        parts.reverse();
        parts.iter().map(|p| format!("{p}/")).collect()
    };
    let mut files = Vec::new();
    for e in entries {
        if e[11] & 0x18 != 0 {
            continue; // labels and directories
        }
        let name = file_name(&e[..8], &e[8..11]);
        let address = word(e, 22);
        let size = u32::from_le_bytes(e[28..32].try_into().unwrap()) as usize;
        let mut data = read_file(usize::from(word(e, 26)), size);
        let ext = koi8(&e[8..11]).to_uppercase();
        let mut note = None;
        let (address, entry, not_code) = if name == "ANDOS.SYS" {
            // On a BK-0011M the boot block reads its first 32 KB to
            // 040000-137777, moves part of it, and jumps to 003600; on a
            // BK-0010 it reads other parts to other places.
            data.truncate(0o100000);
            note = Some("the first 32 KB, as the boot block loads it on a BK-0011M");
            (0o40000, None, None)
        } else if address == 0o177777 {
            (
                address,
                None,
                Some("load address 177777: not loaded as it is"),
            )
        } else if ANDOS_NOT_CODE.contains(&ext.as_str()) {
            (
                address,
                None,
                Some("text, font, archive, compressed or PC file"),
            )
        } else {
            (address, None, None)
        };
        files.push(DiskFile {
            path: format!("{}{name}", folder_path(e[21])),
            address,
            entry,
            data,
            size,
            not_code,
            note,
        });
    }
    Some(files)
}

fn csidos(image: &[u8]) -> Option<Vec<DiskFile>> {
    // The first directory block's header: block 2, then the number of
    // blocks on the disk, then "SSS".
    let header = image.get(2 * BLOCK..2 * BLOCK + 12)?;
    if word(header, 0) != 2 || header[4..10] != [0o123, 0o246, 0o123, 0o246, 0o123, 0o246] {
        return None;
    }
    let mut entries = Vec::new();
    let mut block = 2;
    while let Some(b) = image.get(block * BLOCK..(block + 1) * BLOCK) {
        if usize::from(word(b, 0)) != block {
            break;
        }
        entries.extend(
            b[12..]
                .as_chunks::<20>()
                .0
                .iter()
                .map(|e| &e[..])
                .filter(|e| e[2..13].iter().any(|&c| c != 0)),
        );
        block += 1;
    }
    let is_folder = |e: &[u8]| word(e, 14) == 0 && word(e, 18) == 0;
    let folders: HashMap<u16, (String, u16)> = entries
        .iter()
        .filter(|e| is_folder(e))
        .map(|e| {
            (
                u16::from(e[13]),
                (file_name(&e[2..10], &e[10..13]), word(e, 0)),
            )
        })
        .collect();
    let folder_path = |mut n: u16| -> String {
        let mut parts = Vec::new();
        while let Some((name, parent)) = folders.get(&n) {
            parts.push(name.clone());
            if parts.len() > 8 || *parent == n {
                break;
            }
            n = *parent;
        }
        parts.reverse();
        parts.iter().map(|p| format!("{p}/")).collect()
    };
    let mut files = Vec::new();
    for e in entries.into_iter().filter(|e| !is_folder(e)) {
        let name = file_name(&e[2..10], &e[10..13]);
        let start = usize::from(word(e, 14)) * BLOCK;
        let len = usize::from(word(e, 18));
        let Some(data) = image.get(start..start + len) else {
            continue;
        };
        let ext = koi8(&e[10..13]).to_lowercase();
        let mut data = data.to_vec();
        let mut note = None;
        let (address, entry, not_code) = if word(e, 0) & 0o100000 != 0 {
            // The system file: the boot block reads its first 022400 words
            // to 045000 and calls 045010.
            data.truncate(0o45000);
            note = Some("the part the boot block loads at 045000");
            (0o45000, Some(0o45010), None)
        } else if ext == "exe" || ext == "ovl" {
            (word(e, 16), None, None)
        } else {
            (word(e, 16), None, Some("not a program (.exe or .ovl)"))
        };
        files.push(DiskFile {
            path: format!("{}{name}", folder_path(word(e, 0))),
            address,
            entry,
            data,
            size: len,
            not_code,
            note,
        });
    }
    Some(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(name: &str) -> Option<Vec<u8>> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../software")
                .join(name),
        )
        .ok()
    }

    #[test]
    fn andos_and_csidos_directories() {
        let (Some(andos_disk), Some(csidos_disk)) = (disk("ANDOS330.DSK"), disk("CSIDOS_330.DSK"))
        else {
            eprintln!("skipping: disk images not in software/");
            return;
        };
        let (system, files) = read(&andos_disk).unwrap();
        assert_eq!(system, System::Andos);
        let ansetup = files.iter().find(|f| f.path == "Утилиты/ANSETUP").unwrap();
        assert_eq!((ansetup.address, ansetup.data.len()), (0o1000, 1968));
        assert!(files.iter().any(|f| f.path == "Программы/EDALT/EDALT3M"));
        assert!(
            files
                .iter()
                .find(|f| f.path == "Описания/README.VXT")
                .unwrap()
                .not_code
                .is_some()
        );

        let (system, files) = read(&csidos_disk).unwrap();
        assert_eq!(system, System::Csidos);
        assert_eq!(files.len(), 93);
        let cs = files.iter().find(|f| f.path == "TOOLS/cs.exe").unwrap();
        assert_eq!(
            (cs.address, cs.data.len(), cs.not_code),
            (0o1000, 2896, None)
        );
        assert!(files.iter().any(|f| f.path == "TOOLS/BATCH/create.bat"));
        assert!(boot_block(&csidos_disk).is_some());
        assert!(read(&[0; 1024]).is_none());
    }
}

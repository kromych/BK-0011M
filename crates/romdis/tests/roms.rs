//! Each listing must assemble back into the identical image. This checks the
//! disassembler, the annotations and the assembler together on real
//! firmware and programs: every ROM in `<repo>/roms` (or `BK_ROMS`), and
//! every program `romdis` finds under `<repo>/software` (`.bin` files, the
//! programs on disk images, the files on tape recordings). Whatever isn't
//! there is skipped.

use std::path::PathBuf;

use romdis::disassemble_image;
use romdis::inventory::{self, Item};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn roms_dir() -> PathBuf {
    std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("roms"))
}

/// List `item` as `romdis` does, assemble the listing, and compare.
fn check(item: &Item) {
    let name = item.out.display();
    let ann = item.load_annotations(&repo().join("annotations")).unwrap();
    let listing = disassemble_image(
        &item.data,
        item.origin,
        &item.starts,
        &item.title,
        &item.source,
        &ann,
    );
    assert!(
        listing.warnings.is_empty(),
        "{name}: {:#?}",
        listing.warnings
    );
    let program = vm1::asm::assemble(&listing.text, item.origin)
        .unwrap_or_else(|e| panic!("{name}: listing doesn't assemble: {e}"));
    let data = &item.data;
    if let Some(i) = (0..data.len()).find(|&i| program.bytes.get(i) != Some(&data[i])) {
        panic!(
            "{name}: first difference at {:06o}: image {:03o}, assembled {:?}",
            item.origin as usize + i,
            data[i],
            program.bytes.get(i)
        );
    }
    assert_eq!(program.bytes.len(), data.len(), "{name}: length");
}

#[test]
fn rom_listings_assemble_back_into_the_roms() {
    let found = inventory::roms(&roms_dir());
    for item in &found.items {
        check(item);
    }
    eprintln!("{} ROM listings reproduce their images", found.items.len());
}

#[test]
fn program_listings_assemble_back_into_the_programs() {
    let found = inventory::software(
        &repo().join("software"),
        &roms_dir(),
        &repo().join("annotations"),
    );
    for item in &found.items {
        check(item);
    }
    eprintln!(
        "{} program listings reproduce their programs",
        found.items.len()
    );
}

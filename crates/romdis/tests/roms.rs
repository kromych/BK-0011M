//! Each listing must assemble back into the identical image. This checks the
//! disassembler, the annotations and the assembler together on real
//! firmware and programs. Needs the ROM files in `<repo>/roms` (or
//! `BK_ROMS`) and programs in `<repo>/software`; whatever isn't there is
//! skipped.

use std::path::{Path, PathBuf};

use romdis::{
    Family, Listing, ROMS, disassemble_image, disassemble_rom, load_annotations, parse_bin,
    start_address,
};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn check(name: &str, listing: &Listing, origin: u16, data: &[u8]) {
    assert!(
        listing.warnings.is_empty(),
        "{name}: {:#?}",
        listing.warnings
    );
    let program = vm1::asm::assemble(&listing.text, origin)
        .unwrap_or_else(|e| panic!("{name}: listing doesn't assemble: {e}"));
    if let Some(i) = (0..data.len()).find(|&i| program.bytes.get(i) != Some(&data[i])) {
        panic!(
            "{name}: first difference at {:06o}: image {:03o}, assembled {:?}",
            origin as usize + i,
            data[i],
            program.bytes.get(i)
        );
    }
    assert_eq!(program.bytes.len(), data.len(), "{name}: length");
}

#[test]
fn rom_listings_assemble_back_into_the_roms() {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("roms"));
    let ann_dir = repo().join("annotations");
    let mut checked = 0;
    for spec in ROMS {
        let Ok(data) = std::fs::read(Path::new(&dir).join(spec.file)) else {
            eprintln!("skipping {}: not found in {}", spec.file, dir.display());
            continue;
        };
        let ann = load_annotations(&ann_dir, spec.family, Some(spec.annotations)).unwrap();
        let listing = disassemble_rom(&data, spec.origin, spec.title, spec.file, &ann);
        check(spec.file, &listing, spec.origin, &data);
        checked += 1;
    }
    eprintln!("{checked} ROM listings reproduce their images");
}

#[test]
fn program_listings_assemble_back_into_the_programs() {
    let ann_dir = repo().join("annotations");
    let Ok(entries) = std::fs::read_dir(repo().join("software")) else {
        eprintln!("skipping: no software/ directory");
        return;
    };
    let mut checked = 0;
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("bin"))
        {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let (address, data) = parse_bin(&bytes).unwrap();
        // Shared annotations only: the check is about exact reassembly.
        let ann = load_annotations(&ann_dir, Family::Bk0010, None).unwrap();
        let listing =
            disassemble_image(data, address, &[start_address(address)], &name, &name, &ann);
        check(&name, &listing, address, data);
        checked += 1;
    }
    eprintln!("{checked} program listings reproduce their programs");
}

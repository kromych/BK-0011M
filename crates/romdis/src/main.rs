//! `romdis`: write commented MACRO-11 listings of the BK ROMs and of `.bin`
//! programs.
//!
//!     cargo run -p romdis -- [--roms DIR] [--software DIR] [--annotations DIR] [--out DIR]
//!
//! Defaults: ./roms, ./software, ./annotations, ./listings. Each ROM present
//! becomes `<out>/<name>.mac`; each `.bin` program in the software directory
//! becomes `<out>/software/<name>.mac`.

use std::path::{Path, PathBuf};
use std::{fs, process};

use romdis::annotations::Annotations;
use romdis::{Family, Listing, ROMS, disassemble_image, disassemble_rom, load_annotations};

fn main() {
    let mut roms = PathBuf::from("roms");
    let mut software = PathBuf::from("software");
    let mut ann_dir = PathBuf::from("annotations");
    let mut out = PathBuf::from("listings");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next().map(PathBuf::from).unwrap_or_else(|| {
                eprintln!("romdis: {arg} needs a value");
                process::exit(2);
            })
        };
        match arg.as_str() {
            "--roms" => roms = value(),
            "--software" => software = value(),
            "--annotations" => ann_dir = value(),
            "--out" => out = value(),
            _ => {
                eprintln!(
                    "usage: romdis [--roms DIR] [--software DIR] [--annotations DIR] [--out DIR]"
                );
                process::exit(2);
            }
        }
    }

    let mut failed = false;
    for spec in ROMS {
        let Ok(data) = fs::read(roms.join(spec.file)) else {
            eprintln!("{}: not found, skipped", spec.file);
            continue;
        };
        let result = load_annotations(&ann_dir, spec.family, Some(spec.annotations))
            .map(|ann| disassemble_rom(&data, spec.origin, spec.title, spec.file, &ann));
        failed |= !write(&out, spec.file, result);
    }

    let mut programs: Vec<PathBuf> = fs::read_dir(&software)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    programs.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")));
    programs.sort();
    for path in programs {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let result = fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| disassemble_program(&bytes, &name, &ann_dir));
        failed |= !write(&out.join("software"), &name, result);
    }
    if failed {
        process::exit(1);
    }
}

/// Disassemble a `.bin` program: annotations from `software/<stem>.ann`.
fn disassemble_program(bytes: &[u8], name: &str, ann_dir: &Path) -> Result<Listing, String> {
    let (address, data) = romdis::parse_bin(bytes)?;
    let own = format!(
        "software/{}.ann",
        Path::new(name).file_stem().unwrap().to_string_lossy()
    );
    let family = fs::read_to_string(ann_dir.join(&own))
        .ok()
        .and_then(|src| Annotations::parse(&src).ok())
        .and_then(|a| a.machine)
        .unwrap_or(Family::Bk0010);
    let ann = load_annotations(ann_dir, family, Some(&own))?;
    let title = format!("{name}: program for the {family:?}");
    Ok(disassemble_image(
        data,
        address,
        &[romdis::start_address(address)],
        &title,
        name,
        &ann,
    ))
}

fn write(dir: &Path, source: &str, result: Result<Listing, String>) -> bool {
    let listing = match result {
        Ok(l) => l,
        Err(e) => {
            eprintln!("romdis: {source}: {e}");
            return false;
        }
    };
    let path = dir.join(Path::new(source).with_extension("mac"));
    if let Err(e) = fs::create_dir_all(dir).and_then(|_| fs::write(&path, &listing.text)) {
        eprintln!("romdis: {}: {e}", path.display());
        return false;
    }
    let s = listing.stats;
    println!(
        "{:<18} code {:>5} (scan {:>4})  jump tables {:>4}  data {:>5} bytes  -> {}",
        source,
        s.code,
        s.scanned,
        s.pointers,
        s.data,
        path.display()
    );
    for w in &listing.warnings {
        println!("  warning: {w}");
    }
    true
}

//! `romdis`: write commented MACRO-11 listings of the BK ROMs and of the
//! programs under `software/`: `.bin` files, the programs on disk images,
//! and the files on tape recordings (see `romdis::inventory`).
//!
//!     cargo run --release -p romdis -- [--roms DIR] [--software DIR] [--annotations DIR] [--out DIR]
//!
//! Defaults: ./roms, ./software, ./annotations, ./listings. A ROM becomes
//! `<out>/<name>.mac`; a program `<out>/software/<path>.mac`, beside a
//! `files.txt` for each disk and tape saying what is on it.

use std::path::{Path, PathBuf};
use std::{fs, process};

use romdis::inventory::{self, Item};
use romdis::{Listing, disassemble_image};

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

    let mut found = inventory::roms(&roms);
    let programs = inventory::software(&software, &roms, &ann_dir);
    found.items.extend(programs.items);
    found.notes.extend(programs.notes);
    found.indexes.extend(programs.indexes);

    let mut failed = false;
    for note in &found.notes {
        eprintln!("romdis: {note}");
    }
    for item in &found.items {
        let result = item
            .load_annotations(&ann_dir)
            .map(|ann| listing(item, &ann));
        failed |= !write(&out, item, result);
    }
    for (path, text) in &found.indexes {
        let path = out.join(path);
        if let Err(e) = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| fs::write(&path, text))
        {
            eprintln!("romdis: {}: {e}", path.display());
            failed = true;
        }
    }
    println!("{} listings", found.items.len());
    if failed {
        process::exit(1);
    }
}

fn listing(item: &Item, ann: &romdis::annotations::Annotations) -> Listing {
    disassemble_image(
        &item.data,
        item.origin,
        &item.starts,
        &item.title,
        &item.source,
        ann,
    )
}

fn write(out: &Path, item: &Item, result: Result<Listing, String>) -> bool {
    let listing = match result {
        Ok(l) => l,
        Err(e) => {
            eprintln!("romdis: {}: {e}", item.source);
            return false;
        }
    };
    let path = out.join(&item.out);
    if let Err(e) = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|_| fs::write(&path, &listing.text))
    {
        eprintln!("romdis: {}: {e}", path.display());
        return false;
    }
    let s = listing.stats;
    println!(
        "{:<44} code {:>5} (scan {:>4})  jump tables {:>4}  data {:>5} bytes",
        item.out.display(),
        s.code,
        s.scanned,
        s.pointers,
        s.data,
    );
    for w in &listing.warnings {
        println!("  warning: {w}");
    }
    true
}

//! Disassemble a raw binary image (for example a ROM dump).
//!
//!     cargo run -p vm1 --example disasm -- FILE [ORIGIN] [COUNT]
//!
//! ORIGIN is the octal load address (default 0); COUNT limits the number of
//! instructions printed.

use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: disasm FILE [ORIGIN (octal)] [COUNT]");
        process::exit(2);
    };
    let origin = match args.get(1).map(|s| u16::from_str_radix(s, 8)) {
        None => 0,
        Some(Ok(o)) => o,
        Some(Err(_)) => {
            eprintln!("ORIGIN must be an octal number");
            process::exit(2);
        }
    };
    let count = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);

    let data = fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        process::exit(1);
    });
    let words: Vec<u16> = data
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
        .collect();
    let read = |a: u16| {
        words
            .get(usize::from(a.wrapping_sub(origin) / 2))
            .copied()
            .unwrap_or(0)
    };

    let end = usize::from(origin) + words.len() * 2;
    let mut addr = usize::from(origin);
    for _ in 0..count {
        if addr >= end || addr > 0xffff {
            break;
        }
        let d = vm1::disassemble(addr as u16, read);
        let raw: Vec<String> = (0..d.words)
            .map(|i| format!("{:06o}", read(addr as u16 + 2 * i)))
            .collect();
        println!("{addr:06o}: {:<20} {}", raw.join(" "), d.text);
        addr += 2 * usize::from(d.words);
    }
}

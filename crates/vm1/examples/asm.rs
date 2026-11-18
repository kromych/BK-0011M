//! Assemble a MACRO-11 source file into a `.bin` file (BK format: load
//! address, length, data).
//!
//!     cargo run -p vm1 --example asm -- SOURCE ORIGIN OUTPUT.bin [SYMBOLS]
//!
//! ORIGIN is octal. With SYMBOLS, the labels and symbols are also written
//! there, one `NAME = ADDRESS` (octal) per line, for the emulator's
//! debugger (`--symbols`).

use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (source, origin, output, symbols) = match args.as_slice() {
        [s, o, out] => (s, o, out, None),
        [s, o, out, sym] => (s, o, out, Some(sym)),
        _ => {
            eprintln!("usage: asm SOURCE ORIGIN (octal) OUTPUT.bin [SYMBOLS]");
            process::exit(2);
        }
    };
    let origin = u16::from_str_radix(origin, 8).unwrap_or_else(|_| {
        eprintln!("ORIGIN must be an octal number");
        process::exit(2);
    });
    let text = fs::read_to_string(source).unwrap_or_else(|e| {
        eprintln!("{source}: {e}");
        process::exit(1);
    });
    let program = vm1::asm::assemble(&text, origin).unwrap_or_else(|e| {
        eprintln!("{source}: {e}");
        process::exit(1);
    });
    let mut bin = Vec::with_capacity(program.bytes.len() + 4);
    bin.extend(origin.to_le_bytes());
    bin.extend((program.bytes.len() as u16).to_le_bytes());
    bin.extend(&program.bytes);
    if let Err(e) = fs::write(output, bin) {
        eprintln!("{output}: {e}");
        process::exit(1);
    }
    if let Some(path) = symbols {
        let mut names: Vec<(&String, &u16)> = program.symbols.iter().collect();
        names.sort_by_key(|&(name, &addr)| (addr, name.clone()));
        let text: String = names
            .iter()
            .map(|(name, addr)| format!("{name} = {addr:06o}\n"))
            .collect();
        if let Err(e) = fs::write(path, text) {
            eprintln!("{path}: {e}");
            process::exit(1);
        }
    }
}

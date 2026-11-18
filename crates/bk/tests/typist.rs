//! Pasting text with the Typist, through the real firmware. Needs the ROM
//! files in `<repo>/roms` (or `BK_ROMS`); skipped when they're missing.

use std::path::PathBuf;

use bk::{Bk0010Rom, Config, Machine, Model, Roms, Typist};

fn machine_of(model: Model) -> Option<Machine> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    let roms = match model {
        Model::Bk0011m => Roms::load_bk0011m(&dir, false),
        Model::Bk0010 => Roms::load_bk0010(&dir, Bk0010Rom::Basic),
    };
    roms.map_err(|e| eprintln!("skipping: {e}"))
        .ok()
        .map(|r| Machine::new(r, Config::default()))
}

fn paste(m: &mut Machine, text: &str) {
    let mut t = Typist::new();
    assert_eq!(t.push_text(text), 0);
    for _ in 0..20_000 {
        if t.is_idle() {
            return;
        }
        m.run_frame();
        t.tick(m);
    }
    panic!("still typing; {} keys left", t.pending());
}

/// A pasted multi-line BASIC program, with Russian text and Windows line
/// ends, is entered line by line and runs: it stores 6*7 at 1000 (octal),
/// and its Russian string ends up in memory in KOI-8.
#[test]
fn pasted_basic_program_runs() {
    for model in [Model::Bk0010, Model::Bk0011m] {
        let Some(mut m) = machine_of(model) else {
            return;
        };
        for _ in 0..200 {
            m.run_frame();
        }
        paste(
            &mut m,
            "10 A=6*7\r\n20 POKE 512,A\r\n30 B$=\"Привет, БК!\"\r\nRUN\r\n",
        );
        for _ in 0..100 {
            m.run_frame();
        }
        assert_eq!(
            m.peek_word(0o1000).map(|w| w & 0xff),
            Some(42),
            "{model:?}: the program ran"
        );
        let ram: Vec<u8> = (0..0o40000u16)
            .step_by(2)
            .flat_map(|a| m.peek_word(a).unwrap().to_le_bytes())
            .collect();
        let koi8: Vec<u8> = "Привет, БК!"
            .chars()
            .map(|c| bk::vm1::koi8::encode(c).unwrap())
            .collect();
        assert!(
            ram.windows(koi8.len()).any(|w| w == koi8),
            "{model:?}: the Russian string is in memory"
        );
    }
}

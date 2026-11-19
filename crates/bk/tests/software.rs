//! Smoke test with real BK software: each known program in `<repo>/software`
//! (git-ignored; see README.md for where these came from) must load, run, take
//! its start keys, and still be running with something on screen. Missing
//! files are skipped.

use std::path::PathBuf;

use bk::{BinFile, Bk0010Rom, Config, Machine, Model, Roms, SCREEN_HEIGHT, SCREEN_WIDTH};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// (file, model, frames to run before typing, keys typed after it starts)
const PROGRAMS: &[(&str, Model, usize, &str)] = &[
    ("klad.bin", Model::Bk0010, 250, "\n1"),
    ("valley.bin", Model::Bk0010, 250, ""),
    ("bally.bin", Model::Bk0010, 250, ""),
    ("Xonix.bin", Model::Bk0010, 250, " "),
    ("miner.bin", Model::Bk0010, 250, ""),
    ("PONG.bin", Model::Bk0010, 250, ""),
    ("DIGGER_1.6.BIN", Model::Bk0010, 250, ""),
    ("PONG.bin", Model::Bk0011m, 250, ""),
    ("krakout.bin", Model::Bk0011m, 250, " "),
    // The intro takes about 11 seconds to start.
    ("BoingBall.bin", Model::Bk0011m, 700, ""),
    // From BKEmu's package: Boulder Dash's title, a music collection's menu.
    ("bkemu/BOLDER.BIN", Model::Bk0010, 250, ""),
    ("bkemu/MELMAN.BIN", Model::Bk0010, 250, ""),
];

#[test]
fn known_programs_run() {
    let roms_dir = repo().join("roms");
    let mut ran = 0;
    for &(file, model, frames, keys) in PROGRAMS {
        let Ok(bytes) = std::fs::read(repo().join("software").join(file)) else {
            eprintln!("skipping {file}: not in software/");
            continue;
        };
        let roms = match model {
            Model::Bk0010 => Roms::load_bk0010(&roms_dir, Bk0010Rom::Basic),
            Model::Bk0011m => Roms::load_bk0011m(&roms_dir, false),
        };
        let Ok(roms) = roms else {
            eprintln!("skipping {file}: no ROMs");
            continue;
        };
        let mut m = Machine::new(roms, Config::default());
        for _ in 0..150 {
            m.run_frame();
        }
        let bin = BinFile::parse(&bytes).unwrap();
        m.load_bin(&bin).unwrap();
        m.start_at(bin.start_address());
        for _ in 0..frames {
            m.run_frame();
        }
        m.type_text(keys);
        for _ in 0..100 {
            m.run_frame();
        }
        assert!(!m.cpu().is_halted(), "{file} ({model:?}): CPU halted");
        let mut screen = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
        m.render(&mut screen);
        let lit = screen.iter().filter(|&&p| p != 0).count();
        assert!(
            lit > 200,
            "{file} ({model:?}): screen nearly blank ({lit} pixels)"
        );
        ran += 1;
    }
    eprintln!("{ran} programs ran");
}

/// Digger's title screen loads the game as a second file through EMT 36;
/// with fast tape it comes from `software/`.
#[test]
fn digger_title_loads_the_game_through_fast_tape() {
    let dir = repo().join("software");
    let (Ok(title), Ok(game)) = (
        std::fs::read(dir.join("DIGTIT_1.6.BIN")),
        std::fs::read(dir.join("DIGGER_1.6.BIN")),
    ) else {
        eprintln!("skipping: Digger not in software/");
        return;
    };
    let Ok(roms) = Roms::load_bk0010(&repo().join("roms"), Bk0010Rom::Basic) else {
        return;
    };
    let mut m = Machine::new(roms, Config::default());
    m.set_tape_files(Some(Box::new(bk::DirFiles { dir })));
    for _ in 0..150 {
        m.run_frame();
    }
    let title = BinFile::parse(&title).unwrap();
    m.load_bin(&title).unwrap();
    m.start_at(title.start_address());
    for _ in 0..300 {
        m.run_frame();
    }
    m.type_text(" \n"); // one player, start
    for _ in 0..200 {
        m.run_frame();
    }
    let game = BinFile::parse(&game).unwrap();
    for (i, pair) in game.data.as_chunks::<2>().0.iter().take(64).enumerate() {
        let addr = game.address + 2 * i as u16;
        assert_eq!(
            m.peek_word(addr),
            Some(u16::from_le_bytes([pair[0], pair[1]])),
            "{addr:06o}"
        );
    }
}

/// The Saboteur 2 port is an ANDOS disk: boot it, open ANDOS's file
/// manager, start SABOT2 from it, and check its menu is on screen (a mostly
/// blue picture; the file manager is mostly black).
#[test]
fn saboteur2_boots_from_its_andos_disk() {
    let Ok(image) = std::fs::read(repo().join("software/sabot2-v44.dsk")) else {
        eprintln!("skipping: sabot2-v44.dsk not in software/");
        return;
    };
    let Ok(roms) = Roms::load_bk0011m(&repo().join("roms"), true) else {
        return;
    };
    let mut m = Machine::new(
        roms,
        Config {
            fdd: true,
            ..Default::default()
        },
    );
    m.insert_disk(0, bk::DiskImage::new(image, true)).unwrap();
    let frames = |m: &mut Machine, n: usize| {
        for _ in 0..n {
            m.run_frame();
        }
    };
    frames(&mut m, 900); // ANDOS title screen
    m.type_text(" ");
    frames(&mut m, 600); // file manager
    m.type_text("\x1b"); // down to SABOT2
    frames(&mut m, 50);
    m.type_text("\n");
    frames(&mut m, 1500);
    let mut screen = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    m.render(&mut screen);
    let blue = screen.iter().filter(|&&p| p == 0x0000FF).count();
    assert!(
        blue * 2 > screen.len(),
        "menu not on screen ({blue} blue pixels)"
    );
}

/// A BK-0010.01 with the floppy controller (its ROM 326 and 16 KB of extra
/// RAM): the monitor's S160000 starts the disk ROM, which boots ANDOS from
/// the Saboteur 2 disk; its title screen fills the screen with graphics.
#[test]
fn andos_boots_on_a_bk0010_with_the_floppy_controller() {
    let Ok(image) = std::fs::read(repo().join("software/sabot2-v44.dsk")) else {
        eprintln!("skipping: sabot2-v44.dsk not in software/");
        return;
    };
    let Ok(roms) = Roms::load_bk0010(&repo().join("roms"), Bk0010Rom::Fdd) else {
        eprintln!("skipping: no BK-0010 floppy ROMs (disk_326.rom)");
        return;
    };
    let mut m = Machine::new(
        roms,
        Config {
            fdd: true,
            ..Default::default()
        },
    );
    m.insert_disk(0, bk::DiskImage::new(image, true)).unwrap();
    for _ in 0..200 {
        m.run_frame();
    }
    let lit = |m: &Machine| {
        (0o40000..0o100000u16)
            .step_by(2)
            .filter(|&a| m.peek_word(a) != Some(0))
            .count()
    };
    let before = lit(&m);
    m.type_text("S160000\n");
    for _ in 0..900 {
        m.run_frame();
    }
    assert!(!m.cpu().is_halted());
    let after = lit(&m);
    assert!(
        after > 2000 && after > 10 * before,
        "ANDOS title drawn: {before} → {after} screen words lit"
    );
}

/// Miami Cobra loads its parts with BK-0010-style EMT 36 calls, which only
/// a disk system provides on the BK-0011M (under plain BOS, EMT 36 is the
/// line editor). Its disk (made with tools/mkandos.py) boots ANDOS; started
/// from ANDOS's file manager, the game reaches its menu (mostly cyan).
#[test]
fn miami_cobra_runs_from_its_andos_disk() {
    let Ok(image) = std::fs::read(repo().join("software/miami-cobra/miami-cobra.dsk")) else {
        eprintln!("skipping: software/miami-cobra/miami-cobra.dsk missing");
        return;
    };
    let Ok(roms) = Roms::load_bk0011m(&repo().join("roms"), true) else {
        return;
    };
    let mut m = Machine::new(
        roms,
        Config {
            fdd: true,
            ..Default::default()
        },
    );
    m.insert_disk(0, bk::DiskImage::new(image, true)).unwrap();
    let frames = |m: &mut Machine, n: usize| {
        for _ in 0..n {
            m.run_frame();
        }
    };
    frames(&mut m, 900); // ANDOS title screen
    m.type_text(" ");
    frames(&mut m, 600); // file manager
    m.type_text("\x1b"); // down to MIAMI
    frames(&mut m, 50);
    m.type_text("\n");
    frames(&mut m, 1500);
    let mut screen = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    m.render(&mut screen);
    let cyan = screen.iter().filter(|&&p| p == 0x00FFFF).count();
    assert!(
        cyan * 2 > screen.len(),
        "menu not on screen ({cyan} cyan pixels)"
    );
}

/// Run for up to `frames`, or until the program has printed `until`,
/// collecting what it prints through the monitor: EMT 16 (a character in
/// R0) and EMT 20 (a string at R1; R2's low byte its length, or 0 to print
/// up to the character in R2's high byte).
fn printed_text(m: &mut Machine, frames: usize, until: &str) -> String {
    m.set_opcode_breaks(&[0o104016, 0o104020]);
    let mut text = String::new();
    let mut ran = 0;
    while ran < frames && !text.contains(until) {
        let Some(bk::Break::Opcode(pc)) = m.run_frame() else {
            ran += 1;
            continue;
        };
        let r = m.cpu().r;
        let byte = |m: &Machine, a: u16| {
            m.peek_word(a & !1)
                .map_or(0, |w| (w >> (8 * (a & 1))) as u8)
        };
        if m.peek_word(pc) == Some(0o104016) {
            text.push(char::from(r[0] as u8 & 0o177));
        } else {
            let (len, end) = (r[2] & 0o377, (r[2] >> 8) as u8);
            for i in 0..256u16 {
                let c = byte(m, r[1].wrapping_add(i));
                if (len == 0 && c == end) || (len != 0 && i == len) {
                    break;
                }
                text.push(char::from(c & 0o177));
            }
        }
        m.step_into();
    }
    m.set_opcode_breaks(&[]);
    text
}

/// BKEmu's "K1801BM1 compatibility test v0.4" (Digital Freedom, 2012) runs
/// eight checks and prints a verdict for each through the monitor. On the
/// BK-0010 the seven processor checks pass (SWAB, JMP (Rn)+, opcodes
/// 210-217, JSR/JMP to a register, PC operands, Rn,(Rn)+, the VM1's timer);
/// the eighth, the CPU's speed, needs a BK-0011M and says so.
#[test]
fn k1801vm1_compatibility_test_passes_on_the_bk0010() {
    let Ok(bytes) = std::fs::read(repo().join("software/bkemu/CPUTEST.BIN")) else {
        eprintln!("skipping: software/bkemu/CPUTEST.BIN missing");
        return;
    };
    let Ok(roms) = Roms::load_bk0010(&repo().join("roms"), Bk0010Rom::Basic) else {
        return;
    };
    let mut m = Machine::new(roms, Config::default());
    for _ in 0..150 {
        m.run_frame();
    }
    let bin = BinFile::parse(&bytes).unwrap();
    m.load_bin(&bin).unwrap();
    m.start_at(bin.start_address());
    let text = printed_text(&mut m, 10_000, "restart");
    let verdicts: Vec<&str> = text.split("check #").skip(1).collect();
    assert_eq!(verdicts.len(), 8, "{text:?}");
    for (n, v) in verdicts.iter().enumerate().take(7) {
        assert!(v.ends_with("pass"), "check {}: {v:?}", n + 1);
    }
    assert!(verdicts[7].starts_with("08 - CPU speed") && verdicts[7].contains("FAIL"));
    assert!(text.contains("We need 11M hardware"), "{text:?}");
}

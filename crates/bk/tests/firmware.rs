//! Runs the real BK-0011M and BK-0010 firmware. Needs the ROM files in
//! `<repo>/roms` (or the directory named by `BK_ROMS`); skipped when they're
//! missing.

use std::path::PathBuf;

use bk::vm1::asm::assemble;
use bk::{BinFile, Bk0010Rom, Config, Machine, Model, Roms, SCREEN_HEIGHT, SCREEN_WIDTH};

fn rom_dir() -> PathBuf {
    std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"))
}

fn machine_of(model: Model) -> Option<Machine> {
    let dir = rom_dir();
    let roms = match model {
        Model::Bk0011m => Roms::load_bk0011m(&dir, false),
        Model::Bk0010 => Roms::load_bk0010(&dir, Bk0010Rom::Basic),
    };
    match roms {
        Ok(roms) => Some(Machine::new(roms, Config::default())),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn machine() -> Option<Machine> {
    machine_of(Model::Bk0011m)
}

fn run_frames(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.run_frame();
    }
    assert!(!m.cpu().is_halted(), "CPU halted at {:06o}", m.cpu().pc());
}

#[test]
fn monitor_draws_its_banner() {
    let Some(mut m) = machine() else { return };
    run_frames(&mut m, 100);
    let mut screen = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    m.render(&mut screen);
    let lit = screen.iter().filter(|&&p| p != 0).count();
    assert!(lit > 1000, "screen is (nearly) blank: {lit} pixels lit");
}

#[test]
fn basic_runs_a_program() {
    let Some(mut m) = machine() else { return };
    run_frames(&mut m, 300); // the monitor starts BASIC after a timeout
    m.type_text("10 S=0\n20 FOR I=1 TO 10\n30 S=S+I*I\n40 NEXT\n50 POKE 16000,S\nRUN\n");
    assert_eq!(m.peek_word(16000), Some(385));
}

#[test]
fn stop_key_breaks_into_basic() {
    let Some(mut m) = machine() else { return };
    run_frames(&mut m, 300);
    m.type_text("10 GOTO 10\nRUN\n");
    m.press_stop();
    run_frames(&mut m, 50);
    m.type_text("POKE 16000,7\n");
    assert_eq!(m.peek_word(16000), Some(7), "BASIC is back at its prompt");
}

#[test]
fn bk0010_basic_runs_a_program() {
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    run_frames(&mut m, 150);
    m.type_text("10 S=0\n20 FOR I=1 TO 10\n30 S=S+I*I\n40 NEXT\n50 POKE 16000,S\nRUN\n");
    assert_eq!(m.peek_word(16000), Some(385));
}

/// A tiny program: store a marker, then spin at DONE. Returns the file and
/// DONE's address.
fn test_program() -> (BinFile, u16) {
    let p = assemble("MOV #12345, @#2000\nDONE: BR DONE", 0o1000).unwrap();
    let done = p.symbol("DONE").unwrap();
    (
        BinFile {
            address: 0o1000,
            data: p.bytes,
        },
        done,
    )
}

#[test]
fn bin_programs_load_and_run_on_both_models() {
    for model in [Model::Bk0010, Model::Bk0011m] {
        let Some(mut m) = machine_of(model) else {
            return;
        };
        run_frames(&mut m, 150);
        let (bin, done) = test_program();
        m.load_bin(&bin).unwrap();
        m.start_at(bin.start_address());
        run_frames(&mut m, 1);
        assert_eq!(m.peek_word(0o2000), Some(0o12345), "{model:?}");
        assert_eq!(m.cpu().pc(), done, "{model:?}: spinning at DONE");
    }
}

#[test]
fn bin_must_land_in_ram() {
    let Some(mut m) = machine() else { return };
    let bin = BinFile {
        address: 0o140000,
        data: vec![0; 2],
    };
    assert_eq!(m.load_bin(&bin), Err("140000 is not RAM".to_string()));
}

/// The BK-0010 monitor calls whose names `annotations/bk0010.ann` marks as
/// checked: EMT 14 (init), 26 (cursor position), 16 (print character),
/// 20 (print string) and 6 (read a key).
#[test]
fn bk0010_monitor_calls() {
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    run_frames(&mut m, 150);
    let p = assemble(
        "        EMT 14
                 EMT 26
                 MOV R1, @#2000
                 MOV R2, @#2002
                 MOV #101, R0
                 EMT 16
                 EMT 26
                 MOV R1, @#2004
                 MOV #MSG, R1
                 MOV #3, R2
                 EMT 20
                 EMT 26
                 MOV R1, @#2006
                 EMT 6
                 MOV R0, @#2010
         DONE:   BR DONE
         MSG:    .ASCII /XYZ/",
        0o1000,
    )
    .unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    run_frames(&mut m, 50);
    m.type_text("Q");
    let word = |a| m.peek_word(a).unwrap();
    assert_eq!(
        (word(0o2000), word(0o2002)),
        (0, 0),
        "EMT 14 homes the cursor; EMT 26 reads it"
    );
    assert_eq!(word(0o2004), 1, "EMT 16 printed one character");
    assert_eq!(word(0o2006), 4, "EMT 20 printed three more");
    assert_eq!(word(0o2010), u16::from(b'Q'), "EMT 6 returned the key");
}

/// Instruction timing per model: the tables match the BK-0010; on the
/// BK-0011M each memory access costs 2 more clocks (tools/vm1-timing).
#[test]
fn bk0011m_memory_accesses_cost_more_clocks() {
    for (model, nop, mov_imm) in [(Model::Bk0010, 12, 28), (Model::Bk0011m, 14, 32)] {
        let Some(mut m) = machine_of(model) else {
            return;
        };
        run_frames(&mut m, 150);
        let p = assemble("NOP\nMOV #5, R0\nDONE: BR DONE", 0o1000).unwrap();
        m.load_bin(&BinFile {
            address: 0o1000,
            data: p.bytes,
        })
        .unwrap();
        m.start_at(0o1000);
        assert_eq!(m.step(), nop, "{model:?} NOP");
        assert_eq!(m.step(), mov_imm, "{model:?} MOV #n, R0");
    }
}

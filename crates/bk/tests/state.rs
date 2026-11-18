//! Save states, through the real firmware. Needs the ROM files in
//! `<repo>/roms` (or `BK_ROMS`); skipped when they're missing.

use std::path::PathBuf;

use bk::vm1::asm::assemble;
use bk::{
    BinFile, Bk0010Rom, Config, DiskImage, Machine, Model, Roms, SCREEN_HEIGHT, SCREEN_WIDTH,
};

fn roms(model: Model, fdd: bool) -> Option<Roms> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    let roms = match model {
        Model::Bk0011m => Roms::load_bk0011m(&dir, fdd),
        Model::Bk0010 => Roms::load_bk0010(&dir, Bk0010Rom::Basic),
    };
    roms.map_err(|e| eprintln!("skipping: {e}")).ok()
}

fn screen(m: &Machine) -> Vec<u32> {
    let mut out = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    m.render(&mut out);
    out
}

fn frames(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.run_frame();
    }
}

/// Run, save, run on; restore into a fresh machine and run the same
/// frames: everything (registers, screen, the state itself) matches.
fn check_replay(model: Model, config: Config, setup: impl Fn(&mut Machine)) {
    let Some(roms) = roms(model, config.fdd) else {
        return;
    };
    let mut a = Machine::new(roms.clone(), config);
    setup(&mut a);
    frames(&mut a, 120);
    let saved = a.save_state();
    a.take_audio();
    a.type_text("10 PRINT 6*7\n");
    frames(&mut a, 60);

    let mut b = Machine::new(roms, config);
    setup(&mut b);
    b.load_state(&saved).unwrap();
    b.take_audio();
    b.type_text("10 PRINT 6*7\n");
    frames(&mut b, 60);

    assert_eq!(a.cpu().state(), b.cpu().state(), "{model:?}: CPU");
    assert!(screen(&a) == screen(&b), "{model:?}: screen");
    // The audio output filter isn't machine state (a restored machine starts
    // a fresh one), so only the amount of sound is compared.
    assert!(
        a.take_audio().len().abs_diff(b.take_audio().len()) <= 1,
        "{model:?}: audio"
    );
    let (sa, sb) = (a.save_state(), b.save_state());
    let diff: Vec<usize> = (0..sa.len().min(sb.len()))
        .filter(|&i| sa[i] != sb[i])
        .collect();
    assert!(
        sa == sb,
        "{model:?}: whole state: lengths {} {}, {} bytes differ, first at {:?} of {}",
        sa.len(),
        sb.len(),
        diff.len(),
        diff.first().map(|&i| (i, sa[i], sb[i])),
        sa.len()
    );
}

#[test]
fn replay_matches_after_restore() {
    check_replay(Model::Bk0011m, Config::default(), |_| {});
    check_replay(Model::Bk0010, Config::default(), |_| {});
    check_replay(
        Model::Bk0011m,
        Config {
            ay: true,
            covox: true,
            ..Default::default()
        },
        |_| {},
    );
}

/// With a disk: the ANDOS boot is under way when the state is taken.
#[test]
fn replay_matches_with_a_disk() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../software/sabot2-v44.dsk");
    let Ok(image) = std::fs::read(&path) else {
        eprintln!("skipping: {} not found", path.display());
        return;
    };
    let config = Config {
        fdd: true,
        ..Default::default()
    };
    check_replay(Model::Bk0011m, config, |m| {
        m.insert_disk(0, DiskImage::new(image.clone(), true))
            .unwrap();
    });
}

#[test]
fn restores_a_program_mid_run() {
    let Some(roms) = roms(Model::Bk0010, false) else {
        return;
    };
    let mut m = Machine::new(roms.clone(), Config::default());
    frames(&mut m, 150);
    let p = assemble("LOOP: INC @#2000\nBR LOOP", 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 3);
    let saved = m.save_state();
    let count = m.peek_word(0o2000).unwrap();
    frames(&mut m, 5);
    assert_ne!(m.peek_word(0o2000).unwrap(), count);
    m.load_state(&saved).unwrap();
    assert_eq!(m.peek_word(0o2000).unwrap(), count, "RAM restored");
    assert!(matches!(m.cpu().pc(), 0o1000..=0o1006), "running the loop");
}

#[test]
fn rejects_mismatched_or_damaged_states() {
    let Some(roms11) = roms(Model::Bk0011m, false) else {
        return;
    };
    let Some(roms10) = roms(Model::Bk0010, false) else {
        return;
    };
    let mut m11 = Machine::new(roms11.clone(), Config::default());
    let mut m10 = Machine::new(roms10, Config::default());
    frames(&mut m11, 10);
    let saved = m11.save_state();
    let before = m10.save_state();

    let err = m10.load_state(&saved).unwrap_err();
    assert!(err.contains("BK-0011M") && err.contains("BK-0010"), "{err}");
    let mut with_ay = Machine::new(
        roms11,
        Config {
            ay: true,
            ..Default::default()
        },
    );
    let err = with_ay.load_state(&saved).unwrap_err();
    assert!(err.contains("AY"), "{err}");

    assert!(m10.load_state(b"hello").is_err());
    assert!(
        m11.load_state(&saved[..saved.len() - 1])
            .unwrap_err()
            .contains("truncated")
    );
    let mut longer = saved.clone();
    longer.push(0);
    assert!(m11.load_state(&longer).unwrap_err().contains("unexpected"));
    let mut newer = saved.clone();
    newer[8] = 99;
    assert!(m11.load_state(&newer).unwrap_err().contains("version"));

    assert_eq!(m10.save_state(), before, "failed loads change nothing");
}

/// Switching the power off and on in the middle of a game gives exactly
/// the machine a fresh start gives: nothing of the game survives (RAM,
/// palette, screen page, paging, devices).
#[test]
fn power_cycle_is_a_cold_start() {
    let games = [
        ("BoingBall.bin", Model::Bk0011m),
        ("krakout.bin", Model::Bk0011m),
        ("Xonix.bin", Model::Bk0010),
    ];
    for (file, model) in games {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../software")
            .join(file);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: {} not found", path.display());
            continue;
        };
        let Some(roms) = roms(model, false) else {
            return;
        };
        let mut fresh = Machine::new(roms.clone(), Config::default());
        let mut cycled = Machine::new(roms, Config::default());
        frames(&mut cycled, 150);
        let bin = BinFile::parse(&bytes).unwrap();
        cycled.load_bin(&bin).unwrap();
        cycled.start_at(if model == Model::Bk0011m {
            0o2000
        } else {
            bin.start_address()
        });
        cycled.key_down(0o40, false);
        frames(&mut cycled, 300);
        cycled.power_on();
        frames(&mut fresh, 200);
        frames(&mut cycled, 200);
        assert!(
            screen(&fresh) == screen(&cycled),
            "{file}: screen after the power cycle"
        );
        assert!(
            fresh.save_state() == cycled.save_state(),
            "{file}: machine state after the power cycle"
        );
    }
}

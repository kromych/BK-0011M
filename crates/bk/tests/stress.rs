//! Stress tests: random programs hammering the CPU and every I/O device,
//! damaged save states, random debugger commands, and games played with
//! random keys and replayed from a save state taken mid-game.
//!
//! Everything is seeded, so a failure reproduces: the message names the
//! seed. `BK_STRESS=N` multiplies the amount of work (default 1, sized to
//! finish in a debug build, where overflow checks are on); `BK_STRESS_SEED`
//! picks another base seed. Needs the ROM files in `<repo>/roms` (or
//! `BK_ROMS`); skipped when they're missing.

use std::path::PathBuf;

use bk::debugger::Action;
use bk::{BinFile, Bk0010Rom, Config, Debugger, DiskImage, Machine, Model, Roms};

/// xorshift64*: small, fast and good enough for test inputs.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn word(&mut self) -> u16 {
        self.next() as u16
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

fn scale() -> u64 {
    std::env::var("BK_STRESS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1)
}

fn base_seed() -> u64 {
    std::env::var("BK_STRESS_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x0b_c011)
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn roms(model: Model, fdd: bool) -> Option<Roms> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("roms"));
    let roms = match model {
        Model::Bk0011m => Roms::load_bk0011m(&dir, fdd),
        Model::Bk0010 => Roms::load_bk0010(&dir, Bk0010Rom::Basic),
    };
    roms.map_err(|e| eprintln!("skipping: {e}")).ok()
}

fn frames(m: &mut Machine, n: u64) {
    for _ in 0..n {
        m.run_frame();
    }
}

/// Saving, loading into a fresh machine and saving again gives the same
/// bytes: whatever state the machine got into can be saved faithfully.
fn assert_state_round_trips(m: &Machine, roms: &Roms, config: Config, context: &str) {
    let saved = m.save_state();
    let mut fresh = Machine::new(roms.clone(), config);
    if let Err(e) = fresh.load_state(&saved) {
        panic!("{context}: a saved state doesn't load: {e}");
    }
    assert!(
        fresh.save_state() == saved,
        "{context}: state changed in a save/load round trip"
    );
}

/// Addresses that make random code reach the devices: every I/O register,
/// the paging bit, ROM entry points, vectors, and instructions that stop
/// or wait.
const INTERESTING: &[u16] = &[
    0o177660, 0o177662, 0o177664, 0o177700, 0o177702, 0o177704, 0o177706, 0o177710, 0o177712,
    0o177714, 0o177716, 0o177130, 0o177132, 0o004000, 0o014000, 0o054002, 0o100000, 0o120000,
    0o140000, 0o160000, 0o000004, 0o000060, 0o000100, 0o000274, 0o000000, 0o000001, 0o000002,
    0o000240, 0o177777, 0o000340,
];

/// Random words, often an interesting address instead.
fn random_word(rng: &mut Rng) -> u16 {
    if rng.chance(3) {
        *rng.pick(INTERESTING)
    } else {
        rng.word()
    }
}

/// Every kind of key: the ones games use, and control keys (КТ, СБР, РУС,
/// ЛАТ, a function key) that make the firmware act.
const KEYS: &[u8] = &[
    0o12, 0o40, 0o10, 0o31, 0o32, 0o33, 0o101, 0o61, 0o3, 0o14, 0o16, 0o17, 0o201,
];

/// Keys for playing: arrows, space, Enter, a letter and a digit. Control
/// keys can legitimately crash a game: a BK-0010 game on the BK-0011M
/// draws at 40000-77777, where BASIC keeps its code and data, and BOS's key
/// handling (СБР clears memory from a garbage address) then crashes it.
const PLAY_KEYS: &[u8] = &[0o12, 0o40, 0o10, 0o31, 0o32, 0o33, 0o101, 0o61];

fn random_keys(m: &mut Machine, rng: &mut Rng) {
    if rng.chance(4) {
        let key = *rng.pick(KEYS);
        if rng.chance(2) {
            m.key_down(key, rng.chance(4));
        } else {
            m.key_up(key);
        }
    }
    if rng.chance(40) {
        m.press_stop();
    }
}

/// Random code over random registers, vectors and stacks, with random keys
/// and STOP presses: the emulator must never panic or hang, the frame
/// count must stay exact, and the state must always save faithfully.
fn random_code(model: Model, config: Config, trials: u64, seed: u64) {
    let Some(roms) = roms(model, config.fdd) else {
        return;
    };
    let mut m = Machine::new(roms.clone(), config);
    if config.fdd {
        // A small blank disk, so the controller has something to turn.
        m.insert_disk(0, DiskImage::new(vec![0; 20 * 5120], false))
            .unwrap();
    }
    frames(&mut m, 30);
    let booted = m.save_state();
    // How the trials ended, to show the code really ran.
    let (mut halted, mut waiting, mut in_rom, mut steps) = (0, 0, 0, 0u64);
    for trial in 0..trials {
        let seed = seed ^ trial.wrapping_mul(0x1000_0001);
        let context = format!("{model:?} {config:?}, seed {seed}");
        let mut rng = Rng::new(seed);
        m.load_state(&booted).unwrap();
        for a in (0o1000..0o5000).step_by(2) {
            m.poke_word(a, random_word(&mut rng));
        }
        if rng.chance(3) {
            for a in (0..0o400).step_by(2) {
                m.poke_word(a, random_word(&mut rng));
            }
        }
        for r in 0..6 {
            m.cpu_mut().r[r] = random_word(&mut rng);
        }
        m.cpu_mut().r[6] = if rng.chance(4) {
            random_word(&mut rng)
        } else {
            0o1000
        };
        m.cpu_mut().psw = rng.word() & 0o377;
        m.start_at(0o1000 + 2 * rng.below(0o1000) as u16);
        let start_frames = m.frames();
        m.take_audio();
        for _ in 0..8 {
            random_keys(&mut m, &mut rng);
            assert_eq!(m.run_frame(), None, "{context}: no breakpoints are set");
        }
        assert_eq!(m.frames(), start_frames + 8, "{context}: frame count");
        let samples = m.take_audio().len() as i64;
        let expected = (8 * m.audio_rate() / 50) as i64;
        // A frame ends after the instruction that crosses its end, so 8
        // frames can run up to one long instruction (WAIT: 1140 clocks)
        // more or less, and the output filter's phase adds one sample.
        let slack =
            1 + (1200 * u64::from(m.audio_rate())).div_ceil(u64::from(bk::cpu_hz(model))) as i64;
        assert!(
            (samples - expected).abs() <= slack,
            "{context}: {samples} audio samples for 8 frames, expected {expected} ± {slack}"
        );
        // A few single steps, counted, to measure how much runs.
        for _ in 0..100 {
            if m.cpu().is_halted() {
                break;
            }
            m.step_into();
            steps += 1;
        }
        halted += usize::from(m.cpu().is_halted());
        waiting += usize::from(m.cpu().is_waiting());
        in_rom += usize::from(m.cpu().pc() >= 0o100000);
        assert_state_round_trips(&m, &roms, config, &context);
    }
    eprintln!(
        "{model:?}: {trials} random programs; at the end {halted} halted, {waiting} in WAIT, \
         {in_rom} in ROM or I/O space, {:.0} of 100 extra steps run on average",
        steps as f64 / trials as f64
    );
}

#[test]
fn random_code_bk0010() {
    random_code(Model::Bk0010, Config::default(), 60 * scale(), base_seed());
}

#[test]
fn random_code_bk0011m_all_devices() {
    let config = Config {
        fdd: true,
        ay: true,
        covox: true,
        ..Default::default()
    };
    random_code(Model::Bk0011m, config, 60 * scale(), base_seed() ^ 0x11);
}

/// A BK-0011M with every device, a small disk, and the disk ROM busy
/// trying to boot from it; and its state.
fn busy_machine() -> Option<(Roms, Config, Vec<u8>)> {
    let config = Config {
        fdd: true,
        ay: true,
        covox: true,
        ..Default::default()
    };
    let roms = roms(Model::Bk0011m, true)?;
    let mut m = Machine::new(roms.clone(), config);
    m.insert_disk(0, DiskImage::new(vec![0o240; 3 * 5120], false))
        .unwrap();
    frames(&mut m, 40);
    let state = m.save_state();
    Some((roms, config, state))
}

/// Load a damaged state: it must fail cleanly (leaving the machine as it
/// was) or give a machine that runs and saves faithfully. Returns whether
/// it loaded.
fn try_damaged(roms: &Roms, config: Config, bad: &[u8], seed: u64, context: &str) -> bool {
    let mut m = Machine::new(roms.clone(), config);
    let before = m.save_state();
    match m.load_state(bad) {
        Err(_) => {
            assert!(
                m.save_state() == before,
                "{context}: a failed load changed the machine"
            );
            false
        }
        Ok(()) => {
            let mut rng = Rng::new(seed);
            for _ in 0..3 {
                random_keys(&mut m, &mut rng);
                m.run_frame();
            }
            assert_state_round_trips(&m, roms, config, context);
            true
        }
    }
}

/// Every byte of a state outside RAM, the disk and the raw track, set in
/// turn to each of a few edge values.
#[test]
fn damaged_state_fields_sweep() {
    let Some((roms, config, good)) = busy_machine() else {
        return;
    };
    // Header and CPU, then RAM (4-byte length at 46, 128 KB from 50), then
    // the small device fields up to the disk image's data, ..., and the
    // controller's and machine's fields at the end.
    let ram_end = 50 + 128 * 1024;
    let offsets: Vec<usize> = (0..50)
        .chain(ram_end..ram_end + 90)
        .chain(good.len() - 120..good.len())
        .collect();
    let mut loaded = 0;
    // Collect every failure (panics included) before reporting.
    let mut failures = Vec::new();
    std::panic::set_hook(Box::new(|_| {}));
    for &at in &offsets {
        for value in [0x00, 0xff, 0x80, 0x7f, 0x01, good[at] ^ 0x10] {
            let mut bad = good.clone();
            bad[at] = value;
            let context = format!("byte {at} set to {value:#04x}");
            let started = std::time::Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                try_damaged(&roms, config, &bad, at as u64, &context)
            }));
            if started.elapsed().as_secs_f64() > 2.0 {
                failures.push(format!(
                    "{context}: took {:.1} s",
                    started.elapsed().as_secs_f64()
                ));
            }
            match result {
                Ok(ok) => loaded += usize::from(ok),
                Err(e) => {
                    let msg = e
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_default();
                    failures.push(format!("{context}: {msg}"));
                }
            }
        }
    }
    let _ = std::panic::take_hook();
    eprintln!(
        "{loaded} of {} single-byte changes loaded (and ran)",
        offsets.len() * 6
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Damaged save states at random: several changed bytes, truncation and
/// junk at the end.
#[test]
fn damaged_states_at_random() {
    let Some((roms, config, good)) = busy_machine() else {
        return;
    };
    let ram_end = 50 + 128 * 1024;
    let fields: Vec<usize> = (0..50).chain(ram_end..good.len()).collect();
    let mut accepted = 0;
    let trials = 300 * scale();
    for trial in 0..trials {
        let seed = base_seed() ^ 0xdead ^ trial;
        let mut rng = Rng::new(seed);
        let mut bad = good.clone();
        match rng.below(6) {
            0 => bad.truncate(rng.below(good.len() as u64) as usize),
            1 => bad.extend((0..1 + rng.below(8)).map(|_| rng.word() as u8)),
            _ => {
                for _ in 0..1 + rng.below(6) {
                    let at = *rng.pick(&fields);
                    bad[at] = match rng.below(4) {
                        0 => 0xff,
                        1 => 0,
                        2 => bad[at] ^ (1 << rng.below(8)),
                        _ => rng.word() as u8,
                    };
                }
            }
        }
        accepted += usize::from(try_damaged(
            &roms,
            config,
            &bad,
            seed,
            &format!("seed {seed}"),
        ));
    }
    eprintln!("{accepted} of {trials} damaged states loaded (and ran)");
}

/// Random debugger commands, sensible and not: nothing panics, and the
/// machine stays consistent.
#[test]
fn random_debugger_commands() {
    let config = Config {
        ay: true,
        ..Default::default()
    };
    let Some(roms) = roms(Model::Bk0011m, false) else {
        return;
    };
    let mut m = Machine::new(roms.clone(), config);
    frames(&mut m, 30);
    let mut d = Debugger::new();
    let commands = [
        "r", "s", "n", "c", "b", "bc", "w", "wc", "u", "m", "e", "psw", "r0", "r5", "r7", "sp",
        "pc", "h", "?", "help", "x", "", "  ", "S", "q",
    ];
    let mut rng = Rng::new(base_seed() ^ 0xdeb);
    let n = 1500 * scale();
    for i in 0..n {
        let mut line = rng.pick(&commands).to_string();
        for _ in 0..rng.below(4) {
            line.push(' ');
            line += &match rng.below(9) {
                0 => format!("{:o}", random_word(&mut rng)),
                1 => format!("{}.", rng.below(70_000)),
                2 => rng.pick(&["r0", "r3", "sp", "pc", "PC"]).to_string(),
                3 => "all".into(),
                4 => rng
                    .pick(&["9", "-1", "7777777", "1.5", ".", "0x10", "八", ""])
                    .to_string(),
                5 => format!("{:o}", rng.below(40)),
                _ => format!("{:o}", rng.word() & 0o177776),
            };
        }
        let (_, action) = d.command(&mut m, &line);
        if action == Action::Run {
            d.run(&mut m, 3);
        }
        if i % 100 == 0 {
            assert_state_round_trips(
                &m,
                &roms,
                config,
                &format!("after debugger command {i} {line:?}"),
            );
        }
    }
    d.detach(&mut m);
    frames(&mut m, 2);
}

/// Play each game with random keys; at a random moment save the state, and
/// later replay the same keys from that state in a fresh machine: both
/// runs must end in exactly the same state, with the game still running.
/// (This found the VM1 taking a key interrupt before an EMT's trap, which
/// sent Xonix into BASIC when a key landed on its first EMT.)
#[test]
fn games_replay_identically_from_a_mid_game_save() {
    let games: &[(&str, Model)] = &[
        ("klad.bin", Model::Bk0010),
        ("Xonix.bin", Model::Bk0010),
        ("DIGGER_1.6.BIN", Model::Bk0010),
        ("miner.bin", Model::Bk0010),
        ("valley.bin", Model::Bk0010),
        ("PONG.bin", Model::Bk0010),
        ("krakout.bin", Model::Bk0011m),
        ("BoingBall.bin", Model::Bk0011m),
        // Not PONG on the BK-0011M: a BK-0010 game there draws over memory
        // BASIC and BOS use, and BOS's own key handling then crashes it.
    ];
    for (i, &(file, model)) in games.iter().enumerate() {
        let Ok(bytes) = std::fs::read(repo().join("software").join(file)) else {
            eprintln!("skipping {file}: not in software/");
            continue;
        };
        let Some(roms) = roms(model, false) else {
            return;
        };
        let seed = base_seed() ^ (0x6a3e << 8) ^ i as u64;
        let mut rng = Rng::new(seed);
        let config = Config::default();
        let mut a = Machine::new(roms.clone(), config);
        frames(&mut a, 120);
        let bin = BinFile::parse(&bytes).unwrap();
        a.load_bin(&bin).unwrap();
        a.start_at(bin.start_address());

        let total = 200 * scale();
        let save_at = 20 + rng.below(total - 40);
        // Key events per frame, recorded after the save for the replay.
        let mut script: Vec<Vec<(u8, bool, bool)>> = Vec::new();
        let mut saved = None;
        for f in 0..total {
            if f == save_at {
                saved = Some(a.save_state());
            }
            let mut events = Vec::new();
            if rng.chance(3) {
                let key = *rng.pick(PLAY_KEYS);
                events.push((key, rng.chance(2), false));
            }
            for &(key, down, ar2) in &events {
                if down {
                    a.key_down(key, ar2)
                } else {
                    a.key_up(key)
                }
            }
            if f >= save_at {
                script.push(events);
            }
            a.run_frame();
        }

        let mut b = Machine::new(roms.clone(), config);
        b.load_state(&saved.unwrap()).unwrap();
        for events in &script {
            for &(key, down, ar2) in events {
                if down {
                    b.key_down(key, ar2)
                } else {
                    b.key_up(key)
                }
            }
            b.run_frame();
        }
        assert!(!a.cpu().is_halted(), "{file}, seed {seed}: CPU halted");
        assert_eq!(
            a.cpu().state(),
            b.cpu().state(),
            "{file}, seed {seed}: CPU after replay"
        );
        assert!(
            a.save_state() == b.save_state(),
            "{file}, seed {seed}: state after replay"
        );
    }
}

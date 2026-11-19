//! Instruction times on the whole machine against the MiST core's
//! chip-level VM1 model running the same program from RAM with its model
//! of the 037 memory controller (tools/vm1-timing, `MIST=1`,
//! `timing_ram.bin`). Needs the ROM files (see firmware.rs).

use std::path::PathBuf;

use bk::vm1::asm::assemble;
use bk::{Bk0010Rom, Config, Machine, Model, Roms};

fn machine_of(model: Model) -> Option<Machine> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
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

/// The tests of tools/vm1-timing/gen_timing.py: (name, instruction), each
/// run as 100 copies.
const TESTS: &[(&str, &str)] = &[
    ("NOP", "NOP"),
    ("MOV R1,R2", "MOV R1, R2"),
    ("MOV #n,R0", "MOV #123, R0"),
    ("ADD #n,R5", "ADD #27, R5"),
    ("MOV #n,(R3)", "MOV #160, (R3)"),
    ("CLR (R3)", "CLR (R3)"),
    ("TST (R3)", "TST (R3)"),
    ("MOV (R3),R1", "MOV (R3), R1"),
    ("MOV R1,(R3)", "MOV R1, (R3)"),
    ("INC R1", "INC R1"),
    ("CMP R1,R2", "CMP R1, R2"),
    ("ASL R1", "ASL R1"),
    ("SWAB R1", "SWAB R1"),
    ("MOVB (R3),R1", "MOVB (R3), R1"),
    ("MOVB R1,(R3)", "MOVB R1, (R3)"),
    ("XOR R1,R2", "XOR R1, R2"),
    ("MOV @#a,R1", "MOV @#2000, R1"),
    ("MOV R1,@#a", "MOV R1, @#2000"),
    ("BR .+2", "BR .+2"),
    ("BNE taken", "BNE .+2"),
    ("BEQ not taken", "BEQ .+2"),
    ("JSR+RTS", "JSR PC, SUBR"),
];

/// Clocks per instruction on the chip, BK-0010 and BK-0011M, then the SOB
/// loop. (The chip's own numbers come from probe writes around each block
/// minus an empty block, hence the odd hundredths.)
const CHIP_BK0010: [f64; 23] = [
    12.04, 12.04, 28.0, 28.0, 40.0, 28.0, 24.0, 28.0, 36.0, 12.04, 12.04, 12.04, 12.04, 28.0, 36.0,
    12.04, 40.0, 44.0, 16.0, 16.0, 16.0, 72.0, 20.0,
];
const CHIP_BK0011M: [f64; 23] = [
    16.0, 16.0, 32.0, 32.0, 48.0, 37.33, 31.95, 32.0, 40.0, 16.0, 16.0, 16.0, 16.0, 32.0, 40.0,
    16.0, 40.0, 48.0, 16.0, 16.0, 16.0, 80.0, 21.33,
];

/// Run the benchmark: clocks per instruction for each test, then per SOB
/// loop.
fn measure(m: &mut Machine) -> Vec<f64> {
    let mut src = String::from("MOV #2000, R3\nMOV #1, R4\n");
    for (i, (_, insn)) in TESTS.iter().enumerate() {
        src += &format!("B{i}:\n");
        for _ in 0..100 {
            src += &format!("{insn}\n");
        }
    }
    // 1000 and 2000 loops (the assembler reads octal).
    src += "S1: MOV #1750, R0\nL1: SOB R0, L1\nS2: MOV #3720, R0\nL2: SOB R0, L2\n";
    src += "DONE: BR DONE\nSUBR: RTS PC\n";
    let p = assemble(&src, 0o4000).expect("benchmark");
    for (i, w) in p.bytes.chunks(2).enumerate() {
        assert!(m.poke_word(0o4000 + 2 * i as u16, u16::from_le_bytes([w[0], w[1]])));
    }
    m.start_at(0o4000);
    m.cpu_mut().psw = 0o340; // no interrupts
    let marks: Vec<u16> = (0..TESTS.len())
        .map(|i| p.symbol(&format!("B{i}")).unwrap())
        .chain(["S1", "S2", "DONE"].map(|s| p.symbol(s).unwrap()))
        .collect();
    let mut at = vec![None; marks.len()];
    let mut clock = 0u64;
    while at.last().unwrap().is_none() {
        let pc = m.cpu().pc();
        if let Some(i) = marks.iter().position(|&a| a == pc)
            && at[i].is_none()
        {
            at[i] = Some(clock);
        }
        clock += u64::from(m.step());
        assert!(clock < 10_000_000, "the benchmark didn't finish");
    }
    let at: Vec<f64> = at.into_iter().map(|t| t.unwrap() as f64).collect();
    let n = TESTS.len();
    let mut per: Vec<f64> = (0..n).map(|i| (at[i + 1] - at[i]) / 100.0).collect();
    // S1..S2 is 1000 loops, S2..DONE 2000: the difference is 1000 loops.
    per.push(((at[n + 2] - at[n + 1]) - (at[n + 1] - at[n])) / 1000.0);
    per
}

fn compare(model: Model, chip: &[f64; 23]) {
    let Some(mut m) = machine_of(model) else {
        return;
    };
    for _ in 0..20 {
        m.run_frame();
    }
    let ours = measure(&mut m);
    let names = TESTS.iter().map(|t| t.0).chain(["SOB"]);
    let mut report = String::new();
    let mut worst = 0.0f64;
    for ((name, &c), &o) in names.zip(chip).zip(&ours) {
        report += &format!("{name:>14}: chip {c:6.2}  bk {o:6.2}\n");
        worst = worst.max((c - o).abs());
    }
    let (sum_chip, sum_ours): (f64, f64) = (chip.iter().sum(), ours.iter().sum());
    report += &format!("{:>14}: chip {sum_chip:6.2}  bk {sum_ours:6.2}\n", "sum");
    eprintln!("{model:?}\n{report}");
    assert!(
        worst <= 0.25,
        "{model:?}: off by {worst:.2} clocks\n{report}"
    );
}

#[test]
fn bk0010_instruction_times_match_the_chip() {
    compare(Model::Bk0010, &CHIP_BK0010);
}

#[test]
fn bk0011m_instruction_times_match_the_chip() {
    compare(Model::Bk0011m, &CHIP_BK0011M);
}

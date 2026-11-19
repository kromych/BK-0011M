//! Take a BK-0010.01 to the start of a tape load, for running on the
//! chip-level CPU model (tools/vm1-timing):
//!
//!     cargo run --release -p bk --example tape_snapshot -- ROMS TAPE NAME OUT
//!
//! Boots BASIC, types MONIT, M and NAME with the recording TAPE in the
//! recorder, and stops as the monitor's tape routine (EMT 36) is entered,
//! before the motor has turned the tape. Writes into the folder OUT:
//!
//! - `mem.img`: the 64 KB the CPU sees (RAM, ROM, zeros for I/O), with a
//!   stub at 077000 (the bottom of the screen) that loads the registers and
//!   returns into the tape routine with RTI;
//! - `sel1`: the SEL1 value to start the chip at the stub (octal);
//! - `tape.lvl`: the recording as the emulator hears it: the sample rate
//!   (4 bytes, little-endian), then one byte per sample, 1 while high.

use std::path::PathBuf;

use bk::{Bk0010Rom, Break, Config, Machine, Roms, TapeSignal};

const STUB: u16 = 0o77000;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [roms, tape, name, out] = args.as_slice() else {
        eprintln!("usage: tape_snapshot ROMS TAPE NAME OUT");
        std::process::exit(2);
    };
    let roms = Roms::load_bk0010(&PathBuf::from(roms), Bk0010Rom::Basic).expect("ROMs");
    let mut m = Machine::new(roms, Config::default());
    let wav = bk::parse_recording(std::fs::read(tape).expect("tape")).expect("recording");
    let signal = TapeSignal::from_wav(&wav);
    m.insert_tape(Some(signal.clone()));
    for _ in 0..150 {
        m.run_frame();
    }
    m.type_text("MONIT\n");
    for _ in 0..50 {
        m.run_frame();
    }
    // The tape routine, from the monitor's EMT table.
    let tape_routine = m.peek_word(0o100036).expect("EMT 36 slot");
    m.type_text("M\n");
    m.set_breakpoints(&[tape_routine]);
    // Typing the name ends with ВВОД, which starts the load.
    let mut text = String::new();
    for c in name.chars() {
        match bk::russian_key(c) {
            Some(code) => {
                text.push('\x0e');
                text.push(char::from(code));
                text.push('\x0f');
            }
            None => text.push(c),
        }
    }
    text.push('\n');
    m.type_text(&text);
    let mut hit = false;
    for _ in 0..500 {
        if m.cpu().pc() == tape_routine {
            hit = true;
            break;
        }
        if let Some(Break::Breakpoint(_)) = m.run_frame() {
            hit = true;
            break;
        }
    }
    assert!(hit, "the monitor never called its tape routine");
    assert_eq!(m.tape_position(), Some(0), "the tape moved before the load");

    let cpu = m.cpu().state();
    let mut mem = vec![0u8; 0x10000];
    for a in (0..0o177600u32).step_by(2) {
        if let Some(w) = m.peek_word(a as u16) {
            mem[a as usize..a as usize + 2].copy_from_slice(&w.to_le_bytes());
        }
    }
    // Keyboard status: no key, interrupts masked as the monitor leaves them.
    mem[0o177660] = 0o100;
    let src = format!(
        "
        MOV #{sp:o}, SP
        MOV #{psw:o}, -(SP)
        MOV #{pc:o}, -(SP)
        MOV #{r0:o}, R0
        MOV #{r1:o}, R1
        MOV #{r2:o}, R2
        MOV #{r3:o}, R3
        MOV #{r4:o}, R4
        MOV #{r5:o}, R5
        RTI",
        sp = cpu.r[6],
        psw = cpu.psw,
        pc = cpu.r[7],
        r0 = cpu.r[0],
        r1 = cpu.r[1],
        r2 = cpu.r[2],
        r3 = cpu.r[3],
        r4 = cpu.r[4],
        r5 = cpu.r[5],
    );
    let stub = bk::vm1::asm::assemble(&src, STUB).expect("stub");
    mem[usize::from(STUB)..usize::from(STUB) + stub.bytes.len()].copy_from_slice(&stub.bytes);

    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).expect("output folder");
    std::fs::write(out.join("mem.img"), &mem).expect("mem.img");
    // SEL1: the start address in the high byte; bit 7 (no serial line
    // waiting) and bit 6 (no key held) set.
    std::fs::write(out.join("sel1"), format!("{:06o}\n", STUB | 0o300)).expect("sel1");
    let levels = signal.to_wav();
    let mut lvl = levels.rate.to_le_bytes().to_vec();
    lvl.extend(levels.samples.iter().map(|&s| u8::from(s > 0.0)));
    std::fs::write(out.join("tape.lvl"), lvl).expect("tape.lvl");
    println!(
        "{}: at {:06o} (EMT 36), SP {:06o}; {} samples at {} Hz",
        out.display(),
        cpu.r[7],
        cpu.r[6],
        levels.samples.len(),
        levels.rate
    );
}

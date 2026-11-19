//! Speaker and tape, through the real firmware. Needs the ROM files in
//! `<repo>/roms` (or `BK_ROMS`); skipped when they're missing.

use std::path::PathBuf;

use bk::vm1::asm::assemble;
use bk::{BinFile, Bk0010Rom, Config, Machine, MemFiles, Model, Roms};

fn machine_of(model: Model) -> Option<Machine> {
    machine_with(model, Config::default())
}

fn machine_with(model: Model, config: Config) -> Option<Machine> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    let roms = match model {
        Model::Bk0011m => Roms::load_bk0011m(&dir, false),
        Model::Bk0010 => Roms::load_bk0010(&dir, Bk0010Rom::Basic),
    };
    match roms {
        Ok(roms) => Some(Machine::new(roms, config)),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn frames(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.run_frame();
    }
}

/// Boot, load `src` (assembled at 1000) and start it.
fn run_program(model: Model, src: &str) -> Option<(Machine, bk::vm1::asm::Program)> {
    let mut m = machine_of(model)?;
    frames(&mut m, 150);
    let p = assemble(src, 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes.clone(),
    })
    .unwrap();
    m.start_at(0o1000);
    Some((m, p))
}

/// A square wave from a known instruction loop must come out at the pitch
/// the chip gives: this checks the CPU clock, the bus-cycle timing and the
/// audio resampler together.
#[test]
fn speaker_pitch_matches_instruction_timing() {
    const N: u32 = 100;
    let src = format!(
        "        MOV #177716, R1
         LOOP:   MOV #100, (R1)
                 MOV #{N:o}, R0
         D1:     SOB R0, D1
                 CLR (R1)
                 MOV #{N:o}, R0
         D2:     SOB R0, D2
                 BR LOOP"
    );
    let Some((mut m, _)) = run_program(Model::Bk0010, &src) else {
        return;
    };
    frames(&mut m, 5);
    m.take_audio();
    frames(&mut m, 50); // one second
    let samples = m.take_audio();
    let rising = samples
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count() as f64;
    // The chip-level CPU with MiST's memory timing runs this loop (from
    // RAM, N = 100) in 2053 + 2079 clocks between its writes to 177716
    // (tools/vm1-timing): 20 per SOB, 132 for the rest.
    let period = f64::from(132 + 40 * N);
    let expected = 3_000_000.0 / period;
    assert!(
        (rising - expected).abs() <= 2.0,
        "{rising} Hz, expected {expected:.1} Hz"
    );
    assert_eq!(samples.len(), bk::AUDIO_RATE as usize);
}

/// With the tape sound on, a tape that plays (the motor running) is heard;
/// with it off (the default), only the BK's own sound is.
#[test]
fn the_tape_is_heard_only_with_the_tape_sound_on() {
    let Some((mut m, _)) = run_program(Model::Bk0010, "MOV #20, @#177716\nDONE: BR DONE") else {
        return;
    };
    let file = BinFile {
        address: 0o1000,
        data: vec![0o125; 200],
    };
    let loudness = |m: &mut Machine, on: bool| {
        m.insert_tape(Some(bk::encode_tape("NOISE", &file).unwrap()));
        m.set_tape_sound(on);
        frames(m, 50);
        let s = m.take_audio();
        s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32
    };
    frames(&mut m, 5);
    m.take_audio();
    let off = loudness(&mut m, false);
    let on = loudness(&mut m, true);
    assert!(m.tape_motor_on());
    assert!(off < 1e-6, "silent without the tape sound: {off}");
    assert!(on > 1e-3, "the tape is heard: {on}");
}

/// Save a BASIC program to tape through the ROM, then load it back from the
/// recording, also through the ROM.
#[test]
fn basic_program_survives_a_tape_round_trip() {
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    frames(&mut m, 150);
    m.type_text("10 POKE 16000,4321\n");
    m.start_recording();
    m.type_text("CSAVE \"T\"\n");
    frames(&mut m, 200);
    let mut tape = m.take_recording().unwrap();
    assert!(tape.len_seconds() > 1.0, "nothing recorded");

    m.type_text("NEW\nPOKE 16000,0\n");
    tape.rewind();
    m.insert_tape(Some(tape));
    m.type_text("CLOAD \"T\"\n");
    frames(&mut m, 300);
    m.type_text("RUN\n");
    assert_eq!(m.peek_word(16000), Some(4321));
}

/// Fast tape on the BK-0010: EMT 36 read and write go to the file store.
#[test]
fn bk0010_fast_tape() {
    let src = "        MOV #RBLK, R1
                 EMT 36
                 MOV #WBLK, R1
                 EMT 36
         DONE:   BR DONE
         RBLK:   .BYTE 3, 377
                 .WORD 0, 0
                 .ASCII /GAME            /
         RINFO:  .WORD 0, 0
         RNAME:  .ASCII /                /
         WBLK:   .BYTE 2, 377
                 .WORD 2000, 4
                 .ASCII /SAVED           /";
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    let mut store = MemFiles::default();
    store.files.push((
        "GAME".into(),
        BinFile {
            address: 0o2000,
            data: vec![1, 2, 3, 4],
        },
    ));
    m.set_tape_files(Some(Box::new(store)));
    frames(&mut m, 150);
    let p = assemble(src, 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes.clone(),
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 5);

    let rblk = p.symbol("RBLK").unwrap();
    assert_eq!(m.cpu().pc(), p.symbol("DONE").unwrap());
    assert_eq!(
        m.peek_word(0o2000),
        Some(0x0201),
        "file loaded at its own address"
    );
    assert_eq!(
        m.peek_word(rblk).map(|w| w >> 8),
        Some(0),
        "result byte: OK"
    );
    assert_eq!(m.peek_word(p.symbol("RINFO").unwrap()), Some(0o2000));
    assert_eq!(m.peek_word(p.symbol("RINFO").unwrap() + 2), Some(4));
}

/// Fast tape on the BK-0011M: .BMB10 (JSR PC,@140242, which calls through
/// the slot) reads, and a missing file falls through to the ROM.
#[test]
fn bk0011m_fast_tape() {
    let src = "        MOV #RAREA, R0
                 JSR PC, @140242
                 MFPS R4
                 MOV #MAREA, R0
                 JSR PC, @140242
                 MFPS R5
         DONE:   BR DONE
         RAREA:  .BYTE 1, 0
                 .WORD 0, 0
                 .ASCII /GAME            /
                 .BYTE 377, 377
         RINFO:  .WORD 0, 0
                 .ASCII /                /
         MAREA:  .BYTE 1, 0
                 .WORD 0, 0
                 .ASCII /NOSUCH          /
                 .BYTE 377, 377
                 .WORD 0, 0
                 .ASCII /                /";
    let Some(mut m) = machine_of(Model::Bk0011m) else {
        return;
    };
    let mut store = MemFiles::default();
    store.files.push((
        "GAME".into(),
        BinFile {
            address: 0o3000,
            data: vec![7, 8],
        },
    ));
    m.set_tape_files(Some(Box::new(store)));
    frames(&mut m, 300);
    let p = assemble(src, 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes.clone(),
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 5);
    assert_eq!(m.peek_word(0o3000), Some(0x0807), "loaded");
    assert_eq!(m.peek_word(p.symbol("RINFO").unwrap()), Some(0o3000));
    assert_eq!(m.cpu().r[4] & 1, 0, "C clear after a good read");
    // A missing file falls through to the ROM, which waits for the tape.
    assert!(m.tape_motor_on() || m.cpu().pc() != p.symbol("DONE").unwrap());
}

/// A real cassette: the first 50 seconds of a commercial BK-0010.01 game
/// collection (archive.pdp-11.org.ru, TAPES/another_tapes/4/001.mp3,
/// converted to WAV), in the git-ignored `software/` folder. A program calls
/// the monitor's tape routine (EMT 36) with its own block, so the result
/// fields can be trusted, and loads the first file into a spare area.
///
/// The header must decode exactly. The data decodes the same as from
/// another recording of the same file for its first 144 bytes, but these
/// old recordings have dropouts, so the ROM rejects the checksum (result 2);
/// that is the recording, not the emulator, and isn't asserted here.
#[test]
fn real_tape_header_decodes_through_the_rom() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../software/tape_komplekt17_start.wav");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} not found", path.display());
        return;
    };
    let src = "        MOV #BLK, R1
                 EMT 36
         DONE:   BR DONE
         BLK:    .BYTE 3, 377          ; read; result
                 .WORD 20000, 0        ; load at 20000 instead of the file's address
                 .ASCII /R&O-REKL1       /
         INFO:   .WORD 0, 0            ; file's own address and length
         NAME:   .ASCII /                /";
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    frames(&mut m, 150);
    let p = assemble(src, 0o37000).unwrap();
    m.load_bin(&BinFile {
        address: 0o37000,
        data: p.bytes.clone(),
    })
    .unwrap();
    m.insert_tape(Some(bk::TapeSignal::from_wav(
        &bk::Wav::parse(&bytes).unwrap(),
    )));
    m.start_at(0o37000);
    for _ in 0..2500 {
        m.run_frame();
        if m.cpu().pc() == p.symbol("DONE").unwrap() {
            break;
        }
    }
    let word = |a| m.peek_word(a).unwrap();
    let info = p.symbol("INFO").unwrap();
    let name: String = (0..16u16)
        .map(|i| (word(p.symbol("NAME").unwrap() + (i & !1)) >> (8 * (i & 1))) as u8 as char)
        .collect();
    assert_eq!(m.cpu().pc(), p.symbol("DONE").unwrap(), "EMT 36 returned");
    assert_eq!(
        name.trim_end(),
        "R&O-REKL1",
        "file name from the tape header"
    );
    assert_eq!(
        (word(info), word(info + 2)),
        (0o316, 600),
        "address and length from the header"
    );
    // Words 0-71 decode the same from another recording of this file; the
    // other recording drops out at word 72, this one carries on with code.
    assert_eq!(word(0o20000 + 2 * 72), 0o012700, "MOV #300, R0");
    assert_eq!(word(0o20000 + 2 * 73), 0o000300);
}

/// Count rising zero crossings of the audio (after the DC blocker settles).
fn rising_crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count()
}

/// A program sets AY channel A to a 440 Hz tone the way BK music players
/// do: a word write selects the register, a byte write sets it, both
/// inverted. The sound comes out at that pitch.
#[test]
fn ay_tone_through_the_parallel_port() {
    let config = bk::Config {
        ay: true,
        ..Default::default()
    };
    let Some(mut m) = machine_with(Model::Bk0011m, config) else {
        return;
    };
    frames(&mut m, 150);
    // Period = 1714286 / (16 * 440) = 243.5: 243 = 0o363.
    let src = "        MOV #TABLE, R1
                 MOV #4, R2
         LOOP:   MOVB (R1)+, R0
                 COM R0
                 MOV R0, @#177714      ; select (inverted)
                 MOVB (R1)+, R0
                 COM R0
                 MOVB R0, @#177714     ; write (inverted)
                 SOB R2, LOOP
         DONE:   BR DONE
         TABLE:  .BYTE 0, 363, 1, 0, 7, 76, 10, 17   ; period 243; tone A only; volume 15";
    let p = assemble(src, 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 10);
    m.take_audio();
    frames(&mut m, 50); // one second
    let audio = m.take_audio();
    let hz = rising_crossings(&audio);
    let expected = 1_714_286.0 / (16.0 * 243.0);
    assert!(
        (hz as f64 - expected).abs() < 3.0,
        "{hz} Hz, expected {expected:.1}"
    );

    // Without the chip, the same program is silent.
    let Some(mut m) = machine_with(Model::Bk0011m, bk::Config::default()) else {
        return;
    };
    frames(&mut m, 150);
    m.load_bin(&BinFile {
        address: 0o1000,
        data: assemble(src, 0o1000).unwrap().bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 10);
    m.take_audio();
    frames(&mut m, 50);
    assert!(m.take_audio().iter().all(|s| s.abs() < 1e-3));
}

/// A program writes a 1 kHz square wave to the Covox: alternating bytes
/// 0 and 377, each held for half a millisecond.
#[test]
fn covox_plays_bytes_written_to_the_port() {
    let config = bk::Config {
        covox: true,
        ..Default::default()
    };
    let Some(mut m) = machine_with(Model::Bk0010, config) else {
        return;
    };
    frames(&mut m, 150);
    m.take_audio();
    // Reading 177714 returns the joystick input, so keep the byte in R1.
    let p = assemble(
        "LOOP: COMB R1\nMOVB R1, @#177714\nMOV #N, R0\nWAIT1: SOB R0, WAIT1\nBR LOOP\nN = 106",
        0o1000,
    )
    .unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 10);
    m.take_audio();
    frames(&mut m, 50);
    let audio = m.take_audio();
    let hz = rising_crossings(&audio);
    // Each half-period: COMB 12 + MOVB R1,@# 44 + MOV # 28 + 70 SOBs at 20
    // (N = 106 octal) + BR 16 = 1500 clocks at 3 MHz: a period of 1 ms.
    let expected = 3_000_000.0 / (2.0 * 1500.0);
    assert!(
        (hz as f64 - expected).abs() / expected < 0.05,
        "{hz} Hz, expected about {expected:.0}"
    );
    let peak = audio.iter().fold(0f32, |a, s| a.max(s.abs()));
    assert!(peak > 0.1, "full-scale bytes are loud: peak {peak}");
}

/// A joystick on the parallel port: the input lines read at 177714.
#[test]
fn joystick_lines_read_at_the_port() {
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    frames(&mut m, 150);
    let p = assemble("LOOP: MOV @#177714, @#2000\nBR LOOP", 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes,
    })
    .unwrap();
    m.start_at(0o1000);
    frames(&mut m, 1);
    assert_eq!(m.peek_word(0o2000), Some(0), "nothing connected");
    m.set_port_input(0b10_0001); // up and fire
    frames(&mut m, 1);
    assert_eq!(m.peek_word(0o2000), Some(0b10_0001));
}

/// The speaker is a 3-bit DAC on the tape bits 6, 5 and (BK-0011M) 2: a
/// program toggling bit 5 alone is heard on both models, bit 2 only on
/// the BK-0011M.
#[test]
fn speaker_follows_tape_bits_5_and_2() {
    let toggling = |model: Model, bit: u16| -> f32 {
        let Some(mut m) = machine_of(model) else {
            return -1.0;
        };
        frames(&mut m, 150);
        let src = format!(
            "LOOP: MOV #{bit:o}, @#177716\nMOV #100, R0\nW1: SOB R0, W1\nCLR @#177716\nMOV #100, R0\nW2: SOB R0, W2\nBR LOOP"
        );
        let p = assemble(&src, 0o1000).unwrap();
        m.load_bin(&BinFile {
            address: 0o1000,
            data: p.bytes,
        })
        .unwrap();
        m.start_at(0o1000);
        frames(&mut m, 10);
        m.take_audio();
        frames(&mut m, 10);
        m.take_audio().iter().fold(0f32, |a, s| a.max(s.abs()))
    };
    for model in [Model::Bk0010, Model::Bk0011m] {
        let bit5 = toggling(model, 0o40);
        if bit5 < 0.0 {
            return;
        }
        assert!(bit5 > 0.05, "{model:?}: bit 5 is heard ({bit5})");
    }
    assert!(
        toggling(Model::Bk0010, 0o4) < 1e-3,
        "BK-0010: bit 2 is silent"
    );
    assert!(
        toggling(Model::Bk0011m, 0o4) > 0.02,
        "BK-0011M: bit 2 is heard"
    );
}

/// With `irq2_50hz`, a BK-0010 gets a 50 Hz interrupt on vector 100 once
/// a handler is there: FOCAL's FCLK() counts them (adding 256 to the word
/// at 1746 each time) while a program runs; without it, the count stays.
#[test]
fn bk0010_irq2_option_drives_focals_clock() {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    for on in [false, true] {
        let Ok(roms) = Roms::load_bk0010(&dir, Bk0010Rom::Focal) else {
            return;
        };
        let mut m = Machine::new(
            roms,
            Config {
                irq2_50hz: on,
                ..Default::default()
            },
        );
        frames(&mut m, 200);
        m.type_text("1.1 S A=FCLK()\n1.2 G 1.1\nG\n");
        let counter = |m: &Machine| (m.peek_word(0o1750).unwrap(), m.peek_word(0o1746).unwrap());
        let before = counter(&m);
        frames(&mut m, 100);
        let after = counter(&m);
        assert_eq!(
            after != before,
            on,
            "irq2_50hz {on}: {before:?} then {after:?}"
        );
    }
}

/// A printer on the parallel port: BASIC's LPRINT goes through the ROM's
/// own driver on both machines (strobe and ready on bit 8 on the
/// BK-0010.01, bit 14 under BOS on the BK-0011M).
#[test]
fn lprint_reaches_the_printer() {
    for model in [Model::Bk0010, Model::Bk0011m] {
        let config = Config {
            printer: true,
            ..Default::default()
        };
        let Some(mut m) = machine_with(model, config) else {
            return;
        };
        frames(&mut m, 200);
        m.type_text("LPRINT \"PRINTED\"\n");
        let text = String::from_utf8_lossy(&m.take_printed()).into_owned();
        assert!(text.contains("PRINTED"), "{model:?}: {text:?}");
    }
}

/// The BK-0010's serial line, through the monitor's own driver: EMT 46
/// sends a block, EMT 50 receives one, at the fastest and a slow speed.
#[test]
fn serial_line_through_the_monitor() {
    for speed in [0, 5] {
        let config = Config {
            serial: true,
            ..Default::default()
        };
        let Some(mut m) = machine_with(Model::Bk0010, config) else {
            return;
        };
        frames(&mut m, 150);
        // Typing and the bell don't put anything on the line.
        m.type_text("PRINT 1\n");
        m.type_text("? CHR$(7)\n");
        frames(&mut m, 20);
        assert_eq!(m.take_serial(), b"", "idle line");

        let p = assemble(
            &format!(
                "
            START:  MOV #{speed:o}, R0
                    EMT 40
                    MOV #MSG, R1
                    MOV #5, R2
                    EMT 46
                    MOV #BUF, R1
                    MOV #4, R2
                    EMT 50
            DONE:   BR DONE
            MSG:    .ASCII /HELLO/
                    .EVEN
            BUF:    .WORD 0, 0"
            ),
            0o1000,
        )
        .unwrap();
        m.load_bin(&BinFile {
            address: 0o1000,
            data: p.bytes.clone(),
        })
        .unwrap();
        m.serial_send(b"\x00\xff\x5a\x81");
        m.start_at(0o1000);
        let done = p.symbol("DONE").unwrap();
        for _ in 0..200 {
            m.run_frame();
            if m.cpu().pc() == done {
                break;
            }
        }
        assert_eq!(m.cpu().pc(), done, "speed {speed}: the program finished");
        assert_eq!(m.take_serial(), b"HELLO", "speed {speed}");
        let buf = p.symbol("BUF").unwrap();
        let got = [m.peek_word(buf).unwrap(), m.peek_word(buf + 2).unwrap()];
        assert_eq!(got, [0xff00, 0x815a], "speed {speed}");
        assert_eq!(m.serial_pending(), 0);
    }
}

/// Fast-forward follows the tape: busy while the motor runs with a
/// recording to play, and not once the recording has run out.
#[test]
fn tape_stops_being_busy_at_its_end() {
    let Some((mut m, _)) = run_program(
        Model::Bk0010,
        "
        START:  CLR @#177716        ; motor on
        DONE:   BR DONE",
    ) else {
        return;
    };
    let file = BinFile {
        address: 0o1000,
        data: vec![1, 2, 3, 4],
    };
    m.insert_tape(Some(bk::encode_tape("T", &file).unwrap()));
    frames(&mut m, 2);
    assert!(m.tape_busy(), "the recording plays");
    frames(&mut m, 400);
    assert!(m.tape_ended());
    assert!(!m.tape_busy(), "nothing left to play");
}

/// A copy-protected game from Manwe's reconstructed tapes, loaded the way
/// its page says (the monitor's M and its name) from the gzipped WAV: its
/// loader takes over from the ROM and the title comes up in red.
#[test]
fn a_protected_tape_loads_through_its_own_loader() {
    let tape = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../software/tapes/protected/New-Revolution.wav.gz");
    let Ok(bytes) = std::fs::read(&tape) else {
        eprintln!("skipping: {} missing", tape.display());
        return;
    };
    let Some(mut m) = machine_of(Model::Bk0010) else {
        return;
    };
    frames(&mut m, 150);
    let wav = bk::parse_recording(bytes).unwrap();
    m.insert_tape(Some(bk::TapeSignal::from_wav(&wav)));
    m.type_text("MONIT\n");
    frames(&mut m, 50);
    m.type_text("M\nNEW REVOLUTION\n");
    frames(&mut m, 3500);
    let mut screen = vec![0; bk::SCREEN_WIDTH * bk::SCREEN_HEIGHT];
    m.render(&mut screen);
    let red = screen.iter().filter(|&&p| p == 0xFF0000).count();
    assert!(red > 5000, "title not on screen ({red} red pixels)");
}

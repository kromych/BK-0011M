//! Converting between `.bin` files and tape recordings, outside a running
//! session.
//!
//! - [`encode_tape`] writes a file in the standard BK tape format, with the
//!   pulse lengths of the BK-0010 monitor's own tape routine, so a real BK
//!   (or the emulator) can load it.
//! - [`read_tape_files`] reads every file on a recording through the
//!   BK-0010 monitor ROM itself, in an emulated BK-0010, so it decodes
//!   exactly as a real machine would.

use crate::bin::BinFile;
use crate::roms::{Model, Roms};
use crate::tape::TapeSignal;
use crate::{Config, Machine};

/// The BK-0010's CPU clock, which the pulse lengths below are counted in.
const CPU_HZ: u32 = 3_000_000;

// Half-periods in CPU clocks, from the instruction timings of the monitor
// ROM's tape loops (116404–116620). Each pulse is a high half, then a low
// half.
/// Pilot pulse: 23 and 22 SOB iterations plus the loop's other instructions.
const PILOT: (u32, u32) = (528, 528);
/// The long synchronization pulse after a pilot.
const SYNC: (u32, u32) = (2168, 2140);
/// A data pulse for a 1 bit (twice a 0's).
const ONE: (u32, u32) = (1088, 1088);
/// A data pulse for a 0 bit.
const ZERO: (u32, u32) = (548, 544);
/// The pulse that follows every bit's data pulse.
const TAIL: (u32, u32) = (548, 556);

/// Pilot pulses before the header, before the data, and after the file.
const LEADER_PULSES: u32 = 4096;
const SHORT_PILOT: u32 = 8;
const TRAILER_PULSES: u32 = 256;

/// Silence before and after the file, in CPU clocks (0.25 s).
const SILENCE: u32 = CPU_HZ / 4;

struct Encoder {
    tape: TapeSignal,
}

impl Encoder {
    fn pulse(&mut self, (high, low): (u32, u32)) {
        self.tape.record(high, CPU_HZ, true);
        self.tape.record(low, CPU_HZ, false);
    }

    /// A run of pilot pulses, the sync pulse and a 1 bit (116404–116472).
    fn pilot(&mut self, pulses: u32) {
        for _ in 0..pulses {
            self.pulse(PILOT);
        }
        self.pulse(SYNC);
        self.bit(true);
    }

    fn bit(&mut self, one: bool) {
        self.pulse(if one { ONE } else { ZERO });
        self.pulse(TAIL);
    }

    /// Bytes, each least significant bit first (116504–116620).
    fn bytes(&mut self, data: &[u8]) {
        for &b in data {
            for i in 0..8 {
                self.bit(b >> i & 1 != 0);
            }
        }
    }
}

/// The checksum the ROM keeps: the 16-bit sum of the bytes, with the carry
/// added back in (116622).
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u16 = 0;
    for &b in data {
        let (s, carry) = sum.overflowing_add(u16::from(b));
        sum = s.wrapping_add(u16::from(carry));
    }
    sum
}

/// A tape recording of `file` under `name` (up to 16 characters, KOI-8;
/// longer names are cut, shorter ones padded with spaces).
pub fn encode_tape(name: &str, file: &BinFile) -> Result<TapeSignal, String> {
    let mut name_bytes = Vec::new();
    for c in name.chars() {
        name_bytes
            .push(vm1::koi8::encode(c).ok_or_else(|| format!("{c:?} can't be in a tape name"))?);
    }
    name_bytes.resize(16, b' ');
    let len = u16::try_from(file.data.len()).map_err(|_| "the file is longer than 65535 bytes")?;
    let mut header = Vec::with_capacity(20);
    header.extend(file.address.to_le_bytes());
    header.extend(len.to_le_bytes());
    header.extend(&name_bytes[..16]);

    let mut e = Encoder {
        tape: TapeSignal::default(),
    };
    e.tape.record(SILENCE, CPU_HZ, false);
    e.pilot(LEADER_PULSES);
    e.pilot(SHORT_PILOT);
    e.bytes(&header);
    e.pilot(SHORT_PILOT);
    e.bytes(&file.data);
    e.bytes(&checksum(&file.data).to_le_bytes());
    e.pilot(TRAILER_PULSES);
    e.tape.record(SILENCE, CPU_HZ, false);
    // Recording moved the tape to its end; wind it back to play it.
    e.tape.rewind();
    Ok(e.tape)
}

/// A file read from a recording.
#[derive(Clone, Debug)]
pub struct TapeFile {
    /// The name on the tape, trailing spaces removed.
    pub name: String,
    pub file: BinFile,
    /// The ROM accepted the checksum.
    pub checksum_ok: bool,
    /// The length in the file's header.
    pub length: u16,
    /// The ROM read the whole file. When false, `file.data` holds the
    /// bytes the ROM had read, and `problem` says why it stopped.
    pub complete: bool,
    /// Why an incomplete file wasn't read to its end.
    pub problem: Option<String>,
}

/// How a run of the reader program ended.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    /// The ROM's tape routine returned.
    Done,
    /// The recording ran out first.
    Ended,
    /// The tape stood still (the motor off) for [`STALL_FRAMES`]: the ROM
    /// stopped reading, or what it loaded took the machine over.
    Stalled,
}

/// Frames the tape may stand still before a read counts as stalled (5 s).
const STALL_FRAMES: u32 = 250;

/// Where the reader program sits: the top of the BK-0010's RAM (the last
/// lines of the screen), out of the way of files loaded at 1000.
const PROGRAM: u16 = 0o77000;
/// Files are loaded here, whatever their own address.
const LOAD_AT: u16 = 0o1000;

/// Read every file on `tape` through the BK-0010 monitor's tape routine
/// (EMT 36). `roms` must be a BK-0010 set. Each file is found by asking for
/// a name no file has (the ROM then reports the name it found), and read
/// again from its start under that name. Stops at the end of the tape. A
/// file that can't be read whole (the recording ends inside it, the ROM
/// stops reading it, or it is longer than the reader has room for) comes
/// back marked incomplete, with the reason.
pub fn read_tape_files(roms: Roms, tape: TapeSignal) -> Result<Vec<TapeFile>, String> {
    if roms.model() != Model::Bk0010 {
        return Err("reading a tape needs the BK-0010 ROMs".into());
    }
    let mut m = Machine::new(roms, Config::default());
    for _ in 0..150 {
        m.run_frame();
    }
    let src = "
        START:  MOV #BLK, R1
                EMT 36
        DONE:   BR DONE
        BLK:    .BYTE 3, 0              ; read; result
                .WORD 1000, 0           ; load at 1000; length from the tape
        NAME:   .ASCII /                /
        INFO:   .WORD 0, 0              ; the file's own address and length
        FOUND:  .ASCII /                /";
    let p = vm1::asm::assemble(src, PROGRAM).map_err(|e| e.to_string())?;
    let sym = |n: &str| p.symbol(n).expect("label in the reader program");
    let (start, done, blk, name, info, found) = (
        sym("START"),
        sym("DONE"),
        sym("BLK"),
        sym("NAME"),
        sym("INFO"),
        sym("FOUND"),
    );
    m.load_bin(&BinFile {
        address: PROGRAM,
        data: p.bytes.clone(),
    })?;
    m.insert_tape(Some(tape));

    let mut files = Vec::new();
    let set_name = |m: &mut Machine, bytes: &[u8; 16]| {
        for (i, pair) in bytes.chunks(2).enumerate() {
            m.poke_word(name + 2 * i as u16, u16::from_le_bytes([pair[0], pair[1]]));
        }
    };
    let run = |m: &mut Machine| -> Run {
        m.start_at(start);
        let (mut last, mut still) = (m.tape_position(), 0);
        loop {
            m.run_frame();
            if m.cpu().pc() == done {
                return Run::Done;
            }
            if m.tape_ended() {
                return Run::Ended;
            }
            let at = m.tape_position();
            if at == last {
                still += 1;
                if still >= STALL_FRAMES {
                    return Run::Stalled;
                }
            } else {
                (last, still) = (at, 0);
            }
        }
    };
    let bytes_at = |m: &Machine, addr: u16, n: u16| -> Vec<u8> {
        (0..n)
            .map(|i| (m.peek_word((addr + i) & !1).unwrap_or(0) >> (8 * ((addr + i) & 1))) as u8)
            .collect()
    };
    // Where the last stall left the tape, so a stall that recurs in the
    // same place ends the reading instead of repeating forever.
    let mut stalled_at = None;
    loop {
        // Find the next file: a name that can't be on tape makes the ROM
        // stop after the header with result 1 and the found name in FOUND.
        let before = m.tape_position().unwrap_or(0);
        set_name(&mut m, &[0o377; 16]);
        match run(&mut m) {
            Run::Done => {}
            Run::Ended => break,
            Run::Stalled => {
                // Look on from where the tape stopped.
                let at = m.tape_position();
                if stalled_at == Some(at) {
                    break;
                }
                stalled_at = Some(at);
                continue;
            }
        }
        let result = m.peek_word(blk).unwrap_or(0) >> 8;
        if result != 1 {
            // Not a header the ROM could take: look on from here, if the
            // tape moved at all.
            if m.tape_position().unwrap_or(0) == before {
                break;
            }
            continue;
        }
        let found_name: [u8; 16] = bytes_at(&m, found, 16).try_into().unwrap();
        let address = m.peek_word(info).unwrap_or(0);
        let len = m.peek_word(info + 2).unwrap_or(0);
        let text: String = found_name
            .iter()
            .map(|&b| vm1::koi8::decode(b).unwrap_or('?'))
            .collect();
        let text = text.trim_end().to_string();
        let after_header = m.tape_position().unwrap_or(0);
        let incomplete = |data: Vec<u8>, problem: String| TapeFile {
            name: text.clone(),
            file: BinFile { address, data },
            checksum_ok: false,
            length: len,
            complete: false,
            problem: Some(problem),
        };
        if u32::from(LOAD_AT) + u32::from(len) > u32::from(PROGRAM) {
            files.push(incomplete(
                Vec::new(),
                format!(
                    "its header says {len} bytes, more than the reader has room for ({})",
                    PROGRAM - LOAD_AT
                ),
            ));
            continue;
        }
        // Back to the start of that file and read it for real.
        m.seek_tape(before);
        set_name(&mut m, &found_name);
        let outcome = run(&mut m);
        if outcome != Run::Done {
            // The ROM was shifting bits into the byte at R1, so the bytes
            // before it are whole (unless it was still in the header or the
            // checksum).
            let at = m.cpu().r[1];
            let got = if (LOAD_AT..=LOAD_AT.saturating_add(len)).contains(&at) {
                at - LOAD_AT
            } else {
                0
            };
            let data = bytes_at(&m, LOAD_AT, got);
            if outcome == Run::Ended {
                files.push(incomplete(data, "the recording ends inside it".into()));
                break;
            }
            files.push(incomplete(
                data,
                "the ROM stopped reading it (the tape stood still): a format of its own, or damage"
                    .into(),
            ));
            // Look on from where the tape stopped, or past the header.
            if m.tape_position().unwrap_or(0) <= after_header {
                m.seek_tape(after_header);
            }
            continue;
        }
        let result = m.peek_word(blk).unwrap_or(0) >> 8;
        if result != 0 && result != 2 {
            // Not the file found before (for example a damaged header):
            // skip it.
            m.seek_tape(after_header);
            continue;
        }
        files.push(TapeFile {
            name: text,
            file: BinFile {
                address,
                data: bytes_at(&m, LOAD_AT, len),
            },
            checksum_ok: result == 0,
            length: len,
            complete: true,
            problem: None,
        });
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_adds_the_carry_back() {
        assert_eq!(checksum(&[1, 2, 3]), 6);
        assert_eq!(checksum(&[0xff; 258]), (0xff * 258u32 % 0xffff) as u16);
    }

    #[test]
    fn pulses_have_the_rom_timing() {
        let file = BinFile {
            address: 0o1000,
            data: vec![0, 0xff],
        };
        let tape = encode_tape("T", &file).unwrap();
        let wav = tape.to_wav();
        // The pilot's half-period: 528 clocks at 3 MHz is 176 µs, 7.8 samples.
        let first_high = wav.samples.iter().position(|&s| s > 0.0).unwrap();
        let run = wav.samples[first_high..]
            .iter()
            .take_while(|&&s| s > 0.0)
            .count();
        assert!((7..=9).contains(&run), "{run} samples");
        assert!(encode_tape("ИМЯ", &file).is_ok(), "Russian names are KOI-8");
        assert!(encode_tape("π", &file).is_err());
    }
}

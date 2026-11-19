//! Tape recordings from .bin files and back, through the BK-0010 monitor
//! ROM. Needs the ROM files in `<repo>/roms` (or `BK_ROMS`); skipped when
//! they're missing.

use std::path::PathBuf;

use bk::{BinFile, Bk0010Rom, Roms, TapeSignal, Wav, encode_tape, read_tape_files};

fn roms() -> Option<Roms> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    Roms::load_bk0010(&dir, Bk0010Rom::Basic)
        .map_err(|e| eprintln!("skipping: {e}"))
        .ok()
}

/// Two files encoded one after the other on one recording (one of them at
/// an address below 1000, as autostarting programs are) come back intact
/// through the ROM's own tape routine, names, addresses and checksums too.
#[test]
fn encoded_files_read_back_through_the_rom() {
    let Some(roms) = roms() else { return };
    let a = BinFile {
        address: 0o1000,
        data: (0..3000u32).map(|i| (i * 7 + i / 255) as u8).collect(),
    };
    let b = BinFile {
        address: 0o120,
        data: (0..500u32).map(|i| (i ^ 0o252) as u8).collect(),
    };
    let mut samples = encode_tape("GAME", &a).unwrap().to_wav().samples;
    samples.extend(encode_tape("ИГРА.BIN", &b).unwrap().to_wav().samples);
    // Through a WAV file and back, as the command-line tool would.
    let wav = Wav::parse(
        &Wav {
            rate: 44_100,
            samples,
        }
        .to_bytes(),
    )
    .unwrap();
    let files = read_tape_files(roms, TapeSignal::from_wav(&wav)).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(
        (files[0].name.as_str(), &files[0].file, files[0].checksum_ok),
        ("GAME", &a, true)
    );
    assert_eq!(
        (files[1].name.as_str(), &files[1].file, files[1].checksum_ok),
        ("ИГРА.BIN", &b, true)
    );
}

/// A recording that ends inside a file: the files before it are kept, and
/// the cut-off one comes last with the bytes the ROM had read.
#[test]
fn a_recording_cut_inside_a_file_keeps_what_was_read() {
    let Some(roms) = roms() else { return };
    let a = BinFile {
        address: 0o1000,
        data: (0..400u32).map(|i| (i * 5) as u8).collect(),
    };
    let b = BinFile {
        address: 0o2000,
        data: (0..2000u32).map(|i| (i * 3 + 1) as u8).collect(),
    };
    let first = encode_tape("FIRST", &a).unwrap().to_wav();
    let second = encode_tape("SECOND", &b).unwrap().to_wav();
    let mut samples = first.samples;
    // Cut the second recording two thirds of the way through.
    samples.extend(&second.samples[..second.samples.len() * 2 / 3]);
    let wav = Wav {
        rate: first.rate,
        samples,
    };
    let files = read_tape_files(roms, TapeSignal::from_wav(&wav)).unwrap();
    assert_eq!(files.len(), 2);
    assert!(files[0].complete && files[0].checksum_ok);
    assert_eq!(files[0].file, a);
    let cut = &files[1];
    assert_eq!(
        (cut.name.as_str(), cut.complete, cut.length),
        ("SECOND", false, 2000)
    );
    let n = cut.file.data.len();
    assert!(n > 500 && n < 2000, "{n} bytes read");
    assert_eq!(cut.file.data[..], b.data[..n], "the bytes read are right");
}

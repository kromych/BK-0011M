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

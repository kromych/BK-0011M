//! The floppy controller, driven by the real disk ROM. Needs the ROM files
//! in `<repo>/roms` (or `BK_ROMS`); skipped when they're missing.

use std::path::PathBuf;

use bk::vm1::asm::assemble;
use bk::{Config, DiskImage, IMAGE_BYTES, Machine, Roms};

fn machine_with_disk(image: Vec<u8>) -> Option<Machine> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    let roms = match Roms::load_bk0011m(&dir, true) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let mut m = Machine::new(
        roms,
        Config {
            fdd: true,
            ..Default::default()
        },
    );
    m.insert_disk(0, DiskImage::new(image, false)).unwrap();
    Some(m)
}

/// A disk whose first sector is `src`, assembled at 1000 (where the disk
/// ROM loads it).
fn boot_disk(src: &str) -> Vec<u8> {
    let p = assemble(src, 0o1000).unwrap();
    assert!(p.bytes.len() <= 512, "boot sector too long");
    let mut image = vec![0; IMAGE_BYTES];
    image[..p.bytes.len()].copy_from_slice(&p.bytes);
    image
}

/// BOS boots drive 0 at power-on: the disk ROM reads track 0 sector 1 to
/// 1000 and, since it starts with NOP (the boot marker), jumps there with
/// the drive number in R0.
#[test]
fn boots_from_drive_0() {
    let image = boot_disk("NOP\nMOV R0, @#3002\nMOV #12345, @#3000\nDONE: BR DONE");
    let Some(mut m) = machine_with_disk(image) else {
        return;
    };
    for _ in 0..300 {
        m.run_frame();
    }
    assert_eq!(m.peek_word(0o3000), Some(0o12345), "boot sector ran");
    assert_eq!(m.peek_word(0o3002), Some(0), "booted from drive 0");
}

/// A disk without the NOP marker isn't booted; BOS goes on to BASIC.
#[test]
fn ignores_a_disk_without_the_boot_marker() {
    let image = boot_disk("MOV #12345, @#3000\nDONE: BR DONE");
    let Some(mut m) = machine_with_disk(image) else {
        return;
    };
    for _ in 0..300 {
        m.run_frame();
    }
    assert_ne!(m.peek_word(0o3000), Some(0o12345));
}

/// Write a block through the ROM driver (.BBLCK, JSR PC,@#160004), read it
/// back into another buffer, and check memory, the C flag and the image.
#[test]
fn block_write_and_read_through_the_rom() {
    let image = boot_disk(
        "        NOP
                 MOV #4000, R1          ; fill 4000-4777 with each word's address
                 MOV #400, R2
         FILL:   MOV R1, (R1)+
                 SOB R2, FILL
                 MOV #5, R0             ; write block 5
                 MOV #-400, R1
                 MOV #4000, R2
                 MOV #2000, R3          ; driver work area set up by the boot
                 JSR PC, @#160004
                 BCS FAIL
                 MOV #5, R0             ; read it back to 6000
                 MOV #400, R1
                 MOV #6000, R2
                 MOV #2000, R3
                 JSR PC, @#160004
                 BCS FAIL
                 MOV #1, @#3000
         DONE:   BR DONE
         FAIL:   MOVB @#52, @#3002
                 MOV #2, @#3000
                 BR DONE",
    );
    let Some(mut m) = machine_with_disk(image) else {
        return;
    };
    for _ in 0..600 {
        m.run_frame();
        if m.peek_word(0o3000) != Some(0) {
            break;
        }
    }
    assert_eq!(
        m.peek_word(0o3000),
        Some(1),
        "driver error code {:?}",
        m.peek_word(0o3002)
    );
    for i in 0..0o400u16 {
        assert_eq!(
            m.peek_word(0o6000 + 2 * i),
            Some(0o4000 + 2 * i),
            "read back word {i}"
        );
    }
    let disk = m.disk(0).unwrap();
    assert!(disk.dirty);
    let block5 = &disk.data[5 * 512..6 * 512];
    for i in 0..256usize {
        let word = u16::from_le_bytes([block5[2 * i], block5[2 * i + 1]]);
        assert_eq!(word, 0o4000 + 2 * i as u16, "image word {i}");
    }
}

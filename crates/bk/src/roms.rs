use std::fs;
use std::io;
use std::path::Path;

/// Which machine to emulate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Model {
    /// BK-0010 / BK-0010.01: 32 KB RAM, no paging, 3 MHz.
    Bk0010,
    /// BK-0011M: 128 KB paged RAM, 4 MHz.
    #[default]
    Bk0011m,
}

/// What a BK-0010 has at 120000: BASIC (the BK-0010.01) or FOCAL (the
/// original BK-0010, with the test ROM at 160000); or, with the floppy
/// controller, 16 KB of extra RAM switchable with BASIC parts 1-2, and the
/// controller's ROM 326 at 160000.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bk0010Rom {
    #[default]
    Basic,
    Focal,
    Fdd,
}

/// A machine's ROM images. See `docs/hardware-reference.md` §11.
#[derive(Clone)]
pub enum Roms {
    Bk0011m {
        /// BASIC part 0, 16 KB: window 2 when paging bit 0 is set.
        basic0: Vec<u8>,
        /// BASIC part 1 (8 KB) followed by the second part of BOS (8 KB):
        /// window 2 when paging bit 1 is set.
        basic1_ext: Vec<u8>,
        /// BOS system ROM, 8 KB at 140000.
        bos: Vec<u8>,
        /// At 160000: the floppy controller ROM (4 KB) when a floppy
        /// controller is fitted, otherwise the MSTD test ROM (8 KB).
        high: Vec<u8>,
    },
    /// 8 KB blocks at 100000, 120000, 140000 and 160000; `None` where
    /// nothing is fitted (reads there are bus errors).
    Bk0010 { blocks: [Option<Vec<u8>>; 4] },
}

/// Read a ROM file. `size` is the expected length; the result is padded
/// with zeros to `pad_to`.
fn load(dir: &Path, name: &str, size: usize, pad_to: usize) -> io::Result<Vec<u8>> {
    let path = dir.join(name);
    let mut data = fs::read(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
    if data.len() != size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: expected {size} bytes, found {}",
                path.display(),
                data.len()
            ),
        ));
    }
    data.resize(pad_to, 0);
    Ok(data)
}

impl Roms {
    pub fn model(&self) -> Model {
        match self {
            Roms::Bk0011m { .. } => Model::Bk0011m,
            Roms::Bk0010 { .. } => Model::Bk0010,
        }
    }

    /// Load the BK-0011M set from `dir`. With `fdd`, the floppy controller
    /// ROM `disk_327.rom` goes at 160000 instead of `b11m_mstd.rom`.
    pub fn load_bk0011m(dir: &Path, fdd: bool) -> io::Result<Roms> {
        let mut basic1_ext = load(dir, "basic11m_1.rom", 8192, 8192)?;
        basic1_ext.extend(load(dir, "b11m_ext.rom", 8192, 8192)?);
        Ok(Roms::Bk0011m {
            basic0: load(dir, "basic11m_0.rom", 16384, 16384)?,
            basic1_ext,
            bos: load(dir, "b11m_bos.rom", 8192, 8192)?,
            high: if fdd {
                load(dir, "disk_327.rom", 4096, 4096)?
            } else {
                load(dir, "b11m_mstd.rom", 8192, 8192)?
            },
        })
    }

    /// Load the BK-0010 set from `dir`: the monitor at 100000, plus BASIC
    /// (three ROMs, 120000-177577) or FOCAL (120000) and the test ROM
    /// (160000), or for the floppy controller BASIC parts 1-2 and its ROM
    /// `disk_326.rom` (160000).
    pub fn load_bk0010(dir: &Path, rom: Bk0010Rom) -> io::Result<Roms> {
        const K8: usize = 8192;
        let monitor = load(dir, "monit10.rom", K8, K8)?;
        let blocks = match rom {
            Bk0010Rom::Basic => [
                Some(monitor),
                Some(load(dir, "basic10_1.rom", K8, K8)?),
                Some(load(dir, "basic10_2.rom", K8, K8)?),
                Some(load(dir, "basic10_3.rom", 8064, K8)?),
            ],
            Bk0010Rom::Focal => [
                Some(monitor),
                Some(load(dir, "focal.rom", K8, K8)?),
                None,
                Some(load(dir, "tests.rom", 8064, K8)?),
            ],
            Bk0010Rom::Fdd => [
                Some(monitor),
                Some(load(dir, "basic10_1.rom", K8, K8)?),
                Some(load(dir, "basic10_2.rom", K8, K8)?),
                Some(load(dir, "disk_326.rom", 4096, 4096)?),
            ],
        };
        Ok(Roms::Bk0010 { blocks })
    }
}

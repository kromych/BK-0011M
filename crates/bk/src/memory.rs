//! Address decoding. BK-0011M: fixed RAM page 0, two paged 16 KB windows,
//! system ROM, and the I/O page. BK-0010: 32 KB of RAM, ROM above it.

use crate::roms::{Model, Roms};

pub const PAGE_SIZE: usize = 0o40000; // 16 KB
pub const RAM_SIZE: usize = 8 * PAGE_SIZE;

/// The paging register's 3-bit page fields don't number the physical pages
/// directly. Field value → physical page (BOS manual §1.2.6.2).
const PAGE_MAP: [usize; 8] = [1, 5, 2, 3, 4, 7, 0, 6];

/// Physical pages shown on screen: page 1 or 7 in the paging register's
/// numbering, selected by bit 15 of the 177662 write register.
pub const SCREEN_PAGES: [usize; 2] = [5, 6];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rom {
    Basic0,
    Basic1Ext,
    Bos,
    High,
    /// BK-0010: 8 KB block 0-3 (100000, 120000, 140000, 160000).
    Block(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// Byte offset into the 128 KB RAM.
    Ram(usize),
    Rom(Rom, usize),
    Io,
    /// Nothing answers: bus error.
    None,
}

/// What a BK-0010 with the floppy controller shows at 120000-157777,
/// chosen by bits 2-3 of the controller's command register (177130).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bk0010Window {
    /// The controller's 16 KB of extra RAM (bits 00 or 01; at power-on).
    #[default]
    ExtraRam,
    /// BASIC parts 1-2 (bits 11).
    Basic,
    /// Nothing: bus errors (bits 10).
    Nothing,
}

/// Where the BK-0010 floppy controller's extra RAM sits in the RAM array,
/// above the BK-0010's own 32 KB.
const EXTRA_RAM: usize = 0o100000;

#[derive(Clone)]
pub struct Memory {
    pub ram: Vec<u8>,
    roms: Roms,
    fdd: bool,
    /// Last value written to 177716 with bit 11 set.
    pub paging: u16,
    /// BK-0010 with the floppy controller: what 120000-157777 shows.
    pub window: Bk0010Window,
}

impl Memory {
    pub fn new(roms: Roms, fdd: bool) -> Self {
        Memory {
            ram: vec![0; RAM_SIZE],
            roms,
            fdd,
            paging: 0,
            window: Bk0010Window::ExtraRam,
        }
    }

    /// Power-on: RAM cleared, window 2 showing BASIC part 1 + the BOS
    /// extension.
    pub fn reset(&mut self) {
        self.ram.fill(0);
        self.paging = 0o000002;
        self.window = Bk0010Window::ExtraRam;
    }

    pub fn model(&self) -> Model {
        self.roms.model()
    }

    /// The screen buffer: physical page 5 or 6 on the BK-0011M (`second`
    /// selects 6), always 040000-077777 on the BK-0010.
    pub fn screen(&self, second: bool) -> &[u8] {
        let page = match self.model() {
            Model::Bk0010 => 1,
            Model::Bk0011m => SCREEN_PAGES[usize::from(second)],
        };
        &self.ram[page * PAGE_SIZE..(page + 1) * PAGE_SIZE]
    }

    pub fn translate(&self, addr: u16) -> Region {
        let a = addr as usize;
        if let Roms::Bk0010 { blocks } = &self.roms {
            if self.fdd && a >= 0o120000 {
                // The floppy controller's map: switchable 120000-157777, its
                // ROM at 160000, device registers from 170000.
                return if a >= 0o170000 {
                    Region::Io
                } else if a >= 0o160000 {
                    Region::Rom(Rom::Block(3), a - 0o160000)
                } else {
                    match self.window {
                        Bk0010Window::ExtraRam => Region::Ram(EXTRA_RAM + a - 0o120000),
                        Bk0010Window::Basic => {
                            Region::Rom(Rom::Block((a - 0o100000) >> 13), a & 0o17777)
                        }
                        Bk0010Window::Nothing => Region::None,
                    }
                };
            }
            return if a < 0o100000 {
                Region::Ram(a)
            } else if a >= 0o177600 {
                Region::Io
            } else {
                let block = (a - 0o100000) >> 13;
                match blocks[block] {
                    Some(_) => Region::Rom(Rom::Block(block), a & 0o17777),
                    None => Region::None,
                }
            };
        }
        let window = a & (PAGE_SIZE - 1);
        match a >> 14 {
            0 => Region::Ram(a),
            1 => Region::Ram(PAGE_MAP[usize::from(self.paging >> 12) & 7] * PAGE_SIZE + window),
            2 => {
                if self.paging & 0o033 == 0 {
                    Region::Ram(PAGE_MAP[usize::from(self.paging >> 8) & 7] * PAGE_SIZE + window)
                } else if self.paging & 1 != 0 {
                    Region::Rom(Rom::Basic0, window)
                } else if self.paging & 2 != 0 {
                    Region::Rom(Rom::Basic1Ext, window)
                } else {
                    Region::None // cartridge slots (bits 3, 4): nothing fitted
                }
            }
            _ => {
                let io_start = if self.fdd { 0o170000 } else { 0o177600 };
                if a >= io_start {
                    Region::Io
                } else if a < 0o160000 {
                    Region::Rom(Rom::Bos, a & 0o17777)
                } else if a - 0o160000 < self.high_rom_len() {
                    Region::Rom(Rom::High, a - 0o160000)
                } else {
                    Region::None
                }
            }
        }
    }

    pub fn rom_byte(&self, rom: Rom, offset: usize) -> u8 {
        match (&self.roms, rom) {
            (Roms::Bk0011m { basic0, .. }, Rom::Basic0) => basic0[offset],
            (Roms::Bk0011m { basic1_ext, .. }, Rom::Basic1Ext) => basic1_ext[offset],
            (Roms::Bk0011m { bos, .. }, Rom::Bos) => bos[offset],
            (Roms::Bk0011m { high, .. }, Rom::High) => high[offset],
            (Roms::Bk0010 { blocks }, Rom::Block(b)) => blocks[b].as_ref().map_or(0, |d| d[offset]),
            _ => unreachable!("translate only returns this machine's ROMs"),
        }
    }

    fn high_rom_len(&self) -> usize {
        match &self.roms {
            Roms::Bk0011m { high, .. } => high.len(),
            Roms::Bk0010 { .. } => 0,
        }
    }

    pub fn ram_word(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.ram[offset & !1], self.ram[offset | 1]])
    }

    pub fn set_ram_word(&mut self, offset: usize, value: u16) {
        self.ram[offset & !1..(offset & !1) + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// Read a word without side effects (RAM and ROM only), for debuggers.
    pub fn peek_word(&self, addr: u16) -> Option<u16> {
        match self.translate(addr & !1) {
            Region::Ram(o) => Some(self.ram_word(o)),
            Region::Rom(r, o) => Some(u16::from_le_bytes([
                self.rom_byte(r, o),
                self.rom_byte(r, o + 1),
            ])),
            Region::Io | Region::None => None,
        }
    }
}

impl Memory {
    /// Identifies the ROM set, so a save state isn't loaded over other ROMs.
    pub fn rom_checksum(&self) -> u64 {
        match &self.roms {
            Roms::Bk0011m {
                basic0,
                basic1_ext,
                bos,
                high,
            } => crate::state::checksum(&[basic0, basic1_ext, bos, high]),
            Roms::Bk0010 { blocks } => {
                let parts: Vec<&[u8]> =
                    blocks.iter().map(|b| b.as_deref().unwrap_or(&[])).collect();
                crate::state::checksum(&parts)
            }
        }
    }

    pub fn save(&self, w: &mut crate::state::Writer) {
        w.bytes(&self.ram);
        w.u16(self.paging);
        w.u8(self.window as u8);
    }

    pub fn load(&mut self, r: &mut crate::state::Reader) -> crate::state::Result<()> {
        r.bytes_into(&mut self.ram, "RAM")?;
        self.paging = r.u16()?;
        self.window = match r.u8()? {
            0 => Bk0010Window::ExtraRam,
            1 => Bk0010Window::Basic,
            2 => Bk0010Window::Nothing,
            _ => return Err("save state is corrupt (memory window)".into()),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(fdd: bool) -> Memory {
        let roms = Roms::Bk0011m {
            basic0: vec![1; 16384],
            basic1_ext: vec![2; 16384],
            bos: vec![3; 8192],
            high: vec![4; if fdd { 4096 } else { 8192 }],
        };
        let mut m = Memory::new(roms, fdd);
        m.reset();
        m
    }

    #[test]
    fn windows_follow_the_paging_register() {
        let mut m = memory(false);
        assert_eq!(m.translate(0o000000), Region::Ram(0));
        assert_eq!(m.translate(0o037777), Region::Ram(0o37777));
        m.paging = 0o054002; // value BOS writes at start-up: window 1 = field 5
        assert_eq!(m.translate(0o040002), Region::Ram(7 * PAGE_SIZE + 2));
        assert_eq!(m.translate(0o100000), Region::Rom(Rom::Basic1Ext, 0));
        m.paging = 0o017000; // field 1 in window 1, field 6 in window 2, RAM
        assert_eq!(m.translate(0o040000), Region::Ram(5 * PAGE_SIZE));
        assert_eq!(m.translate(0o137776), Region::Ram(0o37776));
        m.paging = 0o004003; // bit 0 wins over bit 1
        assert_eq!(m.translate(0o120000), Region::Rom(Rom::Basic0, 0o20000));
        m.paging = 0o004010; // a cartridge slot with nothing in it
        assert_eq!(m.translate(0o100000), Region::None);
    }

    #[test]
    fn bk0010_map() {
        let roms = Roms::Bk0010 {
            blocks: [
                Some(vec![1; 8192]),
                Some(vec![2; 8192]),
                None,
                Some(vec![4; 8192]),
            ],
        };
        let m = Memory::new(roms, false);
        assert_eq!(m.translate(0o077776), Region::Ram(0o77776));
        assert_eq!(m.translate(0o100002), Region::Rom(Rom::Block(0), 2));
        assert_eq!(m.translate(0o137776), Region::Rom(Rom::Block(1), 0o17776));
        assert_eq!(m.translate(0o140000), Region::None, "empty slot");
        assert_eq!(m.translate(0o177576), Region::Rom(Rom::Block(3), 0o17576));
        assert_eq!(m.translate(0o177716), Region::Io);
        assert_eq!(m.rom_byte(Rom::Block(3), 0), 4);
    }

    #[test]
    fn high_memory_with_and_without_floppy_controller() {
        let m = memory(false);
        assert_eq!(m.translate(0o140000), Region::Rom(Rom::Bos, 0));
        assert_eq!(m.translate(0o170000), Region::Rom(Rom::High, 0o10000));
        assert_eq!(m.translate(0o177576), Region::Rom(Rom::High, 0o17576));
        assert_eq!(m.translate(0o177600), Region::Io);

        let m = memory(true);
        assert_eq!(m.translate(0o167776), Region::Rom(Rom::High, 0o7776));
        assert_eq!(m.translate(0o170000), Region::Io);
    }
}

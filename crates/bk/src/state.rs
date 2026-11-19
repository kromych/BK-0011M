//! Save states: the whole machine as bytes.
//!
//! The format is private to this emulator: a header (magic, version, model,
//! fitted devices, a checksum of the ROMs), then each device's fields in a
//! fixed order, little-endian. ROMs aren't stored; a state loads only into
//! a machine with the same ROMs and devices. Bump [`VERSION`] whenever the
//! layout changes.

pub const MAGIC: &[u8; 8] = b"BKSTATE\0";
pub const VERSION: u16 = 6;

#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    pub fn u16(&mut self, v: u16) {
        self.bytes.extend(v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes.extend(v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes.extend(v.to_le_bytes());
    }
    /// A length-prefixed byte string.
    pub fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.bytes.extend(v);
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

pub type Result<T> = std::result::Result<T, String>;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or("save state is truncated")?;
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(format!("save state is corrupt (flag byte {b})")),
        }
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn bytes(&mut self) -> Result<Vec<u8>> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    /// A byte string that must be exactly `out.len()` long.
    pub fn bytes_into(&mut self, out: &mut [u8], what: &str) -> Result<()> {
        let n = self.u32()? as usize;
        if n != out.len() {
            return Err(format!(
                "save state has {n} bytes of {what}, expected {}",
                out.len()
            ));
        }
        out.copy_from_slice(self.take(n)?);
        Ok(())
    }
    pub fn finish(&self) -> Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(format!(
                "save state has {} unexpected bytes at the end",
                self.data.len() - self.pos
            ))
        }
    }
}

/// FNV-1a, to recognize a ROM set.
pub fn checksum(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for &b in *part {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_errors() {
        let mut w = Writer::default();
        w.u8(7);
        w.bool(true);
        w.u16(0o177716);
        w.u32(123_456);
        w.u64(u64::MAX - 1);
        w.bytes(b"BK");
        let mut r = Reader::new(&w.bytes);
        assert_eq!(r.u8(), Ok(7));
        assert_eq!(r.bool(), Ok(true));
        assert_eq!(r.u16(), Ok(0o177716));
        assert_eq!(r.u32(), Ok(123_456));
        assert_eq!(r.u64(), Ok(u64::MAX - 1));
        let mut two = [0; 2];
        assert_eq!(r.bytes_into(&mut two, "test"), Ok(()));
        assert_eq!(&two, b"BK");
        assert_eq!(r.finish(), Ok(()));
        assert!(r.u8().is_err(), "past the end");

        let mut r = Reader::new(&w.bytes[..3]);
        r.u8().unwrap();
        r.bool().unwrap();
        assert!(r.u16().is_err(), "truncated");
        assert!(Reader::new(&[2]).bool().is_err(), "bad flag");
    }
}

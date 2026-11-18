//! `.bin` files: the BK's standard binary format. A word with the load
//! address, a word with the length in bytes, then the data.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinFile {
    pub address: u16,
    pub data: Vec<u8>,
}

impl BinFile {
    /// Parse a `.bin` file. Bytes after the declared length (some files
    /// carry a checksum or padding there) are ignored.
    pub fn parse(bytes: &[u8]) -> Result<BinFile, String> {
        if bytes.len() < 4 {
            return Err("too short for a .bin header".into());
        }
        let address = u16::from_le_bytes([bytes[0], bytes[1]]);
        let len = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
        let body = &bytes[4..];
        if body.len() < len {
            return Err(format!("header says {len} bytes, file has {}", body.len()));
        }
        if usize::from(address) + len > 0x10000 {
            return Err(format!(
                "{len} bytes at {address:06o} run past the end of memory"
            ));
        }
        Ok(BinFile {
            address,
            data: body[..len].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.data.len());
        out.extend(self.address.to_le_bytes());
        out.extend((self.data.len() as u16).to_le_bytes());
        out.extend(&self.data);
        out
    }

    /// Where to start the program: its load address, or 1000 for files
    /// loaded below 1000. Those also overwrite the vectors and the stack so
    /// that they start themselves after a tape load; their code begins at
    /// 1000, the usual start of user programs.
    pub fn start_address(&self) -> u16 {
        self.address.max(0o1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_write() {
        let bin = BinFile::parse(&[0o0, 0o2, 3, 0, 1, 2, 3, 0xff]).unwrap();
        assert_eq!(
            (bin.address, bin.data.as_slice()),
            (0o1000, &[1u8, 2, 3][..])
        );
        assert_eq!(bin.to_bytes(), [0, 2, 3, 0, 1, 2, 3]);
        assert_eq!(bin.start_address(), 0o1000);
        assert!(BinFile::parse(&[0, 2, 9, 0, 1]).is_err(), "truncated");
        assert!(BinFile::parse(&[0, 0xff, 0, 2]).is_err(), "past 177777");
        let low = BinFile {
            address: 0o120,
            data: vec![0; 0o2000],
        };
        assert_eq!(low.start_address(), 0o1000);
    }
}

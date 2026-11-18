//! KOI-8R, the character set of BK text: ASCII below 0200, Cyrillic at
//! 0300–0377 (lower case first, in the KOI order).

const CYRILLIC: &str = "юабцдефгхийклмнопярстужвьызшэщчъЮАБЦДЕФГХИЙКЛМНОПЯРСТУЖВЬЫЗШЭЩЧЪ";

/// The printable character for a byte, if it has one.
pub fn decode(b: u8) -> Option<char> {
    match b {
        0x20..=0x7e => Some(b as char),
        0xc0..=0xff => CYRILLIC.chars().nth(usize::from(b - 0xc0)),
        _ => None,
    }
}

/// The byte for a printable ASCII or Cyrillic character.
pub fn encode(c: char) -> Option<u8> {
    match c {
        ' '..='~' => Some(c as u8),
        _ => CYRILLIC
            .chars()
            .position(|x| x == c)
            .map(|i| 0xc0 + i as u8),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        for b in (0x20..=0x7e).chain(0xc0..=0xff) {
            assert_eq!(super::encode(super::decode(b).unwrap()), Some(b));
        }
        // "Монитор", as stored in the BK-0011M ROM.
        let bytes: Vec<u8> = "Монитор"
            .chars()
            .map(|c| super::encode(c).unwrap())
            .collect();
        assert_eq!(bytes, [0o355, 0o317, 0o316, 0o311, 0o324, 0o317, 0o322]);
    }
}

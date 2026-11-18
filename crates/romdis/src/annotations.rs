//! Annotation files: names, entry points, data layout and comments for a ROM.
//!
//! One directive per line; `#` starts a whole-line comment. Addresses are
//! octal. Anything after ` ; ` on a directive line is a comment that goes
//! into the listing.
//!
//! ```text
//! code ADDR [NAME] [; note]     code entry point
//! name ADDR NAME [; note]       name an address (a label inside the ROM,
//!                               an external symbol outside it)
//! ptrs ADDR COUNT [NAME] [; note]  COUNT words of code addresses (a jump table)
//! data ADDR COUNT [NAME] [; note]  COUNT words of data
//! text ADDR COUNT [NAME] [; note]  COUNT bytes of text
//! note ADDR text                comment at the end of that line
//! head ADDR text                comment line above that address (repeatable)
//! emt  N NAME [; note]          what EMT N does (shown on EMT instructions)
//! machine bk0010|bk0011m        which machine a program is for (default bk0010)
//! ```

use std::collections::BTreeMap;

use crate::Family;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Ptrs,
    Data,
    Text,
}

#[derive(Clone, Debug, Default)]
pub struct Annotations {
    pub names: BTreeMap<u16, String>,
    pub entries: Vec<u16>,
    /// (address, kind, count): words for Ptrs/Data, bytes for Text.
    pub layout: Vec<(u16, Layout, u16)>,
    pub notes: BTreeMap<u16, Vec<String>>,
    pub heads: BTreeMap<u16, Vec<String>>,
    pub emts: BTreeMap<u8, (String, String)>,
    pub machine: Option<Family>,
}

fn octal(s: &str, line: usize) -> Result<u16, String> {
    u16::from_str_radix(s, 8).map_err(|_| format!("line {line}: bad octal number {s:?}"))
}

fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '$' || ch == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '$' || ch == '_')
}

impl Annotations {
    pub fn parse(src: &str) -> Result<Annotations, String> {
        let mut a = Annotations::default();
        a.add(src)?;
        Ok(a)
    }

    /// Add the directives in `src` to this set.
    pub fn add(&mut self, src: &str) -> Result<(), String> {
        for (i, raw) in src.lines().enumerate() {
            let line = i + 1;
            let raw = raw.trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let (body, note) = match raw.split_once(" ; ") {
                Some((b, n)) => (b.trim(), Some(n.trim().to_string())),
                None => (raw.strip_suffix(" ;").unwrap_or(raw), None),
            };
            let mut words = body.split_whitespace();
            let directive = words.next().unwrap_or_default();
            let missing = || format!("line {line}: {directive} needs more arguments");

            match directive {
                "note" | "head" => {
                    let addr = octal(words.next().ok_or_else(missing)?, line)?;
                    let rest = raw
                        .splitn(3, char::is_whitespace)
                        .nth(2)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let map = if directive == "note" {
                        &mut self.notes
                    } else {
                        &mut self.heads
                    };
                    map.entry(addr).or_default().push(rest);
                    continue;
                }
                "machine" => {
                    self.machine = Some(match words.next() {
                        Some("bk0010") => Family::Bk0010,
                        Some("bk0011m") => Family::Bk0011m,
                        other => return Err(format!("line {line}: unknown machine {other:?}")),
                    });
                    continue;
                }
                "emt" => {
                    let n = octal(words.next().ok_or_else(missing)?, line)?;
                    let name = words.next().ok_or_else(missing)?.to_string();
                    let n = u8::try_from(n)
                        .map_err(|_| format!("line {line}: EMT {n:o} out of range"))?;
                    self.emts.insert(n, (name, note.unwrap_or_default()));
                    continue;
                }
                _ => {}
            }

            let addr = octal(words.next().ok_or_else(missing)?, line)?;
            let count = match directive {
                "code" | "name" => None,
                "ptrs" | "data" | "text" => Some(octal(words.next().ok_or_else(missing)?, line)?),
                _ => return Err(format!("line {line}: unknown directive {directive:?}")),
            };
            let name = words.next();
            if let Some(extra) = words.next() {
                return Err(format!(
                    "line {line}: unexpected {extra:?} (use ' ; ' before a note)"
                ));
            }
            if let Some(name) = name {
                if !is_name(name) {
                    return Err(format!("line {line}: {name:?} is not a valid symbol name"));
                }
                if let Some(old) = self.names.insert(addr, name.to_string())
                    && old != name
                {
                    return Err(format!("line {line}: {addr:06o} already named {old}"));
                }
            } else if directive == "name" {
                return Err(format!("line {line}: name needs a NAME"));
            }
            if let Some(note) = note {
                self.notes.entry(addr).or_default().push(note);
            }
            match directive {
                "code" => self.entries.push(addr),
                "ptrs" => self.layout.push((addr, Layout::Ptrs, count.unwrap())),
                "data" => self.layout.push((addr, Layout::Data, count.unwrap())),
                "text" => self.layout.push((addr, Layout::Text, count.unwrap())),
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directives() {
        let a = Annotations::parse(
            "# comment\n\
             code 140000 RESET ; power-on entry\n\
             name 177716 SEL1\n\
             ptrs 140010 134 SYSTAB ; system call table\n\
             text 132447 23\n\
             head 140300 Cold start\n\
             head 140300 continues here\n\
             note 140304 map pages ; with a semicolon\n\
             emt 16 BPUTW ; write a word",
        )
        .unwrap();
        assert_eq!(a.entries, [0o140000]);
        assert_eq!(a.names[&0o177716], "SEL1");
        assert_eq!(a.layout[0], (0o140010, Layout::Ptrs, 0o134));
        assert_eq!(a.heads[&0o140300].len(), 2);
        assert_eq!(a.notes[&0o140304], ["map pages ; with a semicolon"]);
        assert_eq!(a.emts[&0o16].0, "BPUTW");
        assert!(Annotations::parse("code 140000 RESET extra").is_err());
        assert!(Annotations::parse("name 1 2BAD").is_err());
    }
}

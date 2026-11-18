//! The floppy controller (КНГМД): registers 177130 (command / status) and
//! 177132 (data). See `docs/hardware-reference.md` §8.
//!
//! The controller works at the level of the raw track: the disk ROM reads
//! and writes the gaps, address marks and data one word at a time, and the
//! controller only finds sync markers and checks CRCs. So each track of a
//! sector image is laid out as the bytes a spinning disk would deliver, and
//! decoded back into sectors when the head leaves a track it has written.

/// Bytes on one raw track: 300 rpm at 250 kbit/s.
pub const TRACK_BYTES: usize = 6250;
/// The index hole covers the start of the track.
const INDEX_BYTES: usize = 30;
pub const SECTOR_BYTES: usize = 512;
pub const SECTORS: usize = 10;
pub const TRACK_DATA: usize = SECTOR_BYTES * SECTORS;
/// A standard image: 80 tracks, 2 sides, 10 sectors of 512 bytes (800 KB).
pub const IMAGE_BYTES: usize = 80 * 2 * TRACK_DATA;
/// The head can step a little past the last formatted track.
const LAST_TRACK: u8 = 82;
/// One word passes the head every 64 µs.
const WORD_MICROS: u32 = 64;

/// 177130 write bits (BK-0011 driver manual §1.3.6.1).
mod cmd {
    pub const DRIVES: u16 = 0o17;
    pub const MOTOR: u16 = 0o20;
    /// Head select: 1 = side 1.
    pub const SIDE: u16 = 0o40;
    /// Step direction: 1 = toward the center (track + 1).
    pub const DIR: u16 = 0o100;
    pub const STEP: u16 = 0o200;
    /// Start reading: when this bit drops, the controller hunts for a marker.
    pub const READ: u16 = 0o400;
    pub const WRITE_MARKER: u16 = 0o1000;
}

/// 177130 read bits.
pub mod status {
    pub const TRACK0: u16 = 0o1;
    pub const READY: u16 = 0o2;
    pub const WRITE_PROTECT: u16 = 0o4;
    /// Read: a word is waiting. Write: the controller wants the next word.
    pub const DATA: u16 = 0o200;
    pub const CRC_OK: u16 = 0o40000;
    pub const INDEX: u16 = 0o100000;
}

/// A disk image in memory: tracks in order (track 0 side 0, track 0 side 1,
/// ...), 5120 bytes each.
#[derive(Clone, Debug)]
pub struct DiskImage {
    pub data: Vec<u8>,
    pub read_only: bool,
    /// Changed since it was inserted.
    pub dirty: bool,
}

impl DiskImage {
    pub fn new(data: Vec<u8>, read_only: bool) -> Self {
        DiskImage {
            data,
            read_only,
            dirty: false,
        }
    }

    fn track(&self, track: u8, side: u8) -> [u8; TRACK_DATA] {
        let mut out = [0; TRACK_DATA];
        let start = (usize::from(track) * 2 + usize::from(side)) * TRACK_DATA;
        if let Some(src) = self.data.get(start..) {
            let n = src.len().min(TRACK_DATA);
            out[..n].copy_from_slice(&src[..n]);
        }
        out
    }

    fn set_track(&mut self, track: u8, side: u8, data: &[u8; TRACK_DATA]) {
        let start = (usize::from(track) * 2 + usize::from(side)) * TRACK_DATA;
        if self.data.len() < start + TRACK_DATA {
            self.data.resize(start + TRACK_DATA, 0);
        }
        self.data[start..start + TRACK_DATA].copy_from_slice(data);
        self.dirty = true;
    }
}

/// CRC-16/CCITT as used by MFM disks (initial value FFFF).
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &b in bytes {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// The CRC of the track bytes from `start` up to `end`, going round the end
/// of the track if need be.
fn crc16_between(bytes: &[u8], start: usize, end: usize) -> u16 {
    if start <= end {
        crc16(&bytes[start..end])
    } else {
        crc16(&[&bytes[start..], &bytes[..end]].concat())
    }
}

/// A raw track and where its sync markers are (one flag per word).
#[derive(Clone)]
struct RawTrack {
    bytes: Vec<u8>,
    markers: Vec<bool>,
}

/// Lay out 10 sectors as an MFM track: gap, sync, ID mark (A1 A1 A1 FE,
/// track, side, sector, size code 2), CRC, gap, sync, data mark
/// (A1 A1 A1 FB), 512 bytes, CRC. Every marker starts on a word boundary.
fn encode_track(data: &[u8; TRACK_DATA], track: u8, side: u8) -> RawTrack {
    let mut bytes = Vec::with_capacity(TRACK_BYTES);
    let mut markers = vec![false; TRACK_BYTES / 2];
    let field = |bytes: &mut Vec<u8>, markers: &mut Vec<bool>, body: &[u8]| {
        markers[bytes.len() / 2] = true;
        let start = bytes.len();
        bytes.extend([0xa1, 0xa1, 0xa1]);
        bytes.extend(body);
        bytes.extend(crc16(&bytes[start..]).to_be_bytes());
    };
    for sector in 0..SECTORS {
        let gap = if sector == 0 { 42 } else { 36 };
        bytes.extend(std::iter::repeat_n(0x4e, gap));
        bytes.extend([0; 12]);
        field(
            &mut bytes,
            &mut markers,
            &[0xfe, track, side, sector as u8 + 1, 2],
        );
        bytes.extend(std::iter::repeat_n(0x4e, 22));
        bytes.extend([0; 12]);
        let mut body = vec![0xfb];
        body.extend(&data[sector * SECTOR_BYTES..(sector + 1) * SECTOR_BYTES]);
        field(&mut bytes, &mut markers, &body);
    }
    bytes.resize(TRACK_BYTES, 0x4e);
    RawTrack { bytes, markers }
}

/// Recover the sectors from a raw track written by the disk ROM. `None` if
/// the track doesn't parse (then the image is left alone).
fn decode_track(raw: &[u8]) -> Option<[u8; TRACK_DATA]> {
    let mut out = [0; TRACK_DATA];
    let mut found = 0;
    let mut p = 0;
    let skip = |p: &mut usize, b: u8| {
        while *p < raw.len() && raw[*p] == b {
            *p += 1;
        }
    };
    let mark = |p: &mut usize, id: u8| -> Option<()> {
        skip(p, 0x4e);
        skip(p, 0);
        for _ in 0..3 {
            (raw.get(*p) == Some(&0xa1)).then_some(())?;
            *p += 1;
        }
        (raw.get(*p) == Some(&id)).then_some(())?;
        *p += 1;
        Some(())
    };
    loop {
        skip(&mut p, 0x4e);
        if p >= raw.len() {
            break;
        }
        mark(&mut p, 0xfe)?;
        let id = raw.get(p..p + 4)?;
        let (sector, size) = (usize::from(id[2]), 128usize << id[3].min(3));
        p += 6; // ID and CRC
        mark(&mut p, 0xfb)?;
        let body = raw.get(p..p + size)?;
        if (1..=SECTORS).contains(&sector) && size == SECTOR_BYTES {
            out[(sector - 1) * SECTOR_BYTES..sector * SECTOR_BYTES].copy_from_slice(body);
            found += 1;
        }
        p += size + 2;
    }
    (found > 0).then_some(out)
}

#[derive(Clone, Default)]
struct Drive {
    image: Option<DiskImage>,
    track: u8,
}

#[derive(Clone)]
pub struct Fdc {
    drives: [Drive; 4],
    selected: Option<usize>,
    side: u8,
    flags: u16,
    status: u16,
    /// The raw track under the selected head, and which one it is.
    raw: Option<RawTrack>,
    raw_of: Option<(usize, u8, u8)>,
    raw_dirty: bool,
    /// Rotation: byte offset of the head in the track (all drives turn together).
    pos: usize,
    data: u16,
    write: Option<(u16, bool)>,
    shift: Option<(u16, bool)>,
    writing: bool,
    read_armed: bool,
    searching: bool,
    crc_running: bool,
    crc_start: usize,
    clock: u32,
    cycles_per_word: u32,
    /// Register traffic, when tracing (for debugging the disk ROM).
    pub trace: Option<Vec<String>>,
    last_status: u16,
}

impl Fdc {
    fn log(&mut self, entry: impl FnOnce() -> String) {
        if let Some(t) = &mut self.trace {
            t.push(entry());
        }
    }

    pub fn new(cpu_hz: u32) -> Self {
        Fdc {
            drives: Default::default(),
            selected: None,
            side: 0,
            flags: 0,
            status: status::TRACK0,
            raw: None,
            raw_of: None,
            raw_dirty: false,
            pos: 0,
            data: 0,
            write: None,
            shift: None,
            writing: false,
            read_armed: false,
            searching: false,
            crc_running: false,
            crc_start: 0,
            clock: 0,
            cycles_per_word: cpu_hz / 1_000_000 * WORD_MICROS,
            trace: None,
            last_status: 0,
        }
    }

    pub fn insert(&mut self, drive: usize, image: DiskImage) {
        self.eject(drive);
        self.drives[drive].image = Some(image);
    }

    /// Remove a disk, writing back any change to the current track first.
    pub fn eject(&mut self, drive: usize) -> Option<DiskImage> {
        self.flush();
        if self.raw_of.is_some_and(|(d, _, _)| d == drive) {
            self.raw = None;
            self.raw_of = None;
        }
        self.drives[drive].image.take()
    }

    pub fn image(&mut self, drive: usize) -> Option<&DiskImage> {
        self.flush();
        self.drives[drive].image.as_ref()
    }

    pub fn motor_on(&self) -> bool {
        self.flags & cmd::MOTOR != 0
    }

    /// Bus reset (INIT).
    pub fn reset(&mut self) {
        self.flush();
        self.selected = None;
        self.side = 0;
        self.flags = 0;
        self.status = status::TRACK0;
        self.write = None;
        self.shift = None;
        self.writing = false;
        self.read_armed = false;
        self.searching = false;
        self.crc_running = false;
    }

    fn image_present(&self) -> bool {
        self.selected
            .is_some_and(|d| self.drives[d].image.is_some())
    }

    pub fn read_status(&mut self) -> u16 {
        let s = self.status_value();
        if s != self.last_status {
            self.last_status = s;
            let pos = self.pos;
            self.log(|| format!("status {s:06o} (pos {pos})"));
        }
        s
    }

    fn status_value(&self) -> u16 {
        let Some(d) = self.selected else { return 0 };
        let track0 = if self.drives[d].track == 0 {
            status::TRACK0
        } else {
            0
        };
        if self.drives[d].image.is_none() {
            return status::INDEX | track0;
        }
        let mut s = self.status & (status::DATA | status::CRC_OK);
        s |= track0;
        if self.pos < INDEX_BYTES {
            s |= status::INDEX;
        }
        if self.drives[d].image.as_ref().is_some_and(|i| i.read_only) {
            s |= status::WRITE_PROTECT;
        }
        if self.motor_on() {
            s |= status::READY;
        } else {
            s &= !(status::DATA | status::CRC_OK);
        }
        s
    }

    pub fn write_command(&mut self, value: u16) {
        let pos = self.pos;
        self.log(|| format!("command {value:06o} (pos {pos})"));
        let read_was_set = self.read_armed;
        self.flags = value;
        // The lowest selected drive bit wins.
        let drive = (0..4).find(|&d| value & cmd::DRIVES & (1 << d) != 0);
        if drive != self.selected {
            self.flush();
            self.selected = drive;
        }
        let Some(d) = drive else { return };
        self.side = u8::from(value & cmd::SIDE != 0);
        if value & cmd::STEP != 0 {
            let t = &mut self.drives[d].track;
            if value & cmd::DIR != 0 {
                *t = (*t + 1).min(LAST_TRACK);
            } else {
                *t = t.saturating_sub(1);
            }
        }
        self.load_track();

        if value & cmd::READ != 0 {
            self.read_armed = true;
        } else if read_was_set {
            self.read_armed = false;
            self.searching = true;
            self.crc_running = true;
            self.status &= !(status::CRC_OK | status::DATA);
        }
        if self.writing && value & cmd::WRITE_MARKER != 0 {
            if let Some((w, _)) = self.write {
                self.write = Some((w, true));
            }
            self.status &= !(status::CRC_OK | status::DATA);
        }
    }

    pub fn read_data(&mut self) -> u16 {
        self.status &= !status::DATA;
        self.writing = false;
        self.searching = false;
        self.write = None;
        self.shift = None;
        let value = if self.image_present() { self.data } else { 0 };
        let pos = self.pos;
        self.log(|| format!("read data {value:06o} (pos {pos})"));
        value
    }

    pub fn write_data(&mut self, value: u16) {
        self.log(|| format!("write data {value:06o}"));
        self.writing = true;
        self.searching = false;
        let marker = self.flags & cmd::WRITE_MARKER != 0;
        match (self.shift.is_some(), self.write.is_some()) {
            (false, false) => {
                self.shift = Some((value, marker));
                self.status |= status::DATA;
            }
            (true, false) => {
                self.write = Some((value, marker));
                self.status &= !status::DATA;
            }
            (false, true) => {
                self.shift = self.write.take();
                self.write = Some((value, marker));
                self.status &= !status::DATA;
            }
            (true, true) => self.write = Some((value, marker)),
        }
    }

    /// Advance by `cycles` CPU clocks.
    pub fn tick(&mut self, cycles: u32) {
        self.clock += cycles;
        while self.clock >= self.cycles_per_word {
            self.clock -= self.cycles_per_word;
            if self.motor_on() {
                self.rotate();
            }
        }
    }

    /// Make sure `raw` holds the selected drive's current track and side.
    fn load_track(&mut self) {
        let Some(d) = self.selected else { return };
        let key = (d, self.drives[d].track, self.side);
        if self.raw_of == Some(key) {
            return;
        }
        self.flush();
        self.raw = self.drives[d]
            .image
            .as_ref()
            .map(|img| encode_track(&img.track(key.1, key.2), key.1, key.2));
        self.raw_of = Some(key);
    }

    /// Write a changed raw track back into its image.
    fn flush(&mut self) {
        if !self.raw_dirty {
            return;
        }
        self.raw_dirty = false;
        let (Some(raw), Some((d, track, side))) = (&self.raw, self.raw_of) else {
            return;
        };
        if let (Some(sectors), Some(img)) =
            (decode_track(&raw.bytes), self.drives[d].image.as_mut())
            && !img.read_only
        {
            img.set_track(track, side, &sectors);
        }
    }

    /// One word passes under the head.
    fn rotate(&mut self) {
        self.pos = (self.pos + 2) % TRACK_BYTES;
        if !self.image_present() {
            return;
        }
        self.load_track();
        let pos = self.pos;
        let Some(raw) = self.raw.as_mut() else { return };

        if !self.writing {
            self.data = u16::from_be_bytes([raw.bytes[pos], raw.bytes[pos + 1]]);
            if self.searching {
                if !raw.markers[pos / 2] {
                    return;
                }
                self.searching = false;
                self.crc_start = pos;
            } else if raw.markers[pos / 2] {
                // The CRC starts again at each marker.
                self.crc_start = pos;
            }
            // The word waits in the data register (an unread one is lost).
            self.status |= status::DATA;
            // The CRC runs over every byte that passes the head from the
            // marker on; with a field's two CRC bytes included it comes to 0
            // if the field is intact, and the controller says so.
            if self.crc_running
                && crc16_between(&raw.bytes, self.crc_start, (pos + 2) % TRACK_BYTES) == 0
            {
                self.crc_running = false;
                self.status |= status::CRC_OK;
            }
            return;
        }

        // Writing: the word in the shift register goes onto the disk, low
        // byte first.
        let Some((word, marker)) = self.shift.take() else {
            return;
        };
        let [lo, hi] = word.to_le_bytes();
        raw.bytes[pos] = lo;
        raw.bytes[pos + 1] = hi;
        raw.markers[pos / 2] = marker;
        self.raw_dirty = true;
        if marker {
            self.crc_running = true;
            self.crc_start = pos;
        }
        if let Some(next) = self.write.take() {
            self.shift = Some(next);
            self.status |= status::DATA;
        } else if self.crc_running {
            // Nothing more to write: the controller appends the CRC.
            let end = (pos + 2).min(TRACK_BYTES);
            let crc = crc16(&raw.bytes[self.crc_start.min(end)..end]);
            let [hi, lo] = crc.to_be_bytes();
            self.shift = Some((u16::from_le_bytes([hi, lo]), false));
            self.crc_running = false;
            self.status |= status::CRC_OK;
        }
    }
}

use crate::state::{Reader, Result, Writer};

fn save_flagged(w: &mut Writer, v: Option<(u16, bool)>) {
    w.bool(v.is_some());
    let (word, flag) = v.unwrap_or_default();
    w.u16(word);
    w.bool(flag);
}

fn load_flagged(r: &mut Reader) -> Result<Option<(u16, bool)>> {
    let some = r.bool()?;
    let v = (r.u16()?, r.bool()?);
    Ok(some.then_some(v))
}

impl Fdc {
    /// The controller and the disks in it (their contents included, so a
    /// state taken while a program is writing a disk restores consistently).
    pub fn save(&self, w: &mut Writer) {
        for d in &self.drives {
            w.bool(d.image.is_some());
            if let Some(image) = &d.image {
                w.bytes(&image.data);
                w.bool(image.read_only);
                w.bool(image.dirty);
            }
            w.u8(d.track);
        }
        w.u8(self.selected.map_or(0xff, |d| d as u8));
        w.u8(self.side);
        w.u16(self.flags);
        w.u16(self.status);
        w.bool(self.raw.is_some());
        if let Some(raw) = &self.raw {
            w.bytes(&raw.bytes);
            w.bytes(
                &raw.markers
                    .iter()
                    .map(|&m| u8::from(m))
                    .collect::<Vec<u8>>(),
            );
        }
        w.bool(self.raw_of.is_some());
        let (d, t, s) = self.raw_of.unwrap_or_default();
        w.u8(d as u8);
        w.u8(t);
        w.u8(s);
        w.bool(self.raw_dirty);
        w.u32(self.pos as u32);
        w.u16(self.data);
        save_flagged(w, self.write);
        save_flagged(w, self.shift);
        w.bool(self.writing);
        w.bool(self.read_armed);
        w.bool(self.searching);
        w.bool(self.crc_running);
        w.u32(self.crc_start as u32);
        w.u32(self.clock);
        w.u16(self.last_status);
    }

    pub fn load(&mut self, r: &mut Reader) -> Result<()> {
        let bad = |what: &str| format!("save state is corrupt ({what})");
        for d in &mut self.drives {
            d.image = if r.bool()? {
                let data = r.bytes()?;
                let read_only = r.bool()?;
                let dirty = r.bool()?;
                Some(DiskImage {
                    data,
                    read_only,
                    dirty,
                })
            } else {
                None
            };
            d.track = r.u8()?;
            if d.track > LAST_TRACK {
                return Err(bad("head position"));
            }
        }
        self.selected = match r.u8()? {
            0xff => None,
            d if d < 4 => Some(usize::from(d)),
            _ => return Err(bad("selected drive")),
        };
        self.side = r.u8()? & 1;
        self.flags = r.u16()?;
        self.status = r.u16()?;
        self.raw = if r.bool()? {
            let bytes = r.bytes()?;
            let markers: Vec<bool> = r.bytes()?.into_iter().map(|m| m != 0).collect();
            if bytes.len() != TRACK_BYTES || markers.len() != TRACK_BYTES / 2 {
                return Err(bad("track length"));
            }
            Some(RawTrack { bytes, markers })
        } else {
            None
        };
        let some = r.bool()?;
        let (d, t, s) = (r.u8()?, r.u8()?, r.u8()?);
        if some && d >= 4 {
            return Err(bad("track's drive"));
        }
        self.raw_of = some.then_some((usize::from(d), t, s));
        self.raw_dirty = r.bool()?;
        self.pos = r.u32()? as usize;
        if self.pos >= TRACK_BYTES {
            return Err(bad("head position"));
        }
        self.data = r.u16()?;
        self.write = load_flagged(r)?;
        self.shift = load_flagged(r)?;
        self.writing = r.bool()?;
        self.read_armed = r.bool()?;
        self.searching = r.bool()?;
        self.crc_running = r.bool()?;
        self.crc_start = r.u32()? as usize;
        if self.crc_start >= TRACK_BYTES {
            return Err(bad("CRC start"));
        }
        self.clock = r.u32()?;
        if self.clock >= self.cycles_per_word {
            return Err(bad("clock"));
        }
        self.last_status = r.u16()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sectors(seed: u8) -> [u8; TRACK_DATA] {
        let mut data = [0; TRACK_DATA];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(seed);
        }
        data
    }

    #[test]
    fn track_layout_round_trips() {
        let data = sectors(3);
        let raw = encode_track(&data, 5, 1);
        assert_eq!(raw.bytes.len(), TRACK_BYTES);
        assert_eq!(
            &raw.bytes[54..62],
            &[0xa1, 0xa1, 0xa1, 0xfe, 5, 1, 1, 2],
            "first ID mark"
        );
        assert!(raw.markers[54 / 2]);
        assert_eq!(decode_track(&raw.bytes), Some(data));
        // CRC of an ID field including its own CRC is zero.
        assert_eq!(crc16(&raw.bytes[54..64]), 0);
    }

    /// Drive the registers the way the disk ROM does: select, motor on,
    /// pulse READ, and collect the words after the first marker.
    #[test]
    fn reading_through_the_registers() {
        let mut image = vec![0; IMAGE_BYTES];
        image[..TRACK_DATA].copy_from_slice(&sectors(9));
        let mut fdc = Fdc::new(4_000_000);
        fdc.insert(0, DiskImage::new(image, false));
        fdc.write_command(cmd::MOTOR | 1);
        assert_eq!(
            fdc.read_status() & (status::TRACK0 | status::READY),
            status::TRACK0 | status::READY
        );
        fdc.write_command(cmd::MOTOR | cmd::READ | 1);
        fdc.write_command(cmd::MOTOR | 1);
        let mut words = Vec::new();
        for _ in 0..20_000 {
            fdc.tick(256);
            if fdc.read_status() & status::DATA != 0 {
                words.push(fdc.data);
                fdc.status &= !status::DATA; // as a read of 177132 does
                if words.len() == 4 {
                    break;
                }
            }
        }
        assert_eq!(
            words,
            [0xa1a1, 0xa1fe, 0x0000, 0x0102],
            "marker, track 0 side 0, sector 1, size 2"
        );
    }

    /// Start a read, wait for the first marker, and let the ID field
    /// (marker 2 words, ID 2 words, CRC 1 word) pass the head; return the
    /// CRC status then.
    fn id_field_crc_ok(fdc: &mut Fdc) -> bool {
        fdc.write_command(cmd::MOTOR | cmd::READ | 1);
        fdc.write_command(cmd::MOTOR | 1);
        for _ in 0..20_000 {
            fdc.tick(fdc.cycles_per_word);
            if fdc.read_status() & status::DATA != 0 {
                break;
            }
        }
        fdc.tick(5 * fdc.cycles_per_word);
        fdc.read_status() & status::CRC_OK != 0
    }

    #[test]
    fn crc_is_checked_on_reads() {
        let mut image = vec![0; IMAGE_BYTES];
        image[..TRACK_DATA].copy_from_slice(&sectors(9));
        let mut fdc = Fdc::new(4_000_000);
        fdc.insert(0, DiskImage::new(image, false));
        fdc.write_command(cmd::MOTOR | 1);
        assert!(id_field_crc_ok(&mut fdc), "a good ID field");
        // Damage sector 1's ID (its track byte) on the track under the head.
        let raw = fdc.raw.as_mut().unwrap();
        let id = raw
            .bytes
            .windows(4)
            .position(|w| w == [0xa1, 0xa1, 0xa1, 0xfe])
            .unwrap();
        raw.bytes[id + 4] ^= 0x40;
        assert!(!id_field_crc_ok(&mut fdc), "a damaged ID field");
    }

    #[test]
    fn write_protect_and_no_disk() {
        let mut fdc = Fdc::new(4_000_000);
        assert_eq!(fdc.read_status(), 0, "nothing selected");
        fdc.write_command(cmd::MOTOR | 2);
        assert_eq!(
            fdc.read_status(),
            status::INDEX | status::TRACK0,
            "drive 1 empty"
        );
        fdc.insert(1, DiskImage::new(vec![0; IMAGE_BYTES], true));
        assert_ne!(fdc.read_status() & status::WRITE_PROTECT, 0);
    }
}

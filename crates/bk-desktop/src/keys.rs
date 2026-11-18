//! Host keyboard → BK keyboard, remappable with an INI file.
//!
//! Each host key is bound to a BK key code (with another code when Shift is
//! held), to СТОП, to one of the modifiers АР2 and СУ, or to the emulator's
//! color/mono switch. The default layout puts letters, digits and
//! punctuation where they are on a US keyboard (Shift gives lower-case
//! letters, and the symbols printed on a US keyboard's keys), and the BK's
//! special keys on Enter, Backspace, Tab, the arrows, F1–F12, Home and End;
//! left Ctrl is АР2 and right Ctrl СУ.
//!
//! The INI file changes this, one key per line, under `[keys]`:
//!
//! ```ini
//! [keys]
//! Tab = ТАБ           ; a BK key by name (Russian or Latin, see below)
//! F1 = 201            ; or by its code, octal
//! Key1 = 061 / 041    ; a second code for Shift
//! RightAlt = АР2      ; a modifier
//! Insert = СТОП
//! Backquote =         ; nothing
//! ```

use std::collections::HashMap;
use std::fmt::Write as _;

use minifb::Key;

/// What a host key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// Sends `plain`, or `shifted` with Shift held.
    Code { plain: u8, shifted: u8 },
    /// The СТОП key: an interrupt.
    Stop,
    /// Held down: the next keys are pressed with АР2.
    Ar2,
    /// Held down: letter keys give control codes (code & 037).
    Su,
    /// Switch between the color and mono monitor.
    Monitor,
    /// Held down: a joystick line on the parallel port (177714 bit 0 up,
    /// 1 right, 2 down, 3 left, 4–7 buttons).
    Joystick(u8),
}

/// Host key names in the INI file: minifb's key names, any case.
const HOST_KEYS: &[(&str, Key)] = &[
    ("A", Key::A),
    ("B", Key::B),
    ("C", Key::C),
    ("D", Key::D),
    ("E", Key::E),
    ("F", Key::F),
    ("G", Key::G),
    ("H", Key::H),
    ("I", Key::I),
    ("J", Key::J),
    ("K", Key::K),
    ("L", Key::L),
    ("M", Key::M),
    ("N", Key::N),
    ("O", Key::O),
    ("P", Key::P),
    ("Q", Key::Q),
    ("R", Key::R),
    ("S", Key::S),
    ("T", Key::T),
    ("U", Key::U),
    ("V", Key::V),
    ("W", Key::W),
    ("X", Key::X),
    ("Y", Key::Y),
    ("Z", Key::Z),
    ("Key0", Key::Key0),
    ("Key1", Key::Key1),
    ("Key2", Key::Key2),
    ("Key3", Key::Key3),
    ("Key4", Key::Key4),
    ("Key5", Key::Key5),
    ("Key6", Key::Key6),
    ("Key7", Key::Key7),
    ("Key8", Key::Key8),
    ("Key9", Key::Key9),
    ("F1", Key::F1),
    ("F2", Key::F2),
    ("F3", Key::F3),
    ("F4", Key::F4),
    ("F5", Key::F5),
    ("F6", Key::F6),
    ("F7", Key::F7),
    ("F8", Key::F8),
    ("F9", Key::F9),
    ("F10", Key::F10),
    ("F11", Key::F11),
    ("F12", Key::F12),
    ("F13", Key::F13),
    ("F14", Key::F14),
    ("F15", Key::F15),
    ("Down", Key::Down),
    ("Left", Key::Left),
    ("Right", Key::Right),
    ("Up", Key::Up),
    ("Apostrophe", Key::Apostrophe),
    ("Backquote", Key::Backquote),
    ("Backslash", Key::Backslash),
    ("Comma", Key::Comma),
    ("Equal", Key::Equal),
    ("LeftBracket", Key::LeftBracket),
    ("Minus", Key::Minus),
    ("Period", Key::Period),
    ("RightBracket", Key::RightBracket),
    ("Semicolon", Key::Semicolon),
    ("Slash", Key::Slash),
    ("Backspace", Key::Backspace),
    ("Delete", Key::Delete),
    ("End", Key::End),
    ("Enter", Key::Enter),
    ("Escape", Key::Escape),
    ("Home", Key::Home),
    ("Insert", Key::Insert),
    ("Menu", Key::Menu),
    ("PageDown", Key::PageDown),
    ("PageUp", Key::PageUp),
    ("Pause", Key::Pause),
    ("Space", Key::Space),
    ("Tab", Key::Tab),
    ("NumLock", Key::NumLock),
    ("CapsLock", Key::CapsLock),
    ("ScrollLock", Key::ScrollLock),
    ("LeftShift", Key::LeftShift),
    ("RightShift", Key::RightShift),
    ("LeftCtrl", Key::LeftCtrl),
    ("RightCtrl", Key::RightCtrl),
    ("NumPad0", Key::NumPad0),
    ("NumPad1", Key::NumPad1),
    ("NumPad2", Key::NumPad2),
    ("NumPad3", Key::NumPad3),
    ("NumPad4", Key::NumPad4),
    ("NumPad5", Key::NumPad5),
    ("NumPad6", Key::NumPad6),
    ("NumPad7", Key::NumPad7),
    ("NumPad8", Key::NumPad8),
    ("NumPad9", Key::NumPad9),
    ("NumPadDot", Key::NumPadDot),
    ("NumPadSlash", Key::NumPadSlash),
    ("NumPadAsterisk", Key::NumPadAsterisk),
    ("NumPadMinus", Key::NumPadMinus),
    ("NumPadPlus", Key::NumPadPlus),
    ("NumPadEnter", Key::NumPadEnter),
];

/// BK keys by name: the name on the key, and a Latin alias.
const BK_KEYS: &[(&str, &str, u8)] = &[
    ("ВВОД", "ENTER", 0o012),
    ("ЗАБОЙ", "BACKSPACE", 0o030),
    ("ТАБ", "TAB", 0o211),
    ("ПОВТ", "REPEAT", 0o201),
    ("КТ", "KT", 0o003),
    ("=|=>|", "TAB-RIGHT", 0o213),
    ("|<==", "DELETE-CHAR", 0o026),
    ("|==>", "INSERT-CHAR", 0o027),
    ("ИНДСУ", "SHOW-CONTROL", 0o202),
    ("БЛОКРЕД", "EDIT-LOCK", 0o204),
    ("ШАГ", "STEP", 0o200),
    ("СБР", "CLEAR", 0o014),
    ("РУС", "RUS", 0o016),
    ("ЛАТ", "LAT", 0o017),
    ("ПРОБЕЛ", "SPACE", 0o040),
    ("ВЛЕВО", "LEFT", 0o010),
    ("ВПРАВО", "RIGHT", 0o031),
    ("ВВЕРХ", "UP", 0o032),
    ("ВНИЗ", "DOWN", 0o033),
];

/// Names of the bindings that aren't codes.
const SPECIAL: &[(&str, &str, Binding)] = &[
    ("СТОП", "STOP", Binding::Stop),
    ("АР2", "AR2", Binding::Ar2),
    ("СУ", "SU", Binding::Su),
    ("МОНИТОР", "MONITOR", Binding::Monitor),
    ("ДЖОЙСТИК-ВВЕРХ", "JOY-UP", Binding::Joystick(0)),
    ("ДЖОЙСТИК-ВПРАВО", "JOY-RIGHT", Binding::Joystick(1)),
    ("ДЖОЙСТИК-ВНИЗ", "JOY-DOWN", Binding::Joystick(2)),
    ("ДЖОЙСТИК-ВЛЕВО", "JOY-LEFT", Binding::Joystick(3)),
    ("ДЖОЙСТИК-4", "JOY-BUTTON4", Binding::Joystick(4)),
    ("ДЖОЙСТИК-ОГОНЬ", "JOY-FIRE", Binding::Joystick(5)),
    ("ДЖОЙСТИК-ОГОНЬ2", "JOY-FIRE2", Binding::Joystick(6)),
    ("ДЖОЙСТИК-7", "JOY-BUTTON7", Binding::Joystick(7)),
];

#[derive(Debug)]
pub struct Keymap {
    map: HashMap<Key, Binding>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut map = HashMap::new();
        let code = |plain: u8, shifted: u8| Binding::Code { plain, shifted };
        let letters = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        for (i, k) in letters.into_iter().enumerate() {
            map.insert(k, code(0o101 + i as u8, 0o141 + i as u8));
        }
        let digits = [
            Key::Key0,
            Key::Key1,
            Key::Key2,
            Key::Key3,
            Key::Key4,
            Key::Key5,
            Key::Key6,
            Key::Key7,
            Key::Key8,
            Key::Key9,
        ];
        // Shift gives the symbol on a US key cap (the panel lights the BK
        // key it is on, e.g. `*` on `:`).
        for (d, (k, shifted)) in digits.into_iter().zip(*b")!@#$%^&*(").enumerate() {
            map.insert(k, code(0o060 + d as u8, shifted));
        }
        let pairs = [
            (Key::Comma, b',', b'<'),
            (Key::Period, b'.', b'>'),
            (Key::Slash, b'/', b'?'),
            (Key::Semicolon, b';', b':'),
            (Key::Apostrophe, b'\'', b'"'),
            (Key::Minus, b'-', b'_'),
            (Key::Equal, b'=', b'+'),
            (Key::LeftBracket, b'[', b'{'),
            (Key::RightBracket, b']', b'}'),
            (Key::Backslash, b'\\', b'|'),
        ];
        for (k, plain, shifted) in pairs {
            map.insert(k, code(plain, shifted));
        }
        let same = |c: u8| code(c, c);
        for (k, c) in [
            (Key::Space, 0o040),
            (Key::Enter, 0o012),
            (Key::NumPadEnter, 0o012),
            (Key::Backspace, 0o030),
            (Key::Tab, 0o211), // ТАБ: 011 through the function-key vector 274
            (Key::Left, 0o010),
            (Key::Right, 0o031),
            (Key::Up, 0o032),
            (Key::Down, 0o033),
            (Key::F1, 0o201),  // ПОВТ
            (Key::F2, 0o003),  // КТ
            (Key::F3, 0o213),  // =|=>|
            (Key::F4, 0o026),  // |<==
            (Key::F5, 0o027),  // |==>
            (Key::F6, 0o202),  // ИНД СУ
            (Key::F7, 0o204),  // БЛОК РЕД
            (Key::F8, 0o200),  // ШАГ
            (Key::F9, 0o014),  // СБР
            (Key::F10, 0o016), // РУС
            (Key::F11, 0o017), // ЛАТ
            // macOS keeps F10 and F11 for itself, so these are also on End
            // and Home, as in bkbtl.
            (Key::End, 0o016),
            (Key::Home, 0o017),
        ] {
            map.insert(k, same(c));
        }
        map.insert(Key::Escape, Binding::Stop);
        map.insert(Key::F12, Binding::Stop);
        map.insert(Key::LeftCtrl, Binding::Ar2);
        map.insert(Key::RightCtrl, Binding::Su);
        map.insert(Key::Backquote, code(b'`', b'~'));
        // The numeric keypad is a joystick: 8 4 6 2 for the directions,
        // 0 and 5 for the fire buttons.
        for (k, line) in [
            (Key::NumPad8, 0),
            (Key::NumPad6, 1),
            (Key::NumPad2, 2),
            (Key::NumPad4, 3),
            (Key::NumPad0, 5),
            (Key::NumPad5, 6),
        ] {
            map.insert(k, Binding::Joystick(line));
        }
        Keymap { map }
    }
}

fn host_key(name: &str) -> Option<Key> {
    HOST_KEYS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, k)| k)
}

fn host_name(key: Key) -> &'static str {
    HOST_KEYS
        .iter()
        .find(|&&(_, k)| k == key)
        .map_or("?", |(n, _)| n)
}

fn octal(s: &str) -> Result<u8, String> {
    let v = u16::from_str_radix(s, 8)
        .map_err(|_| format!("{s:?} isn't a BK key name or an octal code"))?;
    u8::try_from(v).map_err(|_| format!("code {s} is above 377"))
}

/// A binding as written in the INI file.
fn parse_binding(value: &str) -> Result<Option<Binding>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let upper = value.to_uppercase().replace(' ', "");
    if let Some(&(_, _, b)) = SPECIAL
        .iter()
        .find(|(ru, lat, _)| upper == *ru || upper == *lat)
    {
        return Ok(Some(b));
    }
    if let Some(&(_, _, c)) = BK_KEYS
        .iter()
        .find(|(ru, lat, _)| upper == *ru || upper == *lat)
    {
        return Ok(Some(Binding::Code {
            plain: c,
            shifted: c,
        }));
    }
    let (plain, shifted) = match value.split_once('/') {
        Some((p, s)) => (octal(p.trim())?, octal(s.trim())?),
        None => {
            let c = octal(value)?;
            (c, c)
        }
    };
    Ok(Some(Binding::Code { plain, shifted }))
}

impl Keymap {
    /// The default layout with the changes in an INI file's `[keys]` section.
    pub fn from_ini(text: &str) -> Result<Keymap, String> {
        let mut keymap = Keymap::default();
        let mut section = String::new();
        for (n, line) in text.lines().enumerate() {
            let at = |e: String| format!("line {}: {e}", n + 1);
            // A comment starts with ; or # at the start of a line or after a space.
            let line = match line.find([';', '#']) {
                Some(0) => "",
                Some(i) if line[..i].ends_with(char::is_whitespace) => &line[..i],
                _ => line,
            }
            .trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                continue;
            }
            if section != "keys" {
                return Err(at(format!("{line:?} is outside the [keys] section")));
            }
            let (name, value) = line
                .split_once('=')
                .ok_or_else(|| at(format!("expected KEY = VALUE, not {line:?}")))?;
            let key = host_key(name.trim())
                .ok_or_else(|| at(format!("unknown host key {:?}", name.trim())))?;
            match parse_binding(value).map_err(at)? {
                Some(b) => keymap.map.insert(key, b),
                None => keymap.map.remove(&key),
            };
        }
        Ok(keymap)
    }

    /// The whole mapping as an INI file, to start a custom one from.
    pub fn to_ini(&self) -> String {
        let mut out = String::from(
            "; BK emulator key map: host key = BK key.\n\
             ; A BK key is a name (ВВОД ЗАБОЙ ТАБ ПОВТ КТ =|=>| |<== |==> ИНДСУ БЛОКРЕД ШАГ\n\
             ; СБР РУС ЛАТ ПРОБЕЛ ВЛЕВО ВПРАВО ВВЕРХ ВНИЗ, or the Latin aliases ENTER\n\
             ; BACKSPACE TAB REPEAT KT TAB-RIGHT DELETE-CHAR INSERT-CHAR SHOW-CONTROL\n\
             ; EDIT-LOCK STEP CLEAR RUS LAT SPACE LEFT RIGHT UP DOWN), an octal key\n\
             ; code, or two codes PLAIN / SHIFTED. СТОП (STOP), АР2 (AR2) and СУ (SU)\n\
             ; are the BK's stop key and modifiers; МОНИТОР (MONITOR) switches between\n\
             ; the color and mono monitors. JOY-UP JOY-RIGHT JOY-DOWN JOY-LEFT\n\
             ; JOY-FIRE JOY-FIRE2 JOY-BUTTON4 JOY-BUTTON7 are joystick lines on\n\
             ; the parallel port. An empty value unmaps a key. Host key\n\
             ; names are minifb's (A, Key1, F1, Enter, LeftBracket, NumPad5, ...).\n\
             ; Alt and Cmd are kept for the emulator's own commands.\n\n[keys]\n",
        );
        let mut entries: Vec<(&str, Binding)> =
            self.map.iter().map(|(&k, &b)| (host_name(k), b)).collect();
        let order = |name: &str| {
            HOST_KEYS
                .iter()
                .position(|(n, _)| *n == name)
                .unwrap_or(usize::MAX)
        };
        entries.sort_by_key(|(name, _)| order(name));
        for (name, binding) in entries {
            let value = match binding {
                Binding::Code { plain, shifted } if plain == shifted => {
                    match BK_KEYS.iter().find(|(_, _, c)| *c == plain) {
                        Some((ru, _, _)) => ru.to_string(),
                        None => format!("{plain:03o}"),
                    }
                }
                Binding::Code { plain, shifted } => format!("{plain:03o} / {shifted:03o}"),
                other => SPECIAL
                    .iter()
                    .find(|(_, _, b)| *b == other)
                    .map_or("", |(ru, _, _)| ru)
                    .to_string(),
            };
            let _ = writeln!(out, "{name} = {value}");
        }
        out
    }

    pub fn get(&self, key: Key) -> Option<Binding> {
        self.map.get(&key).copied()
    }

    /// Whether any key bound to `binding` (a modifier) is in `down`.
    pub fn held(&self, binding: Binding, down: impl Fn(Key) -> bool) -> bool {
        self.map.iter().any(|(&k, &b)| b == binding && down(k))
    }

    /// The joystick lines held down, as the port reads them.
    pub fn joystick(&self, down: impl Fn(Key) -> bool) -> u8 {
        self.map
            .iter()
            .filter(|&(&k, _)| down(k))
            .fold(0, |bits, (_, b)| match b {
                Binding::Joystick(line) => bits | 1 << line,
                _ => bits,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout() {
        let m = Keymap::default();
        let shifted = |k| match m.get(k) {
            Some(Binding::Code { shifted, .. }) => shifted,
            _ => 0,
        };
        assert_eq!(
            [Key::Key8, Key::Key6, Key::Key2, Key::Key0].map(shifted),
            *b"*^@)",
            "US key caps"
        );
        assert_eq!(
            m.get(Key::A),
            Some(Binding::Code {
                plain: 0o101,
                shifted: 0o141
            })
        );
        assert_eq!(
            m.get(Key::Key1),
            Some(Binding::Code {
                plain: 0o061,
                shifted: 0o041
            })
        );
        assert_eq!(
            m.get(Key::Enter),
            Some(Binding::Code {
                plain: 0o012,
                shifted: 0o012
            })
        );
        assert_eq!(m.get(Key::Escape), Some(Binding::Stop));
        assert_eq!(m.get(Key::LeftCtrl), Some(Binding::Ar2));
        assert_eq!(
            m.get(Key::Backquote),
            Some(Binding::Code {
                plain: b'`',
                shifted: b'~'
            })
        );
        assert!(m.held(Binding::Ar2, |k| k == Key::LeftCtrl));
        assert!(m.held(Binding::Su, |k| k == Key::RightCtrl));
        assert_eq!(
            m.joystick(|k| k == Key::NumPad8 || k == Key::NumPad0),
            0b10_0001,
            "up and fire"
        );
        assert_eq!(m.joystick(|_| false), 0);
    }

    #[test]
    fn ini_changes_the_layout() {
        let m = Keymap::from_ini(
            "; my layout\n\
             [keys]\n\
             tab = тАб            ; by name, any case\n\
             F1 = 201\n\
             Key1 = 061 / 041\n\
             Menu = АР2\n\
             CapsLock = su\n\
             Insert = СТОП\n\
             Backquote =\n\
             F3 = =|=>|\n\
             PageUp = блок ред\n\
             Up = joy-up\n",
        )
        .unwrap();
        assert_eq!(
            m.get(Key::Tab),
            Some(Binding::Code {
                plain: 0o211,
                shifted: 0o211
            })
        );
        assert_eq!(
            m.get(Key::F1),
            Some(Binding::Code {
                plain: 0o201,
                shifted: 0o201
            })
        );
        assert_eq!(m.get(Key::Menu), Some(Binding::Ar2));
        assert_eq!(m.get(Key::CapsLock), Some(Binding::Su));
        assert_eq!(m.get(Key::Insert), Some(Binding::Stop));
        assert_eq!(m.get(Key::Backquote), None);
        assert_eq!(
            m.get(Key::F3),
            Some(Binding::Code {
                plain: 0o213,
                shifted: 0o213
            })
        );
        assert_eq!(
            m.get(Key::PageUp),
            Some(Binding::Code {
                plain: 0o204,
                shifted: 0o204
            })
        );
        assert_eq!(m.get(Key::Up), Some(Binding::Joystick(0)));
        assert_eq!(
            m.get(Key::A),
            Some(Binding::Code {
                plain: 0o101,
                shifted: 0o141
            }),
            "others keep the default"
        );
    }

    #[test]
    fn ini_errors_name_the_line() {
        let e = |text: &str| Keymap::from_ini(text).unwrap_err();
        assert!(e("[keys]\nNoSuchKey = 1").starts_with("line 2: unknown host key"));
        assert!(e("[keys]\nA = БУКВА").contains("isn't a BK key name"));
        assert!(e("[keys]\nA = 400").contains("above 377"));
        assert!(e("A = 101").contains("outside the [keys] section"));
        assert!(e("[keys]\nA 101").contains("expected KEY = VALUE"));
    }

    #[test]
    fn written_ini_reads_back_the_same() {
        let m = Keymap::default();
        let back = Keymap::from_ini(&m.to_ini()).unwrap();
        assert_eq!(back.map, m.map);
    }
}

//! The panel under the screen: indicator lights and a picture of the BK
//! keyboard. Keys light up while pressed (from the PC keyboard or the
//! mouse), and clicking one types it.
//!
//! The main block is laid out as on the BK: the operating manual's keyboard
//! test (M1.492.000 РЭ §6.3.4) lists its rows as
//! `; 1 2 3 4 5 6 7 8 9 0 -`, `J C U K E N G [ ] Z H :`,
//! `F Y W A P R O L D V \ .` and `Q ^ S M I T X B @ , / _`, each Latin key
//! carrying its KOI-7 Russian partner (J Й, C Ц, ...). The function and
//! control keys are grouped around it.

use crate::font;

/// Height of the panel in buffer pixels.
pub const PANEL_HEIGHT: usize = 192;

const UNIT: f32 = 34.0;
const KEY_HEIGHT: usize = 24;
const ROW_STEP: usize = 27;
const KEYS_TOP: usize = 30;

const BACKGROUND: u32 = 0x2a2a2e;
const KEY: u32 = 0xd9d4c7;
const KEY_TEXT: u32 = 0x1e1e1e;
const KEY_RUSSIAN: u32 = 0x8a1c1c;
const FUNCTION_KEY: u32 = 0x5b5d66;
const FUNCTION_TEXT: u32 = 0xf2f2f2;
const PRESSED: u32 = 0xf0c040;
const LABEL: u32 = 0xc8c8c8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A key that sends this code.
    Code(u8),
    /// СТОП: an interrupt, not a code.
    Stop,
    /// АР2, СУ and the register (shift) key: modifiers; a click latches
    /// them for the next key.
    Ar2,
    Su,
    Shift,
}

struct Key {
    x: usize,
    y: usize,
    w: usize,
    /// Upper-left label (Latin letter, or the shifted symbol), and
    /// lower-right label (Russian letter, or the plain symbol).
    top: &'static str,
    bottom: &'static str,
    action: Action,
    function: bool,
}

/// What happened on the panel, for the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    KeyDown(u8, bool),
    KeyUp(u8),
    Stop,
    Power,
}

/// The state shown by the lights.
pub struct Lights<'a> {
    pub power: bool,
    /// The BK's tape motor relay.
    pub tape: bool,
    /// The recording in the recorder: seconds played and its length, and
    /// whether it is moving (the motor runs and it hasn't ended).
    pub tape_time: Option<(f64, f64)>,
    pub tape_moving: bool,
    pub disk: Option<bool>,
    pub debugger: bool,
    /// Fast-forward: Alt+F, or the tape moving with its sound off.
    pub fast: bool,
    pub model: &'a str,
}

pub struct Panel {
    width: usize,
    keys: Vec<Key>,
    pub ar2_latched: bool,
    pub su_latched: bool,
    pub shift_latched: bool,
    /// The key a mouse button is holding down, and the code it sent.
    held: Option<(usize, u8)>,
    button_was_down: bool,
}

const POWER_LIGHT: (usize, usize) = (8, 8);

impl Panel {
    pub fn new(width: usize) -> Self {
        use Action::*;
        let mut keys = Vec::new();
        let mut row =
            |y: usize, start: f32, items: &[(f32, &'static str, &'static str, Action, bool)]| {
                let mut x = start;
                for &(w, top, bottom, action, function) in items {
                    keys.push(Key {
                        x: (x * UNIT) as usize + 1,
                        y: KEYS_TOP + y * ROW_STEP,
                        w: (w * UNIT) as usize - 3,
                        top,
                        bottom,
                        action,
                        function,
                    });
                    x += w;
                }
            };
        let f = |label: &'static str, action: Action| (1.5, "", label, action, true);
        row(
            0,
            0.0,
            &[
                f("ПОВТ", Code(0o201)),
                f("КТ", Code(0o003)),
                f("=|=>|", Code(0o213)),
                f("|<==", Code(0o026)),
                f("|==>", Code(0o027)),
                f("ИНД СУ", Code(0o202)),
                f("БЛОК РЕД", Code(0o204)),
                f("ШАГ", Code(0o200)),
                f("СБР", Code(0o014)),
                f("СТОП", Stop),
            ],
        );
        let k = |top: &'static str, bottom: &'static str, code: u8| {
            (1.0, top, bottom, Code(code), false)
        };
        row(
            1,
            0.0,
            &[
                k("+", ";", 0o073),
                k("!", "1", 0o061),
                k("\"", "2", 0o062),
                k("#", "3", 0o063),
                k("¤", "4", 0o064),
                k("%", "5", 0o065),
                k("&", "6", 0o066),
                k("'", "7", 0o067),
                k("(", "8", 0o070),
                k(")", "9", 0o071),
                k("", "0", 0o060),
                k("=", "-", 0o055),
                (3.0, "", "ЗАБОЙ", Code(0o030), true),
            ],
        );
        row(
            2,
            0.0,
            &[
                (1.5, "", "ТАБ", Code(0o211), true),
                k("J", "Й", 0o112),
                k("C", "Ц", 0o103),
                k("U", "У", 0o125),
                k("K", "К", 0o113),
                k("E", "Е", 0o105),
                k("N", "Н", 0o116),
                k("G", "Г", 0o107),
                k("[", "Ш", 0o133),
                k("]", "Щ", 0o135),
                k("Z", "З", 0o132),
                k("H", "Х", 0o110),
                k("*", ":", 0o072),
                (1.5, "", "ВВОД", Code(0o012), true),
            ],
        );
        row(
            3,
            0.0,
            &[
                (2.0, "", "СУ", Su, true),
                k("F", "Ф", 0o106),
                k("Y", "Ы", 0o131),
                k("W", "В", 0o127),
                k("A", "А", 0o101),
                k("P", "П", 0o120),
                k("R", "Р", 0o122),
                k("O", "О", 0o117),
                k("L", "Л", 0o114),
                k("D", "Д", 0o104),
                k("V", "Ж", 0o126),
                k("\\", "Э", 0o134),
                k(">", ".", 0o056),
                (1.0, "", "⇧", Shift, true),
            ],
        );
        row(
            4,
            0.0,
            &[
                (1.5, "", "ЛАТ", Code(0o017), true),
                k("Q", "Я", 0o121),
                k("^", "Ч", 0o136),
                k("S", "С", 0o123),
                k("M", "М", 0o115),
                k("I", "И", 0o111),
                k("T", "Т", 0o124),
                k("X", "Ь", 0o130),
                k("B", "Б", 0o102),
                k("@", "Ю", 0o100),
                k("<", ",", 0o054),
                k("?", "/", 0o057),
                k("_", "Ъ", 0o137),
                (1.5, "", "РУС", Code(0o016), true),
            ],
        );
        row(
            5,
            0.0,
            &[
                (2.0, "", "АР2", Ar2, true),
                (8.0, "", "ПРОБЕЛ", Code(0o040), false),
                (1.25, "", "←", Code(0o010), true),
                (1.25, "", "↑", Code(0o032), true),
                (1.25, "", "↓", Code(0o033), true),
                (1.25, "", "→", Code(0o031), true),
            ],
        );
        Panel {
            width,
            keys,
            ar2_latched: false,
            su_latched: false,
            shift_latched: false,
            held: None,
            button_was_down: false,
        }
    }

    /// Draw the panel into `buf` (the panel's own `width` × [`PANEL_HEIGHT`]
    /// pixels). `pressed` says whether a key code is held down from the PC
    /// keyboard; `ar2_held` whether Ctrl is.
    pub fn draw(
        &self,
        buf: &mut [u32],
        lights: &Lights,
        pressed: &dyn Fn(u8) -> bool,
        ar2_held: bool,
    ) {
        let w = self.width;
        buf.fill(BACKGROUND);
        // Lights: a lamp and its label.
        let mut x = POWER_LIGHT.0;
        let lamp = |buf: &mut [u32], x: &mut usize, on: bool, color: u32, label: &str| {
            let c = if on { color } else { dim(color) };
            fill_rect(buf, w, *x, POWER_LIGHT.1, 10, 10, c);
            font::draw(buf, w, *x + 15, POWER_LIGHT.1 + 2, label, LABEL);
            *x += 15 + font::width(label) + 18;
        };
        lamp(buf, &mut x, lights.power, 0x40e040, "СЕТЬ");
        lamp(buf, &mut x, lights.tape, TAPE_COLOR, "МАГНИТОФОН");
        // The recording: how far it has played, and a bar of it over the
        // keyboard, bright while it moves.
        if let Some((at, len)) = lights.tape_time {
            let mmss = |s: f64| {
                let s = s as u64;
                format!("{}:{:02}", s / 60, s % 60)
            };
            let text = format!("{} / {}", mmss(at), mmss(len));
            font::draw(buf, w, x - 10, POWER_LIGHT.1 + 2, &text, TAPE_TIME);
            x += font::width(&text) + 8;
            let (left, right) = (POWER_LIGHT.0, w - POWER_LIGHT.0);
            let full = right - left;
            let done = if len > 0.0 {
                ((at / len).clamp(0.0, 1.0) * full as f64).round() as usize
            } else {
                0
            };
            let color = if lights.tape_moving {
                TAPE_COLOR
            } else {
                dim_half(TAPE_COLOR)
            };
            fill_rect(buf, w, left, TAPE_BAR_Y, full, 3, TAPE_TRACK);
            fill_rect(buf, w, left, TAPE_BAR_Y, done, 3, color);
        }
        if let Some(disk) = lights.disk {
            lamp(buf, &mut x, disk, 0xff4040, "ДИСК");
        }
        if lights.debugger {
            lamp(buf, &mut x, true, 0x40a0ff, "ОТЛАДКА");
        }
        if lights.fast {
            lamp(buf, &mut x, true, 0xc060ff, "УСКОРЕНИЕ");
        }
        let model = lights.model;
        font::draw(
            buf,
            w,
            w - 8 - font::width(model),
            POWER_LIGHT.1 + 2,
            model,
            LABEL,
        );

        for (i, key) in self.keys.iter().enumerate() {
            let down = match key.action {
                Action::Code(c) => lit_by(c, pressed) || self.held.is_some_and(|(k, _)| k == i),
                Action::Ar2 => ar2_held || self.ar2_latched,
                Action::Su => self.su_latched,
                Action::Shift => self.shift_latched,
                Action::Stop => false,
            };
            let (face, text) = match (down, key.function) {
                (true, _) => (PRESSED, KEY_TEXT),
                (false, true) => (FUNCTION_KEY, FUNCTION_TEXT),
                (false, false) => (KEY, KEY_TEXT),
            };
            fill_rect(buf, w, key.x, key.y, key.w, KEY_HEIGHT, face);
            if key.top.is_empty() {
                // One label, centered; on two lines if it doesn't fit on one.
                let lines = label_lines(key.bottom, key.w);
                let total = lines.len() * (font::HEIGHT + 2) - 2;
                for (i, line) in lines.iter().enumerate() {
                    let lx = key.x + key.w.saturating_sub(font::width(line)) / 2;
                    let ly = key.y + (KEY_HEIGHT - total) / 2 + i * (font::HEIGHT + 2);
                    font::draw(buf, w, lx, ly, line, text);
                }
            } else {
                font::draw(buf, w, key.x + 3, key.y + 3, key.top, text);
                let russian = if down || key.bottom.is_ascii() {
                    text
                } else {
                    KEY_RUSSIAN
                };
                let lx = key.x + key.w - 3 - font::width(key.bottom);
                font::draw(
                    buf,
                    w,
                    lx,
                    key.y + KEY_HEIGHT - 3 - font::HEIGHT,
                    key.bottom,
                    russian,
                );
            }
        }
    }

    /// Mouse state, in panel coordinates (`None` when outside). Returns
    /// what the press or release did.
    pub fn mouse(&mut self, pos: Option<(f32, f32)>, button_down: bool) -> Vec<Event> {
        let mut events = Vec::new();
        let pressed_now = button_down && !self.button_was_down;
        let released_now = !button_down && self.button_was_down;
        self.button_was_down = button_down;
        if released_now && let Some((_, code)) = self.held.take() {
            events.push(Event::KeyUp(code));
        }
        if !pressed_now {
            return events;
        }
        let Some((x, y)) = pos else { return events };
        let (x, y) = (x as isize, y as isize);
        let (px, py) = (POWER_LIGHT.0 as isize, POWER_LIGHT.1 as isize);
        if (px - 4..px + 14 + 4 + font::width("СЕТЬ") as isize).contains(&x)
            && (py - 4..py + 14).contains(&y)
        {
            events.push(Event::Power);
            return events;
        }
        let hit = self.keys.iter().position(|k| {
            (k.x as isize..(k.x + k.w) as isize).contains(&x)
                && (k.y as isize..(k.y + KEY_HEIGHT) as isize).contains(&y)
        });
        match hit.map(|i| (i, self.keys[i].action)) {
            Some((i, Action::Code(code))) => {
                let code = if self.shift_latched {
                    shifted(code)
                } else {
                    code
                };
                // СУ makes control codes from the letter keys, as on the BK.
                let code = if self.su_latched && (0o100..=0o177).contains(&code) {
                    code & 0o37
                } else {
                    code
                };
                events.push(Event::KeyDown(code, self.ar2_latched));
                self.held = Some((i, code));
                self.ar2_latched = false;
                self.su_latched = false;
                self.shift_latched = false;
            }
            Some((_, Action::Stop)) => events.push(Event::Stop),
            Some((_, Action::Ar2)) => self.ar2_latched = !self.ar2_latched,
            Some((_, Action::Su)) => self.su_latched = !self.su_latched,
            Some((_, Action::Shift)) => self.shift_latched = !self.shift_latched,
            None => {}
        }
        events
    }
}

/// The code a key gives with its register (shift) key: the other case on
/// letter keys, and on digit and punctuation keys the symbol on the key's
/// other half (`1` `!`, `;` `+`, `-` `=`, `:` `*`), as printed on the panel.
fn shifted(code: u8) -> u8 {
    match code {
        0o100..=0o177 => code ^ 0o40,
        0o041..=0o077 if code != 0o060 => code ^ 0o20,
        _ => code,
    }
}

/// Whether the key whose plain code is `code` is down: its own code, or the
/// code it gives with Shift (lower-case letters; on the digit and
/// punctuation keys the code with bit 4 flipped, e.g. `1` and `!`, `;` and
/// `+`, `-` and `=`).
fn lit_by(code: u8, pressed: &dyn Fn(u8) -> bool) -> bool {
    pressed(code)
        || match code {
            0o100..=0o137 => pressed(code + 0o40),
            0o041..=0o077 if code != 0o060 => pressed(code ^ 0o20),
            _ => false,
        }
}

/// A label as one line, or split at its space into two when it is too wide
/// for a key `width` pixels wide.
fn label_lines(label: &str, width: usize) -> Vec<&str> {
    match label.split_once(' ') {
        Some((a, b)) if font::width(label) + 4 > width => vec![a, b],
        _ => vec![label],
    }
}

/// The tape lamp's and bar's color, the bar's track, and its row (between
/// the lights and the keys).
const TAPE_COLOR: u32 = 0xffb020;
const TAPE_TRACK: u32 = 0x404046;
const TAPE_TIME: u32 = 0xe0c080;
const TAPE_BAR_Y: usize = 23;

fn dim_half(color: u32) -> u32 {
    let ch = |shift: u32| ((color >> shift) & 0xff) / 2;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn dim(color: u32) -> u32 {
    let ch = |shift: u32| ((color >> shift) & 0xff) / 5;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn fill_rect(buf: &mut [u32], width: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let height = buf.len() / width;
    for row in y..(y + h).min(height) {
        let line = &mut buf[row * width..row * width + width];
        for px in line.iter_mut().take((x + w).min(width)).skip(x) {
            *px = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center_of(panel: &Panel, action: Action) -> (f32, f32) {
        let k = panel.keys.iter().find(|k| k.action == action).unwrap();
        ((k.x + k.w / 2) as f32, (k.y + KEY_HEIGHT / 2) as f32)
    }

    #[test]
    fn keys_fit_and_do_not_overlap() {
        let p = Panel::new(512);
        for (i, a) in p.keys.iter().enumerate() {
            assert!(
                a.x + a.w <= 512 && a.y + KEY_HEIGHT <= PANEL_HEIGHT,
                "{}",
                a.bottom
            );
            for line in label_lines(a.bottom, a.w) {
                assert!(font::width(line) + 4 <= a.w, "label {} fits", a.bottom);
            }
            for b in &p.keys[i + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y != b.y;
                assert!(apart, "{} and {} overlap", a.bottom, b.bottom);
            }
        }
    }

    #[test]
    fn clicks_type_keys_and_latch_modifiers() {
        let mut p = Panel::new(512);
        let j = center_of(&p, Action::Code(0o112));
        assert_eq!(p.mouse(Some(j), true), vec![Event::KeyDown(0o112, false)]);
        assert_eq!(p.mouse(Some(j), true), vec![], "held, not repeated");
        assert_eq!(p.mouse(Some(j), false), vec![Event::KeyUp(0o112)]);

        // СУ then L gives code 014; АР2 is passed with the next key, once.
        let su = center_of(&p, Action::Su);
        p.mouse(Some(su), true);
        p.mouse(Some(su), false);
        let l = center_of(&p, Action::Code(0o114));
        assert_eq!(p.mouse(Some(l), true), vec![Event::KeyDown(0o014, false)]);
        p.mouse(Some(l), false);
        let ar2 = center_of(&p, Action::Ar2);
        p.mouse(Some(ar2), true);
        p.mouse(Some(ar2), false);
        assert_eq!(p.mouse(Some(j), true), vec![Event::KeyDown(0o112, true)]);
        p.mouse(Some(j), false);
        assert_eq!(p.mouse(Some(j), true), vec![Event::KeyDown(0o112, false)]);
        p.mouse(Some(j), false);

        // The register key: the other case, or the key's other symbol.
        let shift = center_of(&p, Action::Shift);
        let colon = center_of(&p, Action::Code(0o072));
        p.mouse(Some(shift), true);
        p.mouse(Some(shift), false);
        assert_eq!(
            p.mouse(Some(colon), true),
            vec![Event::KeyDown(b'*', false)]
        );
        p.mouse(Some(colon), false);
        p.mouse(Some(shift), true);
        p.mouse(Some(shift), false);
        assert_eq!(
            p.mouse(Some(j), true),
            vec![Event::KeyDown(0o152, false)],
            "J shifted"
        );
        p.mouse(Some(j), false);

        let stop = center_of(&p, Action::Stop);
        assert_eq!(p.mouse(Some(stop), true), vec![Event::Stop]);
        p.mouse(None, false);
        assert_eq!(p.mouse(Some((12.0, 12.0)), true), vec![Event::Power]);
    }

    #[test]
    fn shifted_codes_light_their_key() {
        let only = |c: u8| move |x: u8| x == c;
        assert!(lit_by(0o101, &only(0o141)), "a lights A");
        assert!(lit_by(0o061, &only(0o041)), "! lights 1");
        assert!(lit_by(0o073, &only(0o053)), "+ lights ;");
        assert!(lit_by(0o055, &only(0o075)), "= lights -");
        assert!(!lit_by(0o060, &only(0o040)), "space doesn't light 0");
        assert!(lit_by(0o040, &only(0o040)), "space lights the space bar");
    }

    /// `PANEL_PNG=out.png cargo test -p bk-desktop panel_picture` saves a picture.
    /// The tape bar over the keyboard fills as the recording plays: bright
    /// while it moves, dim once it has stopped or ended.
    #[test]
    fn tape_bar_shows_how_far_the_recording_has_played() {
        let p = Panel::new(512);
        let bar = |at: f64, moving: bool| {
            let mut buf = vec![0; 512 * PANEL_HEIGHT];
            let lights = Lights {
                power: true,
                tape: true,
                tape_time: Some((at, 60.0)),
                tape_moving: moving,
                disk: None,
                debugger: false,
                fast: false,
                model: "БК-0010.01",
            };
            p.draw(&mut buf, &lights, &|_| false, false);
            buf[TAPE_BAR_Y * 512..(TAPE_BAR_Y + 1) * 512].to_vec()
        };
        let quarter = bar(15.0, true);
        let lit = quarter.iter().filter(|&&c| c == TAPE_COLOR).count();
        assert_eq!(lit, (512 - 2 * POWER_LIGHT.0) / 4);
        assert_eq!(quarter[POWER_LIGHT.0], TAPE_COLOR);
        assert_eq!(quarter[512 - POWER_LIGHT.0 - 1], TAPE_TRACK);
        let ended = bar(60.0, false);
        assert!(
            ended[POWER_LIGHT.0..512 - POWER_LIGHT.0]
                .iter()
                .all(|&c| c == dim_half(TAPE_COLOR))
        );
    }

    #[test]
    fn panel_picture() {
        let Some(path) = std::env::var_os("PANEL_PNG") else {
            return;
        };
        let mut p = Panel::new(512);
        p.su_latched = true;
        let mut buf = vec![0; 512 * PANEL_HEIGHT];
        let lights = Lights {
            power: true,
            tape: true,
            tape_time: Some((32.0, 55.0)),
            tape_moving: true,
            disk: Some(false),
            debugger: false,
            fast: true,
            model: "БК-0011М",
        };
        p.draw(&mut buf, &lights, &|c| c == 0o101 || c == 0o012, false);
        let rgb: Vec<u8> = buf
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        let file = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 512, PANEL_HEIGHT as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
}

//! Typing text into the machine at a pace it can take, a frame at a time:
//! for pasting from the host clipboard while the machine runs.
//!
//! The BK has no keyboard buffer: a key that arrives before the program has
//! read the previous one is lost, and BASIC reads (and drops) keys while it
//! is busy with a line. So each key waits until the last one has been read,
//! is held for a few frames, and Enter is followed by a pause.
//!
//! Text is converted to BK key codes. Russian letters are typed in РУС mode
//! by their KOI-7 codes, with РУС and ЛАТ pressed to switch as needed.

use std::collections::VecDeque;

use crate::Machine;

/// Frames each key is held down.
const HOLD_FRAMES: u32 = 3;
/// Frames between keys.
const GAP_FRAMES: u32 = 2;
/// Frames after Enter, for the program to deal with the line.
const ENTER_FRAMES: u32 = 25;
/// Frames after a function key (codes 200 and up). The monitor's driver
/// handles some of these itself, and a key that follows within about 50 ms
/// is echoed but lost: FOCAL drops the key after ТАБ (measured: lost at 2
/// frames, kept at 5).
const FUNCTION_FRAMES: u32 = 10;

const ENTER: u8 = 0o012;
const RUS: u8 = 0o016;
const LAT: u8 = 0o017;

/// Russian letters in KOI-7 order: the lower-case letter for code 0100 + i,
/// the capital for 0140 + i (in РУС mode).
const KOI7_RUSSIAN: &str = "юабцдефгхийклмнопярстужвьызшэщчъ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Ready,
    Holding { code: u8, frames: u32 },
    Pause(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Lat,
    Rus,
}

pub struct Typist {
    queue: VecDeque<u8>,
    state: State,
    /// The keyboard mode the queued text leaves the BK in, if known.
    mode: Option<Mode>,
}

impl Default for Typist {
    fn default() -> Self {
        Self::new()
    }
}

impl Typist {
    pub fn new() -> Self {
        Typist {
            queue: VecDeque::new(),
            state: State::Ready,
            mode: None,
        }
    }

    /// Queue `text`. Line ends become Enter, tabs spaces; characters the BK
    /// keyboard can't type are skipped. Returns how many were skipped.
    pub fn push_text(&mut self, text: &str) -> usize {
        let mut skipped = 0;
        let mut switched_to_rus = false;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let (code, mode) = match c {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        continue;
                    }
                    (ENTER, None)
                }
                '\n' => (ENTER, None),
                '\t' => (0o040, None),
                ' '..='?' => (c as u8, None),
                '@'..='~' => (c as u8, Some(Mode::Lat)),
                _ => match russian_key(c) {
                    Some(code) => (code, Some(Mode::Rus)),
                    None => {
                        skipped += 1;
                        continue;
                    }
                },
            };
            if let Some(mode) = mode
                && self.mode != Some(mode)
            {
                self.queue
                    .push_back(if mode == Mode::Rus { RUS } else { LAT });
                self.mode = Some(mode);
                switched_to_rus |= mode == Mode::Rus;
            }
            self.queue.push_back(code);
        }
        // Leave the keyboard in ЛАТ, where it usually is.
        if switched_to_rus && self.mode == Some(Mode::Rus) {
            self.queue.push_back(LAT);
            self.mode = Some(Mode::Lat);
        }
        skipped
    }

    /// Nothing left to type.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty() && self.state == State::Ready
    }

    /// Keys still to type.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// The key being held down now, if any.
    pub fn holding(&self) -> Option<u8> {
        match self.state {
            State::Holding { code, .. } => Some(code),
            _ => None,
        }
    }

    /// Stop typing (releasing a key held down).
    pub fn cancel(&mut self, m: &mut Machine) {
        if let State::Holding { code, .. } = self.state {
            m.key_up(code);
        }
        self.queue.clear();
        self.state = State::Ready;
        self.mode = None;
    }

    /// Advance by one frame: call once after each [`Machine::run_frame`].
    pub fn tick(&mut self, m: &mut Machine) {
        self.state = match self.state {
            State::Ready => match self.queue.front() {
                Some(&code) if !m.key_pending() => {
                    self.queue.pop_front();
                    m.key_down(code, false);
                    State::Holding {
                        code,
                        frames: HOLD_FRAMES,
                    }
                }
                _ => State::Ready,
            },
            State::Holding { code, frames: 1 } => {
                m.key_up(code);
                State::Pause(match code {
                    ENTER => ENTER_FRAMES,
                    0o200.. => FUNCTION_FRAMES,
                    _ => GAP_FRAMES,
                })
            }
            State::Holding { code, frames } => State::Holding {
                code,
                frames: frames - 1,
            },
            State::Pause(1) | State::Pause(0) => State::Ready,
            State::Pause(n) => State::Pause(n - 1),
        };
        if self.queue.is_empty() && self.state == State::Ready {
            self.mode = None;
        }
    }
}

/// The key code of a Russian letter: its KOI-7 code, typed in РУС mode
/// (ё is typed as е).
pub fn russian_key(c: char) -> Option<u8> {
    match c {
        'ё' => return Some(0o105),
        'Ё' => return Some(0o145),
        _ => {}
    }
    let lower = c.to_lowercase().next()?;
    let i = KOI7_RUSSIAN.chars().position(|r| r == lower)? as u8;
    Some(if lower == c { 0o100 + i } else { 0o140 + i })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(text: &str) -> (Vec<u8>, usize) {
        let mut t = Typist::new();
        let skipped = t.push_text(text);
        (t.queue.into_iter().collect(), skipped)
    }

    #[test]
    fn converts_text_to_key_codes() {
        assert_eq!(codes("1+2\r\n").0, vec![0o061, 0o053, 0o062, ENTER]);
        // Latin letters switch to ЛАТ once.
        assert_eq!(codes("Ab").0, vec![LAT, 0o101, 0o142]);
        // Russian: РУС, the KOI-7 codes (lower case 0100 + i, capitals
        // 0140 + i), then ЛАТ again.
        assert_eq!(codes("Да").0, vec![RUS, 0o144, 0o101, LAT]);
        assert_eq!(
            codes("A=\"Ю\"").0,
            vec![LAT, 0o101, 0o075, 0o042, RUS, 0o140, 0o042, LAT]
        );
        assert_eq!(codes("\tя€").0, vec![0o040, RUS, 0o121, LAT]);
        assert_eq!(codes("π€x").1, 2);
    }
}

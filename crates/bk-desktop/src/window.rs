//! The window. winit opens it and delivers its events; softbuffer shows the
//! picture, which is drawn here at the window's own size: the BK's screen at
//! the 4:3 of a TV (as screenshots have it), the keyboard panel under it, as
//! large as the window allows and centred on black. Full screen, resizing
//! and high-density displays are the window system's business, done right.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use bk::{Machine, Model, Monitor, SCREEN_HEIGHT, SCREEN_WIDTH};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::hostkey::Key;
use crate::{Outputs, Pacer, PanelSetup, console, keys, panel, sound};

/// The picture in its own units, as screenshots have it: the screen 1024 x
/// 768 (4:3), the panel 1024 x 384 under it.
const PICTURE_W: f64 = crate::OUT_WIDTH as f64;
const SCREEN_H: f64 = crate::OUT_SCREEN_HEIGHT as f64;
const PICTURE_H: f64 = SCREEN_H + crate::OUT_PANEL_HEIGHT as f64;

/// Where the picture goes in a window: as large as fits, centred.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    x: f64,
    y: f64,
    scale: f64,
}

/// A rectangle of window pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Layout {
    fn new(w: u32, h: u32) -> Layout {
        let (w, h) = (f64::from(w), f64::from(h));
        let scale = (w / PICTURE_W).min(h / PICTURE_H);
        Layout {
            x: (w - PICTURE_W * scale) / 2.0,
            y: (h - PICTURE_H * scale) / 2.0,
            scale,
        }
    }

    fn rect(&self, top: f64, bottom: f64) -> Rect {
        let x0 = self.x.round();
        let x1 = (self.x + PICTURE_W * self.scale).round();
        let y0 = (self.y + top * self.scale).round();
        let y1 = (self.y + bottom * self.scale).round();
        Rect {
            x: x0.max(0.0) as usize,
            y: y0.max(0.0) as usize,
            w: (x1 - x0).max(0.0) as usize,
            h: (y1 - y0).max(0.0) as usize,
        }
    }

    fn screen(&self) -> Rect {
        self.rect(0.0, SCREEN_H)
    }

    fn panel(&self) -> Rect {
        self.rect(SCREEN_H, PICTURE_H)
    }

    /// A window position in the panel's own pixels ([`SCREEN_WIDTH`] x
    /// [`panel::PANEL_HEIGHT`]).
    fn to_panel(self, (mx, my): (f64, f64)) -> (f32, f32) {
        let p = self.panel();
        let (pw, ph) = (p.w.max(1) as f64, p.h.max(1) as f64);
        (
            ((mx - p.x as f64) / pw * SCREEN_WIDTH as f64) as f32,
            ((my - p.y as f64) / ph * panel::PANEL_HEIGHT as f64) as f32,
        )
    }
}

/// Scale `src` (`sw` pixels wide) into `rect` of `dst` (`dw` wide), each
/// window pixel taking the nearest source pixel.
fn blit(src: &[u32], sw: usize, dst: &mut [u32], dw: usize, rect: Rect) {
    let sh = src.len() / sw;
    let dh = dst.len() / dw;
    let w = rect.w.min(dw.saturating_sub(rect.x));
    let h = rect.h.min(dh.saturating_sub(rect.y));
    if w == 0 || h == 0 {
        return;
    }
    let cols: Vec<usize> = (0..w).map(|x| (x * sw / rect.w).min(sw - 1)).collect();
    let mut last = None;
    for y in 0..h {
        let sy = (y * sh / rect.h).min(sh - 1);
        let row = (rect.y + y) * dw + rect.x;
        if last == Some(sy) {
            // The same source line as the window line above.
            dst.copy_within(row - dw..row - dw + w, row);
        } else {
            let line = &src[sy * sw..(sy + 1) * sw];
            for (d, &c) in dst[row..row + w].iter_mut().zip(&cols) {
                *d = line[c];
            }
            last = Some(sy);
        }
    }
}

/// What the window's events say about the keyboard and mouse, between
/// ticks.
#[derive(Default)]
struct Input {
    down: HashSet<Key>,
    /// Went down since the last tick (not counting the host's auto-repeat).
    pressed: Vec<Key>,
    released: Vec<Key>,
    /// In window pixels.
    mouse: Option<(f64, f64)>,
    mouse_down: bool,
    /// Anything happened that the picture should show.
    changed: bool,
}

impl Input {
    fn key(&mut self, key: Key, down: bool, repeat: bool) {
        self.changed = true;
        if down {
            if !repeat {
                self.pressed.push(key);
            }
            self.down.insert(key);
        } else {
            self.down.remove(&key);
            self.released.push(key);
            // A Mac doesn't report keys let go while Cmd is held: take Cmd
            // going up as the end of them.
            if matches!(key, Key::LeftSuper | Key::RightSuper) {
                let held: Vec<Key> = self.down.iter().copied().collect();
                for k in held {
                    if !k.is_command_modifier() {
                        self.down.remove(&k);
                        self.released.push(k);
                    }
                }
            }
        }
    }

    /// Focus lost: nothing is held any more.
    fn release_all(&mut self) {
        self.released.extend(self.down.drain());
        self.mouse_down = false;
        self.changed = true;
    }
}

/// Keys typed on the host light on the panel for at least this long, so a
/// quick tap shows too.
const FLASH: Duration = Duration::from_millis(150);

struct App<'a> {
    machine: &'a mut Machine,
    state_file: &'a Path,
    fdd: bool,
    label: &'static str,
    keymap: keys::Keymap,
    disks: &'a mut Vec<PathBuf>,
    outputs: Outputs,
    sound: Option<sound::Sound>,
    no_sound: bool,

    window: Option<Rc<Window>>,
    context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    input: Input,
    error: Option<String>,
    shown_title: String,
    last_draw: Instant,
    redraw: bool,

    /// Fast-forward: on by itself while the tape moves with something to
    /// play or record, unless the tape is to be heard; Alt+F fast-forwards
    /// a tape that is heard, until it stops.
    fast_forward: bool,
    fast: bool,
    pacer: Pacer,
    frame: Vec<u32>,
    panel_pixels: Vec<u32>,
    /// Host key → BK code sent when it went down, so the release matches.
    down: HashMap<Key, u8>,
    console: console::Console,
    panel: panel::Panel,
    powered: bool,
    flash: HashMap<u8, Instant>,
    /// Text pasted from the clipboard, typed in as the BK takes it.
    typist: bk::Typist,
    /// Gamepads, as joysticks on the parallel port.
    gamepads: Option<gilrs::Gilrs>,
    joystick_shown: u8,
    ar2: bool,
}

/// Open the window and run the machine in it until the window is closed.
pub fn run(
    machine: &mut Machine,
    mute: bool,
    state_file: &Path,
    debug: bool,
    setup: PanelSetup<'_>,
) -> Result<(), String> {
    let PanelSetup {
        fdd,
        label,
        keymap,
        disks,
        symbols,
        outputs,
    } = setup;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let sound = if mute {
        None
    } else {
        match sound::Sound::open() {
            Ok(s) => {
                machine.set_audio_rate(s.rate);
                Some(s)
            }
            Err(e) => {
                eprintln!("bk: no sound: {e}");
                None
            }
        }
    };
    machine.take_audio(); // drop the sound of booting
    let mut console = console::Console::new();
    console.load_symbols(&symbols);
    if debug {
        console.enter(machine);
    }
    let mut app = App {
        no_sound: sound.is_none() && !mute,
        sound,
        machine,
        state_file,
        fdd,
        label,
        keymap,
        disks,
        outputs,
        window: None,
        context: None,
        surface: None,
        input: Input::default(),
        error: None,
        shown_title: String::new(),
        last_draw: Instant::now(),
        redraw: true,
        fast_forward: false,
        fast: false,
        pacer: Pacer::new(),
        frame: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
        panel_pixels: vec![0; SCREEN_WIDTH * panel::PANEL_HEIGHT],
        down: HashMap::new(),
        console,
        panel: panel::Panel::new(SCREEN_WIDTH),
        powered: true,
        flash: HashMap::new(),
        typist: bk::Typist::new(),
        gamepads: gilrs::Gilrs::new().ok(),
        joystick_shown: 0,
        ar2: false,
    };
    app.shown_title = app.title();
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

impl App<'_> {
    /// The title names the machine and monitor, the settings that differ
    /// from the start, and says how to paste (or how much of a paste is
    /// left).
    fn title(&self) -> String {
        let m = &*self.machine;
        let model = match m.model() {
            Model::Bk0010 => "BK-0010",
            Model::Bk0011m => "BK-0011M",
        };
        let monitor = if m.monitor() == Monitor::Mono {
            "mono"
        } else {
            "color"
        };
        let mut notes = Vec::new();
        if self.fast {
            notes.push("fast-forward");
        }
        if self.no_sound {
            notes.push("no sound");
        }
        let settings: Vec<&str> = [
            (m.tape_sound(), "tape sound"),
            (!m.motor_control(), "no motor control"),
            (m.tape_invert(), "inverted tape"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        let settings = settings.join(", ");
        if !settings.is_empty() {
            notes.push(&settings);
        }
        let pasting = self.typist.pending();
        let paste_key = if cfg!(target_os = "macos") {
            "Cmd+V"
        } else {
            "Alt+V"
        };
        let end = if pasting > 0 {
            format!("pasting, {pasting} keys to go")
        } else {
            format!("{paste_key} pastes text")
        };
        notes.push(&end);
        format!("{model} ({monitor}) — {}", notes.join(" — "))
    }

    /// One step of the emulator's own loop: the keys and mouse since the
    /// last tick, then the frames real time has made due. Returns whether
    /// it ran any.
    fn tick(&mut self) -> bool {
        let layout = self.layout();
        let input = &mut self.input;
        let down_now = |k: Key| input.down.contains(&k);
        let shift = down_now(Key::LeftShift) || down_now(Key::RightShift);
        let ar2 = self.keymap.held(keys::Binding::Ar2, down_now);
        let su = self.keymap.held(keys::Binding::Su, down_now);
        // Alt (or Cmd) + key: emulator commands, not BK keys.
        let host = input.down.iter().any(|k| k.is_command_modifier());
        self.ar2 = ar2;
        let pressed = std::mem::take(&mut input.pressed);
        let released = std::mem::take(&mut input.released);
        let machine = &mut *self.machine;
        for key in pressed {
            if host {
                let result = match key {
                    Key::S => crate::save_state(machine, self.state_file),
                    Key::L => crate::load_state(machine, self.state_file),
                    Key::D => {
                        self.console.enter(machine);
                        Ok(())
                    }
                    Key::V => crate::paste(&mut self.typist),
                    Key::F => {
                        // Only the tape being read: it isn't kept on past
                        // the load.
                        if machine.tape_busy() {
                            self.fast_forward = !self.fast_forward;
                        } else {
                            eprintln!("bk: no tape is moving: nothing to fast-forward");
                        }
                        Ok(())
                    }
                    Key::T => {
                        machine.set_tape_sound(!machine.tape_sound());
                        Ok(())
                    }
                    Key::R => {
                        machine.set_motor_control(!machine.motor_control());
                        Ok(())
                    }
                    Key::I => {
                        machine.set_tape_invert(!machine.tape_invert());
                        Ok(())
                    }
                    Key::M => {
                        crate::toggle_monitor(machine);
                        Ok(())
                    }
                    Key::O => crate::open_file(machine, self.disks),
                    _ => Ok(()),
                };
                if let Err(e) = result {
                    eprintln!("bk: {e}");
                }
            } else {
                match self.keymap.get(key) {
                    Some(keys::Binding::Stop) if self.powered => machine.press_stop(),
                    Some(keys::Binding::Monitor) => crate::toggle_monitor(machine),
                    // The BK doesn't see keys while stopped in the debugger.
                    Some(keys::Binding::Code { plain, shifted })
                        if self.powered && !self.console.stopped =>
                    {
                        let mut code = if shift { shifted } else { plain };
                        // СУ makes control codes from the letter keys.
                        if su && (0o100..=0o177).contains(&code) {
                            code &= 0o37;
                        }
                        self.down.insert(key, code);
                        self.flash.insert(code, Instant::now() + FLASH);
                        machine.key_down(code, ar2);
                    }
                    _ => {}
                }
            }
        }
        for key in released {
            if let Some(code) = self.down.remove(&key) {
                machine.key_up(code);
            }
        }

        // The joystick on the parallel port: keys bound to it (the numeric
        // keypad by default) and any gamepad.
        let input = &self.input;
        let joystick = self.keymap.joystick(|k| input.down.contains(&k))
            | self.gamepads.as_mut().map_or(0, crate::gamepad_lines);
        if joystick != self.joystick_shown {
            machine.set_port_input(u16::from(joystick));
            self.joystick_shown = joystick;
        }

        // The panel: clicks on its keys and the power light.
        let mouse = self
            .input
            .mouse
            .zip(layout)
            .map(|(m, layout)| layout.to_panel(m));
        for event in self.panel.mouse(mouse, self.input.mouse_down) {
            self.input.changed = true;
            match event {
                panel::Event::KeyDown(code, ar2_latched)
                    if self.powered && !self.console.stopped =>
                {
                    machine.key_down(code, ar2 || ar2_latched)
                }
                panel::Event::KeyDown(..) => {}
                panel::Event::KeyUp(code) => machine.key_up(code),
                panel::Event::Stop if self.powered => machine.press_stop(),
                panel::Event::Stop => {}
                panel::Event::Power => {
                    // Off: the machine stops. On: a cold start, RAM
                    // cleared, as when the emulator starts.
                    if self.powered {
                        self.powered = false;
                    } else {
                        machine.power_on();
                        self.powered = true;
                    }
                }
            }
        }

        if self.console.stopped {
            self.console.poll(machine);
        }
        // Alt+F's fast-forward ends with the tape it was for.
        if self.fast_forward && !machine.tape_busy() {
            self.fast_forward = false;
        }
        let tape_fast = machine.tape_busy() && !machine.tape_sound();
        let fast = self.powered && (self.fast_forward || tape_fast);
        if fast != self.fast {
            // Say why: fast-forward drops the sound and races the clock.
            eprintln!(
                "bk: fast-forward {}",
                match (fast, self.fast_forward) {
                    (false, _) => "off",
                    (true, true) => "on until the tape stops (Alt+F)",
                    (true, false) =>
                        "on while the tape moves (Alt+T to hear it at its speed instead)",
                }
            );
            self.fast = fast;
            self.pacer = Pacer::new();
        }
        let mut ran = 0;
        if self.powered && !self.console.stopped {
            // Normally the frames real time has made due (at most 4 at a
            // time); fast: as many as fit in 15 ms, with the sound dropped.
            let started = Instant::now();
            loop {
                let more = if fast {
                    started.elapsed() < Duration::from_millis(15)
                } else {
                    ran < 4 && self.pacer.due() > 0
                };
                if !more {
                    break;
                }
                self.pacer.ran();
                ran += 1;
                let stop = machine.run_frame();
                self.typist.tick(machine);
                let audio = machine.take_audio();
                if let Some(s) = &self.sound
                    && !fast
                {
                    s.push(&audio);
                }
                if let Some(why) = stop
                    && self.console.on_break(machine, why)
                {
                    break;
                }
                self.console.flush_trace(machine);
                if let Err(e) = self.outputs.write(machine) {
                    eprintln!("bk: {e}");
                }
            }
            if let Some(s) = &self.sound
                && !fast
            {
                self.pacer.trim(s.queued(), s.rate);
            }
        } else {
            self.pacer = Pacer::new();
        }

        let wanted = self.title();
        if wanted != self.shown_title {
            if let Some(w) = &self.window {
                w.set_title(&wanted);
            }
            self.shown_title = wanted;
        }
        ran > 0
    }

    /// The layout for the window's present size.
    fn layout(&self) -> Option<Layout> {
        let size = self.window.as_ref()?.inner_size();
        Some(Layout::new(size.width, size.height))
    }

    /// Draw the screen and the panel at the window's size and show them.
    fn draw(&mut self) -> Result<(), String> {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return Ok(());
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimized
        };
        surface.resize(w, h).map_err(|e| e.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|e| e.to_string())?;
        buffer.fill(0);
        let layout = Layout::new(size.width, size.height);
        let dw = size.width as usize;

        let machine = &*self.machine;
        if self.powered {
            machine.render(&mut self.frame);
        } else {
            self.frame.fill(0);
        }
        blit(&self.frame, SCREEN_WIDTH, &mut buffer, dw, layout.screen());

        let lights = panel::Lights {
            power: self.powered,
            tape: self.powered && machine.tape_motor_on(),
            tape_time: machine.tape_time(),
            tape_moving: self.powered && machine.tape_busy(),
            disk: self.fdd.then(|| self.powered && machine.disk_motor_on()),
            debugger: self.console.stopped,
            fast: self.fast,
            model: self.label,
        };
        let now = Instant::now();
        self.flash.retain(|_, until| *until > now);
        let held: Vec<u8> = self
            .down
            .values()
            .chain(self.flash.keys())
            .chain(self.typist.holding().as_ref())
            .copied()
            .collect();
        self.panel.draw(
            &mut self.panel_pixels,
            &lights,
            &|code| held.contains(&code),
            self.ar2,
        );
        blit(
            &self.panel_pixels,
            SCREEN_WIDTH,
            &mut buffer,
            dw,
            layout.panel(),
        );
        buffer.present().map_err(|e| e.to_string())?;
        self.last_draw = now;
        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, e: String) {
        self.error = Some(e);
        event_loop.exit();
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // As a video player opens the picture: a point of the display for
        // each pixel of it, or as much as fits the display, centred.
        let mut attributes = Window::default_attributes()
            .with_title(&self.shown_title)
            .with_inner_size(LogicalSize::new(PICTURE_W / 2.0, PICTURE_H / 2.0))
            .with_min_inner_size(LogicalSize::new(PICTURE_W / 4.0, PICTURE_H / 4.0));
        if let Some(monitor) = event_loop.primary_monitor() {
            let scale = monitor.scale_factor();
            let area = monitor.size().to_logical::<f64>(scale);
            let origin = monitor.position().to_logical::<f64>(scale);
            // Leave room for the menu bar, the title bar and a dock.
            let fit = (0.95 * area.width / PICTURE_W)
                .min(0.88 * area.height / PICTURE_H)
                .min(1.0);
            let (w, h) = (PICTURE_W * fit, PICTURE_H * fit);
            attributes = attributes
                .with_inner_size(LogicalSize::new(w, h))
                .with_position(LogicalPosition::new(
                    origin.x + (area.width - w) / 2.0,
                    origin.y + (area.height - h) / 2.0,
                ));
        }
        let window = match event_loop.create_window(attributes) {
            Ok(w) => Rc::new(w),
            Err(e) => return self.fail(event_loop, e.to_string()),
        };
        let surface = softbuffer::Context::new(window.clone()).and_then(|context| {
            let surface = softbuffer::Surface::new(&context, window.clone())?;
            Ok((context, surface))
        });
        match surface {
            Ok((context, surface)) => {
                self.context = Some(context);
                self.surface = Some(surface);
                self.window = Some(window);
            }
            Err(e) => self.fail(event_loop, e.to_string()),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.draw() {
                    self.fail(event_loop, e);
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.redraw = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = Key::from_code(code)
                {
                    self.input
                        .key(key, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::Focused(false) => self.input.release_all(),
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse = Some((position.x, position.y));
            }
            WindowEvent::CursorLeft { .. } => self.input.mouse = None,
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.input.mouse_down = state == ElementState::Pressed;
                self.input.changed = true;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let ran = self.tick();
        // Show the picture when it has changed, and now and then anyway
        // (the panel's key lights go out by themselves).
        let changed = std::mem::take(&mut self.input.changed);
        let resized = std::mem::take(&mut self.redraw);
        let stale = self.last_draw.elapsed() > Duration::from_millis(100);
        if (ran || changed || resized || stale)
            && let Some(w) = &self.window
        {
            w.request_redraw();
        }
        event_loop.set_control_flow(if self.fast {
            ControlFlow::Poll
        } else {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(4))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picture keeps its proportions in any window: a wide window gets
    /// black at the sides, a tall one above and below, and the screen is
    /// always 4:3.
    #[test]
    fn the_picture_fits_the_window_at_its_proportions() {
        let exact = Layout::new(1024, 1152);
        assert_eq!(
            exact.screen(),
            Rect {
                x: 0,
                y: 0,
                w: 1024,
                h: 768
            }
        );
        assert_eq!(
            exact.panel(),
            Rect {
                x: 0,
                y: 768,
                w: 1024,
                h: 384
            }
        );
        let wide = Layout::new(3000, 1152);
        assert_eq!(
            wide.screen(),
            Rect {
                x: 988,
                y: 0,
                w: 1024,
                h: 768
            }
        );
        let tall = Layout::new(512, 2000);
        let s = tall.screen();
        assert_eq!((s.w, s.h), (512, 384));
        assert_eq!(s.y, (2000 - 576) / 2);
    }

    /// Clicks map back to the panel's own pixels however it is scaled.
    #[test]
    fn panel_positions_undo_the_scaling() {
        let l = Layout::new(2048, 2304);
        assert_eq!(l.to_panel((0.0, 1536.0)), (0.0, 0.0));
        assert_eq!(l.to_panel((1024.0, 1536.0 + 384.0)), (256.0, 96.0));
    }

    /// Scaling takes the nearest source pixel, every line.
    #[test]
    fn blit_scales_by_the_nearest_pixel() {
        let src = [1, 2, 3, 4];
        let mut dst = vec![0; 4 * 4];
        blit(
            &src,
            2,
            &mut dst,
            4,
            Rect {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
        );
        assert_eq!(dst, [1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4]);
    }
}

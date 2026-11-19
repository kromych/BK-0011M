//! `bk`: run the BK-0011M (or BK-0010) emulator in a window, or headless to
//! take a screenshot.

mod console;
mod font;
mod hostkey;
mod keys;
mod panel;
mod sound;
mod window;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use bk::{
    BinFile, Bk0010Rom, Config, DirFiles, DiskImage, Machine, Model, Monitor, Roms, SCREEN_HEIGHT,
    SCREEN_WIDTH, TapeSignal, Wav,
};

const USAGE: &str = "\
usage: bk [options]

  --roms DIR         directory with the ROM files (default: ./roms)
  --bk0010           emulate a BK-0010.01 (monitor and BASIC) instead of a
                     BK-0011M
  --focal            with --bk0010: the original BK-0010 (FOCAL instead of
                     BASIC)
  --mono             black-and-white monitor instead of color
  --fdd              floppy controller fitted (its ROM at 160000; on a
                     BK-0010, also 16 KB of RAM at 120000 in place of BASIC)
  --disk FILE        put a disk image (800 KB, .img/.bkd/.dsk) in the next
                     drive, 0 to 3; implies --fdd. Changes are written back
                     to the file on exit unless it is read-only
  --load FILE        load a .bin program after booting and start it
  --start ADDR       start address for --load, octal (default: the load
                     address, or 1000 if that's lower)
  --files DIR        fast tape: when a program loads or saves a named file,
                     use .bin files in DIR (default: the directory of --load)
  --tape FILE        put a recorded tape (.wav or .mp3, or either gzipped) in
                     the recorder; it plays while the BK runs the tape motor
  --record FILE.wav  record what the BK writes to tape, saved on exit
  --ay               an AY-3-8910 sound chip on the parallel port
  --covox            a Covox (8-bit DAC) on the parallel port
  --printer FILE     a printer on the parallel port (instead of a joystick):
                     what the BK prints (BASIC LPRINT, BOS's printer calls)
                     is added to FILE as text
  --serial-in FILE   BK-0010: the serial line's far end sends FILE's bytes
                     to the BK (the monitor's EMT 44 and 50 receive them)
  --serial-out FILE  BK-0010: what the BK sends on the serial line (EMT 42,
                     46) is added to FILE, as raw bytes
  --irq2-50hz        BK-0010: a 50 Hz interrupt on vector 100 once a program
                     has set the vector (FOCAL's FCLK counts them)
  --mute             no sound
  --tape-sound       hear the tape being read, as from the cassette recorder;
                     it then plays at its real speed (Alt+T switches)
  --no-motor-control the recorder isn't wired to the BK's motor relay: once
                     the BK starts the motor, the tape plays to its end, as
                     some protected tapes need (Alt+R switches)
  --invert-tape      invert the recording's signal, for loaders that need
                     the other polarity (Alt+I switches)
  --state FILE       where Alt+S saves the machine state and Alt+L loads it
                     (default: bk0011m.state or bk0010.state)
  --restore FILE     start from a saved state (same model, ROMs and devices)
  --save-state FILE  headless: save the machine state when done
  --keys FILE        remap the keyboard from an INI file (default: bk-keys.ini
                     in the current directory, if there is one)
  --write-keys FILE  write the key map in use to FILE as an INI file, as a
                     starting point for your own, and exit
  --bin2wav FILE.bin OUT.wav
                     write FILE.bin as a tape recording in the standard BK
                     format, for a real BK or --tape, and exit
  --tape-name NAME   the name on the tape for --bin2wav (default: the file
                     name, without .bin, up to 16 characters)
  --wav2bin FILE DIR read every file on a tape recording (.wav or .mp3)
                     through the BK-0010 monitor ROM, write each to
                     DIR/NAME.bin, and exit
  --symbols FILE     names for the debugger (NAME = ADDR lines, as the
                     assembler writes with a 4th argument); the ROMs' names
                     come from ./annotations if it is there. Repeatable
  --debug            start stopped in the debugger, which reads commands
                     from the terminal (h for help). Headless, it runs the
                     commands from stdin after the other options are done
  --frames N         headless: run N frames (default 100), then exit
  --type TEXT        headless: type TEXT after booting (\\n = Enter, \\NNN = key
                     code in octal up to 377, e.g. \\017 for ЛАТ or \\201 for
                     ПОВТ, \\p = pause for 1 second; Russian letters are typed
                     in РУС mode, switching back to ЛАТ after them)
  --wait N           headless: frames to run after typing (default 50)
  --screenshot FILE  headless: save the screen as PNG when done
  --panel            with --screenshot: include the keyboard panel, as the
                     window shows it
  --wav FILE         headless: save the sound as a WAV file

Keys (by default; --keys remaps them): letters, digits and punctuation as
on a US keyboard; left Ctrl = АР2, right Ctrl = СУ; Home = ЛАТ, End = РУС
(also F11, F10); Esc = STOP (also F12); F1 ПОВТ, F2 КТ, F3 =|=>|,
F4 |<==, F5 |==>, F6 ИНД СУ, F7 БЛОК РЕД, F8 ШАГ, F9 СБР.

Emulator commands (Cmd also works on a Mac): Alt+O opens a file (a .bin
program is loaded and started, a .wav or .mp3 goes in the tape recorder,
a disk image in drive 0, a .state is restored), Alt+S saves the state,
Alt+L loads it, Alt+D stops in the debugger, Alt+V types the text on the
clipboard into the BK, Alt+F fast-forwards the tape being read until it
stops (or not, pressed again), Alt+M
switches between the color and mono monitors (programs using 512-dot mode
look right only in mono), Alt+T switches the tape sound on and off, Alt+R
switches the recorder's motor control (see --no-motor-control), Alt+I
inverts the tape signal (see --invert-tape). The
emulator also runs fast by itself while the tape moves with a recording to
play or record, unless the tape sound is on.

The numeric keypad (8 4 6 2, fire 0 and 5) and any gamepad are a joystick
on the parallel port. The keyboard under the screen shows the keys
pressed; click its keys to type them (АР2, СУ and the register key ⇧ latch
for the next key), and click the СЕТЬ light to switch the power off and on
(a cold start: RAM is cleared). Close the window to quit.";

/// Frames to let the firmware boot before a program is loaded.
const BOOT_FRAMES: u64 = 150;

struct Options {
    roms: PathBuf,
    model: Model,
    bk0010_rom: Bk0010Rom,
    load: Option<PathBuf>,
    start: Option<u16>,
    config: Config,
    frames: Option<u64>,
    text: Option<String>,
    screenshot: Option<PathBuf>,
    files: Option<PathBuf>,
    disks: Vec<PathBuf>,
    tape: Option<PathBuf>,
    record: Option<PathBuf>,
    wav: Option<PathBuf>,
    mute: bool,
    tape_sound: bool,
    no_motor_control: bool,
    invert_tape: bool,
    wait: u64,
    state: Option<PathBuf>,
    restore: Option<PathBuf>,
    save_state: Option<PathBuf>,
    debug: bool,
    keys: Option<PathBuf>,
    write_keys: Option<PathBuf>,
    printer: Option<PathBuf>,
    serial_in: Option<PathBuf>,
    serial_out: Option<PathBuf>,
    symbols: Vec<PathBuf>,
    panel: bool,
    bin2wav: Option<(PathBuf, PathBuf)>,
    wav2bin: Option<(PathBuf, PathBuf)>,
    tape_name: Option<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut o = Options {
        roms: PathBuf::from("roms"),
        model: Model::Bk0011m,
        bk0010_rom: Bk0010Rom::Basic,
        load: None,
        start: None,
        config: Config::default(),
        frames: None,
        text: None,
        screenshot: None,
        files: None,
        disks: Vec::new(),
        tape: None,
        record: None,
        wav: None,
        mute: false,
        tape_sound: false,
        no_motor_control: false,
        invert_tape: false,
        wait: 50,
        state: None,
        restore: None,
        save_state: None,
        debug: false,
        keys: None,
        write_keys: None,
        printer: None,
        serial_in: None,
        serial_out: None,
        symbols: Vec::new(),
        panel: false,
        bin2wav: None,
        wav2bin: None,
        tape_name: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--roms" => o.roms = value("--roms")?.into(),
            "--bk0010" => o.model = Model::Bk0010,
            "--focal" => o.bk0010_rom = Bk0010Rom::Focal,
            "--load" => o.load = Some(value("--load")?.into()),
            "--start" => {
                let v = value("--start")?;
                o.start =
                    Some(u16::from_str_radix(&v, 8).map_err(|_| "--start needs an octal address")?)
            }
            "--files" => o.files = Some(value("--files")?.into()),
            "--disk" => {
                o.disks.push(value("--disk")?.into());
                o.config.fdd = true;
            }
            "--tape" => o.tape = Some(value("--tape")?.into()),
            "--record" => o.record = Some(value("--record")?.into()),
            "--wav" => o.wav = Some(value("--wav")?.into()),
            "--mute" => o.mute = true,
            "--tape-sound" => o.tape_sound = true,
            "--no-motor-control" => o.no_motor_control = true,
            "--invert-tape" => o.invert_tape = true,
            "--state" => o.state = Some(value("--state")?.into()),
            "--restore" => o.restore = Some(value("--restore")?.into()),
            "--save-state" => o.save_state = Some(value("--save-state")?.into()),
            "--debug" => o.debug = true,
            "--symbols" => o.symbols.push(value("--symbols")?.into()),
            "--keys" => o.keys = Some(value("--keys")?.into()),
            "--panel" => o.panel = true,
            "--write-keys" => o.write_keys = Some(value("--write-keys")?.into()),
            "--bin2wav" => {
                let from = value("--bin2wav")?;
                o.bin2wav = Some((from.into(), value("--bin2wav")?.into()));
            }
            "--wav2bin" => {
                let from = value("--wav2bin")?;
                o.wav2bin = Some((from.into(), value("--wav2bin")?.into()));
            }
            "--tape-name" => o.tape_name = Some(value("--tape-name")?),
            "--wait" => {
                o.wait = value("--wait")?
                    .parse()
                    .map_err(|_| "--wait needs a number")?
            }
            "--mono" => o.config.monitor = Monitor::Mono,
            "--fdd" => o.config.fdd = true,
            "--ay" => o.config.ay = true,
            "--covox" => o.config.covox = true,
            "--printer" => {
                o.printer = Some(value("--printer")?.into());
                o.config.printer = true;
            }
            "--serial-in" => {
                o.serial_in = Some(value("--serial-in")?.into());
                o.config.serial = true;
            }
            "--serial-out" => {
                o.serial_out = Some(value("--serial-out")?.into());
                o.config.serial = true;
            }
            "--irq2-50hz" => o.config.irq2_50hz = true,
            "--frames" => {
                o.frames = Some(
                    value("--frames")?
                        .parse()
                        .map_err(|_| "--frames needs a number")?,
                )
            }
            "--type" => o.text = Some(unescape(&value("--type")?)?),
            "--screenshot" => o.screenshot = Some(value("--screenshot")?.into()),
            "-h" | "--help" => return Err(String::new()),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    Ok(o)
}

fn report(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("bk: {e}");
            1
        }
    }
}

/// `--bin2wav`: a `.bin` file as a tape recording.
fn bin2wav(from: &Path, to: &Path, name: Option<&str>) -> Result<(), String> {
    let bytes = std::fs::read(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let file = BinFile::parse(&bytes).map_err(|e| format!("{}: {e}", from.display()))?;
    let stem = from
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name: String = name.unwrap_or(&stem).chars().take(16).collect();
    let tape = bk::encode_tape(&name, &file)?;
    std::fs::write(to, tape.to_wav().to_bytes()).map_err(|e| format!("{}: {e}", to.display()))?;
    eprintln!(
        "{}: {name:?}, {} bytes at {:06o}, {:.1} s of tape",
        to.display(),
        file.data.len(),
        file.address,
        tape.len_seconds()
    );
    Ok(())
}

/// `--wav2bin`: every file on a recording, read through the BK-0010 ROM.
fn wav2bin(roms_dir: &Path, from: &Path, dir: &Path) -> Result<(), String> {
    let roms = Roms::load_bk0010(roms_dir, Bk0010Rom::Basic)
        .map_err(|e| format!("can't load the BK-0010 ROMs: {e}"))?;
    let wav = read_recording(from)?;
    let files = bk::read_tape_files(roms, TapeSignal::from_wav(&wav))?;
    if files.is_empty() {
        return Err(format!("{}: no files found", from.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Tapes often carry a file twice; number the copies.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for f in &files {
        // A file name the host accepts: no spaces, no path separators.
        let safe: String = f
            .name
            .chars()
            .filter(|c| *c != ' ')
            .map(|c| {
                if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let safe = if safe.is_empty() {
            "NONAME".to_string()
        } else {
            safe
        };
        let copy = seen.entry(safe.clone()).or_insert(0);
        *copy += 1;
        let copy = match *copy {
            1 => String::new(),
            n => format!("~{n}"),
        };
        // A file the recording ends inside is written as far as it was read.
        let part = if f.complete { "" } else { "-part" };
        let path = dir.join(format!("{safe}{copy}{part}.bin"));
        std::fs::write(&path, f.file.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        let state = if !f.complete {
            format!(
                " — {}: {} of its {} bytes read",
                f.problem.as_deref().unwrap_or("incomplete"),
                f.file.data.len(),
                f.length
            )
        } else if !f.checksum_ok {
            " — CHECKSUM ERROR (damaged, or a file not meant to pass the ROM's check)".into()
        } else {
            String::new()
        };
        eprintln!(
            "{}: {:?}, {} bytes at {:06o}{state}",
            path.display(),
            f.name,
            f.file.data.len(),
            f.file.address,
        );
    }
    Ok(())
}

/// The key map: from `--keys FILE`, else from `bk-keys.ini` if it exists,
/// else the default.
fn load_keymap(path: Option<&Path>) -> Result<keys::Keymap, String> {
    let default = Path::new("bk-keys.ini");
    let path = match path {
        Some(p) => p,
        None if default.exists() => default,
        None => return Ok(keys::Keymap::default()),
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let keymap = keys::Keymap::from_ini(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("key map from {}", path.display());
    Ok(keymap)
}

fn main() {
    let options = parse_args().unwrap_or_else(|e| {
        // An empty error is --help: the usage goes to stdout, and it's not
        // a failure.
        if e.is_empty() {
            println!("{USAGE}");
            process::exit(0);
        }
        eprintln!("bk: {e}\n");
        eprintln!("{USAGE}");
        process::exit(2);
    });
    let keymap = load_keymap(options.keys.as_deref()).unwrap_or_else(|e| {
        eprintln!("bk: {e}");
        process::exit(1);
    });
    if let Some((from, to)) = &options.bin2wav {
        let result = bin2wav(from, to, options.tape_name.as_deref());
        process::exit(report(result));
    }
    if let Some((from, to)) = &options.wav2bin {
        let result = wav2bin(&options.roms, from, to);
        process::exit(report(result));
    }
    if let Some(path) = &options.write_keys {
        if let Err(e) = std::fs::write(path, keymap.to_ini()) {
            eprintln!("bk: {}: {e}", path.display());
            process::exit(1);
        }
        eprintln!("wrote the key map to {}", path.display());
        return;
    }
    let roms = match options.model {
        Model::Bk0011m => Roms::load_bk0011m(&options.roms, options.config.fdd),
        Model::Bk0010 if options.config.fdd => {
            if options.bk0010_rom == Bk0010Rom::Focal {
                eprintln!("bk: the floppy controller goes with BASIC, not FOCAL (drop --focal)");
                process::exit(2);
            }
            Roms::load_bk0010(&options.roms, Bk0010Rom::Fdd)
        }
        Model::Bk0010 => Roms::load_bk0010(&options.roms, options.bk0010_rom),
    };
    let roms = roms.unwrap_or_else(|e| {
        eprintln!("bk: can't load ROMs: {e}");
        process::exit(1);
    });
    let mut machine = Machine::new(roms, options.config);
    machine.set_tape_sound(options.tape_sound);
    machine.set_motor_control(!options.no_motor_control);
    machine.set_tape_invert(options.invert_tape);
    if let Some(path) = &options.serial_in {
        match std::fs::read(path) {
            Ok(bytes) => machine.serial_send(&bytes),
            Err(e) => {
                eprintln!("bk: {}: {e}", path.display());
                process::exit(1);
            }
        }
    }
    if options.disks.len() > 4 {
        eprintln!("bk: at most 4 disks");
        process::exit(2);
    }
    for (drive, path) in options.disks.iter().enumerate() {
        let result = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|data| {
                let read_only = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
                machine.insert_disk(drive, DiskImage::new(data, read_only))
            });
        if let Err(e) = result {
            eprintln!("bk: {}: {e}", path.display());
            process::exit(1);
        }
    }
    if let Err(e) = set_up_tape(&mut machine, &options) {
        eprintln!("bk: {e}");
        process::exit(1);
    }
    if options.load.is_some() && options.restore.is_some() {
        eprintln!("bk: use --load or --restore, not both");
        process::exit(2);
    }
    if let Some(path) = &options.load
        && let Err(e) = boot_and_load(&mut machine, path, options.start)
    {
        eprintln!("bk: {}: {e}", path.display());
        process::exit(1);
    }
    if let Some(path) = &options.restore
        && let Err(e) = load_state(&mut machine, path)
    {
        eprintln!("bk: {e}");
        process::exit(1);
    }

    let headless = options.frames.is_some()
        || options.text.is_some()
        || options.screenshot.is_some()
        || options.wav.is_some()
        || options.save_state.is_some();
    let state_file = options.state.clone().unwrap_or_else(|| {
        PathBuf::from(match machine.model() {
            Model::Bk0010 => "bk0010.state",
            Model::Bk0011m => "bk0011m.state",
        })
    });
    let mut disks = options.disks.clone();
    let result = if headless {
        run_headless(&mut machine, &options)
    } else {
        let label = model_label(&options);
        let panel = PanelSetup {
            fdd: options.config.fdd,
            label,
            keymap,
            disks: &mut disks,
            symbols: symbol_text(&options, machine.model()),
            outputs: Outputs::of(&options),
        };
        window::run(
            &mut machine,
            options.mute,
            &state_file,
            options.debug,
            panel,
        )
    }
    .and_then(|()| save_recording(&mut machine, options.record.as_deref()))
    .and_then(|()| save_disks(&mut machine, &disks));
    if let Err(e) = result {
        eprintln!("bk: {e}");
        process::exit(1);
    }
}

fn set_up_tape(machine: &mut Machine, options: &Options) -> Result<(), String> {
    let files = options.files.clone().or_else(|| {
        let parent = options.load.as_ref()?.parent()?;
        Some(if parent.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            parent.to_path_buf()
        })
    });
    if let Some(dir) = files {
        machine.set_tape_files(Some(Box::new(DirFiles { dir })));
    }
    if let Some(path) = &options.tape {
        let wav = read_recording(path)?;
        machine.insert_tape(Some(TapeSignal::from_wav(&wav)));
    }
    if options.record.is_some() {
        machine.start_recording();
    }
    Ok(())
}

fn save_state(machine: &Machine, path: &Path) -> Result<(), String> {
    std::fs::write(path, machine.save_state()).map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("saved the state to {}", path.display());
    Ok(())
}

fn load_state(machine: &mut Machine, path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    machine
        .load_state(&bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("loaded the state from {}", path.display());
    Ok(())
}

/// Write back disk images the BK changed.
fn save_disks(machine: &mut Machine, paths: &[PathBuf]) -> Result<(), String> {
    for (drive, path) in paths.iter().enumerate() {
        if let Some(image) = machine.disk(drive)
            && image.dirty
            && !image.read_only
        {
            std::fs::write(path, &image.data).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("saved changes to {}", path.display());
        }
    }
    Ok(())
}

fn save_recording(machine: &mut Machine, path: Option<&Path>) -> Result<(), String> {
    let (Some(path), Some(tape)) = (path, machine.take_recording()) else {
        return Ok(());
    };
    std::fs::write(path, tape.to_wav().to_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!(
        "recorded {:.1} s of tape to {}",
        tape.len_seconds(),
        path.display()
    );
    Ok(())
}

/// Let the firmware boot, then load a `.bin` program and start it.
fn boot_and_load(machine: &mut Machine, path: &Path, start: Option<u16>) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let bin = BinFile::parse(&bytes)?;
    for _ in 0..BOOT_FRAMES {
        machine.run_frame();
    }
    machine.load_bin(&bin)?;
    let start = start.unwrap_or_else(|| bin.start_address());
    machine.start_at(start);
    eprintln!(
        "loaded {} bytes at {:06o}, started at {start:06o}",
        bin.data.len(),
        bin.address
    );
    Ok(())
}

/// Stands for `\p` (a one-second pause) in `--type` text.
const PAUSE: char = '\u{fffe}';

/// `--type` text: `\n` is Enter and `\NNN` (octal) is any BK key code,
/// e.g. `\017` for ЛАТ.
fn unescape(s: &str) -> Result<String, String> {
    const RUS: char = '\x0e';
    const LAT: char = '\x0f';
    let mut out = String::new();
    // Inside a run of Russian letters, typed in РУС mode.
    let mut russian = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(code) = bk::russian_key(c) {
            if !russian {
                out.push(RUS);
                russian = true;
            }
            out.push(char::from(code));
            continue;
        }
        // Back to ЛАТ when the run ends (spaces don't end it).
        if russian && c != ' ' {
            out.push(LAT);
            russian = false;
        }
        if c != '\\' {
            if !c.is_ascii() {
                return Err(format!(
                    "can't type {c:?}: only ASCII, Russian letters and \\NNN codes"
                ));
            }
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'n') {
            chars.next();
            out.push('\n');
            continue;
        }
        if chars.peek() == Some(&'p') {
            chars.next();
            out.push(PAUSE);
            continue;
        }
        let digits: String = std::iter::from_fn(|| chars.next_if(|d| ('0'..='7').contains(d)))
            .take(3)
            .collect();
        // Codes 200-377 (function keys and АР2 combinations) travel as
        // the characters U+0080-U+00FF.
        let code = u8::from_str_radix(&digits, 8)
            .map_err(|_| format!("bad escape in --type: \\{digits} (codes go up to 377)"))?;
        out.push(char::from(code));
    }
    if russian {
        out.push(LAT);
    }
    Ok(out)
}

fn run_headless(machine: &mut Machine, options: &Options) -> Result<(), String> {
    for _ in 0..options.frames.unwrap_or(100) {
        machine.run_frame();
    }
    if let Some(text) = &options.text {
        for (i, part) in text.split(PAUSE).enumerate() {
            if i > 0 {
                for _ in 0..50 {
                    machine.run_frame();
                }
            }
            machine.type_text(part);
        }
        for _ in 0..options.wait {
            machine.run_frame();
        }
    }
    if options.debug {
        let mut console = console::Console::new();
        console.load_symbols(&symbol_text(options, machine.model()));
        console.run_headless(machine);
    }
    let cpu = machine.cpu();
    println!(
        "frames {}  PC {:06o}  PSW {:06o}  SP {:06o}{}",
        machine.frames(),
        cpu.pc(),
        cpu.psw,
        cpu.r[6],
        if cpu.is_halted() { "  HALTED" } else { "" }
    );
    if let Some(path) = &options.screenshot {
        let panel = options
            .panel
            .then(|| (model_label(options), options.config.fdd));
        save_png(machine, path, panel)?;
    }
    if let Some(path) = &options.save_state {
        save_state(machine, path)?;
    }
    Outputs::of(options).write(machine)?;
    if let Some(path) = &options.wav {
        let wav = Wav {
            rate: machine.audio_rate(),
            samples: machine.take_audio(),
        };
        std::fs::write(path, wav.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The picture as shown: the BK's 512 x 256 dots at the 4:3 of a TV, each
/// dot 2 pixels wide and each line 3 high (1024 x 768).
const OUT_WIDTH: usize = 2 * SCREEN_WIDTH;
const OUT_SCREEN_HEIGHT: usize = 3 * SCREEN_HEIGHT;
/// The keyboard panel, drawn at its own size and doubled to match.
const OUT_PANEL_HEIGHT: usize = 2 * panel::PANEL_HEIGHT;

/// Keeps the emulator to the real clock: a 20 ms frame is due every 20 ms.
/// The sound device has a clock of its own, so with sound the queue of
/// samples waiting to be played trims the drift between the two.
struct Pacer {
    start: std::time::Instant,
    frames: u64,
}

impl Pacer {
    const FRAME_US: u128 = 20_000;
    /// Sound queued ahead of the speaker: the margin against the window
    /// pausing (dragged, covered) without a delay you'd hear.
    const QUEUE_MS: u64 = 60;

    fn new() -> Self {
        let now = std::time::Instant::now();
        // Start with the queue's worth of frames due, to fill it.
        let ahead = std::time::Duration::from_millis(Self::QUEUE_MS);
        Pacer {
            start: now.checked_sub(ahead).unwrap_or(now),
            frames: 0,
        }
    }

    /// Frames due and not yet run. After a stall (the window dragged, the
    /// computer busy) it starts afresh rather than racing to catch up.
    fn due(&mut self) -> u64 {
        let due = (self.start.elapsed().as_micros() / Self::FRAME_US) as u64;
        if due > self.frames + 10 {
            *self = Pacer::new();
            return 1;
        }
        due.saturating_sub(self.frames)
    }

    fn ran(&mut self) {
        self.frames += 1;
    }

    /// Nudge the clock to keep the sound queue near [`Pacer::QUEUE_MS`]:
    /// a fraction of a millisecond per update, too little to hear.
    fn trim(&mut self, queued: usize, rate: u32) {
        let ms = queued as u64 * 1000 / u64::from(rate.max(1));
        let step = std::time::Duration::from_micros(200);
        if ms > 2 * Self::QUEUE_MS {
            self.start += step;
        } else if ms < Self::QUEUE_MS / 2 {
            self.start = self.start.checked_sub(step).unwrap_or(self.start);
        }
    }
}

/// Render the screen into `out` at [`OUT_WIDTH`] x [`OUT_SCREEN_HEIGHT`].
fn tv_frame(machine: &Machine, frame: &mut [u32], out: &mut Vec<u32>) {
    machine.render(frame);
    out.clear();
    for line in frame.as_chunks::<SCREEN_WIDTH>().0 {
        let wide: Vec<u32> = line.iter().flat_map(|&p| [p, p]).collect();
        for _ in 0..3 {
            out.extend_from_slice(&wide);
        }
    }
}

/// Copy the panel drawn at its own size (`small`, [`SCREEN_WIDTH`] wide)
/// into `out` doubled.
fn double_panel(small: &[u32], out: &mut [u32]) {
    for (y, line) in small.as_chunks::<SCREEN_WIDTH>().0.iter().enumerate() {
        for dy in 0..2 {
            let row = &mut out[(2 * y + dy) * OUT_WIDTH..(2 * y + dy + 1) * OUT_WIDTH];
            for (x, &p) in line.iter().enumerate() {
                row[2 * x] = p;
                row[2 * x + 1] = p;
            }
        }
    }
}

/// Save the screen as a PNG; with `panel`, the keyboard panel under it, as
/// the window shows it (labelled `model`).
fn save_png(machine: &Machine, path: &Path, panel: Option<(&str, bool)>) -> Result<(), String> {
    let mut frame = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut pixels = Vec::new();
    tv_frame(machine, &mut frame, &mut pixels);
    if let Some((model, fdd)) = panel {
        let screen = pixels.len();
        pixels.resize(screen + OUT_WIDTH * OUT_PANEL_HEIGHT, 0);
        let mut small = vec![0; SCREEN_WIDTH * panel::PANEL_HEIGHT];
        let lights = panel::Lights {
            power: true,
            tape: machine.tape_motor_on(),
            tape_time: machine.tape_time(),
            tape_moving: machine.tape_busy(),
            disk: fdd.then(|| machine.disk_motor_on()),
            debugger: false,
            fast: false,
            model,
        };
        panel::Panel::new(SCREEN_WIDTH).draw(&mut small, &lights, &|_| false, false);
        double_panel(&small, &mut pixels[screen..]);
    }
    let height = pixels.len() / OUT_WIDTH;
    let rgb: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();

    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        OUT_WIDTH as u32,
        height as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(&rgb))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The joystick lines from the gamepads: the D-pad or the left stick for
/// the directions (bits 0 up, 1 right, 2 down, 3 left), the face buttons
/// for bits 5 (fire), 6, 4 and 7.
fn gamepad_lines(gilrs: &mut gilrs::Gilrs) -> u8 {
    use gilrs::{Axis, Button};
    while gilrs.next_event().is_some() {}
    let mut bits = 0;
    for (_, pad) in gilrs.gamepads() {
        let x = pad.value(Axis::LeftStickX);
        let y = pad.value(Axis::LeftStickY);
        let lines = [
            (pad.is_pressed(Button::DPadUp) || y > 0.5, 0),
            (pad.is_pressed(Button::DPadRight) || x > 0.5, 1),
            (pad.is_pressed(Button::DPadDown) || y < -0.5, 2),
            (pad.is_pressed(Button::DPadLeft) || x < -0.5, 3),
            (pad.is_pressed(Button::West), 4),
            (pad.is_pressed(Button::South), 5),
            (pad.is_pressed(Button::East), 6),
            (pad.is_pressed(Button::North), 7),
        ];
        for (on, line) in lines {
            if on {
                bits |= 1 << line;
            }
        }
    }
    bits
}

/// Alt+O: choose a file in a dialog and use it by its type: a program is
/// loaded and started, a recording goes in the tape recorder, a disk image
/// in drive 0 (the one there written back first), a state is restored.
fn open_file(machine: &mut Machine, disks: &mut Vec<PathBuf>) -> Result<(), String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(
            "BK files",
            &["bin", "wav", "mp3", "gz", "dsk", "img", "bkd", "state"],
        )
        .add_filter("All files", &["*"])
        .pick_file()
    else {
        return Ok(());
    };
    open_path(machine, &path, disks)
}

/// Read a tape recording, WAV or MP3 ([`bk::parse_recording`]).
fn read_recording(path: &Path) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    bk::parse_recording(bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Use a file by its type, as [`open_file`] does.
fn open_path(machine: &mut Machine, path: &Path, disks: &mut Vec<PathBuf>) -> Result<(), String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let shown = path.display();
    let read = || std::fs::read(path).map_err(|e| format!("{shown}: {e}"));
    match ext.as_str() {
        "bin" => {
            let bin = BinFile::parse(&read()?).map_err(|e| format!("{shown}: {e}"))?;
            machine
                .load_bin(&bin)
                .map_err(|e| format!("{shown}: {e}"))?;
            if let Some(dir) = path.parent() {
                machine.set_tape_files(Some(Box::new(DirFiles {
                    dir: dir.to_path_buf(),
                })));
            }
            machine.start_at(bin.start_address());
            eprintln!(
                "loaded {shown}: {} bytes at {:06o}",
                bin.data.len(),
                bin.address
            );
        }
        "wav" | "mp3" | "gz" => {
            let wav = read_recording(path)?;
            machine.insert_tape(Some(TapeSignal::from_wav(&wav)));
            eprintln!("tape {shown} in the recorder");
        }
        "dsk" | "img" | "bkd" => {
            let data = read()?;
            let read_only = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
            // Write back the disk being replaced, as on exit.
            save_disks(machine, &disks[..disks.len().min(1)])?;
            machine.insert_disk(0, DiskImage::new(data, read_only))?;
            if disks.is_empty() {
                disks.push(path.to_path_buf());
            } else {
                disks[0] = path.to_path_buf();
            }
            eprintln!("disk {shown} in drive 0");
        }
        "state" => load_state(machine, path)?,
        _ => {
            return Err(format!(
                "{shown}: not a .bin, .wav, disk image or .state file"
            ));
        }
    }
    Ok(())
}

/// Switch between the color and the mono monitor.
fn toggle_monitor(machine: &mut Machine) {
    let next = match machine.monitor() {
        Monitor::Color => Monitor::Mono,
        Monitor::Mono => Monitor::Color,
    };
    machine.set_monitor(next);
}

/// Queue the text on the host clipboard for typing into the BK.
fn paste(typist: &mut bk::Typist) -> Result<(), String> {
    let text = arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map_err(|e| format!("clipboard: {e}"))?;
    let skipped = typist.push_text(&text);
    eprintln!(
        "pasting {} keys{}",
        typist.pending(),
        if skipped > 0 {
            format!(" ({skipped} characters the BK can't type left out)")
        } else {
            String::new()
        }
    );
    Ok(())
}

/// The debugger's symbols: the ROMs' names from `./annotations` (the files
/// for this model), and any `--symbols` files.
fn symbol_text(options: &Options, model: Model) -> String {
    let rom_files: &[&str] = match (model, options.config.fdd) {
        (Model::Bk0010, _) => &["devices.ann", "bk0010.ann", "monit10.ann"],
        (Model::Bk0011m, false) => &[
            "devices.ann",
            "bk0011m.ann",
            "b11m_bos.ann",
            "b11m_ext.ann",
            "b11m_mstd.ann",
        ],
        (Model::Bk0011m, true) => &[
            "devices.ann",
            "bk0011m.ann",
            "b11m_bos.ann",
            "b11m_ext.ann",
            "disk_327.ann",
        ],
    };
    let mut text = String::new();
    for f in rom_files {
        if let Ok(t) = std::fs::read_to_string(Path::new("annotations").join(f)) {
            text += &t;
            text.push('\n');
        }
    }
    for path in &options.symbols {
        match std::fs::read_to_string(path) {
            Ok(t) => {
                text += &t;
                text.push('\n');
            }
            Err(e) => eprintln!("bk: {}: {e}", path.display()),
        }
    }
    text
}

/// Files that collect what the BK sends out: the printer's text and the
/// serial line's bytes.
#[derive(Clone, Debug, Default)]
struct Outputs {
    printer: Option<PathBuf>,
    serial: Option<PathBuf>,
}

impl Outputs {
    fn of(options: &Options) -> Self {
        Outputs {
            printer: options.printer.clone(),
            serial: options.serial_out.clone(),
        }
    }

    /// Add what has come out since the last call to the files.
    fn write(&self, machine: &mut Machine) -> Result<(), String> {
        let printed = machine.take_printed();
        if let Some(path) = &self.printer
            && !printed.is_empty()
        {
            append(path, printer_text(&printed).as_bytes())?;
        }
        let sent = machine.take_serial();
        if let Some(path) = &self.serial
            && !sent.is_empty()
        {
            append(path, &sent)?;
        }
        Ok(())
    }
}

fn append(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// What the printer printed, as text: KOI-8 decoded, line ends kept,
/// carriage returns and other control codes dropped (form feeds kept).
fn printer_text(printed: &[u8]) -> String {
    printed
        .iter()
        .filter_map(|&b| match b {
            0o012 => Some('\n'),
            0o014 => Some('\x0c'),
            0o040..=0o176 => Some(b as char),
            0o300..=0o377 => bk::vm1::koi8::decode(b),
            _ => None,
        })
        .collect()
}

/// The model's name on the panel.
fn model_label(options: &Options) -> &'static str {
    match (options.model, options.bk0010_rom) {
        (Model::Bk0011m, _) => "БК-0011М",
        (Model::Bk0010, Bk0010Rom::Basic | Bk0010Rom::Fdd) => "БК-0010.01",
        (Model::Bk0010, Bk0010Rom::Focal) => "БК-0010",
    }
}

/// What the panel under the screen shows that the machine doesn't know.
struct PanelSetup<'a> {
    fdd: bool,
    label: &'static str,
    keymap: keys::Keymap,
    /// The image file of each drive's disk, to write changes back to.
    disks: &'a mut Vec<PathBuf>,
    /// The debugger's symbols.
    symbols: String,
    /// Where the printer's and serial line's output goes.
    outputs: Outputs,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roms_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms")
    }

    /// The pacer starts with the sound queue's worth of frames due, and
    /// trims its clock against the queue: a full one slows it, an empty one
    /// speeds it up.
    #[test]
    fn the_pacer_fills_the_sound_queue_then_keeps_to_it() {
        let mut p = Pacer::new();
        let first = p.due();
        assert_eq!(
            first,
            (Pacer::QUEUE_MS * 1000 / Pacer::FRAME_US as u64),
            "frames due at the start"
        );
        for _ in 0..first {
            p.ran();
        }
        assert_eq!(p.due(), 0);
        let start = p.start;
        p.trim(48_000, 48_000); // a second queued: too much
        assert!(p.start > start);
        let start = p.start;
        p.trim(0, 48_000); // nothing queued: too little
        assert!(p.start < start);
    }

    /// `--type` takes Russian letters: РУС, their KOI-7 codes, then ЛАТ.
    #[test]
    fn typed_text_switches_to_russian_and_back() {
        assert_eq!(unescape("M\\nПЕРЕВАЛ\\n").unwrap(), "M\n\x0eperewal\x0f\n");
        assert_eq!(unescape("ДВА НЕГРА").unwrap(), "\x0edwa negra\x0f");
        assert_eq!(unescape("ab").unwrap(), "ab");
        assert!(unescape("π").is_err());
    }

    fn here(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
    }

    /// An MP3 tape (stereo, 128 kbit/s, the right channel 2 samples late)
    /// reads back exactly through the ROM.
    #[test]
    fn mp3_tapes_read_back_exactly() {
        let Ok(roms) = Roms::load_bk0010(&roms_dir(), Bk0010Rom::Basic) else {
            eprintln!("skipping: no ROMs");
            return;
        };
        let wav = read_recording(&here("testdata/hello-stereo.mp3")).unwrap();
        assert_eq!(wav.rate, 44_100);
        let files = bk::read_tape_files(roms, TapeSignal::from_wav(&wav)).unwrap();
        let expected = BinFile::parse(&std::fs::read(here("testdata/hello.bin")).unwrap()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "HELLO");
        assert!(files[0].checksum_ok);
        assert_eq!(files[0].file, expected);
        assert_eq!(
            bk::parse_recording(b"not audio".to_vec()).unwrap_err(),
            "not a WAV or MP3 file"
        );
    }

    /// A real cassette from the archive, read by a stereo head whose
    /// channels are 2 samples apart: averaged as they were, no file was
    /// found; lined up, the first file's header reads.
    #[test]
    fn stereo_channels_lined_up_on_a_real_tape() {
        let Ok(roms) = Roms::load_bk0010(&roms_dir(), Bk0010Rom::Basic) else {
            eprintln!("skipping: no ROMs");
            return;
        };
        let Ok(wav) = read_recording(&here("../../software/tapes/survey-4.mp3")) else {
            eprintln!("skipping: software/tapes/survey-4.mp3 missing");
            return;
        };
        let files = bk::read_tape_files(roms, TapeSignal::from_wav(&wav)).unwrap();
        assert!(files.iter().any(|f| f.name == "R&O-REKL1"), "{files:?}");
    }

    /// Alt+O's file handling: each type of file goes where it should.
    #[test]
    fn opened_files_go_where_they_belong() {
        let Ok(roms) = Roms::load_bk0011m(&roms_dir(), true) else {
            eprintln!("skipping: no ROMs");
            return;
        };
        let dir = std::env::temp_dir().join(format!("bk-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut m = Machine::new(
            roms,
            Config {
                fdd: true,
                ..Default::default()
            },
        );
        let mut disks = Vec::new();

        let bin = BinFile {
            address: 0o1000,
            data: vec![0o001, 0o000, 0o377, 0o001],
        }; // WAIT, BR .
        std::fs::write(dir.join("p.bin"), bin.to_bytes()).unwrap();
        open_path(&mut m, &dir.join("p.bin"), &mut disks).unwrap();
        assert_eq!(
            (m.cpu().pc(), m.peek_word(0o1000)),
            (0o1000, Some(1)),
            "loaded and started"
        );

        let tape = bk::encode_tape("T", &bin).unwrap();
        std::fs::write(dir.join("t.wav"), tape.to_wav().to_bytes()).unwrap();
        open_path(&mut m, &dir.join("t.wav"), &mut disks).unwrap();
        assert_eq!(m.tape_position(), Some(0), "tape in");

        std::fs::write(dir.join("d.dsk"), vec![0; bk::IMAGE_BYTES]).unwrap();
        open_path(&mut m, &dir.join("d.dsk"), &mut disks).unwrap();
        assert!(m.disk(0).is_some());
        assert_eq!(disks, [dir.join("d.dsk")], "remembered for writing back");

        std::fs::write(dir.join("s.state"), m.save_state()).unwrap();
        m.start_at(0o2000);
        open_path(&mut m, &dir.join("s.state"), &mut disks).unwrap();
        assert_eq!(m.cpu().pc(), 0o1000, "state restored");

        assert!(open_path(&mut m, &dir.join("x.txt"), &mut disks).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

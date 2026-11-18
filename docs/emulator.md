# Using the emulator

This emulator runs three machines of the Elektronika BK family: the
**BK-0011M**, the **BK-0010.01** and the original **BK-0010**. It runs their
original ROMs, so everything described in the other guides (BASIC, FOCAL, the
monitors, the system calls) works the way the manuals say. See
[machine.md](machine.md) for what the computers are.

- [Building](#building)
- [The ROM files](#the-rom-files)
- [Choosing a machine](#choosing-a-machine)
- [The keyboard](#the-keyboard)
- [Loading software](#loading-software)
- [Save states](#save-states)
- [The debugger](#the-debugger)
- [Headless runs and automation](#headless-runs-and-automation)
- [Other tools in the repository](#other-tools-in-the-repository)
- [What is and isn't emulated](#what-is-and-isnt-emulated)

## Building

You need a Rust toolchain (edition 2024). From the repository root:

```sh
cargo build --release
```

The program is `target/release/bk`. `bk --help` lists every option. With no
options it opens a window showing a BK-0011M, which boots into BASIC:

```
БЕЙСИК БК11/10 (C) ОКБ "ЭКСИТОН"
Ok
```

Close the window to quit.

## The ROM files

The ROM images are in the repository's `roms` directory, which `bk` uses
when run from the repository; elsewhere, point `--roms DIR` at them. They
are the images the bkbtl emulator's repository publishes under the same
names, in its [`roms`](https://github.com/nzeemin/bkbtl/tree/master/roms)
folder (they match the checksums below); their copyright status is
unclear. `roms/` also holds `disk_253.rom`, another floppy controller ROM
that `bk` doesn't use, and in `roms/mist/` the MiST FPGA core's combined
128 KB images.

| File | Size (bytes) | Machine | Mapped at | Contents | MD5 |
|---|---|---|---|---|---|
| `b11m_bos.rom` | 8192 | BK-0011M | 140000 | BOS, the base operating system | `fe4627d1e3a1535874085050733263e7` |
| `b11m_ext.rom` | 8192 | BK-0011M | window 100000 (with `basic11m_1`) | second half of BOS | `dc52f365d56fa1951f5d35b1101b9e3f` |
| `basic11m_0.rom` | 16384 | BK-0011M | window 100000 | BASIC, part 0 | `946f6f23ded03c0e26187f0b3ca75993` |
| `basic11m_1.rom` | 8192 | BK-0011M | window 100000 | BASIC, part 1 | `1e6637f32aa7d1de03510030cac40bcf` |
| `b11m_mstd.rom` | 8192 | BK-0011M | 160000 | МСД test and diagnostic monitor | `cbfecc57c911b0cd425092bb6c4db7f5` |
| `disk_327.rom` | 4096 | BK-0011M with `--fdd` | 160000 | floppy controller ROM (replaces МСД) | `5015228eeeb238e65da8edcd1b6dfac7` |
| `disk_326.rom` | 4096 | BK-0010.01 with `--fdd` | 160000 | floppy controller ROM (replaces BASIC part 3) | `30c9e751d2c937f1b0e0fdae14086acf` |
| `monit10.rom` | 8192 | BK-0010 | 100000 | driver-monitor system | `95f8c41c6abf7640e35a6a03cecebd01` |
| `basic10_1.rom` | 8192 | BK-0010.01 | 120000 | BASIC, part 1 | `4a4530347ee18c547a0563aca73cf43d` |
| `basic10_2.rom` | 8192 | BK-0010.01 | 140000 | BASIC, part 2 | `86fc2f7797a0333300159aa222c3ad3f` |
| `basic10_3.rom` | 8064 | BK-0010.01 | 160000 | BASIC, part 3 | `fb8875a62b9b02a66670dcefc270d441` |
| `focal.rom` | 8192 | BK-0010 (`--focal`) | 120000 | FOCAL | `eb9e1cf1c1b36a2dece89624bfc59323` |
| `tests.rom` | 8064 | BK-0010 (`--focal`) | 160000 | test monitor (МСТД block) | `93d2776ecf9abf49fb45f58ce3182143` |

Addresses are octal, as everywhere in these guides. The MD5 sums identify
the images the emulator was tested with. [machine.md](machine.md) describes
what each ROM does.

## Choosing a machine

| Options | Machine |
|---|---|
| (none) | BK-0011M: 128 KB RAM, 4 MHz, color, BOS + BASIC |
| `--bk0010` | BK-0010.01: 32 KB RAM, 3 MHz, monitor + BASIC |
| `--bk0010 --focal` | BK-0010: monitor + FOCAL + test ROM (the МСТД block) |

Hardware options:

| Option | Effect |
|---|---|
| `--mono` | black-and-white monitor instead of color. In the window, Alt+M (Cmd+M) switches between the two |
| `--fdd` | floppy disk controller fitted. Its ROM goes at 160000 (in place of МСД on the BK-0011M, of BASIC part 3 on the BK-0010.01, where it also brings 16 KB of RAM at 120000) |
| `--disk FILE` | put a disk image in the next drive (0 to 3); implies `--fdd`. Repeat for more drives |
| `--ay` | an AY-3-8910 sound chip on the parallel port |
| `--covox` | a Covox (8-bit DAC) on the parallel port |
| `--printer FILE` | a printer on the parallel port, in place of a joystick: what the BK prints (BASIC's `LPRINT`, BOS's printer calls) is added to FILE as text |
| `--serial-in FILE` | BK-0010: the serial line's far end sends FILE's bytes to the BK, as the BK asks for them (the monitor's `L` command, `EMT 44`, `EMT 50`) |
| `--serial-out FILE` | BK-0010: what the BK sends on the serial line (`EMT 42`, `EMT 46`) is added to FILE, as raw bytes |
| `--irq2-50hz` | BK-0010: a 50 Hz interrupt on vector 100, once a program has set the vector (for FOCAL's `FCLK()`, see [focal.md](focal.md#fclk)) |
| `--mute` | no sound |

The color/mono switch matters: the BK has one video mode of 512 × 256 dots
in black and white or 256 × 256 in four colors, and the same screen memory
is shown either way. Text in the 64-column mode and programs that draw in
512-dot mode look right only on a mono monitor; in color they show red and
green fringes, as on a real color TV.

## The keyboard

Under the screen the window shows the BK's keyboard (the on-screen
keyboard) and its indicator lights. The main block of keys is laid out as
on the BK (the operating manual's keyboard test lists its rows:
`; 1 2 3 4 5 6 7 8 9 0 -`, `J C U K E N G [ ] Z H :`,
`F Y W A P R O L D V \ .`, `Q ^ S M I T X B @ , / _`), each key with its
Latin and Russian letter; the function and control keys are grouped around
it.

- Keys light up while they are pressed, and for at least 150 ms after a
  PC key is typed, so you can see what each PC key does.
- Clicking a key types it. АР2, СУ and the register key ⇧ (shift: the
  other case, or the symbol on the key's other half) latch: click one, then
  the key it should apply to. СТОП interrupts the processor, as on the BK.
- The lights show power (СЕТЬ), the tape motor (МАГНИТОФОН), the floppy
  motor (ДИСК, with `--fdd`) and the debugger (ОТЛАДКА, while stopped in
  it). Clicking the power light switches the machine off; clicking it again
  switches it on again: a cold start, with RAM cleared, exactly like
  starting the emulator.

By default the BK keyboard is mapped onto a PC keyboard like this (see
[Remapping keys](#remapping-keys) to change it). Letters, digits and
punctuation are where they are on a US keyboard.

| PC key | BK key | Code (octal) |
|---|---|---|
| Enter | ВВОД (enter) | 012 |
| Backspace | ЗАБОЙ (delete back) | 030 |
| Tab | ТАБ (tab) | 211 (code 011 through the function-key vector) |
| ← → ↑ ↓ | cursor keys | 010, 031, 032, 033 |
| Left Ctrl (held) | АР2 (alternate register 2) | — |
| Right Ctrl (held) | СУ (control: letter keys give control codes, e.g. ВС = СУ+S) | — |
| `` ` `` / Shift+`` ` `` | `` ` `` and `~` | 140, 176 |
| Home (also F11) | ЛАТ (Latin letters) | 017 |
| End (also F10) | РУС (Russian letters) | 016 |
| Esc (also F12) | СТОП (stop) | — (interrupt) |
| F1 | ПОВТ (repeat) | 201 |
| F2 | КТ | 003 |
| F3 | =\|=>\| | 213 |
| F4 | \|<== | 026 |
| F5 | \|==> | 027 |
| F6 | ИНД СУ (show control codes) | 202 |
| F7 | БЛОК РЕД (editing lock) | 204 |
| F8 | ШАГ (step) | 200 |
| F9 | СБР (reset / clear screen) | 014 |

Shift with a letter gives the other case, and with a digit or punctuation
key the symbol printed on a US key cap (Shift+2 is `@`, Shift+6 `^`,
Shift+8 `*`). The BK's own keyboard pairs some symbols differently (its 2
key gives `"` with ⇧, its 6 key `&`), so where a guide writes a BK key
with ⇧, such as АР2+⇧+2, type the symbol instead (`"` is Shift+' on a US
keyboard), or use ⇧ on the on-screen keyboard, which gives the BK's pairs.

For Russian letters press РУС (End); each Latin letter key then gives the
Russian letter paired with it in the KOI-7 code, mostly by sound (A а,
B б, C ц, D д…; the full list is in
[monitor.md](monitor.md#letters-and-case)). Unshifted keys give lower
case, Shift capitals. ЛАТ (Home) switches back. [monitor.md](monitor.md)
describes what the special keys do.

Emulator commands use Alt (or Cmd on a Mac) so they never reach the BK:

| Keys | Does |
|---|---|
| Alt+O | open a file: a `.bin` program is loaded and started (and its directory becomes the fast-tape directory, as with `--load`), a `.wav` goes in the tape recorder, a disk image in drive 0, a `.state` is restored |
| Alt+S | save the machine state (see [Save states](#save-states)) |
| Alt+L | load the saved state |
| Alt+D | stop in the debugger (see [The debugger](#the-debugger)) |
| Alt+V | type the text on the clipboard into the BK |
| Alt+F | fast-forward on or off: run as fast as the PC allows, without sound (it also runs fast by itself while the tape moves with a recording to play or record) |
| Alt+M | switch between the color and the mono monitor |

On macOS, F10 and F11 are usually taken by the system, which is why РУС and
ЛАТ are also on End and Home.

### Pasting text

Alt+V (Cmd+V on a Mac) types the host clipboard's text into the BK, as if
typed by hand: each key waits until the program has read the previous one,
and each line end (ВВОД) is followed by half a second for the program to
deal with the line. So you can paste a BASIC or FOCAL program listing into
the editor. Russian letters are typed in РУС mode (the paste presses РУС
and ЛАТ as needed and ends in ЛАТ); characters the BK keyboard can't type
are left out, and the terminal says how many. The keys light up on the
on-screen keyboard as they are typed. The window title says which key pastes, and while
a paste is being typed, how many keys are left.

### Joystick

A joystick on the parallel port, as many games support, is played with the
numeric keypad: 8 4 6 2 for up, left, right and down, 0 and 5 for the fire
buttons; or with any gamepad (the D-pad or the left stick, and the face
buttons). Games read it at 177714: bit 0 up, 1 right, 2 down, 3 left, 5 and
6 fire (4 and 7 are further buttons). The key map can put it on other keys
(`JOY-UP`, `JOY-FIRE` and so on, below). With `--printer`, a printer takes
the joystick's place on the port.

### Remapping keys

An INI file changes the mapping. The emulator reads `bk-keys.ini` from the
current directory if there is one, or the file given with `--keys FILE`.
`bk --write-keys FILE` writes the mapping in use (the default, or yours) to
a file to start from. Each line under `[keys]` binds one PC key; keys the
file doesn't mention keep their default:

```ini
[keys]
Tab = ТАБ            ; a BK key by name...
F1 = 201             ; ...or by its octal code
Key1 = 061 / 041     ; a second code for Shift
Menu = АР2           ; the modifiers АР2 and СУ
CapsLock = СУ
Insert = СТОП        ; the stop key
Backquote =          ; nothing
```

PC keys have minifb's names: `A`–`Z`, `Key0`–`Key9`, `F1`–`F15`, `Enter`,
`Backspace`, `Tab`, `Space`, `Escape`, `Left`, `Right`, `Up`, `Down`,
`Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`, `Comma`,
`Period`, `Slash`, `Semicolon`, `Apostrophe`, `Minus`, `Equal`,
`LeftBracket`, `RightBracket`, `Backslash`, `Backquote`, `LeftShift`,
`RightShift`, `LeftCtrl`, `RightCtrl`, `CapsLock`, `NumLock`,
`ScrollLock`, `Pause`, `Menu`, `NumPad0`–`NumPad9`, `NumPadDot`,
`NumPadSlash`, `NumPadAsterisk`, `NumPadMinus`, `NumPadPlus`,
`NumPadEnter`. Alt and Cmd can't be remapped: they're the emulator's
commands.

BK keys can be named in Russian or with a Latin alias: ВВОД (`ENTER`),
ЗАБОЙ (`BACKSPACE`), ТАБ (`TAB`), ПОВТ (`REPEAT`), КТ (`KT`), `=|=>|`
(`TAB-RIGHT`), `|<==` (`DELETE-CHAR`), `|==>` (`INSERT-CHAR`), ИНДСУ
(`SHOW-CONTROL`), БЛОКРЕД (`EDIT-LOCK`), ШАГ (`STEP`), СБР (`CLEAR`), РУС,
ЛАТ, ПРОБЕЛ (`SPACE`), ВЛЕВО, ВПРАВО, ВВЕРХ, ВНИЗ (`LEFT`, `RIGHT`, `UP`,
`DOWN`); any other key by its octal code. СТОП (`STOP`), АР2 (`AR2`), СУ
(`SU`: with it held, letter keys give control codes), МОНИТОР
(`MONITOR`: the color/mono switch) and the joystick lines `JOY-UP`,
`JOY-RIGHT`, `JOY-DOWN`, `JOY-LEFT`, `JOY-FIRE` (bit 5), `JOY-FIRE2`
(bit 6), `JOY-BUTTON4` and `JOY-BUTTON7` (in Russian ДЖОЙСТИК-ВВЕРХ and so
on) are the remaining bindings. Names are case-insensitive and spaces in
them are ignored (`БЛОК РЕД` works). A comment starts with `;` or `#`, at
the start of a line or after a space. A mistake stops the emulator with
the file name and line.

## Loading software

BK software comes as `.bin` files (a program image with a 4-byte header: load
address and length), as tape recordings (`.wav`), and, for a machine with a
floppy controller, as disk images. The options below load them at start-up;
Alt+O (Cmd+O) loads any of them while the emulator runs, from a file dialog.

### Programs (`.bin`)

```sh
bk --bk0010 --load software/klad.bin
```

`--load` lets the firmware boot (3 seconds), puts the program in memory and
starts it at its load address. Many programs load below 1000 so that they
start by themselves when loaded from tape; for those the emulator starts them
at 1000. `--start ADDR` (octal) gives another start address.

BK-0010 programs need `--bk0010`. Most don't run on a BK-0011M: they call
the BK-0010 monitor with `EMT`, and BOS numbers its calls differently, and
they use memory at 40000–77777 that holds BASIC's code and data on a
BK-0011M. Programs written for the BK-0011M (or for both) run without it.

### Fast tape: files on the host disk

When a program loads or saves a named file through the ROM's tape routine
(BASIC's `LOAD`, `SAVE` and the like, the BK-0010 monitor's `M`, the
BK-0011M monitor's `L` and `U`, FOCAL's `LIBRARY`, games that load their
next part), `--files DIR` makes the emulator use `DIR/NAME.bin` instead of
a tape signal, instantly. Names are matched without regard to case or spaces
(BASIC pads names before their type, `MEM   .BIN`), with or without
`.bin`; files are written without the spaces (`MEM.BIN.bin`). A file that
isn't in `DIR` is read from the tape signal as usual (if a tape is
inserted). With `--load`, the program's directory is used by default, so
multi-part games find their parts:

```sh
bk --bk0010 --load software/DIGTIT_1.6.BIN     # loads DIGGER_1.6.BIN itself
```

Saving works the same way: a program that writes a file named `GAME` creates
`DIR/GAME.bin`.

### Real tapes (`.wav`)

```sh
bk --bk0010 --tape recording.wav
```

The recording plays while the BK runs the tape motor (the emulator runs as
fast as it can meanwhile, so a long load takes seconds, not minutes; the
window title says "fast-forward"), and the ROM decodes it exactly as it
would a cassette, so you start the load from the BK (for example with the
monitor's `M` command, see [monitor.md](monitor.md)). The WAV can be 8 or
16 bits, any rate, mono or stereo. Programs in the standard
BK format load; the emulator squares up noisy recordings, but cassettes with
dropouts fail the ROM's checksum, as they would on a real machine. Turbo
loaders that use their own format and speed need their own loader program.

`--record FILE.wav` records everything the BK writes to tape while the motor
runs (again at full speed) and saves it when you quit. Such recordings load
back with `--tape`, into the emulator or (played from a sound card) a real
BK.

### Converting between `.bin` files and recordings

```sh
bk --bin2wav game.bin game.wav [--tape-name NAME]
bk --wav2bin tape.wav DIR
```

`--bin2wav` writes a `.bin` file as a recording in the standard BK tape
format, with the pulse lengths of the BK-0010 monitor's own tape routine,
for a real BK or for `--tape`. The name on the tape is the file name (up to
16 characters) unless `--tape-name` gives another.

`--wav2bin` reads every file on a recording through the BK-0010 monitor ROM
itself, in an emulated BK-0010, so it decodes exactly as a real machine
would, and writes each to `DIR/NAME.bin` (the name without its spaces; a
second copy of the same name, which tapes often carry, becomes
`NAME~2.bin`). It reports each file's name, address and
length, and whether the ROM accepted the checksum; a file that failed is
still written, as the ROM read it. It needs the BK-0010 ROMs.

### Disks

```sh
bk --disk game.dsk                # a BK-0011M
bk --bk0010 --disk game.dsk       # a BK-0010.01 with the controller
```

Disk images are raw 800 KB images (80 tracks, 2 sides, 10 sectors of 512
bytes, as `.img`, `.bkd` or `.dsk`). `--disk` can be given up to four times
for drives 0 to 3. Changes the BK makes are written back to the image file
when you quit, unless the file is read-only.

On the **BK-0011M** the floppy ROM boots at power-on from the first of
drives 0 to 3 whose disk has a boot sector (its first word must be 000240,
a NOP); see [monitor.md](monitor.md#booting-the-bk-0011m-from-a-floppy).

On the **BK-0010.01** the controller block brings its own ROM (version 326,
at 160000, in place of BASIC part 3) and 16 KB of RAM at 120000–157777 in
place of BASIC
([hardware-reference.md §12](hardware-reference.md#12-bk-0010--bk-001001-second-machine-model)).
The machine starts in the monitor (`?`); `S160000` starts the disk ROM,
which boots the disk (ANDOS, for example). It needs `disk_326.rom`.

## Save states

A save state is a snapshot of the whole machine: CPU, all RAM, every device,
and the disks in the drives. In the window, Alt+S saves it and Alt+L loads
it back. The file is `bk0011m.state` or `bk0010.state` in the current
directory unless `--state FILE` names another. `--restore FILE` starts the
emulator from a state.

A state loads only into the same configuration: the same model, the same
ROMs, and the same devices (`--fdd`, `--ay`, `--covox`, `--printer`).
Otherwise the emulator says what differs and leaves the machine alone.
ROMs, tape files and the fast-tape directory aren't part of a state; the
position of an inserted tape is.

## The debugger

The debugger stops the machine and lets you look at it from the terminal you
started `bk` from. Start in it with `--debug`, or press Alt+D in the window
at any time. While it's stopped, the window stays open but the BK doesn't
see keys; type commands in the terminal, and `c` to continue.

Numbers are octal. A trailing dot makes a number decimal (`m 1000 8.`), as in
MACRO-11. Where an address is expected, a register name (`u pc`, `m sp`) or
a symbol (`b KEYIN`) also works, and for `m`, `e` and `f` a physical RAM page
and offset, `pN:OFF` (`m p7:0` shows the BK-0011M's system page whatever is
mapped in).

**Symbols.** When the emulator runs from the repository, it loads the ROMs'
names from `annotations/` (the monitor's or BOS's routines and calls, the
device registers), so listings read `JSR PC, KEYIN`, `MOV @#KBDDAT, R0` and
`EMT 016 ; PUTC`. `--symbols FILE` adds your own: the assembler writes them
when given a fourth argument (`asm prog.mac 1000 prog.bin prog.sym`), one
`NAME = ADDRESS` per line.

| Command | Does |
|---|---|
| `r` | show registers, flags and the next instruction |
| `s [N]` | step N instructions (default 1), into calls and traps |
| `n` | step over: run a JSR, EMT, TRAP, IOT or BPT until it returns |
| `c` | continue until a breakpoint or watchpoint |
| `b [ADDR [COND]]` | set a breakpoint, or list them. COND is `REG=VALUE` or `ADDR=VALUE` (a memory word): stop only when it holds (`b LOOP r1=3`) |
| `be N` / `bt N` | stop before any `EMT N` or `TRAP N` |
| `bc ADDR` / `bc all` / `bc emt N` / `bc trap N` | clear breakpoints |
| `w [ADDR]` | stop when the CPU writes ADDR (RAM or an I/O register), or list them |
| `wc ADDR` / `wc all` | clear watchpoints |
| `u [ADDR] [N]` | disassemble N instructions (default 16) |
| `m [ADDR] [N]` | show N words of memory (default 64), with their characters in KOI-8 |
| `e ADDR WORD...` | store words in RAM |
| `f VALUE [FROM [TO]]` | find a word in memory |
| `sym [NAME [ADDR]]` | count the symbols, show one, or define one |
| `t FILE` / `t off` | write every instruction executed (address, words, disassembly, registers) to FILE |
| `r0`..`r5`, `sp`, `pc`, `psw` `VALUE` | set a register |
| `h` | help |

An empty line repeats `s`, `n`, `u` or `m`; `u` and `m` carry on from where
they stopped. The current instruction is marked `>`, breakpoints `*`.

An example: where does the BK-0010 monitor's keyboard interrupt go?

```
$ bk --bk0010 --debug
debugger: h for help, c to continue
R0 000000  R1 000000  R2 000000  R3 000000  R4 000000  R5 000000  SP 000000  PC 100000
PSW 000340 (P-----)  frame 0
> 100000  000167 000254        JMP    100260
bk> c
```

Press Alt+D after the boot to stop again. Then:

```
bk> m 60 2
000060  101136 000200                                            ^...
bk> u 101136 4
  101136  004467 007176        JSR    R4, 110340
  101142  010546               MOV    R5, -(SP)
  101144  004767 000662        JSR    PC, 102032
  101150  120027 000020        CMPB   R0, #000020
```

The keyboard vector (60) holds the handler's address, 101136, and the PSW
it runs with, 200 (priority bit set).

When standard input ends (for example when it's a pipe), the debugger clears
its breakpoints and lets the machine run on.

## Headless runs and automation

With any of `--frames`, `--type`, `--screenshot`, `--wav` or `--save-state`,
`bk` runs without a window, as fast as it can, and exits. This is how the
other guides' examples were checked.

| Option | Effect |
|---|---|
| `--frames N` | run N frames (1/50 s each; default 100) first |
| `--type TEXT` | then type TEXT: `\n` is ВВОД, `\NNN` any key code in octal up to 377 (`\017` ЛАТ, `\016` РУС, `\003` КТ, `\201` ПОВТ, `\211` ТАБ; codes 200 and up go through the function-key vector 274 with their low 7 bits, as АР2 combinations do, so `\261` is АР2+1), `\p` a one-second pause |
| `--wait N` | frames to run after typing (default 50) |
| `--screenshot FILE` | save the screen as a PNG at the end (`--panel`: with the on-screen keyboard under it, as in the window) |
| `--wav FILE` | save the sound as a WAV file |
| `--save-state FILE` | save the machine state at the end |
| `--debug` | after the rest, run debugger commands from standard input |

`--type` types the way a patient person would: it waits until the BK has
taken each key, and gives each ВВОД two seconds for the command to finish.
At the end `bk` prints the frame count and the CPU state:

```sh
$ bk --frames 200 --type 'PRINT 2+2\n' --screenshot out.png
frames 407  PC 142416  PSW 000350  SP 001754
```

A scripted debugger session:

```sh
printf 'm 60 2\nu 101136 4\n' | bk --bk0010 --frames 200 --debug
```

## Other tools in the repository

- `cargo run -p vm1 --example asm -- SOURCE.mac ORIGIN OUT.bin [SYMBOLS]`
  assembles a MACRO-11 source into a `.bin` file; ORIGIN is octal, and
  SYMBOLS names a file of labels for `--symbols`. See
  [machine-code.md](machine-code.md#the-assembler).
- `cargo run -p vm1 --example disasm -- FILE [ORIGIN] [COUNT]` disassembles
  a raw image (a ROM dump, say) loaded at ORIGIN (octal).
- `cargo run -p romdis` writes commented disassembly listings of every ROM
  and of the programs in `software/` into `listings/`. Comments come from
  `annotations/`. Each listing assembles back into its exact image.
- `cargo test` runs the test suite: CPU, devices, the firmware booting, real
  games, tapes, disks, save states, the debugger, and seeded stress tests
  (`BK_STRESS=N` makes them bigger). Tests that need the ROMs or software
  skip themselves when the files are missing.
- `tools/vm1-timing` measures instruction timing on the MiST FPGA core's
  chip-level model of the CPU, under Verilator.

## What is and isn't emulated

Emulated: the K1801VM1 CPU with its traps and interrupt order and its timer,
instruction timing (per-access memory waits on the BK-0011M), the keyboard,
the screen (both monitors, scrolling, the BK-0011M's palettes and two screen
pages), the BK-0011M's memory paging and 50 Hz interrupt (and, with
`--irq2-50hz`, a BK-0010 one), the speaker, the tape interface in both
directions, the floppy controller on both models at the level of raw tracks
(with the BK-0010's extra RAM), the BK-0010's serial line, and on the
parallel port the AY-3-8910, Covox, a joystick and a printer.

Not emulated: the ИРПС serial registers of the school model (BK-0010Ш),
the mouse on the parallel port, and the analog imperfections of a TV. Instruction
timing is accurate to within a few percent, averaged per memory access.

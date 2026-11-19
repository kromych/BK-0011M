# BK-0011M

An emulator of the Soviet Elektronika BK home computers, the BK-0011M,
BK-0010.01 and BK-0010, written in Rust. The BK is a PDP-11: its K1801VM1
processor runs the instruction set of DEC's minicomputers. The emulator runs
the machines' original ROMs, so BASIC, FOCAL, the monitors and the system
software behave as they did, and BK software from tape and disk runs on it.

It comes with English guides to the computers, written from the original
Soviet manuals: using the machine, BASIC, FOCAL, the monitors, machine code
and system programming ([docs/](docs/README.md)).

<p align="center">
  <img src="docs/images/basic.png" width="32%" alt="BK-0011M BASIC running a short program, with the keyboard panel">
  <img src="docs/images/boingball.png" width="32%" alt="Boing Ball on the BK-0011M">
  <img src="docs/images/pong.png" width="32%" alt="PONG Charged on the BK-0010.01">
</p>

BASIC on the BK-0011M; [Boing Ball](https://github.com/Manwe-SandS/BoingBall)
and [PONG Charged](https://github.com/Manwe-SandS/PONG-Charged), MIT-licensed
homebrew, on the BK-0011M and the BK-0010.01.

## Features

- **Three models**: the BK-0011M (128 KB paged RAM, 4 MHz, BOS and BASIC),
  the BK-0010.01 (monitor and BASIC) and the BK-0010 (monitor, FOCAL and the
  test ROM).
- **The CPU**: every K1801VM1 instruction, its traps and interrupt order and
  its timer, timed bus cycle by bus cycle as a chip-level model of the
  processor and the BK's memory controller does, with the tape and other
  devices clocked to each access.
- **Video**: the 512×256 mono and 256×256 four-color screen, hardware
  scrolling, the BK-0011M's 16 palettes and two screen pages; switchable
  color and mono monitors.
- **Sound**: the built-in speaker, plus an AY-3-8910 and a Covox on the
  parallel port.
- **Joystick** on the parallel port, from the numeric keypad or a gamepad,
  or a **printer** there that writes to a text file; the BK-0010's
  **serial line**, to and from files.
- **Tape**: real recordings (`.wav` or `.mp3`) load through the ROM,
  everything the BK writes can be recorded, "fast tape" reads and writes
  `.bin` files on the host instead, and `.bin` files convert to and from
  recordings.
- **Disks**: the floppy controller on both models at the level of raw
  tracks (with the BK-0010's extra 16 KB of RAM), up to four 800 KB disk
  images; operating systems such as ANDOS boot.
- **The BK keyboard on screen**, lighting up as you type and clickable,
  with indicator lights and a power switch; keys remappable from an INI
  file.
- **Save states**, a **debugger** (breakpoints, conditional or on a system
  call; watchpoints; stepping over calls; disassembly with the ROMs'
  symbols; tracing), and **headless runs** for scripting and tests.
- **Tools**: a MACRO-11 assembler and disassembler, and commented
  disassembly listings of every ROM.

## Getting started

You need Rust (edition 2024). The ROM images are in `roms/`, the same
files the bkbtl emulator's repository publishes in its
[`roms`](https://github.com/nzeemin/bkbtl/tree/master/roms) folder (names
and checksums in [docs/emulator.md](docs/emulator.md#the-rom-files)).
Their copyright status is unclear: take `roms/` and `software/` out
before making this repository public.

```sh
cargo build --release
target/release/bk                      # a BK-0011M, booting into BASIC
target/release/bk --bk0010             # a BK-0010.01
target/release/bk --bk0010 --focal     # a BK-0010 with FOCAL
target/release/bk --bk0010 --load game.bin
target/release/bk --disk andos.dsk     # boot a disk on the BK-0011M
```

Try BASIC:

```
10 FOR I=1 TO 5
20 PRINT I, I*I
30 NEXT I
RUN
```

Keys: letters, digits and punctuation are where they are on a US keyboard;
Enter is ВВОД, left Ctrl АР2, right Ctrl СУ, Home and End ЛАТ and РУС, Esc
СТОП, and F1–F9 the BK's function keys. Alt (Cmd on a Mac) gives the
emulator's commands: O opens a program, tape, disk or state file, S and L
save and load the state, D stops in the debugger, V pastes text, F
fast-forwards, M switches between the color and mono monitors, T lets you
hear the tape load (at its real speed). Under the
screen, a picture of the BK keyboard lights the keys as you type and can be
clicked, beside lights for power, the tape motor and the disk, and a bar
showing how far the recording in the recorder has played. An INI file
remaps the keys (`--keys`, `--write-keys`). `bk --help` lists every option;
[docs/emulator.md](docs/emulator.md) explains them.

## Documentation

| Guide | Contents |
|---|---|
| [docs/emulator.md](docs/emulator.md) | running the emulator, loading software, save states, the debugger |
| [docs/machine.md](docs/machine.md) | the computers, their hardware and their ROMs |
| [docs/monitor.md](docs/monitor.md) | the monitors, the keyboard and the screen |
| [docs/basic.md](docs/basic.md) | BASIC |
| [docs/focal.md](docs/focal.md) | FOCAL |
| [docs/machine-code.md](docs/machine-code.md) | the processor, the assembler, first programs |
| [docs/system-programming.md](docs/system-programming.md) | system calls, interrupts, devices, memory |
| [docs/hardware-reference.md](docs/hardware-reference.md) | register-level detail, as implemented, with sources |
| [docs/manuals/](docs/manuals/README.md) | the original manuals (Russian) |

## Repository layout

| Path | Contents |
|---|---|
| `crates/vm1` | the K1801VM1 CPU: decoder, executor, timing, disassembler, assembler. No I/O |
| `crates/bk` | the machine: memory and paging, devices, video, sound, tape, floppy, save states, debugger. No window, sound or file-system dependencies |
| `crates/bk-desktop` | the `bk` program: window (minifb), sound (cpal), options |
| `crates/romdis` | writes commented disassembly listings of the ROMs and of the programs in `software/`, on disks and tapes too |
| `annotations/` | names and comments for the listings (system calls from the manuals) |
| `tools/mkandos.py` | makes an ANDOS boot disk holding BK programs |
| `tools/vm1-timing/` | measures the CPU's bus cycles on the MiST FPGA core's chip-level VM1 model, under Verilator |
| `docs/` | the guides and the original manuals |
| `roms/` | the ROM images (`mist/`: the MiST core's combined images); [roms/README.md](roms/README.md) says how to start each |
| `software/` | programs, disk images (Saboteur 2, Miami Cobra, ANDOS, CSIDOS) and tape recordings for testing; [software/README.md](software/README.md) says how to start each |

Git-ignored: `listings/` (generated by `cargo run -p romdis`).

## Testing

```sh
cargo test
```

The suite covers the CPU instruction by instruction, every opcode's
disassembly and reassembly, the devices, the firmware booting on each
model, BASIC and tape round trips, real games, a disk boot, save states,
the debugger, and seeded stress tests: random programs hammering every
device, damaged save states, random debugger commands, and games played
with random keys and replayed from a mid-game save (`BK_STRESS=N` makes them
bigger, `BK_STRESS_SEED` changes the seed). Tests that need the ROMs or
software files skip themselves when they're missing.

The test programs came from https://archive.pdp-11.org.ru/BKGAMES/,
https://github.com/kalininskiy/bk-catalog, the bkbtl test bench
(https://github.com/nzeemin/bkbtl-testbench) and MIT-licensed homebrew
(https://github.com/Manwe-SandS/PONG-Charged,
https://github.com/Manwe-SandS/BoingBall).

## Accuracy and limitations

Timing is measured, not guessed: `tools/vm1-timing` runs the MiST core's
reverse-engineered VM1 with a model of the BK's memory controller, and the
emulator's instruction times match it to within a few percent. Real
cassette recordings decode through the ROM with the emulated timing. The
same harness settled the order in which the VM1 takes traps and interrupts.
[docs/hardware-reference.md](docs/hardware-reference.md) records the
evidence and marks the details that still rest on a single source
(**[verify]**).

Not emulated: the mouse, and the exact
alignment of each memory access to the memory controller's time slots
(waits are averaged per access). Turbo-format tapes need their own loader.

## Sources and acknowledgments

- The original manuals, from https://gid.pdp-11.net/docstable.html (in
  [docs/manuals/](docs/manuals/README.md)).
- bkbtl (https://github.com/nzeemin/bkbtl, LGPL-3) for hardware facts and
  for comparing results; no code was taken from it.
- The MiST BK0011M FPGA core (https://github.com/sorgelig/BK0011M) for its
  chip-level model of the VM1, used for timing measurements; it isn't
  included.
- The Wikipedia article on the Electronika BK, for history.

## License

MIT; see [LICENSE](LICENSE). This covers the emulator's source and the
documentation written for it. It doesn't cover the original manuals in
`docs/manuals/`, which are included as public-domain Soviet documentation,
or the ROM images, which aren't included.

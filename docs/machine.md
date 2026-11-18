# The Elektronika BK computers

The BK (БК, *бытовой компьютер*, "household computer") is a family of Soviet
home computers sold under the Elektronika (Электроника) brand from the
mid-1980s. Unlike most home computers of its time it is a PDP-11: its
processor runs the instruction set of DEC's minicomputers (and of the Soviet
Elektronika-60 and DVK). This guide describes the three models the emulator
runs, their hardware as a programmer sees it, and the software in their ROMs.

- [The models](#the-models)
- [Inside the machine](#inside-the-machine)
- [Memory maps](#memory-maps)
- [The screen](#the-screen)
- [Keyboard, sound and storage](#keyboard-sound-and-storage)
- [The standard ROMs](#the-standard-roms)
- [What happens at power-on](#what-happens-at-power-on)
- [Where to go next](#where-to-go-next)

Numbers that are addresses or machine words are octal throughout, as in all
BK documentation.

## The models

| | BK-0010 | BK-0010.01 | BK-0011M (МС 0513) |
|---|---|---|---|
| Processor | K1801VM1, 3 MHz | K1801VM1, 3 MHz | K1801VM1, 4 MHz |
| RAM | 32 KB (16 KB of it the screen) | 32 KB | 128 KB in eight 16 KB pages |
| ROM | 32 KB: monitor, FOCAL, tests | 32 KB: monitor, BASIC | 48 KB: BOS 16, BASIC 24, МСД 8 |
| Language at power-on | FOCAL | BASIC (Vilnius) | BASIC (БК11/10) |
| Screen | 512×256 mono or 256×256 in 4 colors | same | same, 16 palettes, 2 screen pages |
| 50 Hz interrupt | no | no | yes |
| Keyboard | 93 keys | 74 keys | — |
| Tape | 1200 baud, cassette recorder | same | same |
| Disk | optional controller | optional controller | optional controller |

Sources: BK-0010 operating manual M1.492.000 РЭ §3.3; BOS system programmer's
manual 00015-01.32.01 §1.1. The emulator runs the BK-0011M by default,
`--bk0010` for the BK-0010.01 and `--bk0010 --focal` for the BK-0010 (see
[emulator.md](emulator.md)).

**BK-0010.** The first model. Its ROM holds the driver-monitor system and
the FOCAL interpreter, and a test monitor for diagnostics and
machine-code work. The same FOCAL and test ROMs came in the separate МСТД
block (*мониторная система тестирования и диагностики*, "monitor system for
testing and diagnostics"), a 16 KB ROM unit that plugs into the BK-0010.01's
bus connector. With it fitted, the machine starts in FOCAL instead of BASIC
and can run the tests (M1.492.000 РЭ §3.3, §5).

**BK-0010.01.** The same machine with BASIC in place of FOCAL: the BASIC
written in Vilnius, which signs on as `БЕЙСИК (ВИЛЬНЮС,1986.07.24)`. The
operating manual calls it a compiler: it translates a program into an
internal code before running it (M1.492.000 РЭ §5).

**BK-0011M.** The second generation (designation МС 0513): a faster
processor, 128 KB of paged memory, a palette register, two screen pages, a
50 Hz interrupt and a new system ROM, the BOS (*базовая операционная
система*, base operating system). Its BASIC signs on as `БЕЙСИК БК11/10 (C)
ОКБ "ЭКСИТОН"`. It doesn't run most BK-0010 software: BOS's system calls
are numbered differently from the BK-0010 monitor's, and the memory a
BK-0010 program uses at 40000–77777 holds BASIC's code and data on a
BK-0011M (see [system-programming.md](system-programming.md)). Programs
written for the BK-0011M, or for both, run on it.

## Inside the machine

The BK is built from the K1801 chip set (BOS manual §1.1):

- **K1801VM1** (К1801ВМ1), a single-chip 16-bit processor running the PDP-11
  instruction set without the multiply and divide instructions (MUL, DIV,
  ASH, ASHC) and without floating point. It has the eight registers R0–R5,
  SP and PC, a processor status word, vectored interrupts, and a built-in
  programmable timer. [machine-code.md](machine-code.md) describes it.
- **K1801ВП1-037**, the memory and video controller: it shares the RAM
  between the processor and the screen and generates the TV picture.
- **K1801ВП1-014**, the keyboard controller.
- A cassette recorder interface, a 16-bit parallel port (for a joystick,
  printer, or add-ons such as an AY-3-8910 sound chip or a Covox), and on
  the BK-0011M the paging logic.
- The МПИ bus (*межмодульный параллельный интерфейс*, a Q-bus), on a
  connector at the back, for the МСТД block, a floppy disk controller, a
  network adapter or ROM cartridges.

The machine has no expansion of its own beyond that bus. It connects to a
TV (black-and-white or color) and a cassette recorder, and takes 5 V from
a separate power supply box (M1.492.000 РЭ §3.1).

## Memory maps

The processor addresses 64 KB. The top 128 bytes (177600–177777) hold the
device registers; the system ROM is in the upper half; RAM is in the lower
half.

### BK-0010 and BK-0010.01

| Addresses | BK-0010.01 | BK-0010 |
|---|---|---|
| 000000–037777 | RAM (low part: vectors, system variables, stack) | same |
| 040000–077777 | RAM: the screen | same |
| 100000–117777 | monitor ROM | same |
| 120000–157777 | BASIC ROM | 120000–137777 FOCAL ROM |
| 160000–177577 | BASIC ROM (continued) | test ROM |
| 177600–177777 | device registers | same |

A program has 000000–037777 (16 KB) for itself, less what the system uses
at the bottom: the interrupt vectors, the monitor's variables and the stack,
up to address 1000 where programs normally start.

### BK-0011M

| Addresses | Content |
|---|---|
| 000000–037777 | RAM page 0, always |
| 040000–077777 | window 1: any of the eight RAM pages |
| 100000–137777 | window 2: any RAM page, or a ROM page (BASIC, or BOS's second part) |
| 140000–157777 | BOS ROM |
| 160000–177577 | МСД test ROM, or the floppy controller's ROM and registers |
| 177600–177777 | device registers |

A write of a word with bit 11 set to register 177716 chooses what the two
windows show. RAM pages 5 and 6 are the two screen buffers, page 7 holds
BOS's data. [system-programming.md](system-programming.md) explains paging;
[hardware-reference.md](hardware-reference.md) §2 has the bit-level detail.

## The screen

There is no text mode: the screen is a 16 KB bitmap, 256 lines of 64 bytes,
and the ROM draws characters into it. The same bitmap is shown in one of two
ways, which depends only on the monitor attached:

- on a **black-and-white** monitor, 512 × 256 dots, one bit per dot;
- on a **color** monitor, 256 × 256 dots, two bits per dot, in black and
  three colors (blue, green, red on the BK-0010; one of 16 palettes on the
  BK-0011M).

The BK-0010's screen driver shows 25 lines of text: 24 for the user and a
**service line** at the top, which shows the keyboard modes (for example
`ЛАТ`) (M1.492.000 РЭ §3.3). It has a 64-column mode with 8×8-dot
characters and a 32-column mode with 16×8-dot characters; on a color
monitor only the 32-column mode is readable. A scroll register (177664)
moves the picture up or down in hardware, and can show just the top quarter
of the screen, leaving the rest of the screen memory free for programs. On
the BK-0011M a register selects the palette and which of the two screen
pages is shown. The user's side of the screen is in
[monitor.md](monitor.md); the registers are in
[system-programming.md](system-programming.md).

## Keyboard, sound and storage

**Keyboard.** A typewriter keyboard with Latin and Russian letters (ЛАТ and
РУС select which), control keys (АР2, СУ), editing keys and function keys
such as ПОВТ (repeat) and СТОП (stop). Codes are 7-bit KOI-7. The STOP key
isn't a key code: it interrupts the processor. [monitor.md](monitor.md)
describes every key; [emulator.md](emulator.md#the-keyboard) gives the PC
key for each.

**Sound.** A speaker driven by the tape output bits of register 177716:
mainly bit 6, with bit 5 (and on the BK-0011M bit 2) at lower levels
([hardware-reference.md §5](hardware-reference.md#5-tape-and-speaker-177716-write-bit-11--0)).
Music comes from toggling bit 6 in timed loops. Add-ons on the parallel port
give more: the AY-3-8910 three-voice chip, or a Covox 8-bit DAC; the
emulator has both (`--ay`, `--covox`).

**Tape.** The standard storage: an ordinary cassette recorder, controlled by
the ROM at 1200 baud. A file has a name of up to 16 characters, a load
address and a length; the monitor, BASIC and FOCAL all read and write this
format through the same ROM routine. A 60-minute cassette holds about
0.5 MB.

**Disks.** A floppy disk controller (КНГМД) plugs into the bus, with its own
ROM: a disk loader and a driver. Disks hold 800 KB (80 tracks, two sides,
ten 512-byte sectors). Operating systems such as ANDOS and MK-DOS came on
disks. The emulator supports the controller on both models; on the
BK-0010.01 it also brings 16 KB of RAM in place of BASIC.

## The standard ROMs

### BK-0010 and BK-0010.01

| ROM | Address | What it is |
|---|---|---|
| Driver-monitor system (`monit10`) | 100000 | Drivers for the keyboard, screen, tape, serial line and parallel port, called by `EMT` instructions; the start-up code; a small monitor with commands to load and save programs from tape and start them |
| BASIC (`basic10_1`–`3`) | 120000–177577 | BK-0010.01: the Vilnius BASIC system |
| FOCAL (`focal`) | 120000 | BK-0010: the FOCAL interpreter |
| Test monitor (`tests`) | 160000 | BK-0010: diagnostic tests, and a monitor for working in machine code (examine and change memory, run programs) |
| Floppy ROM (`disk_326`) | 160000 | BK-0010.01 with the floppy controller: disk loader and driver, in place of BASIC's third part (the controller's 16 KB of RAM replaces the rest of BASIC) |

The **driver-monitor system** is the heart of the BK-0010. Every program,
including BASIC and FOCAL, prints characters, reads keys, draws on the
screen and uses the tape through it, with `EMT n` calls (the number selects
the function). The monitor part is small; from BASIC the command `MONIT`
enters it, and its `?` prompt accepts commands such as `M` to load a file
from tape. See [monitor.md](monitor.md) for the commands and
[system-programming.md](system-programming.md) for the EMT calls (driver-
monitor system manual 00001-01.32.03).

**BASIC** on the BK-0010.01 is described in [basic.md](basic.md), **FOCAL**
in [focal.md](focal.md).

### BK-0011M

| ROM | Address | What it is |
|---|---|---|
| BOS (`b11m_bos`, `b11m_ext`) | 140000, and window 2 | Base operating system: start-up program, keyboard monitor, drivers for memory pages, keyboard, screen, tape and the rest, called by `EMT` |
| BASIC (`basic11m_0`, `basic11m_1`) | window 2 | BASIC БК11/10 |
| МСД (`b11m_mstd`) | 160000 | Test and diagnostic monitor |
| Floppy ROM (`disk_327`) | 160000 | On the floppy controller: disk loader and driver (in place of МСД) |

**BOS** extends the BK-0010's driver-monitor system for paged memory and
the BK-0011M's screen. Its **keyboard monitor** has commands modeled on the
console of the Elektronika-60 and DVK minicomputers: load programs from
tape, examine and change memory and registers, save memory to tape, boot
from disk (BOS manual §2.2). BOS's system calls are listed in
[system-programming.md](system-programming.md).

**BASIC БК11/10** lives in ROM pages that BOS switches into window 2. It is
an extended version of the BK-0010.01's BASIC ([basic.md](basic.md)).

**МСД** (*мониторная система тестовой диагностики*, test and diagnostic
monitor) tests the machine's units (operator's guide 00015-01.34.01;
[monitor.md](monitor.md)).

The **floppy controller's ROM** (versions 326 for the BK-0010 and 327 for
the BK-0011M) holds the disk loader, which BOS calls at power-on, and a
driver with calls to read and write disk blocks (driver-monitor manual
00008-01.32.01).

## What happens at power-on

**BK-0010 and BK-0010.01.** The processor starts in the monitor ROM at
100000. The monitor sets up its drivers and the screen and calls the
language ROM at 120000: BASIC on the BK-0010.01, FOCAL on the BK-0010. To
get to the monitor, type `MONIT` in BASIC or `P M` in FOCAL. A BK-0010.01
with the floppy controller has RAM at 120000 instead of BASIC, so it stops
at the monitor's `?` prompt; `S160000` there starts the disk ROM, which
boots a disk such as ANDOS.

**BK-0011M.** The processor starts at 140000 in BOS, whose start-up program
(BOS manual §2.1):

1. finds out which ROMs and controllers are fitted;
2. if a key is held down, goes straight to the keyboard monitor;
3. otherwise, with a floppy controller fitted, calls its disk loader, which
   tries to boot an operating system from each drive in turn (the manual
   says drives 0 and 1; the ROM tries 0 to 3, see
   [monitor.md](monitor.md#booting-the-bk-0011m-from-a-floppy));
4. then tries the ROM pages, highest first; a ROM whose first word is NOP
   (000240) is called, which is how BASIC starts;
5. if nothing took over, enters the keyboard monitor.

In the emulator a plain BK-0011M therefore boots into BASIC, and one with a
bootable disk (`--disk FILE`) into the disk's operating system.

## Where to go next

- [emulator.md](emulator.md): running the emulator, loading software
- [monitor.md](monitor.md): the monitors, the keyboard and the screen
- [basic.md](basic.md): programming in BASIC
- [focal.md](focal.md): programming in FOCAL
- [machine-code.md](machine-code.md): the processor and assembly programming
- [system-programming.md](system-programming.md): system calls, interrupts,
  devices and memory
- [hardware-reference.md](hardware-reference.md): register-level detail, as
  the emulator implements it
- [manuals/](manuals/): the original Soviet manuals (in Russian)

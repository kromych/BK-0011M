# Software

Programs, disk images and tape recordings that the tests, the guides and
`romdis` use, and how to start each. Commands are run from the repository's
top folder. A BK-0010 program needs `bk --bk0010`; a BK-0011M one runs
with plain `bk`.

**Inside the emulator**, Alt+O (Cmd+O on a Mac) opens a file: a `.bin`
program is loaded and started (and its folder becomes the fast-tape folder,
so a program that loads more files finds them), a `.wav` or `.mp3` goes in
the tape recorder, and a disk image goes in drive 0. The machine itself is
chosen on the command line: start `bk --bk0010` for BK-0010 programs, and
add `--fdd` to have a floppy controller to put disks in.

## Programs

| File | Machine | Command line | Then |
|---|---|---|---|
| `klad.bin` (Клад) | BK-0010 | `bk --bk0010 --load software/klad.bin` | ВВОД (Enter), then `1` |
| `valley.bin` (King's Valley) | BK-0010 | `bk --bk0010 --load software/valley.bin` | |
| `bally.bin` (Bally) | BK-0010 | `bk --bk0010 --load software/bally.bin` | |
| `Xonix.bin` | BK-0010 | `bk --bk0010 --load software/Xonix.bin` | Space |
| `miner.bin` (Miner) | BK-0010 | `bk --bk0010 --load software/miner.bin` | |
| `DIGTIT_1.6.BIN` (Digger 1.6) | BK-0010 | `bk --bk0010 --load software/DIGTIT_1.6.BIN` | the title: Space for one player, ВВОД to start; it loads `DIGGER_1.6.BIN` itself, from the same folder |
| `DIGGER_1.6.BIN` | BK-0010 | `bk --bk0010 --load software/DIGGER_1.6.BIN` | Digger without its title |
| `PONG.bin` (PONG Charged) | either | `bk --bk0010 --load software/PONG.bin`, or `bk --load software/PONG.bin` | |
| `krakout.bin` (Krakout) | BK-0011M | `bk --load software/krakout.bin` | Space |
| `BoingBall.bin` (Boing Ball) | BK-0011M | `bk --load software/BoingBall.bin` | the demo starts after about 11 seconds |
| `miami-cobra/` (Miami Cobra) | BK-0011M | `bk --disk software/miami-cobra/miami-cobra.dsk` | see [miami-cobra/README.md](miami-cobra/README.md) |

## Disks

| File | Machine | Command line | Then |
|---|---|---|---|
| `ANDOS330.DSK` (ANDOS 3.30 with its utilities, languages and manuals) | BK-0011M or BK-0010.01 | `bk --disk software/ANDOS330.DSK`; on the BK-0010.01 `bk --bk0010 --disk software/ANDOS330.DSK`, then `S160000` and ВВОД at the `?` | ANDOS's title; Space opens its file manager: arrows choose, ВВОД starts a program |
| `sabot2-v44.dsk` (Saboteur 2) | BK-0011M | `bk --disk software/sabot2-v44.dsk` | at the ANDOS title press Space; in the file manager ↓ to `SABOT2`, then ВВОД: the game's menu |
| `CSIDOS_330.DSK`, `CSIDOS_332MIR.DSK` (CSIDOS 3.30 and 3.32, with demos and tools) | BK-0011M | `bk --disk software/CSIDOS_330.DSK` | CSIDOS's title; any key opens its file manager: arrows choose, ВВОД starts a program (checked with `hi-star7.exe`, the HI-STAR debugger) |

Inside the emulator: start `bk --fdd` (or `bk --bk0010 --fdd`), open the
disk image with Alt+O, then switch the machine off and on with the СЕТЬ
light: the BK-0011M boots from it. The BK-0010.01 stops at its `?`; type
`S160000` and ВВОД. On the BK-0011M, `0B` in the keyboard monitor also boots
drive 0. A disk is written back to its file when you quit, unless the file
is read-only.

## Tapes

`tape_komplekt17_start.wav` is the first 50 seconds of set 17 of a series
of BK-0010.01 game tapes, used by the tape tests. To play it:

    bk --bk0010 --tape software/tape_komplekt17_start.wav

then `MONIT` in BASIC, and at the monitor's `?` type `M`, then `R&O-REKL1`
and ВВОД. The tape plays (the emulator runs fast meanwhile) and the set's
loader starts itself, but the recording ends before the program it loads,
so nothing more happens. More recordings, some of which load whole:
[tapes/README.md](tapes/README.md); copy-protected games:
[tapes/protected/README.md](tapes/protected/README.md).

## Listings

`cargo run --release -p romdis` disassembles everything here into
`listings/software/`: the programs, the boot block and programs of each
disk (with a `files.txt` of what is on it), and every file the BK-0010 ROM
reads from each recording.

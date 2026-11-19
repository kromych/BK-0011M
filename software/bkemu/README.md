# From BKEmu

Programs, disk images and tapes that come with gid's BKEmu emulator for
Windows (version 3.14.2510.10760, from <https://gid.pdp-11.net/builds/>),
which `software/` didn't already have. "In `bk`" is what `bk` shows, run
as given; the files are kept as BKEmu ships them, apart from the hard
disk images, which are gzipped.

## Programs

| File | Machine | Command line | In `bk` |
|---|---|---|---|
| `BOLDER.BIN` (Boulder Dash, by MRLC) | BK-0010 | `bk --bk0010 --load software/bkemu/BOLDER.BIN` | the title, "Press any key" |
| `MELMAN.BIN` (a music collection by Black Software) | BK-0010 | `bk --bk0010 --load software/bkemu/MELMAN.BIN` | a menu of 20 tunes beside a picture |
| `covox.bin` (music through a Covox) | BK-0010 | `bk --bk0010 --covox --load software/bkemu/covox.bin` | it plays through the Covox; the file reaches 073160, into screen memory, so the screen shows its data as noise |
| `CPUTEST.BIN` ("K1801BM1 compatibility test v0.4", Digital Freedom, 2012) | BK-0010 | `bk --bk0010 --load software/bkemu/CPUTEST.BIN` | checks 1 to 7 pass (SWAB, `JMP (Rn)+`, opcodes 210–217, `JSR`/`JMP` to a register, PC operands, `Rn,(Rn)+`, the VM1 timer); check 8, the CPU's speed, fails with "We need 11M hardware to run test". Loaded on the BK-0011M after BASIC, it runs but draws on a screen page that isn't shown |
| `MULTICOL.BIN` (208 bytes, a multicolor demo) | either | `bk --bk0010 --load software/bkemu/MULTICOL.BIN` | blocks of color with patterned edges; what it should look like isn't known yet |

## Disks

`bk` writes a disk image back to its file when you quit, unless the file
is read-only: to keep these as they are, use a copy.

| File | Command line | In `bk` |
|---|---|---|
| `WRKANDOS.IMG` (gid's ANDOS 3.1 work disk) | `bk --disk software/bkemu/WRKANDOS.IMG` | ANDOS 3.1's title |
| `WRKANDOS2.IMG` (gid's ANDOS 3.30 work disk) | `bk --disk software/bkemu/WRKANDOS2.IMG` | ANDOS 3.30's title |
| `WRKCSI.IMG` (a CSIDOS 3.30 work disk) | `bk --disk software/bkemu/WRKCSI.IMG` | CSIDOS boots, then reports "Item not found CSIDEMOS" |
| `AltPRO_ReLoad.IMG` (an ANDOS disk of tools and firmware for the AltPro controllers: A16M, SMK, their hard disks) | `bk --disk software/bkemu/AltPRO_ReLoad.IMG` | doesn't boot: BASIC starts. `romdis` lists its programs |
| `BK_SYS.DSK` (an RT-11 system disk, 400 KB) | `bk --disk software/bkemu/BK_SYS.DSK` | doesn't boot: "Ошибка памяти" over and over. `bk` reads 800 KB images |
| `test.hdi.gz`, `test_samara.hdi.gz` (hard disks for the AltPro (SMK) and Samara controllers) | — | `bk` has no hard disk controller |

## Tapes

`circle.tap`, `rnd.tap` and `screen.tap` are in BKEmu's own tape format,
which stores the signal in about a sixtieth of a WAV's size. `bk` doesn't
read it.

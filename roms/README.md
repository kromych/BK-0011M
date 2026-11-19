# ROM images

`bk` reads its ROMs from here when run from the repository (elsewhere,
point `--roms DIR` at this folder). Sizes and checksums are in
[docs/emulator.md](../docs/emulator.md#the-rom-files); how to use each
system is in [docs/monitor.md](../docs/monitor.md).

The machine, and with it the set of ROMs, is chosen when `bk` starts, so
each ROM is reached by starting `bk` with the options below. Inside the
emulator, the СЕТЬ (power) light under the screen switches the machine off
and on again (a cold start with the same ROMs); a ROM can't be swapped in
from the file dialog.

| ROM | Machine | Start it with | What you get |
|---|---|---|---|
| `b11m_bos.rom`, `b11m_ext.rom` | BK-0011M | `bk` | BOS, the system: it starts BASIC. `MONIT` in BASIC, or a key held down at power-on, gives its keyboard monitor (prompt `@`) |
| `basic11m_0.rom`, `basic11m_1.rom` | BK-0011M | `bk` | BASIC БК11/10 (`Ok`), which BOS starts |
| `b11m_mstd.rom` | BK-0011M | `bk`, then `B` or `160000G` in the keyboard monitor | МСД, the test and diagnostics monitor |
| `disk_327.rom` | BK-0011M | `bk --fdd`, or `bk --disk IMAGE` | the floppy controller's ROM (in place of МСД): at power-on it boots a system disk from drive 0 to 3. With none it shows `FDD/HDD BIOS 327 V12` and BOS goes on to BASIC after about 16 seconds |
| `monit10.rom` | BK-0010 | `bk --bk0010`, then `MONIT` in BASIC | the monitor (prompt `?`) |
| `basic10_1.rom`, `basic10_2.rom`, `basic10_3.rom` | BK-0010.01 | `bk --bk0010` | Vilnius BASIC (`Ok`) |
| `focal.rom` | BK-0010 | `bk --bk0010 --focal` | FOCAL (`*`) |
| `tests.rom` | BK-0010 | `bk --bk0010 --focal`, then `P T` in FOCAL (or `T` in the monitor) | МСТД, the test monitor |
| `disk_326.rom` | BK-0010.01 | `bk --bk0010 --fdd`, or `bk --bk0010 --disk IMAGE` | the BK-0010.01's floppy ROM, in place of BASIC (with 16 KB of RAM in its other half): the machine stops at the monitor's `?`, and `S160000` boots the disk in drive 0 |
| `disk_253.rom` | — | — | another version of the floppy ROM, which `bk` doesn't use; kept for reference (`listings/disk_253.mac`) |
| `mist/` | — | — | the MiST FPGA core's ROM images: see [mist/README.md](mist/README.md) |

`cargo run --release -p romdis` writes a commented listing of every ROM
here into `listings/`.

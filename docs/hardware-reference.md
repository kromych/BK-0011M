# BK-0011M hardware reference (for the emulator)

All addresses and values are **octal** (PDP-11 convention) unless marked `0x`.

Sources: Wikipedia "Electronika BK" (overview only), and the source of the
bkbtl emulator (https://github.com/nzeemin/bkbtl, `emulator/emubase/Board.cpp`,
`Processor.cpp`, `Emulator.cpp`), read 2026-11-17. bkbtl is **LGPL-3**: use it
as a factual reference and for differential testing, but don't transliterate
its code into this project unless we accept LGPL terms.

Original Soviet documentation (found 2026-11-17, all on
https://gid.pdp-11.net/docstable.html):

- **BOS manual**: «Базовая операционная система. Руководство системного
  программиста» 00015-01.32.01 (1990), the BK-0011M system programmer's
  manual. Retyped: https://gid.pdp-11.net/books/00015-01.32.01.html. Cited
  below as "BOS §x".
- **BK-0011 driver manual**: «Драйвер-мониторная система» 00008-01.32.01.
  Retyped: https://gid.pdp-11.net/books/00008-01.32.01.html. Documents the
  floppy controller.
- **Operating manual**: «Руководство по эксплуатации» 3.057.017 РЭ, 1990 and
  1991 editions (DjVu scans, no text layer).
- **BASIC**: «Бейсик. Руководство оператора» 00008-01.34.04 and «Язык Бейсик»
  00008-01.35.04.

Where the manuals and bkbtl disagree, the manuals win. Items marked
**[verify]** would have no confirmation from a second source (none is left).

---

## 1. CPU: K1801VM1 (PDP-11 subset)

- Clock: **4 MHz** on BK-0011M (12 MHz / 3). BK-0010 is 3 MHz. Wikipedia's
  infobox "4.6 MHz" contradicts its own body text; every emulator uses 4 MHz.
- Instruction set: PDP-11 base set **without EIS/FIS** (no MUL/DIV/ASH/ASHC
  on the standard VM1). Includes `XOR`, `SOB`, `SXT`, `MARK`, `RTT`,
  `MTPS`/`MFPS`. VM1-specific halt-mode opcodes 000010–000017 (START/STEP).
  `MUL` (070xxx) exists only on the K1801VM1G variant; otherwise it traps.
- Reset: `PC = SEL1 & 0177400`, `PSW = 0340`. On BK-0011M the high byte of
  SEL1 (177716 read) is `0140000`, so execution starts in the BOS ROM.
- PSW bit 8 (0400) = HALT mode; bit 7 (0200) = priority mask for IRQ2/VIRQ;
  bits 10/11 mask ACLO/IRQ1 in VM1.
- VM1 internal registers: 177700 reads 0177740, 177702 reads 0177777,
  177704 reads 0177440 (bkbtl returns fixed values; writes ignored).
- Bus timeout (no device at address) → trap to vector 4. The instruction is
  abandoned at the failing bus cycle; register autoincrements done before
  that cycle remain.
- Word accesses ignore address bit 0 (no odd-address trap).
- `JMP`/`JSR` with a register operand (mode 0) → vector 4, not 10.
- `MTPS` loads PSW bits 0–3 and 5–7; it can't change the T bit.
  `MFPS` to a register sign-extends.
- `RTI`/`RTT` load only the low byte of the PSW and keep the halt-mode bit,
  unless the new PC is ≥ 160000.
- Trap/interrupt entry loads only the low byte of the vector's PSW.
- Byte writes are read-modify-write: `MOVB`, `CLRB`, `MFPS` to memory read the
  destination first. This matters for registers whose reads have side
  effects (177662, 177716). Word `MOV`/`CLR` don't read first.
- `HALT`, `START`, `STEP` and the STOP key enter halt mode, which writes
  SEL1 (setting its bit 2) and saves PC/PSW at 177674/177676. The BK has
  nothing there, so this ends as a bus-error trap to vector 4. BOS §1.3
  confirms HALT sets the SEL1 write flag; that is how BOS tells a stop from
  a bus error. (bkbtl writes SEL1 only for the STOP key.)

### Interrupt priority and vectors (highest first)

| Source | Vector | Masked by |
|---|---|---|
| Double bus timeout | 4 | – |
| Bus timeout (RPLY) | 4 | – |
| Reserved instruction | 10 | – |
| T-bit | 14 | – |
| ACLO (power fail) | 24 | PSW bit 10 |
| HALT line (user mode) | 170 (halt mode) | – |
| IRQ1 = **STOP key** | 4 (writes 0210 to 177716 first, so handlers can tell STOP from a bus hang via bit 2) | PSW bits 10, 11 |
| IRQ2 = **50 Hz vsync** (BK-0011M; on the BK-0010 only with `--irq2-50hz`, see §12) | 100 | PSW bit 7, and bit 14 of 177662-write |
| VM1G timer | 270 | PSW bit 7 |
| VIRQ: keyboard | 60 (normal) / 274 (AR2 combos) | PSW bit 7, 177660 bit 6 |
| VIRQ: serial TX | 64 | PSW bit 7 |
| HALT instruction | 2 (halt mode) | – |

BPT / IOT / EMT / TRAP (vectors 14 / 20 / 30 / 34) are not in this list:
the VM1's microcode runs their trap sequence as part of the instruction, so
they come before everything above, which is then judged against the
handler's PSW. An EMT executed with a key interrupt pending enters the EMT
handler first (whose PSW, 200 in the BK monitor, holds the key off until
RTI); one executed with T set takes no trace trap until the handler's RTI
restores T. Both checked on the MiST core's VM1 model (`tools/vm1-timing`,
`EVNT_AT`). Taking the interrupt first would leave the EMT handler reading
its call number from the wrong return address.

Interrupts are **not** checked after `RTT` when it sets the T bit.

### Timing

An instruction's time is its bus cycles' and the gaps between them. Each
cycle waits for its device's reply, then runs 4 more clocks; the gaps are
fixed by the microcode (e.g. 4 clocks from a register operation's fetch to
the next fetch, 9 for a branch, 14 for `SOB`). Measured on the MiST core's
chip-level VM1 for every instruction and addressing mode
(`tools/vm1-timing`); the emulator's CPU follows that schedule cycle by
cycle and reproduces every bus cycle of the measurements
(`crates/vm1/tests/bus_cycles.rs`). Accurate timing matters for tape I/O
loops, sound, and many games.

The replies:

- **RAM** goes through the 037 memory controller, which shares it with the
  screen: the CPU gets one slot per 8 phases of a 6 MHz clock. The delay
  depends on the strobe's clock: 4, 3, 6, 5 repeating on the BK-0010
  (3 MHz), 16 clocks of 2 to 8 on the BK-0011M (4 MHz), from the MiST
  core's model of the 037. A NOP from RAM takes 12 clocks on the BK-0010
  (bkbtl's tables, measured on real machines, are these RAM times) and
  16 on the BK-0011M, so the BK-0011M is only about 15% faster in real
  time, as the bin2wav-cli note that the BK-0011 reads tapes "about 17%
  faster" says.
- **ROM and I/O registers** reply in 2 clocks (MiST answers at once; the
  ROMs are separate chips on the bus). At 0 or 3 clocks the chip model
  fails to load Ferrari, a protected tape whose loader works at the
  tape's limit; at 2 it loads, and the monitor's tape loop writes 164 µs
  pilot half-periods, against 166 µs on real cassettes (§5).
- **SEL1, SEL2 and the CPU's timer** (177700–177717) answer inside the
  CPU.
- On the BK-0011M writes are held 2 clocks longer before the strobe.

Devices are brought up to the clock of each bus cycle before it happens,
so the tape level, the timer and the floppy controller are read at the
exact clock of the access. `WAIT` and `RESET` take bkbtl's 1140 clocks
(not measured).

---

## 2. Memory map (BK-0011M)

| Address | Content |
|---|---|
| 000000–037777 | RAM, physical page 0, fixed |
| 040000–077777 | **Window 1**: any RAM page, selected by 177716 bits 14–12 |
| 100000–137777 | **Window 2**: RAM page (bits 10–8) **or** ROM (bits 0,1,3,4) |
| 140000–157777 | BOS system ROM (8 KB, `b11m_bos.rom`) |
| 160000–167777 | Floppy controller ROM (4 KB, `disk_327.rom`) if FDC fitted; otherwise the MSTD test ROM (`b11m_mstd.rom`) |
| 170000–177577 | With the FDC: device-register space (BOS §1.2: "170000–177777"); unused addresses → bus error |
| 177600–177777 | I/O registers |

RAM: 128 KB = 8 physical pages × 16 KB.

### Paging register: write to 177716 with bit 11 (04000) set

- Bits 14–12: page for window 1. Bits 10–8: page for window 2.
- The 3-bit field is **not** the physical page number. Field → physical
  page is `{1, 5, 2, 3, 4, 7, 0, 6}` (BOS §1.2.6.2; matches bkbtl). Page 5
  is screen buffer 0, page 6 screen buffer 1, page 7 the system page.
- Only **word** writes reach the paging register (BOS §1.2.6.2). A byte
  write to 177716 goes to the tape latch.
- ROM select for window 2 (any of mask 033 set means ROM, not RAM):
  - bit 0 → BASIC part 0 (16 KB, `basic11m_0.rom`)
  - bit 1 → BASIC part 1 + BOS extension (`basic11m_1.rom` 8 KB + `b11m_ext.rom` 8 KB)
  - bits 3, 4 → cartridge slots (bus error if empty)
- Value after reset: `000002` (bit 1 set: BASIC-1/EXT ROM in window 2).
- ROM pages per BOS §1.2.6.2: page 0 and 100000–117777 of page 1 are BASIC;
  120000–137777 of page 1 is BOS code (`b11m_ext.rom`); page 2 (bit 3) is
  for the local-network driver ROM; page 3 (bit 4) is for external ROMs.
- A write to 177716 **without** bit 11 goes to the tape/speaker latch (§5).

### Video pages

Two 16 KB screen buffers: physical pages **5** and **6** (field values 1 and 7
under the table above). Bit 15 of the 177662 write register selects which one
is displayed.

---

## 3. I/O registers

| Addr | R/W | Function |
|---|---|---|
| 177130 | R/W | FDC command (write) / status (read) |
| 177132 | R/W | FDC data |
| 177560–177566 | R/W | Serial port (DL-11-like); TX ready bit 7 of 177564, vector 64 |
| 177660 | R/W | Keyboard status: bit 7 = key ready (RO); bit 6 = **interrupt mask** (1 = masked). Bus reset (power-on or `RESET`) sets bit 6 and clears bit 7 (BOS §1.2.1) |
| 177662 | R | Keyboard data: 7-bit code; reading clears 177660 bit 7 |
| 177662 | W | BK-0011M only: bits 11–8 **palette** (0–15); bit 14 = disable 50 Hz IRQ; bit 15 = display page select. Reset value 047400 |
| 177664 | R/W | Scroll: bits 7–0 = vertical offset (0330 = first buffer byte on the top line); bit 9 (01000) = full screen. Bit 9 = 0 shows only the top 64 lines of the raster; the rows shown follow the offset as in full-screen mode (the MiST core's video blanks the rest and changes nothing else). The BK-0010 monitor's extended memory mode writes 0230 here, which puts the last quarter of the buffer on those lines, as BOS §1.2.4 describes, and it scrolls in software in that mode. Write mask 01377. Byte writes zero-extend into the whole word (bkbtl). |
| 177706 | R/W | Timer reload value |
| 177710 | R | Timer counter (read-only, counts down) |
| 177712 | R/W | Timer control (see §6) |
| 177714 | R/W | Parallel port: read = input bus (joystick bits 0–7); write = output (printer, Covox DAC; AY-3-8910 address/data, **inverted**). With `--printer` a read returns the printer's ready lines instead, bits 8 and 14 (040400) when ready and 0 while busy; a rising strobe on either bit prints the low byte, and the printer stays busy until the strobe falls (BASIC on the BK-0010.01 uses bit 8, BOS bit 14) |
| 177716 | R | SEL1: high byte 0140000 (start address); bit 7 = 1 ("no arithmetic extender"); bit 6 = 0 while any key held; bit 5 = tape input; bit 2 = "written since last read" flag (cleared on read). Other bits read 0 (BOS §1.2.6.1) |
| 177716 | W | bit 11 = 1 and a word write → paging register (§2); bit 11 = 0 → tape/speaker latch (§5) |

Unknown I/O address → bus timeout (vector 4).

### Low memory used by BOS (BOS §1.4)

| Address | Use |
|---|---|
| 4, 6 | Bus error, HALT, STOP key |
| 10, 12 | Reserved instruction (unused by BOS) |
| 14, 16 | BPT and T-bit (used by the ШАГ key) |
| 20, 22 | IOT (unused) |
| 24, 26 | Power fail (unused) |
| 30, 32 | EMT |
| 34, 36 | TRAP (unused) |
| 40 | Start address for the monitor's "G" command (tape loads) |
| 52 | Low byte: return code of system calls |
| 60, 62 | Keyboard (alphanumeric and control codes) |
| 100, 102 | 50 Hz timer (EVNT) |
| 110, 112 | Keyboard program vector (user termination routine) |
| 114 | Copy of the last paging-register write |
| 116 | Copy of the last tape-register write |
| 150–156, 164, 250, 252 | BOS internal |
| 166 | EMT pseudo-vector: EMTs BOS doesn't handle go here |
| 270, 272 | Parallel port interrupt |
| 274, 276 | Keyboard: AR2 combinations and function keys |

---

## 4. Video

- No text mode; BOS draws characters in software (32×25 / 64×25).
- 256 lines × 64 bytes per line = 16 KB per screen.
- Mono: 512×256, 1 bpp, **LSB first** within each 16-bit word.
- Color: 256×256, 2 bpp, LSB-first pairs; color 0 is always black.
- Scroll: displayed line `y` reads memory line `(y + offset) & 0377`, where
  `offset = scroll >= 0330 ? scroll − 0330 : scroll + 050`.
- Monitor type (mono vs color) is a user setting, not a register.
- 16 palettes × 3 colors (plus black). RGB values used by bkbtl:

| # | 1 | 2 | 3 |
|---|---|---|---|
| 0 | 0000FF | 00FF00 | FF0000 |
| 1 | FFFF00 | FF00FF | FF0000 |
| 2 | 00FFFF | 0000FF | FF00FF |
| 3 | 00FF00 | 00FFFF | FFFF00 |
| 4 | FF00FF | 00FFFF | FFFFFF |
| 5 | FFFFFF | FFFFFF | FFFFFF |
| 6 | C00000 | 8E0000 | FF0000 |
| 7 | C0FF00 | 8EFF00 | FFFF00 |
| 8 | C000FF | 8E00FF | FF00FF |
| 9 | 8EFF00 | 8E00FF | 8E0000 |
| 10 | C0FF00 | C000FF | C00000 |
| 11 | 00FFFF | FFFF00 | FF0000 |
| 12 | FF0000 | 00FF00 | 00FFFF |
| 13 | 00FFFF | FFFF00 | FFFFFF |
| 14 | FFFF00 | 00FF00 | FFFFFF |
| 15 | 00FFFF | 00FF00 | FFFFFF |

---

## 5. Tape and speaker (177716 write, bit 11 = 0)

- Bit 12: **STOP-key lock** (BOS §1.2.6.3), 1 = locked: the emulator
  ignores STOP while it is set, as the MiST core does
  (`key_stop && !key_stop_block`).
- Bit 7: tape motor, **0 = on**. Reset value 0200. (The BK-0010 tape routine
  writes 020 to start the motor and 220 to stop it.)
- Bits 6, 5 and (BK-0011M) 2: data to the tape recorder (BOS §1.2.6.3).
  They also drive the speaker, as a 3-bit DAC weighted 4:2:1 in the MiST
  core; the emulator mixes them the same way. It records bit 6 as the tape
  signal.
- Tape input appears on bit 5 of 177716 read, inverted (signal high → bit
  0); a change also sets SEL1 bit 2.
- The tape format (FSK) is encoded and decoded by the ROMs. The emulator
  plays a recorded signal into bit 5 while the motor runs, and can record
  what the BK writes. Saving a BASIC program with `CSAVE` and loading the
  recording back with `CLOAD` works, all through the ROM.
- Real recordings agree with the emulated timing. Of ten cassette
  recordings sampled from archive.pdp-11.org.ru (TAPES/another_tapes),
  three BK-0010.01 game collections are in the standard format, with pilot
  half-periods of 166 µs against the emulator's 164 µs (within cassette
  speed variation); five use turbo loaders (about 95 µs). Through
  the ROM the emulator decodes their headers exactly, and the same file
  from two recordings decodes identically until the recordings' dropouts,
  where the ROM rejects the checksum. No complete file has loaded from
  these (MP3) recordings; `real_tape_header_decodes_through_the_rom`
  (`crates/bk/tests/sound_tape.rs`) checks a header when the sample is in
  `software/`.

### Tape calls (used by fast tape)

**BK-0010 monitor EMT 36** (the routine is found through the EMT table at
100000+36). R1 points at the block, read from the ROM's code:

| Offset | Contents |
|---|---|
| 0 | operation: 0 stop motor, 1 start motor, 2 write, 3 read, 4 dummy read |
| 1 | result (0 OK; 4 bus error or STOP) |
| 2, 4 | address (0 on reading: the file's own), length |
| 6–25 | file name, 16 characters |
| 26, 30, 32–51 | on reading: the file's address, length, name |

**BK-0011M BOS `.BMB10`** (EMT 115, slot 140242), BK-0010 tape format. R0
points at the area (BOS manual §3.5.2):

| Offset | Contents |
|---|---|
| 0 | operation: 0 write, 1 read, 2 read the header, 3 dummy read |
| 2, 4 | address (0 on reading: the file's own), length |
| 6–25 | file name, 16 characters; an empty name reads the first file |
| 26, 27 | RAM pages for 040000 and 100000 during the transfer (negative: as now) |
| 30, 32, 34–53 | on reading: the file's address, length, name |

Errors set C and put a code in byte 52: 1 file with another name found,
2 checksum error, 3 bad address range, 4 bus error or STOP.

**Fast tape** steps in at the routine's entry when a named file is in the
file store: it copies the data, fills in the result fields and returns to
the caller as the routine would. Anything else (motor control, empty names,
files the store doesn't have) runs the ROM code against the tape signal.

## 6. Programmable timer (inside the VM1)

Base tick = CPU clock / 128 = **31 250 Hz** (32 µs) at 4 MHz.

177712 control bits:

| Bit | Meaning |
|---|---|
| 0 | STOP: counter held at reload value |
| 1 | WRAPAROUND: keep counting past 0, never set EXPIRY |
| 2 | EXPENABLE: set EXPIRY (bit 7) when counter reaches 0 |
| 3 | ONESHOT: clear RUN on expiry |
| 4 | RUN |
| 5 | Prescaler ÷16 |
| 6 | Prescaler ÷4 (both set = ÷64) |
| 7 | EXPIRY flag |

Reads return `0177400 | flags`. A write to 177712 reloads the counter.
After reset: reload = 011000, counter = 0177777. The timer keeps running even
when the CPU is paused (a known source of game bugs).

---

## 7. Keyboard

- Key down: SEL1 bit 6 = 0. If 177660 bit 7 is clear, latch the 7-bit code into
  177662, set bit 7, and raise VIRQ at vector 60 (or 274 for AR2/high codes)
  unless masked by bit 6. If masked, the interrupt is **deferred** and fires
  when software clears bit 6, provided the key hasn't been read.
- Key up: SEL1 bit 6 = 1.
- **STOP** key is not a normal key. It asserts IRQ1 → vector 4.
- RUS/LAT state lives in software (bit 15 of word at 000042 per bkbtl).
- Joystick: maps onto 177714 input bits (up 0, right 1, down 2, left 3,
  buttons 4–7), 1 while held. bkbtl uses this layout, and so does the game
  Digger 1.6: it tests bits 0–3 against its key table ↑ → ↓ ← (032, 031,
  033, 010) and bits 5–6 as fire. (The MiST core passes its own joystick
  format through unchanged: right 0, left 1, down 2, up 3.)

---

## 8. Floppy controller (KNGMD, К1801ВП1-128 family)

Confirmed by the BK-0011 driver manual 00008-01.32.01 §1.3.6.1:

- 177130 write: bits 3–0 drive select, 020 motor on, 040 head select
  (0 = upper, 1 = lower), 0100 step direction, 0200 step, 0400 start read,
  01000 write marker, 02000 precompensation. Bits 11–15 unused.
- 177130 read: 01 track 0, 02 drive ready (unused by the НГМД-6022 drive),
  04 write-protect, 0200 data request, 040000 CRC written (in read mode),
  0100000 index ("sector 0"). Other bits unused.
- 177132: data register, all bits read/write.
- To stop the motor, software writes 0 to 177130 and to the driver's CSRW
  cell (BOS §3.7.3).
- Driver entry points in the disk ROM (BOS §3): 160000 .BABOOT (auto boot),
  160002 .BBOOT (boot from a chosen drive), 160004 .BBLCK (block I/O),
  160006 .BSECT (sector I/O), 160010 .BDINI (driver init), 160012 .BFORM
  (format a track).
- Raw-track model (from bkbtl's behavior; the emulator's own code):
  6250 bytes per track, one word under the head every 64 µs (300 rpm),
  index hole over the first 30 bytes. Each of the 10 sectors: 4E gap, 12 ×
  00, ID mark A1 A1 A1 FE + track, side, sector (1–10), size code 2, CRC;
  22 × 4E, 12 × 00, data mark A1 A1 A1 FB + 512 bytes + CRC. Markers start
  on word boundaries. Words read big-endian (first byte in the high half);
  words written go out low byte first.
- Reading: setting and then clearing the "start read" bit (0400) makes the
  controller hunt for the next marker; from there each new word raises
  "data ready" (0200), and reading 177132 clears it. When the program stops
  reading, the CRC check ends and sets 040000 (always OK: images come from
  files).
- Writing: writing 177132 switches to write mode; the controller
  double-buffers words and asks for the next with 0200. A word written with
  the "write marker" bit (01000) becomes a marker, and the controller
  appends the CRC when the program stops supplying words. Written tracks
  are decoded back into sectors when the head moves away.
- Standard images: 80 tracks × 2 sides × 10 sectors × 512 B = 800 KB
  (`.img`/`.bkd`/`.dsk`), tracks in order track 0 side 0, track 0 side 1, …
  Block n of the ROM driver is byte offset n × 512. Systems: ANDOS, MK-DOS,
  CSI-DOS.
- Booting (BOS manual §3.7.1, checked in the emulator): .BABOOT sets up the
  driver work area at 2000, reads track 0 sector 1 to 1000, and jumps there
  with the drive number in R0 **only if the first word is 000240 (NOP)**;
  otherwise it tries drives 1–3 and returns with C set.
- .BBLCK (`JSR PC,@#160004`, §3.7.3): R0 block, R1 word count (negative =
  write), R2 buffer, R3 work area; C and byte 52 report errors (1 data CRC /
  write-protected, 2 header CRC, 3 no track 0, 4 seek error, 5 sector not
  found, 6 no disk or not turning).

---

## 9. Sound options

1. Built-in speaker: the tape output bits of 177716, weighted 4:2:1 (bits
   6, 5 and, on the BK-0011M, 2; §5). Needs cycle-accurate toggle times.
2. Covox: 8-bit DAC on the low byte of the 177714 output, unsigned.
   Reading 177714 returns the input bus (joystick), not the output, so a
   read-modify-write (`COMB @#177714`) doesn't toggle the DAC.
3. AY-3-8910 (aftermarket) on 177714: register select and data written
   **inverted** (`value ^ 0xFF`). Clock ≈ 1.714 MHz (12 MHz / 7). A word
   write (both byte strobes) is the address latch (BC1 high); a byte write
   to 177714 is a data write. This is how the MiST core wires its YM2149
   (BDIR = port write, BC = high-byte strobe). Tone frequency is
   clock / (16 × period); noise and envelope step at clock / 16.

---

## 10. Frame timing model (bkbtl)

One frame = 20 ms (50 Hz) = 80 000 CPU clocks at 4 MHz. Per frame:

- the CPU runs clock by clock (each instruction occupies N clocks);
- the timer ticks every 128 CPU clocks;
- IRQ2 (vsync, vector 100) fires once at the start of the frame;
- the FDC advances 312.5 times (one raw byte each);
- tape and audio sample at their own rates.

---

## 11. ROM set for BK-0011M

| File | Size | Mapped at |
|---|---|---|
| `b11m_bos.rom` | 8 KB | 140000–157777 |
| `b11m_ext.rom` | 8 KB | window 2 upper half (with bit 1) |
| `basic11m_0.rom` | 16 KB | window 2 (bit 0) |
| `basic11m_1.rom` | 8 KB | window 2 lower half (bit 1) |
| `disk_327.rom` | 4 KB | 160000 (FDC) |
| `b11m_mstd.rom` | 8 KB | 160000 (no FDC) |

These are Soviet-era firmware dumps with unclear copyright status. Keep them
out of the repository and load them from a user-supplied directory.

---

## 12. BK-0010 / BK-0010.01 (second machine model)

Most BK software was written for the BK-0010, whose monitor EMT numbers
mean different things under the BK-0011M's BOS. The emulator therefore also
models the BK-0010 (from bkbtl's BK-0010 configurations):

| Address | BK-0010.01 | BK-0010 |
|---|---|---|
| 000000–037777 | RAM | RAM |
| 040000–077777 | RAM: the screen buffer | same |
| 100000–117777 | Monitor ROM (`monit10.rom`) | same |
| 120000–137777 | BASIC (`basic10_1.rom`) | FOCAL (`focal.rom`) |
| 140000–157777 | BASIC (`basic10_2.rom`) | nothing (bus error) |
| 160000–177577 | BASIC (`basic10_3.rom`, 8064 bytes) | test ROM (`tests.rom`, 8064 bytes) |
| 177600–177777 | I/O | I/O |

Differences from the BK-0011M:
- CPU clock 3 MHz (60 000 clocks per 20 ms frame); the timer still ticks
  every 128 clocks.
- SEL1's high byte is 100000, so the CPU starts in the monitor ROM.
- No paging: every write to 177716 goes to the tape/speaker latch.
- 177662 is read-only (keyboard data); writing it is a bus error.
- No 50 Hz interrupt. With `--irq2-50hz` the emulator raises IRQ2 (vector
  100) once a frame, but only after a program has set vector 100: with it
  wired permanently, the interrupt would come before the ROM set the vector.
  FOCAL's `FCLK()` counts these.
- Fixed colors, the same as BK-0011M palette 0 (black, blue, green, red).
- The tape register has no bit 2 output; the speaker mixes bits 6 and 5.
- Floppy controller (BK-0010.01 only, `--fdd`): its ROM `disk_326.rom`
  replaces `basic10_3.rom` at 160000, and 16 KB of RAM at 120000–157777
  replaces the rest of BASIC; bits 2–3 of the controller's command register
  switch that window to BASIC parts 1–2 or to nothing (see
  [emulator.md](emulator.md#disks)).
- Serial line, in software: writes to 177716 put the transmit data on bit
  4 (1 when idle) and the BK's ready-to-receive signal on bit 5; reads
  give the received data on bit 4 and the other end's ready signal on
  bit 7 (00001 §4.1.5). The monitor's EMT 42 sends a start bit, eight
  data bits (least significant first) and two stop bits, one write per
  bit; EMT 44 waits for the start bit and reads each bit once, timing the
  bits with SOB loops whose count (cell 254) EMT 40 sets. The emulator's
  far end (`--serial-in`, `--serial-out`) keeps step with those writes
  and reads instead of a clock, ignoring writes with the tape motor on
  (bit 7 = 0). Measured in the emulator, the monitor's "9600" setting
  sends at about 6700 baud and "50" at about 43; whether a real BK-0010
  is as slow is unverified **[verify]**.

### `.bin` files

A word with the load address, a word with the length, then the data.
`Machine::load_bin` copies the data through the CPU's current memory map
(every byte must land in RAM); `Machine::start_at` starts it with SP = 1000
and PSW = 0. The default start address is the load address, or 1000 for
files loaded below 1000: those overwrite the vectors and stack so that they
start themselves after a tape load, and their code begins at 1000.

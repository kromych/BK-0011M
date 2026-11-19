# Copy-protected tapes

BK-0010 games whose tapes carry copy protection: loaders of their own,
files that overwrite the monitor's tape settings as they load, parts in
formats the ROM can't read. From Manwe's page "Кассеты с защитой"
(<https://manwe.pdp-11.net/?/games/tapes>): the cassettes were digitized by
members of the zx-pk.ru forum, and the recordings reconstructed by Ivanq
and Manwe so that every wave period is an exact square of a whole number of
samples, at one amplitude. Each WAV's sample rate is tuned to the BK-0010's
CPU so that the game's loader reads at the fastest speed it can, which
leaves no margin for timing errors: these tapes test the emulator's timing
as clean tapes don't.

## Loading

The WAVs are kept gzipped (a hundredth of their size); `bk` reads them as
they are. To load one, as the page says:

    bk --bk0010 --tape software/tapes/protected/Expert.wav.gz

then `MONIT` in BASIC, and at the monitor's `?` type `M`, the name from the
table, and ВВОД. The game's loader takes over from the ROM. Inside the
emulator: start `bk --bk0010`, open the recording with Alt+O (Cmd+O on a
Mac), then `MONIT`, `M` and the name. For a game on several recordings, open
the next one with Alt+O when the game asks for it.

- **The name must be exact**: the one in the table, with its spaces
  (`INDIANA JONES`), not the recording's file name. The monitor loads only a
  file of that name; passing another it prints the other's name and searches
  on. Opening the recording again with Alt+O winds it back to the start.
- **How long**: the time beside МАГНИТОФОН on the keyboard panel, and the
  bar over the keys, show how far the recording has played. The emulator
  fast-forwards while the tape moves; with `--tape-sound` (Alt+T) you hear
  the load at its real speed, from 6 seconds (Перевал's loader) to 5 minutes
  (Robocop).
- **`--no-motor-control`** (Alt+R): nine of these games stop the tape motor
  in the middle of their load, through the BK's relay, and go on reading as
  if the tape were still moving. On a recorder not wired to the relay (most
  were run by hand) it is; on one that is, the load stops. With this switch
  the tape, once the BK has started it, plays to its end, and the recorder
  stays in play: the next recording, opened with Alt+O when the game asks
  for it, plays at once.
- **`--invert-tape`** (Alt+I): the ROM measures which way up a recording is
  and reads either; Indiana Jones' level 1 and 2 loaders assume one, and
  from these recordings need the signal inverted.

| Game | Author, year | Recordings | Name for `M` | In `bk` |
|---|---|---|---|---|
| Bally | BIL corp., 1990 | `Bally` | `BALLY` | "1 GAME BALLY", then stops; with `--no-motor-control` the game |
| Destroyer | AD corp., Laser software, 1992 | `Destroyer` (title), `Destroyer-1-City` … `Destroyer-4-End` | `DESTROYER` | the title |
| Два негра | Лимонов, Мубинов, 1992 | `Dva-Negra` | `ДВА НЕГРА` | the title |
| Exolon One | Soft of Future, 1992 | `Exolon-1` (intro), `Exolon-2` (game) | `EXOLON ONE` | the intro picture, then stops; with `--no-motor-control` the intro, then a key, ВВОД at the menu and a key on the information screen, and `Exolon-2` (opened then) loads the game |
| Exolon.RUN | Skama, 1992 | `Exolon-Run` | `EXOLON.RUN` | the information screen; a key starts the game |
| Expert | DIM corp., 1992 | `Expert` | `EXPERT` | the title, then stops; with `--no-motor-control` the rest loads, and a key gives the menu |
| Ferrari | Bubon production, 1992 | `Ferrari` | `FERRARI` | the title |
| Hobbit | ELM Soft, 1992 | `Hobbit` | `HOBBIT` | stops at "PROTECTION BY TOKEN CORP."; with `--no-motor-control` the menu |
| Indiana Jones | Smash corp., 1991 | `Indiana-Jones` (intro), `Indiana-Jones-1` … `-3` (levels) | `INDIANA JONES` | the title and story; levels 1 and 2 need `--invert-tape` |
| New Revolution | Smash corp., 1992 | `New-Revolution` | `NEW REVOLUTION` | the title (a test checks it) |
| Перевал | BIL corp., 1991 | `Pereval-1` (loader), `-2` (intro), `-3` (game), `-4` (check) | `ПЕРЕВАЛ` | the loader waits for the next recording; the later ones don't load (below) |
| Planet | BIL corp., 1991 | `Planet` | `PLANET` | "3 GAME PLANET"; with `--no-motor-control` the picture, then a crash (below) |
| Return of the Jedi | SSV, 1991 | `Return-Jedi-1` (loader), `-2` (game) | `RETURNJ` | the title; a key, then `Return-Jedi-2`, gives the menu |
| Robocop | Smash corp., 1992 | `Robocop` | `ROBOCOP` | the information screen, then the title |
| Runing | BIL corp., 1991 | `Runing` | `RUNING` | as Planet |
| Second | BIL corp., 1991 | `Second` | `SECOND` | as Planet |
| Soldier | DIM corp., 1991 | `Soldier-1` (game), `-2`, `-3` (levels) | `SOLDIER` | a © on an empty screen: it has loaded and waits for a key, then the title |
| Superman | BIL corp., 1991 | `SuperMan` | `SUPERMAN` | as Planet |
| Terminator | Smash corp., 1992 | `Terminator-1` … `-5` (levels; passwords for 2 to 5: HARLEY, TYSON, CONNOR, ANGELOS) | `TERMINATOR1` | the choice of keyboard or joystick |

The chip-level CPU model of `tools/vm1-timing`, given the same recordings
and memory timing, does the same as `bk` with Ferrari, Expert, Hobbit and
Soldier (the others weren't run on it); Ferrari loads only with the ROM
replying to the CPU as the emulator's does. Each recording's files as the
ROM sees them: `bk --wav2bin FILE DIR`, or the listings `cargo run
--release -p romdis` writes to `listings/software/tapes/protected/`.

## How the protection works

What follows was traced in `bk` (breakpoints, memory dumps, the tape
position) and checked against the recordings' pulses decoded directly.
Addresses are octal. The monitor ROM's tape code is in `listings/monit10.mac`
and docs/system-programming.md (EMT 36).

### What they have in common

**The ROM's own format.** Nearly every part, including the ones the ROM
can't read, uses the monitor's bit code: each bit is a period whose length
says what it is (a short one 0, one twice as long 1), then a short period.
A file is a pilot of equal periods, a long sync period, a header (address,
length, 16-character name), the data and a checksum. The ROM times the
pilot at the start (cell 314 gets 1.5 times its period, the threshold
between a 0 and a 1) and the signal's polarity (cell 300). What the
protections change is what comes around the bits.

**Starting itself.** The first file a game's tape gives the ROM loads
below 001000, over the monitor's stack, filling it with copies of its own
start address. The ROM's read routine stores the last data byte and returns
with `RTS` into the file, before it reads the checksum: the game runs as
soon as it is in, and the ROM never checks it. So these files "fail" the
checksum when `--wav2bin` reads them (it loads them where they can't start
themselves), and several carry 000000 or other bytes in its place.

**Blocks the ROM can't find.** The rest of the tape is blocks with no
pilot the ROM accepts, no header and usually no checksum: a few periods,
a sync and the bytes. The loaders read them by calling into the ROM's
tape code below its header search, or with copies of it: 117260 (wait for
a sync, read bytes), 117336 (read bytes), 116712 (find a pilot and time
it), 117376 (time one period), 116622 (checksum). They reuse the threshold
the ROM measured on the first file, and the recordings run at the fastest
speed their loader still reads, often faster than the ROM itself writes.

**Decoys and traps.**
- Headers whose name or address mislead, and "headers" the ROM makes of
  the first 20 bytes of a raw block (the junk names `--wav2bin` reports).
- Contents scrambled (XOR, complement, swapped or reversed bytes), and
  checked: sums over memory, the name typed at `M`, the timer.
- Vector 4 (bus error, and the СТОП key) pointed at code that wipes memory.
- A store into ROM or a reserved instruction used as a jump.

**The motor.** Several loaders switch the tape motor off as a side
effect and go on reading: the ROM's bell (`EMT 16`) and its terminal reset
(`EMT 14`) write 220 to 177716, and a `BIS` or `XOR` on 177716 reads SEL1,
whose bit 7 is 1, and writes it back. With the recorder wired to the BK's
relay the tape stops; these tapes were made for one that isn't
(`--no-motor-control`).

### BIL corp.: Bally, Planet, Runing, Second, Superman

One loader, 2384 bytes at 000400, differing per game only in names, colors
and encryption constants. It checks that the monitor loaded it at its own address (cell
264), stops the CPU's timer and starts its real code through the trace
trap and a reserved instruction (177643, vector 10), decrypting 31 words
and then the loader proper into screen memory, hidden by the scroll
register. It shows "N GAME name" and rings the bell, which stops the motor.

The picture (`.PIC`) and the game (`.GMC`) are ordinary files in the ROM's
format, with real checksums, read by a copy of the ROM's pilot search and
data reader that runs one instruction of the title animation after every
bit (the trace trap again). It ignores the addresses in their headers
(decoys) and decrypts each in place: the PIC with `NEG`/`SWAB` or
`SWAB`/`COM` before its own code unpacks the picture, the GMC with
`DEC`/`SWAB` or `INC`/`COM`. Then a stub decrypts the game again with a
word count taken from the timer, read after timing the ROM's checksum
routine over 2304 bytes, and a start address from a checksum of the
loader: change the loader, or the machine's speed, and the game decrypts
to garbage. Planet's PIC also has a sync 4 times the threshold, which the
loader allows and the ROM (2 times) rejects.

Copied with the monitor (`M BALLY.GMC`), the parts load but are encrypted
junk at the wrong addresses. Re-recorded at the ROM's own speed with
`--bin2wav`, Bally's three files still load and run.

### DIM corp. (Expert, Soldier) and AD corp. (Destroyer)

The first file is a 64-byte stub at 000700:

```
        MOV #700, R1
        MOV #100, R2
        MOV #770, SP
        JMP @#117336
```

It reads the next 64 bytes over itself through the ROM, with no sync in
between, and each block plants the next stage's address where the ROM's
`RTS` will find it (word 770): a chain of stages, all read by the ROM's
code, with no header or checksum. Expert's stub waits for a sync before
each block (117260) and re-reads copies of itself.

- **Expert** loads the title picture in 4096-byte blocks, then writes 320
  to 177716, which stops the motor, and calls the ROM's pilot search
  (116712) for the second half of the tape: a long leader, more stub
  copies, then the game. The ROM's "header" of 65486 bytes at 012703 is
  the start of one of those copies (`MOV #177716, R3`). Its vector 4 wipes
  memory.
- **Soldier** is one 320-byte bit stream with no syncs (five stages,
  three drawing the ©), then 15166 bytes. The game waits for a key on an
  empty screen. Its levels, `SOLDIER2` and `SOLDIER3`, are ordinary files
  that the game reads with the ROM's tape routine (116076) and decrypts
  with XOR 111111.
- **Destroyer**'s last block ends in screen memory (040000), where the
  chain jumps: a `JMP` hidden in the picture. Its levels `CITY`, `SEA`,
  `BASE` and `END` are ordinary files read with `EMT 36`, but their words
  702–776 hold 100000, so `M CITY` restarts the monitor instead of
  returning.

The stub and the 1:2:4 period ratio are the ROM's; the recordings run at
187–363 µs per short period, Soldier's slowest and the levels fastest.

### Smash corp.: Indiana Jones, New Revolution, Robocop, Terminator

- **New Revolution**: 304 bytes at 000320 overwrite the tape parameter
  block and start themselves; they set vector 4 to a memory wipe, then
  read two raw blocks (the title picture, then 10088 bytes at 001000) with
  a copy of the ROM's reader, flickering the screen on every bit. No
  checksums.
- **Terminator**: each level's 128-byte loader at 000600 reads a 1042-byte
  picture unpacker for the loading screen, then the packed level to
  001000, stops the motor and runs it. The levels differ only in lengths
  and a 3-word routine planted at 000100. Level 2 on its own asks for its
  password.
- **Indiana Jones**: the intro is an ordinary file (with a checksum) that
  starts itself. JONES1 and JONES2 complement and byte-swap their own
  code, rebuild a stage in screen memory through a table based on the EMT
  vector (wrong if it doesn't point into the ROM), read raw blocks with a
  copy of 117260 that forces the polarity (cell 300) to 0, check the
  result with the ROM's checksum routine, and finally reach the next stage
  by a `HALT` through vector 4. Then they read the level's overlay
  (`jones1.ovl`, an ordinary file at double speed). With the other
  polarity every 1 bit times right at the threshold and reads as 0: hence
  `--invert-tape`. JONES3 has no raw blocks and loads either way.
- **Robocop** checks its own bytes and the name found on the tape
  (`EMT 150` on a mismatch), clears memory up to the ROM so that the bus
  error shows the information screen, and builds its block loader over the
  interrupt vectors with a small bytecode interpreter. The rest is in
  numbered blocks with ROM-style headers, each checked, accepted in any
  order and marked in a progress bar on screen: 32 for the title, 32 for
  the game, 100 that the running game reads later.

### Ferrari (Bubon production)

The header claims 32970 bytes at 000000: the file loads over the
monitor's own tape cells. Its bytes 304–307 set the ROM's store step (cell
304) to 100001, so the next store lands in ROM at 100306; the bus error
traps through vector 4, which the file has set to its loader at 000010.
That waits for a sync, reads 248 bytes at 010000 through the ROM's byte
reader and jumps there. The 248 bytes read 229 blocks of 64 bytes into
screen memory, scrolling the screen a line per block: each block has its
own pilot (the threshold re-timed every time), two sync marks and a word
checksum; a bad one or a long sync ends at the monitor. A last block puts
16176 bytes at 000320, which sum their own 8072 words (it must give
102723) and wipe memory on failure or on СТОП.

The tape runs at 250 µs per short period, faster than the ROM's own 330,
and the loader's first stage reads at the ROM's threshold with a margin
of one count: the CPU must run exactly as fast as the real one.

### Hobbit (ELM Soft)

990 bytes at 000600 start themselves, call `EMT 14` (which stops the
motor) and unpack the "CRACKED BY BILL GILBERT / PROTECTION BY TOKEN
CORP." screen. Then, through 117260 with the ROM's threshold, a 4-byte
header, 8704 bytes that unpack a picture, 256 bytes of loader and 249
blocks of 64 bytes (each written as 70, the next block's first bytes
repeated as slack), with an animation between blocks. No checksums;
`CLR @#100000` must bus-error, or execution falls into a memory wipe. The
screen is a wait for the next block with the motor stopped, not a failed
check. Two never-read filler blocks pass while the unpackers run.

### Перевал (BIL corp.)

1170 bytes at 000700 start themselves, set vector 4 to the monitor's
memory clear, and move a loader to 036000. It times 8 pilot periods
through the ROM but keeps their average as the threshold (not 1.5 times
it), reads with its own sync search and the ROM's period timer, writes
SEL1 back to 177716 after every bit (stopping the motor at the first), and
runs one instruction of an animation per bit through the trace trap.
Pereval-2 is the intro picture, Pereval-3 the game (XOR-encrypted, key
036400), Pereval-4 the last kilobyte of the game, read by the game itself.

In these recordings a 1 bit is exactly as long as a pilot period, so with
the threshold at the pilot's length 1 bits time right at it, and most read
as 0: the later recordings don't load in `bk` even with
`--no-motor-control`. With the threshold set between the two lengths they
load to the game ("ЭТАП 1"). Whether the original's pilot was a little
shorter, and the reconstruction rounded it, isn't known.

### Exolon One, Exolon.RUN (Soft of Future, Skama)

- **Exolon One**: the intro is an ordinary file that starts itself, draws
  the picture and copies the ROM's pilot search, period timer and checksum
  into RAM as its loader. That reads one fixed-length block (7224 bytes
  with a ROM-style checksum, retried on a mismatch) while an animation runs
  one instruction per bit through the trace trap. Its first code does
  `BIS #100,@#177716`, which stops the motor; nothing starts it again. The
  junk headers `--wav2bin` reports are the first bytes of the blocks.
  After the menu a second loader reads the game from `Exolon-2`.
- **Exolon.RUN**: 203 bytes at 000600 start themselves, make vector 0 an
  `RTI` and clear vector 4 (СТОП does nothing), then read three raw blocks
  with 117260: the loading picture, the game (over the running code, whose
  `RTS` lands in the new code) and the vectors. The information screen
  waits for a key; then a `HALT` traps to code that clears memory up to
  itself and lands in the game.

### Два негра (Лимонов, Мубинов)

1122 bytes at 000400 whose stack area is a chain of returns through ROM
code: clear memory, draw "Загружается файл ДВА НЕГРА" with a 0–100 %
scale, set up registers through the ROM's EMT exit, and jump to its own
reader. That reads one period per bit (twice the ROM's density) in
512-byte blocks, each ending with the next block's jump address and
registers. On the way it checks the name typed at `M` (its sum must be
071336), that a stretch of memory is clear, and finally the sum of the
whole program; a mismatch restarts the monitor.

### Return of the Jedi (SSV)

RETURNJ is an ordinary file with a good checksum; the ROM's EMT exit
returns into it. It quietly points the EMT vector at its own handler (by
writing through a ROM word that holds 000030) and checks its own code with
an `XOR R3, PC`: altered code jumps astray. The handler draws `RETURNJ.1`,
an ordinary 9472-byte file, as 426 records of screen columns instead of
loading it. After the title, the game reads `RETURNJ.2` with the ROM.

## What `bk` doesn't do yet

- **Planet, Runing, Second, Superman** crash after their picture, even
  with `--no-motor-control`: the game's decryption key comes from the
  CPU's timer after timing the ROM's checksum routine, and `bk`'s count is
  about 2 % off what the game expects (Second's about 5 %). With the count
  corrected all four reach their menus. Which part of the emulated
  timing differs from the machines these tapes were made on isn't known.
- **Перевал** (above): a 1 bit as long as the pilot.
- **Polarity**: Indiana Jones' levels 1 and 2 need `--invert-tape`; the
  other tapes load either way. Whether the BK's input or the recordings
  are the other way up isn't known.

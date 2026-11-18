# System programming: memory, interrupts, devices and system calls

This guide is for machine-code programs that use the system: the
BK-0010's monitor (its "driver-monitor system") and the BK-0011M's BOS
(«базовая операционная система», base operating system). It covers the
memory maps, interrupts, the system calls with working examples, the
device registers for programs that bypass the ROM, the tape and `.bin`
file formats, the floppy driver, and what changes when a BK-0010 program
meets a BK-0011M.

It assumes you can already write, assemble and run a program: see
[machine-code.md](machine-code.md) for the CPU, the instruction set and the
assembler. [hardware-reference.md](hardware-reference.md) has the register
and timing detail behind the short register descriptions here,
[monitor.md](monitor.md) the monitors' own commands, and
[emulator.md](emulator.md) the emulator's options.

All addresses and numbers are **octal** unless written with a trailing dot
(`50.` is decimal), as in MACRO-11 and in the Soviet manuals.

Sources are cited compactly: **(00001 §8.3.5)** is the BK-0010.01
driver-monitor system manual 00001-01.32.03, **(00015 §3.2.3)** the
BK-0011M BOS system programmer's manual 00015-01.32.01, **(00008 §3.6)**
the BK-0011 driver-monitor system manual 00008-01.32.01 (its floppy
chapter matches the BK-0011M's). All three are in
[manuals/](manuals/). "The ROM" means the commented disassembly in
`listings/` (`monit10.mac` is the BK-0010 monitor, `b11m_bos.mac` and
`b11m_ext.mac` the BK-0011M's BOS, `disk_327.mac` the floppy ROM). Every
example below was assembled and run in the emulator, on the model or models
named; the screen output shown is what the emulator displayed. Where the
ROM does something other than its manual says, this guide follows the ROM
and says so.

- [1. Running the examples](#1-running-the-examples)
- [2. Memory](#2-memory)
- [3. Interrupts and traps](#3-interrupts-and-traps)
- [4. BK-0010 monitor calls](#4-bk-0010-monitor-calls)
- [5. BK-0011M BOS calls](#5-bk-0011m-bos-calls)
- [6. Devices at register level](#6-devices-at-register-level)
- [7. Tape files and .bin files](#7-tape-files-and-bin-files)
- [8. The floppy disk driver (BK-0011M)](#8-the-floppy-disk-driver-bk-0011m)
- [9. BK-0010 programs on a BK-0011M](#9-bk-0010-programs-on-a-bk-0011m)

## 1. Running the examples

Every example is a complete source file for the project's assembler. To
run one, assemble it at 1000 and load it:

```sh
target/release/examples/asm hello10.mac 1000 hello10.bin
target/release/bk --bk0010 --load hello10.bin      # a BK-0010.01
target/release/bk --load hello11.bin               # a BK-0011M
```

`target/release/examples/asm` is the project's assembler
([machine-code.md](machine-code.md#the-assembler)), built by
`cargo build --release -p vm1 --examples`. `--load` lets the machine boot
into BASIC, copies the program into memory and starts it at 1000 with
SP = 1000 and PSW = 0
([emulator.md](emulator.md#loading-software)). The examples were run
headless, the same way, with `--frames 200 --screenshot out.png` added,
plus `--mono` for text and `--type` for keyboard input where an example
reads keys.

Most examples end in a loop, `DONE: BR DONE`, so that the result stays on
the screen. A real program would return to whatever started it:

- On the BK-0010 the monitor's `S` (Старт) command starts a program with
  `JSR PC`, so `RTS PC` returns to the monitor (00001 §6.1). A program
  started by `--load` has no return address on its stack.
- On the BK-0011M, `.BEXIT` ([§5.15](#515-ending-a-program)) goes to the
  BOS keyboard monitor from anywhere.

Several examples print numbers. They call `OCTAL`, which prints R1 as six
octal digits; append the version for your machine to the example's source
(the assembler has no include directive). For the BK-0010:

```
; OCTAL: print R1 as six octal digits with EMT 16. Changes R0-R2.
OCTAL:  MOV #6, R2
        CLR R0
        ROL R1              ; the first digit is a single bit
        ROL R0
        BR DIGIT
NEXT:   CLR R0
        ROL R1
        ROL R0
        ROL R1
        ROL R0
        ROL R1
        ROL R0
DIGIT:  ADD #60, R0         ; '0'
        EMT 16
        SOB R2, NEXT
        RTS PC
```

For the BK-0011M, the same with `EMT 63` (.BTOUT) instead of `EMT 16`:

```
; OCTAL: print R1 as six octal digits with EMT 63 (.BTOUT). Changes R0-R2.
OCTAL:  MOV #6, R2
        CLR R0
        ROL R1              ; the first digit is a single bit
        ROL R0
        BR DIGIT
NEXT:   CLR R0
        ROL R1
        ROL R0
        ROL R1
        ROL R0
        ROL R1
        ROL R0
DIGIT:  ADD #60, R0         ; '0'
        EMT 63              ; .BTOUT
        SOB R2, NEXT
        RTS PC
```

The assembler has no `*` operator and no character constants, so the
examples write character codes in octal with the character in a comment,
and work products out by hand.

## 2. Memory

### 2.1 BK-0010 and BK-0010.01 memory map

The CPU addresses 64 KB. On the BK-0010 the map is fixed
(00001 §4; [hardware-reference.md §12](hardware-reference.md#12-bk-0010--bk-001001-second-machine-model)):

| Addresses | BK-0010.01 | BK-0010 with the МСТД block (FOCAL) |
|---|---|---|
| 000000–000777 | stack and system variables | same |
| 001000–037777 | RAM for programs (16 KB less the system area) | same |
| 040000–077777 | screen RAM (16 KB) | same |
| 100000–117777 | monitor ROM: start-up monitor and drivers | same |
| 120000–137777 | BASIC, part 1 | FOCAL |
| 140000–157777 | BASIC, part 2 | nothing (bus error) |
| 160000–177577 | BASIC, part 3 | test ROM |
| 177600–177777 | device registers | same |

The stack pointer starts at 1000 and the stack may grow down to 300;
while the tape driver runs it also uses 300–352 (00001 §4).

**Extended memory mode.** The screen driver can give 12 KB of the screen
back to programs: control code 214 (РП, «расширенная память»), sent with
`EMT 16`, moves the display to 70000–77777, which holds the service line
and four text lines, and frees 40000–67777, so programs get one
continuous area from 1000 to 67777 (00001 §4, §8.3.2). The driver does
this by writing 230 to the scroll register: bit 9 clear shows only the top
64 lines of the picture, and the offset puts the last quarter of the
buffer there ([§6.2](#62-screen-memory-and-the-scroll-register-177664)).
Sending 214 again after clearing the screen goes back to the full screen.

```
; BK-0010: extended memory mode (РП, code 214): the screen shrinks to 70000-77777
        EMT 14
        MOV #214, R0
        EMT 16              ; extended memory on
        EMT 34              ; status word: bit 2 = extended memory
        MOV R0, R1
        JSR PC, OCTAL
        MOV @#177664, R1    ; the scroll register: bit 9 is now 0
        MOV #40, R0
        EMT 16
        JSR PC, OCTAL
        MOV #40000, R1      ; 40000-67777 is now free RAM for programs
        MOV #14000, R2      ; 14000 words
FILL:   MOV #177777, (R1)+  ; fill it: nothing appears on the screen
        SOB R2, FILL
DONE:   BR DONE
```

On a BK-0010.01, with `OCTAL` appended, the top of the screen shows

```
000004 000230
```

the status word (bit 2: extended memory, [§4.5](#45-cursor-service-line-modes-and-the-status-word))
and the scroll register with bit 9 clear. Only the top quarter of the
screen is used, closed by a line the monitor draws at 77700; what the
program writes to 40000–67777 doesn't appear.

### 2.2 BK-0010 low memory

| Address | Use | Source |
|---|---|---|
| 0–36 | trap vectors (4, 10, 14, 20, 24, 30, 34) | 00001 §5.1 |
| 40–56 | the screen driver's mode flags, one byte each; `EMT 34` packs them into the status word | ROM |
| 60, 62 | keyboard interrupt vector | 00001 §5.1 |
| 100, 102 | IRQ2 vector | 00001 §5.1 |
| 254 | serial line: delay constant for the speed set by `EMT 40` | ROM |
| 256 | copy of what the program last wrote to the port 177714 (kept by the program) | 00001 §11 |
| 260 | address of a routine to call for every key; 0 = none | 00001 §7.2 |
| 262 | 0: ВВОД gives code 12; nonzero: it gives 15 | 00001 §7.2 |
| 264, 266 | address and length of the last file loaded from tape | 00001 §6.1, §9.1.4 |
| 274, 276 | keyboard interrupt vector for АР2 combinations | 00001 §5.1 |
| 300–352 | tape driver work area | 00001 §4 |
| 320–371 | free for an `EMT 36` parameter block, if the stack stays clear | 00001 §9.1 |
| 360, 364 | BK-0010Ш serial receiver and transmitter vectors | 00001 §5.1 |

`EMT 14` points vectors 0–36 at the cold start (100000, which restarts
the machine), then sets vector 4 to the monitor and vector 30 to the EMT
dispatcher, and clears 40–277 (ROM, 100140). So after `EMT 14` an
unexpected `TRAP`, `IOT` or reserved instruction restarts the computer.

### 2.3 BK-0011M memory: pages and windows

The BK-0011M has 128 KB of RAM in eight pages of 16 KB, numbered 0–7,
and up to four 16 KB ROM pages, numbered 10–13 (ROM pages 0–3)
(00015 §1.1.1, §1.2.6.2):

| Addresses | Content |
|---|---|
| 000000–037777 | RAM page 0, always |
| 040000–077777 | window 1: any RAM page |
| 100000–137777 | window 2: any RAM page, or a ROM page |
| 140000–157777 | BOS ROM (system calls, drivers) |
| 160000–177577 | the floppy controller's ROM if one is fitted, otherwise the МСД test ROM |
| 177600–177777 | device registers |

A page can be in both windows at once (page 0 even in three places).
Some pages have fixed jobs (00015 §1.1.1, §1.2.6.2):

| Page | Use |
|---|---|
| RAM 0 | always at 0–37777: vectors, system cells, your program and stack |
| RAM 5 | screen buffer 0 |
| RAM 6 | screen buffer 1 |
| RAM 7 | the BOS system page: its variables, stack and the character generator (at offset 10000). BOS won't map it for you; parts of it can be read and written with `.BGETW`/`.BPUTW` |
| ROM 10 (0) | BASIC |
| ROM 11 (1) | 100000–117777: BASIC; 120000–137777: the second half of BOS, with the keyboard monitor |
| ROM 12 (2) | reserved for a network driver ROM |
| ROM 13 (3) | external ROM cartridges |

What is in the windows when your program starts depends on what started
it. The BOS cold start sets window 1 to page 5 and window 2 to page 4
(ROM, 140370); BASIC changes that to page 3 and ROM page 10. A program
loaded by `--load`, or from the monitor after BASIC has run, sees BASIC's
mapping: `.BMEM` returns 004003 (page 3 at 40000, ROM 10 at 100000). So
the screen is **not** at 40000 unless your program puts it there. Ask BOS
with `.BMEM`, and map what you need.

### 2.4 Switching pages safely

BOS's rule is that programs switch pages only through BOS
(00015 §1.1.1): `.BPAGE` maps a page, `.BMEM` and `.BTEST` report, and
`.BJSR`, `.BMOVE`, `.BGETW` and the rest reach into a page without mapping
it ([§5.9](#59-pages)). BOS checks the page and refuses its own page 7.

Writing the register directly is faster and works, if you follow BOS's
bookkeeping. The paging register is the write side of 177716 with bit 11
set (00015 §1.2.6.2):

| Bits | Meaning |
|---|---|
| 14–12 | page code for window 1 (40000) |
| 11 | must be 1 (otherwise the write goes to the tape register) |
| 10–8 | page code for window 2 (100000), when bits 4, 3, 1, 0 are 0 |
| 4, 3, 1, 0 | ROM page 3, 2, 1 or 0 in window 2 instead of RAM |

The page code is not the page number:

| Page | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| Code (3 bits) | 6 | 0 | 2 | 3 | 4 | 1 | 7 | 5 |

The rules:

- **Word writes only.** Only a word write reaches the paging register; a
  byte write to 177716 goes to the tape register (00015 §1.2.6.2).
- **Keep the copy in 114 up to date.** The register can't be read back.
  BOS keeps the last value written in cell 114 and puts it back into the
  register at the end of every BOS call and keyboard interrupt
  (00015 §1.4; ROM, 142536 and 147072). A program that writes 177716
  without updating 114 gets its old mapping back at the next BOS call.
- **Mask interrupts** (`MTPS #200`) between writing 114 and writing the
  register, so that no interrupt restores the wrong value in between.
- **Keep the stack and the running code out of the window you switch.**
  Page 0 (0–37777) is the safe place (00015 §3.2.11).

This program maps screen buffer 0 at 40000 by writing the register,
calls BOS, and draws:

```
; BK-0011M: switch a page by writing 177716 directly, keeping BOS's copy
; at 114 up to date. Maps screen buffer 0 (page 5) at 40000.
        MOV @#114, R4       ; the mapping now (BOS's copy of the register)
        MOV R4, R3
        BIC #70000, R3      ; bits 14-12: the page at 40000
        BIS #14000, R3      ; code 001 = page 5; bit 11 = 1: paging register
        MTPS #200           ; no interrupts between the two writes
        MOV R3, @#114       ; first the copy...
        MOV R3, @#177716    ; ...then the register (a word write)
        MTPS #0
        EMT 62              ; .BCLS: BOS restores the mapping from 114
        MOV #40000, R1      ; top 32 lines of the screen: a pattern
        MOV #2000, R2
FILL:   MOV #031463, (R1)+  ; a striped pattern
        SOB R2, FILL
        MTPS #200           ; and back
        MOV R4, @#114
        MOV R4, @#177716
        MTPS #0
DONE:   BR DONE
```

On a BK-0011M the top 32 lines of the screen fill with stripes: the
mapping survived `.BCLS`. Without the `MOV R3, @#114`, the `.BCLS` call
puts the old page back and the stripes go into page 3, unseen (checked in
the emulator).

### 2.5 BK-0011M low memory

BOS uses these cells (00015 §1.4); the rest of 0–777 is free, apart from
the vectors you use:

| Address | Use |
|---|---|
| 0 | BOS puts `JMP @#.BEXIT` here, so a jump to 0 enters the monitor (ROM, 150176) |
| 4, 6 | vector: bus error, `HALT`, the СТОП key |
| 10, 12 | vector: reserved instruction (not used by BOS) |
| 14, 16 | vector: `BPT` and the T bit (used by the monitor's ШАГ key) |
| 20, 22 | vector: `IOT` (not used) |
| 24, 26 | vector: power failure (not used) |
| 30, 32 | vector: `EMT`, the BOS dispatcher |
| 34, 36 | vector: `TRAP` (not used) |
| 40 | start address for the monitor's `G` command; set by its tape load |
| 52 | low byte: error code of the last system call |
| 60, 62 | vector: keyboard (letters, digits, control codes) |
| 100, 102 | vector: the 50 Hz timer |
| 110, 112 | keyboard routine: address, and the paging register value to run it with (0: as it is) |
| 114 | copy of the last write to the paging register |
| 116 | copy of the last write to the tape register |
| 150–156, 164, 250, 252 | BOS internal |
| 166 | address for EMTs that BOS doesn't handle; 0 = none |
| 270, 272 | vector: parallel port |
| 274, 276 | vector: keyboard (АР2 combinations and function keys) |

At start-up BOS points every vector from 4 to 376 at its own catch-all
handler, except 30 (EMT) and 100 and 274, which point at an `RTI` until
something sets them (ROM, 150170); the keyboard driver then takes 60 and
274. BASIC replaces vector 4 with a handler of its own, which lives in
page 3 at 61176 (seen in the emulator).

## 3. Interrupts and traps

### 3.1 Vectors

| Vector | BK-0010 (00001 §5.1) | BK-0011M (00015 §1.4) |
|---|---|---|
| 4 | bus timeout ("зависание"), СТОП key | bus error, `HALT`, СТОП key |
| 10 | reserved instruction | reserved instruction |
| 14 | T bit (and `BPT`) | `BPT`, T bit |
| 20 | `IOT` | `IOT` |
| 24 | power failure | power failure |
| 30 | `EMT` | `EMT` |
| 34 | `TRAP` | `TRAP` |
| 60 | keyboard | keyboard |
| 100 | IRQ2 | 50 Hz timer |
| 270 | — | parallel port |
| 274 | keyboard, АР2 codes | keyboard, АР2 codes and function keys |
| 360, 364 | serial receiver and transmitter (BK-0010Ш only) | — |

An interrupt or trap pushes the PSW and the PC, then loads the PC from
the vector and the PSW from the next word. The K1801VM1 loads only the
low byte of that PSW word ([hardware-reference.md §1](hardware-reference.md#1-cpu-k1801vm1-pdp-11-subset)).
A handler ends with `RTI`.

### 3.2 Priority

On the K1801VM1 one PSW bit decides: **bit 7** (200) masks the keyboard,
the 50 Hz timer and the other device interrupts. `MTPS #200` (or `#340`)
holds them off, `MTPS #0` lets them in
([hardware-reference.md §1](hardware-reference.md#interrupt-priority-and-vectors-highest-first)).
Each device also has its own enable:

- keyboard: bit 6 of 177660 must be 0 (00001 §4.1.1; 00015 §1.2.1);
- 50 Hz timer (BK-0011M): bit 14 of the 177662 write register must be 0,
  which `.BTIM` sets (00015 §1.2.3, §3.1.4).

The СТОП key is not a device interrupt: it raises the CPU's IRQ1 line,
which bit 7 doesn't mask, and arrives at vector 4. The BK-0011M can lock
the key with bit 12 of the tape register (00015 §1.2.6.3).

`EMT`, `TRAP`, `IOT` and `BPT` are part of the instruction, so on the
VM1 they are taken **before** an interrupt that is already pending; the
interrupt then waits for the handler's PSW. The monitor's and BOS's EMT
handlers run with bit 7 set, so a key pressed during a system call is
handled after the call returns
([hardware-reference.md §1](hardware-reference.md#interrupt-priority-and-vectors-highest-first)).

### 3.3 Writing an interrupt handler

A handler must leave every register as it found it (save what it uses on
the stack), finish quickly, and end with `RTI`. Set the vector with
interrupts masked. This BK-0010 program takes the keyboard over from the
monitor, at vectors 60 and 274:

```
; BK-0010: our own keyboard interrupt handlers (vectors 60 and 274)
        EMT 14
        MTPS #200           ; no interrupts while the vectors change
        MOV #KEY60, @#60
        MOV #200, @#62      ; handler PSW: priority 4, keeps other keys out
        MOV #KEY274, @#274
        MOV #200, @#276
        CLR @#177660        ; bit 6 = 0: keyboard interrupts on
        MTPS #0             ; let interrupts in
MAIN:   TST COUNT           ; wait for a key
        BEQ MAIN
        MTPS #200           ; take the key with interrupts off
        MOV CODE, R1
        CLR COUNT
        MTPS #0
        JSR PC, OCTAL       ; print the code the handler saw
        MOV #40, R0
        EMT 16
        BR MAIN

KEY60:  MOV @#177662, CODE  ; reading the data register clears bit 7 of 177660
        INC COUNT
        RTI
KEY274: MOV @#177662, CODE
        BIS #100000, CODE   ; mark keys that came through vector 274
        INC COUNT
        RTI

CODE:   .WORD 0
COUNT:  .WORD 0
```

Run on a BK-0010.01 with `OCTAL` appended and
`--type 'A1\n\003\014\261\202'` (A, 1, ВВОД, КТ, СБР, АР2+1, ИНД СУ), it
prints

```
000101 000061 000012 000003 000014 100061 100002
```

The last two came through vector 274, with the 7-bit codes 061 and 002.
Reading 177662 clears the ready bit, and the keyboard holds the next key
until then (00001 §4.1.2).

The monitor itself handles keys this way, and offers a simpler hook,
cell 260 ([§4.4](#44-the-keyboard)); BOS has cells 110 and 112
([§5.7](#57-the-keyboard)). On the BK-0011M, taking vector 60 from BOS
also stops BOS's keyboard input until you put the old vector back.

### 3.4 TRAP: system calls of your own

`TRAP n` (104400 + n) traps to vector 34; the handler finds n in the low
byte of the instruction before the return address. Neither the BK-0010
monitor nor BOS uses `TRAP` (00015 §1.4), so it is free for programs:

```
; BK-0010: a TRAP handler. TRAP n prints the character with code n.
        EMT 14
        MOV #TRAPH, @#34    ; vector 34: TRAP
        MOV #340, @#36      ; handler PSW: priority 7
        TRAP 102            ; 'B'
        TRAP 113            ; 'K'
        TRAP 41             ; '!'
DONE:   BR DONE

TRAPH:  MOV R0, -(SP)
        MOV 2(SP), R0       ; the return address: just after the TRAP
        MOVB -2(R0), R0     ; low byte of the TRAP instruction
        EMT 16              ; print it (BK-0011M: EMT 63, .BTOUT)
        MOV (SP)+, R0
        RTI
```

On a BK-0010.01 it prints `BK!`. With `EMT 14` replaced by `EMT 62` and
the `EMT 16` by `EMT 63` it prints the same on a BK-0011M.

### 3.5 The 50 Hz interrupt (BK-0011M)

The BK-0011M interrupts at vector 100 once a frame, 50 times a second,
when the timer is on (00015 §1.2.3). `.BTIM` switches it; set the vector
first (00015 §3.1.4):

```
; BK-0011M: count 50 Hz interrupts (vector 100), switched on with .BTIM
BTIM  = 140016
BTOUT = 140156
BCLS  = 140154

        JSR PC, @BCLS       ; clear the screen
        MOV #TICK, @#100    ; vector 100: the 50 Hz interrupt
        MOV #200, @#102     ; handler PSW
        MOV #1, R0          ; nonzero: timer on
        JSR PC, @BTIM
        MTPS #0             ; make sure interrupts are allowed
        MOV #5, R3          ; five seconds
SECOND: CMP TICKS, #50.     ; wait for 50 ticks
        BLO SECOND
        SUB #50., TICKS
        MOV #52, R0         ; '*'
        JSR PC, @BTOUT
        SOB R3, SECOND
        CLR R0              ; timer off again
        JSR PC, @BTIM
DONE:   BR DONE

TICK:   INC TICKS           ; the interrupt handler: count and return
        RTI
TICKS:  .WORD 0
```

It prints five asterisks, one a second (checked against the emulator's
frame count: one interrupt per 20 ms frame). The BK-0010 has no such
interrupt; use the CPU's timer ([§6.4](#64-the-cpu-timer-177706177712)).
(The emulator's `--irq2-50hz` adds one at the BK-0010's IRQ2 vector, 100,
once a program has set the vector; see
[emulator.md](emulator.md#choosing-a-machine).)

### 3.6 STOP, HALT and bus errors

All of these arrive at vector 4.

- **BK-0010.** СТОП interrupts through vector 4; if the program doesn't
  handle it, the monitor resets the stack to 1000 and the system port,
  reinitializes the keyboard and enters its dialog (00001 §6.1). BASIC
  installs its own vector 4 handler: a `HALT` in a program started from
  BASIC prints «СТОП» (stop) and BASIC's `Ok` (seen in the emulator).
- **BK-0011M.** `HALT` and СТОП also set bit 2 of 177716 (read), which
  is how BOS's vector 4 handler tells them from a bus error. BOS handles
  those two itself: it enters the monitor and prints the PC, and `P`
  continues if the stack was valid (00015 §1.3). A program can take bus
  errors only, with `.BTRPS` (EMT 2): R0 = the address of a handler that
  ends with `RTI`, or 0 to stop; either way vector 4 goes back to BOS's
  handler (00015 §3.1.3; checked: a read of the unused address 177650
  reached the handler). Under BASIC, vector 4 is BASIC's until a program
  calls `.BTRPS`.

## 4. BK-0010 monitor calls

This section is for the BK-0010 and BK-0010.01, which share the monitor
ROM. The calls mean other things on a BK-0011M
([§9](#9-bk-0010-programs-on-a-bk-0011m)).

### 4.1 Calling convention

A program calls the monitor with `EMT n`, n even, from 4 to 110
(00001 §5). The EMT vector (30) points at the dispatcher, which takes the
instruction's low byte as an offset into a table of addresses at 100000,
calls the routine at `100000+n` with `JSR PC`, and returns with `RTI`
(ROM, 100112). Parameters go in registers; R0–R5 come back unchanged
except where the table says otherwise (00001 §5). The dispatcher runs
with PSW 200, so the keyboard waits during a call; `EMT 6` and `EMT 44`
let interrupts in while they wait (ROM).

### 4.2 Reference

From 00001 §5.2 and chapters 7–10, checked against the ROM. A "run" marks
calls exercised by the examples here.

| EMT | Function | Input | Output, registers changed | Run |
|---|---|---|---|---|
| 4 | initialize the keyboard driver: vectors 60 and 274, keyboard interrupts on, keys passed on request, clears 260 and 262 (§7.3.1) | — | R0 changed | |
| 6 | wait for a key (§7.3.2) | — | R0 = code, high byte 0 | run |
| 10 | read a line with echo; ЗАБОЙ deletes (§7.3.3) | R1 = buffer; R2 low byte = maximum length, high byte = a code that ends the line (stored at the end) | R1 = address after the last byte; R2 = maximum minus bytes stored | run |
| 12 | set function key 1–10, typed by АР2 + digit (§7.3.4) | R0 = key; R1 = text, first byte its length; R1 = 0 clears | R0 changed | |
| 14 | initialize all the drivers: vectors, screen cleared and modes reset, port 177714 cleared, system port, serial line 9600 baud; not the stack (§8.2.1) | — | R0–R4 changed | run |
| 16 | print a character or obey a control code (§8.2.2) | R0 = code | — | run |
| 20 | print a string (§8.3.3) | R1 = address; R2 low byte = length, high byte = a code that ends the string (printed) | R1 = after the last byte printed; R2 = length minus bytes printed | run |
| 22 | write in the service (top) line (§8.3.4) | R0 = character, 0 clears the line; R1 = position from 0 | — | run |
| 24 | move the cursor (§8.3.5) | R1 = column X, R2 = line Y, from (0,0) at the top left | — | run |
| 26 | read the cursor position (§8.3.6) | — | R1 = X, R2 = Y | run |
| 30 | draw or erase a point (§8.3.7) | R0 = 1 draw, 0 erase; R1 = X (0–511, or 0–255 in 32-column mode); R2 = Y (0–239) | — | run |
| 32 | draw or erase a line from the last point (§8.3.8) | R0 = 1 or 0; R1, R2 = the end point | — | run |
| 34 | read the display status word (§8.3.9) | — | R0 = status ([§4.5](#45-cursor-service-line-modes-and-the-status-word)) | run |
| 36 | tape operation (§9.1) | R1 = parameter block ([§4.8](#48-tape)) | result in the block | run |
| 40 | set the serial line speed (§10.1.1) | R0 = speed number ([§4.9](#49-the-serial-line)) | | |
| 42 | send a byte (§10.1.2) | R0 = byte | | |
| 44 | receive a byte (§10.1.3) | — | R0 = byte | |
| 46 | send bytes (§10.1.4) | R1 = address, R2 = count | R1 = after the last, R2 = 0 | |
| 50 | receive bytes (§10.1.5) | R1 = address, R2 = count | R1 = after the last | |
| 52–110 | jump to 160000 + (n − 52)×2: entries for a ROM at 160000 (§5.2.5) | | | |

A length of 0 for `EMT 10` or `EMT 20` means no limit in practice: the
ROM counts down from 0 with `SOB`, so 200000 (65536.) bytes. The manual
says 200000 for `EMT 20` and 20000 for `EMT 10`.

### 4.3 Printing

`EMT 16` prints the character in R0, or obeys it if it is a control code;
`EMT 20` prints a string.

```
; BK-0010: print a string (EMT 20) and a character (EMT 16)
        MOV #MSG, R1        ; address of the text
        MOV #MSGLEN, R2     ; low byte: length; high byte: no terminator
        EMT 20              ; print the string
        MOV #41, R0         ; '!'
        EMT 16              ; print one character
DONE:   BR DONE
MSG:    .ASCII /HELLO, BK-0010/
        .BYTE 12
MSGEND:
MSGLEN = MSGEND - MSG
```

On a BK-0010.01, under BASIC's banner and `Ok`:

```
HELLO, BK-0010
!
```

The codes that do something when printed (00001 §8.3.2; the key column
is the key that types the code, where the manual names it; АР2/: means
АР2 held down with the `:` key, and ⇧ is the BK's register key; the PC
keys are in [monitor.md](monitor.md#ар2-combinations-on-the-bk-0010)):

| Code | Key | Action |
|---|---|---|
| 7 | | bell |
| 10, 31, 32, 33 | ←, →, ↑, ↓ | move the cursor |
| 34, 35, 36, 37 | СУ/Э, СУ/Ш, СУ/Ч, СУ/Ь | move the cursor diagonally |
| 12 | ВВОД | new line (to the start of the next line) |
| 14 | СБР | clear the screen, cursor to the top left |
| 22 | СУ/П | cursor to the top left |
| 23 | ВС | move the part of the screen below the cursor up a line |
| 24 | СУ/Т | move the part below the cursor down a line |
| 25 | СУ/У | cursor to the start of the next line |
| 26 | \|<== | move the rest of the line left one place (delete) |
| 27 | \|==> | move the rest of the line right one place (insert) |
| 30 | ЗАБОЙ | delete the last character |
| 231 | =\|=>\| | clear to the end of the line |
| 202 | ИНД СУ | show control codes instead of obeying them (toggle) |
| 204 | БЛОК РЕД | editing lock: editing codes are shown, not obeyed (toggle) |
| 214 | АР2/СБР | extended memory mode ([§2.1](#21-bk-0010-and-bk-001001-memory-map)) |
| 221–224 | АР2/⇧/1–4 (АР2 with `!` `"` `#` `¤`) | color: red, green, blue, black (32-column mode, color TV) |
| 225 | | ГРАФ: text-graphics mode, a cross-hair cursor moved by the arrow codes (toggle) |
| 226, 227 | | ЗАП, СТИР: in ГРАФ mode, draw or erase as the cursor moves |
| 232 | АР2/: | hide or show the cursor (printing is faster without it) |
| 233 | АР2/; | 32 or 64 characters per line (toggle) |
| 234 | АР2/, | inverse characters (toggle) |
| 235 | АР2/- | inverse screen (toggle) |
| 236 | АР2/. | set the service-line indicators to the current modes |
| 237 | АР2// | underlined characters (toggle) |

The codes 16 and 17 (РУС, ЛАТ) select the keyboard's alphabet and never
reach a program.

### 4.4 The keyboard

`EMT 6` waits for a key; `EMT 10` reads a whole line, echoing it and
obeying ЗАБОЙ (backspace).

```
; BK-0010: read a key (EMT 6) and a line (EMT 10). Append OCTAL.
        EMT 14              ; reset the drivers, clear the screen
        MOV #PROMPT, R1
        CLR R2              ; no length limit; stop after the 0 byte
        EMT 20
        EMT 6               ; wait for a key: code in R0
        MOV R0, R1
        JSR PC, OCTAL       ; show its code
        MOV #12, R0         ; new line
        EMT 16
        MOV #ASK, R1
        CLR R2
        EMT 20
        MOV #BUF, R1        ; where the line goes
        MOV #5050, R2       ; high byte 12: stop at ВВОД; low byte: at most 50 bytes
        EMT 10              ; read the line, with echo; ЗАБОЙ deletes
        MOV R1, R3          ; R1 = address after the last byte stored
        SUB #BUF, R3        ; R3 = bytes stored, the 12 included
        MOV #YOU, R1
        CLR R2
        EMT 20
        MOV #BUF, R1
        MOV R3, R2          ; print exactly what was typed
        EMT 20
DONE:   BR DONE

PROMPT: .ASCII /PRESS A KEY: /<0>
ASK:    .ASCII /TYPE A LINE: /<0>
YOU:    .ASCII /YOU TYPED: /<0>
BUF:    . = . + 50          ; the line
        .EVEN
```

With `OCTAL` appended, run on a BK-0010.01 with
`--type 'A\pHELLO WORLX\030D\n'` (`\030` is ЗАБОЙ):

```
PRESS A KEY: 000101
TYPE A LINE: HELLO WORLD
YOU TYPED: HELLO WORLD
```

**Keys as they happen.** If cell 260 holds an address, the monitor's
keyboard interrupt calls it for every key, with the code in R0; the
routine ends with `RTS PC` (00001 §7.2). It runs inside the interrupt, so
the main program keeps going. All registers are saved around the call
(ROM, 101136).

```
; BK-0010: the monitor calls a routine of ours for every key (cell 260)
        EMT 14
        MOV #ONKEY, @#260   ; nonzero: call ONKEY from the keyboard interrupt
MAIN:   MOV #56, R0         ; the main program prints dots
        EMT 16
        MOV #30000, R1
WAIT:   SOB R1, WAIT
        BR MAIN

; Called from the keyboard interrupt with the key code in R0.
ONKEY:  MOV R0, -(SP)
        MOV #133, R0        ; '['
        EMT 16
        MOV (SP)+, R0       ; the key
        EMT 16
        MOV #135, R0        ; ']'
        EMT 16
        RTS PC              ; back into the monitor's interrupt handler
```

With `--type '\pA\pB\pC'` the screen fills with dots, broken by `[A]`,
`[B]` and `[C]` as the keys arrive.

Cell 262 chooses what ВВОД sends: 0 (after `EMT 4`) gives 12, anything
else gives 15 (00001 §7.2). The ten function keys of `EMT 12` type their
text when АР2 and a digit key are pressed together (00001 §7.3.4); this
call wasn't run.

### 4.5 Cursor, service line, modes and the status word

```
; BK-0010: cursor, service line, character modes, status word
        EMT 14              ; reset the drivers and clear the screen
        MOV #233, R0        ; 32/64: switch to 32 characters per line
        EMT 16
        MOV #10, R1         ; column 8
        MOV #3, R2          ; row 3
        EMT 24              ; move the cursor
        MOV #WIDE, R1
        CLR R2
        EMT 20
        MOV #234, R0        ; ИНВ С: inverse characters on
        EMT 16
        MOV #INV, R1
        CLR R2
        EMT 20
        MOV #234, R0        ; and off again
        EMT 16
        MOV #237, R0        ; ПОДЧ: underline on
        EMT 16
        MOV #UND, R1
        CLR R2
        EMT 20
        EMT 26              ; where is the cursor now?
        MOV R1, R3
        MOV R2, R4
        MOV #237, R0        ; underline off
        EMT 16
        MOV #12, R0
        EMT 16
        MOV #XY, R1
        CLR R2
        EMT 20
        MOV R3, R1          ; column
        JSR PC, OCTAL
        MOV #54, R0         ; ','
        EMT 16
        MOV R4, R1          ; row
        JSR PC, OCTAL
        MOV #12, R0
        EMT 16
        EMT 34              ; display status word
        MOV R0, R3
        MOV #ST, R1
        CLR R2
        EMT 20
        MOV R3, R1
        JSR PC, OCTAL
        MOV #102, R0        ; 'B' into the service line
        MOV #2, R1          ; at position 2
        EMT 22
        MOV #113, R0        ; 'K'
        MOV #3, R1
        EMT 22
DONE:   BR DONE

WIDE:   .ASCII /WIDE /<0>
INV:    .ASCII /INVERSE/<0>
UND:    .ASCII / UNDERLINED/<0>
XY:     .ASCII /CURSOR AT /<0>
ST:     .ASCII /STATUS /<0>
        .EVEN
```

With `OCTAL` appended, on a BK-0010.01 (and on the BK-0010 with FOCAL)
the screen switches to 32 columns and shows

```
 BK                                                        (service line)

        WIDE INVERSE UNDERLINED
CURSOR AT 000037,000003
STATUS 000001
```

with `INVERSE` in inverse video and ` UNDERLINED` underlined. Column 37
(31.) is the last of a 32-column line.

The status word from `EMT 34` (00001 §8.3.9, table 2); a 1 means the
mode is on:

| Bit | Mode | Bit | Mode |
|---|---|---|---|
| 0 | 32 characters per line | 8 | text graphics (ГРАФ) |
| 1 | inverse screen | 9 | drawing in ГРАФ |
| 2 | extended memory | 10 | erasing in ГРАФ |
| 3 | Russian letters (РУС) | 11 | 32 characters in the service line |
| 4 | underline | 12 | underline in the service line |
| 5 | inverse characters | 13 | inverse in the service line |
| 6 | show control codes (ИНД СУ) | 14 | cursor hidden |
| 7 | editing lock (БЛОК РЕД) | 15 | unused |

### 4.6 Points and lines

The screen is 512 × 256 dots in black and white. In 32-column mode the
driver uses two bits per dot, 256 × 256 dots in four colors on a color
TV; the color codes 221–224 choose the color for what follows (00001
§8.1, §8.3.2). Graphics coordinates start at the top left of the area
below the service line, with Y up to 239 (00001 §8.3.7).

```
; BK-0010: points and lines in four colors (32-column mode, color TV)
        EMT 14              ; reset the drivers, clear the screen
        MOV #233, R0        ; 32 characters per line: 256 x 240 dots, 2 bits each
        EMT 16
        MOV #232, R0        ; hide the cursor
        EMT 16
        MOV #1, R0          ; R0 = 1: draw (0 would erase)
        MOV #20, R1         ; first point (16,16)
        MOV #20, R2
        EMT 30              ; a point; it is also where the next line starts
        MOV #340, R1        ; line to (224,16)
        EMT 32
        MOV #222, R0        ; color code: green
        EMT 16
        MOV #1, R0
        MOV #340, R1        ; to (224,200)
        MOV #310, R2
        EMT 32
        MOV #223, R0        ; blue
        EMT 16
        MOV #1, R0
        MOV #20, R1         ; to (16,200)
        EMT 32
        MOV #221, R0        ; red
        EMT 16
        MOV #1, R0
        MOV #20, R2         ; back to (16,16)
        EMT 32
        MOV #340, R1        ; and a diagonal to (224,200)
        MOV #310, R2
        EMT 32
DONE:   BR DONE
```

Run on a BK-0010.01 with a color monitor, it draws a rectangle with a
red top edge (red is the color after `EMT 14`), a green right edge, a
blue bottom edge, and a red left edge and diagonal.

### 4.7 Sound

The BK-0010's speaker is driven by bit 6 of the system register 177716,
the same bit that sends data to the tape recorder and makes the key click
(00001 §4.1.5). In the MiST FPGA core, and so in the emulator, bit 5 (the
other tape output) also reaches the speaker, at half the level
([hardware-reference.md §5](hardware-reference.md#5-tape-and-speaker-177716-write-bit-11--0)).
A program makes a tone by switching bit 6 on and off. Keep bit 7 at 1
(tape motor stopped) and bit 4 at 1 (serial line idle), as the monitor
does:

```
; BK-0010: a tone on the built-in speaker (bit 6 of 177716)
        MOV #1000., R1      ; number of periods
LOOP:   MOV #320, @#177716  ; speaker bit 6 on (bit 7 = motor off, bit 4 = line idle)
        MOV #100, R0        ; half a period
D1:     SOB R0, D1
        MOV #220, @#177716  ; speaker bit 6 off
        MOV #100, R0
D2:     SOB R0, D2
        SOB R1, LOOP
DONE:   BR DONE
```

The emulator's sound output (`--wav`) shows a tone of about 1080 Hz for
0.93 s on a BK-0010.01; the same program gives about 1300 Hz on a
BK-0011M, whose CPU is faster. The pitch depends only on the delay
loops, so measure rather than calculate.

### 4.8 Tape

`EMT 36` does every tape operation, described by a parameter block whose
address is in R1 (00001 §9.1). Byte offsets in octal:

| Offset | Contents |
|---|---|
| 0 | command: 0 stop the motor, 1 start the motor, 2 write, 3 read, 4 dummy read (find a file without loading it) |
| 1 | result: 0 done, 1 a file with another name was found, 2 checksum error, 4 stopped by the СТОП key |
| 2 | address: where to write from; where to read to (0: the address in the file) |
| 4 | length in bytes, for writing |
| 6–25 | file name, 16 characters (pad with spaces) |
| 26 | on reading: the address in the file found |
| 30 | its length |
| 32–51 | its name |

On a name mismatch (result 1) the motor keeps running and the program can
print the name found and call `EMT 36` again to keep searching; that is
what the monitor's `M` command does (00001 §9.1.4). After a read the
monitor also leaves the address in 264 and the length in 266. The block
may be anywhere at an even address, or at 320–371, which the monitor
uses for its own (00001 §9.1).

```
; BK-0010: save a block to tape with EMT 36, then read it back elsewhere
        EMT 14
        MOV #BLOCK, R1
        MOVB #2, (R1)       ; command 2: write
        MOV #TEXT, 2(R1)    ; address
        MOV #TEXTLN, 4(R1)  ; length in bytes
        EMT 36
        MOVB 1(R1), R3      ; result byte

        MOVB #3, (R1)       ; command 3: read
        MOV #4000, 2(R1)    ; load at 4000 instead of the file's own address
        EMT 36
        MOVB 1(R1), R4
        MOV #RES, R1
        CLR R2
        EMT 20
        MOV R3, R1
        JSR PC, OCTAL
        MOV #40, R0
        EMT 16
        MOV R4, R1
        JSR PC, OCTAL
        MOV #12, R0
        EMT 16
        MOV #4000, R1       ; print what was read
        MOV @#266, R2       ; the monitor leaves the length in 266
        EMT 20
DONE:   BR DONE

TEXT:   .ASCII /THIS CAME BACK FROM TAPE/<12>
TEXTEND:
TEXTLN = TEXTEND - TEXT
RES:    .ASCII /RESULTS: /<0>
        .EVEN
BLOCK:  .BYTE 0, 0          ; 0: command, 1: result
        .WORD 0, 0          ; 2: address, 4: length
        .ASCII /DEMO            / ; 6: name, 16 characters
        .WORD 0, 0          ; 26: address and length of the file found
        .ASCII /                / ; 32: its name
```

With `OCTAL` appended, on a BK-0010.01:

```
RESULTS: 000000 000000
THIS CAME BACK FROM TAPE
```

Run with `--files DIR`, the emulator's fast tape writes `DIR/DEMO.bin`
and reads it back ([emulator.md](emulator.md)). Split into a writing
and a reading program, the same calls also worked through a real tape
signal (`--record` and then `--tape`), so the ROM's own tape code was
tested too. [§7.1](#71-tape-files) describes what goes onto the tape.

### 4.9 The serial line

`EMT 40`–`EMT 50` drive a serial line through bits 4–7 of 177716
(00001 §10): writes put the data on bit 4 and "ready to receive" on bit
5, reads give the data on bit 4 and the other end's "ready" on bit 7. On
the BK-0010 this needs a modification of the computer (00001 §1). The
emulator plays the other end from files (`--serial-in`, `--serial-out`);
`EMT 46` and `EMT 50` were checked with it at speeds 0 and 5, and the
monitor's `L` command loads a program through it
([monitor.md](monitor.md#commands)). Speed numbers for `EMT 40` (00001
table 3):

| R0 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 10 |
|---|---|---|---|---|---|---|---|---|---|
| Baud | 9600 | 4800 | 2400 | 1200 | 600 | 300 | 150 | 75 | 50 |

`EMT 42` waits for the other side to be ready and masks interrupts while
it sends. `EMT 44` lets interrupts in while it waits, so a keyboard
routine at 260 can run meanwhile; it must put the line back by writing
220 to 177716 (00001 §10.1.3). СТОП breaks off any of them.

### 4.10 The I/O port

The 16-bit parallel port at 177714 has two registers at one address: what
a program writes goes to the output lines, and a read returns the input
lines, never the output (00001 §4.1.4). The monitor doesn't drive the
port; a program writes the address directly, and the manual asks it to
keep a copy of the output in cell 256 (00001 §11).

```
; BK-0010: the parallel port. Output: a sawtooth for a Covox DAC on bits 0-7.
        EMT 14
        MOV @#177714, R1    ; read the input lines
        JSR PC, OCTAL       ; and show them
        MOV #20000., R3     ; about 1.5 seconds of sound
        CLR R2
SAW:    ADD #4, R2          ; next sample
        MOV R2, @#256       ; the monitor's copy of the port output
        MOV R2, @#177714    ; to the port
        SOB R3, SAW
DONE:   BR DONE
```

With `OCTAL` appended and `--covox` (an 8-bit DAC on the low byte of the
output), on a BK-0010.01 the screen shows `000000` (nothing drives the
inputs) and the sound output has a 0.9 s sawtooth at about 340 Hz.

### 4.11 EMT 52–110

The dispatcher sends `EMT 52` to 160000, `EMT 54` to 160004 and so on to
`EMT 110` at 160074, for a ROM at 160000 to provide (00001 §5.2.5; ROM
table at 100052). On the BK-0010.01 that space holds part 3 of BASIC, and
on the BK-0010 with the МСТД block the test ROM; neither puts entry
points there, so don't use these numbers.

## 5. BK-0011M BOS calls

### 5.1 Calling convention

BOS has a table of routine addresses in ROM from 140010, one word per
call (00015 §2.3; the ROM's table matches the manual's addresses). There
are two ways to call:

- **`JSR PC, @slot`**, e.g. `JSR PC, @140156` for `.BTOUT`. The `@`
  without `#` is the relative-deferred mode (77): the CPU fetches the word
  stored at 140156 and calls that address. `JSR PC, @#140156` would jump
  *to* 140156 and execute the table entry as an instruction.
- **`EMT n`**, where slot = 140010 + 2n, e.g. `EMT 63` for `.BTOUT`. The
  BOS dispatcher at vector 30 looks the slot up (00015 §2.4; ROM, 152112).
  EMT code is shorter and position-independent, and slower. Its numbers
  are BOS's own: `EMT 16` is not "print a character" as on the BK-0010.

Parameters go in R0, and for some calls R1; a few use a parameter block
whose address is in R0. All registers come back unchanged except the
documented results (00015 §2.3). Errors set the C bit and leave a code in
the low byte of cell 52 (00015 §3.2.3). The ROM wraps most calls in a
routine (142356) that masks interrupts and switches to a stack in the
system page, so your stack only holds the return address.

Special cases:

- **`.BJSR` and `.BJMP`** take their argument on the stack; call them with
  `JSR` only (00015 §3.2.12). `.BJSR` has no EMT number.
- **Reserved slots** return at once with C set (ROM, 150164). Slot
  140220 (`EMT 104`) holds an undocumented routine (the manual's table
  calls it «секретный вызов», a secret call).
- **EMT numbers above 133** go to the address in cell 166, if it isn't 0;
  otherwise BOS prints `?BK-ILL EMT` and enters the monitor
  ([§5.13](#513-emt-numbers-of-your-own)).

The manual's macro names (`.BTOUT` and so on) come from a macro library
for the ОС БК-11 assembler (00015 §3); the project's assembler has no
macros, so the examples write the calls out.

### 5.2 Reference

From 00015 §2.3 and §3. "Run" marks calls exercised in the emulator,
with the examples here or with small test programs; ‡ marks rows where the
ROM does something other than the manual says ([§5.3](#53-where-the-rom-and-the-manual-disagree)).
Page numbers: 0–7 RAM, 10–13 ROM pages 0–3.

**General**

| Call | Slot | EMT | Function, input | Output | Run |
|---|---|---|---|---|---|
| .BINIT | 140010 | 0 | initialize all drivers: keyboard, screen (32 columns, palette 15, white on black, both buffers cleared, buffer 0 shown), tape motor off, printer, floppy motor off (§3.1.1) | | |
| .BEXIT | 140012 | 1 | enter the keyboard monitor; `P` returns (§3.1.2) | | run |
| .BTRPS | 140014 | 2 | R0 = handler for bus errors at vector 4 (ends with `RTI`); 0 = none. `HALT` and СТОП still go to the monitor (§3.1.3) | | run |
| .BTIM | 140016 | 3 | R0 = 0: 50 Hz timer off; other: on (set vector 100 first). R0 with bit 15 set: ask (§3.1.4) | ‡ R0 unchanged | run |
| .BEMT | 140020 | 4 | if vector 30 isn't BOS's dispatcher, move it to cell 166 and put the dispatcher back (§3.1.5) | | |

**Memory (RAM/ROM driver)**

| Call | Slot | EMT | Function, input | Output | Run |
|---|---|---|---|---|---|
| .BMEM | 140030 | 10 | which pages are mapped (§3.2.1) | R0 low byte = page at 40000, high byte = page at 100000 | run |
| .BTEST | 140032 | 11 | R0 = page (§3.2.2) | R0: bit 0 at 40000, bit 1 at 100000, bit 2 used by BOS (or ROM page absent), bit 3 a working page | run |
| .BPAGE | 140034 | 12 | R0 low byte = page; high byte 0: map at 40000, other: at 100000 (§3.2.3) | C and 52: 1 no such page, 2 ROM at 40000, 3 BOS's page | run |
| .BWORK | 140036 | 13 | R0 = page: bit 7 = 0 sets the read/write working page, 1 the page for .BJSR/.BJMP; bit 15 set: ask (§3.2.4) | R0 (ask): low byte the read/write page, high byte the call page | run |
| .BGETW | 140040 | 14 | ‡ R1 = offset in the working page (top two bits ignored) (§3.2.5) | R0 = word; C on error | run |
| .BGETB | 140042 | 15 | ‡ R1 = offset (§3.2.6) | R0 = byte, high byte 0 | run |
| .BPUTW | 140044 | 16 | R1 = offset, ‡ 0–37776; R0 = word (§3.2.7) | C and 52 = 1 on error: ROM, no such page, offset beyond the page, or page 7 below 10000 | run |
| .BPUTB | 140046 | 17 | R1 = offset; R0 = byte (§3.2.8) | as .BPUTW | run |
| .BMOVE | 140050 | 20 | R0 = parameter block, below (§3.2.9) | C and 52: 1 bad start, 2 bad end, 3 bus error; R0 = words not moved | run |
| .BMOVB | 140052 | 21 | the same for bytes (§3.2.10) | | |
| .BJSR | 140054 | — | push the address, then `JSR PC, @140054`: call it in the call page, mapped at 40000 or 100000 according to the address; the mapping is restored after (§3.2.11) | the subroutine's registers | run |
| .BJMP | 140056 | 23 | the same without return (§3.2.12) | | |

`.BMOVE` block: `.WORD MEMADR` (address in the current mapping, above
400, the move ending below 160000), `.WORD HIDADR` (address in the hidden
pages), `.BYTE LOW, HIGH` (pages to map at 40000 and 100000 for the move;
both must be valid pages), `.WORD COUNT` (positive: hidden pages to
memory; negative: memory to the hidden pages).

**Keyboard**

| Call | Slot | EMT | Function, input | Output | Run |
|---|---|---|---|---|---|
| .BKINI | 140070 | 30 | initialize the keyboard driver (§3.3.1) | | |
| .BKSET | 140072 | 31 | R0 = mode, below (§3.3.2) | | |
| .BKGET | 140074 | 32 | read the mode (§3.3.3) | R0 | run |
| .BTTIN | 140076 | 33 | wait for a character (§3.3.4) | R0 = code, high byte 0 | run |
| .BINKEY | 140100 | 34 | take a character if there is one (§3.3.5) | R0 = code, C = 0; or R0 = 0, C = 1 | run |
| .BGLIN | 140102 | 35 | read a line with editing, ended by ВВОД (not stored); R0 = buffer, R1 = maximum length (0: 128.) (§3.3.6) | the line ends with a 0 byte; R0 = address after it; ‡ R1 = length + 1 | run |
| .BEDIT | 140104 | 36 | print the 0-terminated line at R0 and let the user edit it in place, R1 = maximum (§3.3.7) | | |
| .BKRES | 140106 | 37 | empty the keyboard buffer (§3.3.8) | | |
| .BSFUN | 140110 | 40 | R0 = function key 1–10 (АР2 + digit; 10 is the 0 key), 0 = clear all; R1 = its text, 0-terminated, up to 64. characters (§3.3.9) | | run |
| .BGFUN | 140112 | 41 | R0 = key; R1 = where to copy its text (§3.3.10) | | run |

`.BKSET` mode: bit 0 = 0 KOI-8, 1 KOI-7; bit 1 with KOI-7: КОИ-7Н2; bit 6
= 1 no key click; bit 7 = 1 recode the function keys as escape sequences
(00015 appendix 1). After `.BINIT` the mode is 0, KOI-8 (`.BKGET` returned
0 under BASIC).

**Screen**

| Call | Slot | EMT | Function, input | Output | Run |
|---|---|---|---|---|---|
| .BTINI | 140130 | 50 | initialize the screen driver (§3.4.1) | | |
| .BTSET | 140132 | 51 | R0 = mode word ([§5.5](#55-screen-modes)) (§3.4.2) | | run |
| .BTGET | 140134 | 52 | read the mode word (§3.4.3) | R0 | run |
| .BSCOL | 140136 | 53 | text colors: ‡ R0 low byte = background, high byte = characters; 200 or more keeps that one (§3.4.4) | | run |
| .BGCOL | 140140 | 54 | read the text colors (§3.4.5) | R0, as for .BSCOL | run |
| .BPAL | 140142 | 55 | R0 = palette 0–15.; 200: ask (§3.4.6) | R0 = palette (ask) | run |
| .BBUF | 140144 | 56 | ‡ R0 low byte = buffer to write to, high byte = buffer to show (0 or 1) (§3.4.7) | | run |
| .BSPOS | 140146 | 57 | R0 = line × 400 + column (§3.4.8) | | run |
| .BGPOS | 140150 | 60 | read the cursor position (§3.4.9) | R0 = line × 400 + column | run |
| .BSCRL | 140152 | 61 | R0 = TV lines to scroll, positive up, negative down (§3.4.10) | | |
| .BCLS | 140154 | 62 | clear the screen to the background color, cursor to the top left (§3.4.11) | | run |
| .BTOUT | 140156 | 63 | R0 = character or control code ([§5.4](#54-text-output)) (§3.4.12) | | run |
| .BPRIN | 140160 | 64 | R0 = string ended by 0 (then a new line) or 200 (no new line) (§3.4.13) | R0 = address after the end byte | run |
| .BSTR | 140162 | 65 | R0 = string ended by 0, no new line (§3.4.14) | R0 = address after the 0 | run |
| .BSGRF | 140164 | 66 | R0 = X, R1 = Y: set the current graphics point (§3.4.15) | | run |
| .BGGRF | 140166 | 67 | read it (§3.4.16) | R0 = X, R1 = Y | run |
| .BSCGR | 140170 | 70 | R0 = graphics color 0–3 (§3.4.17) | | run |
| .BGCGR | 140172 | 71 | read it (§3.4.18) | R0 | run |
| .BSMOD | 140174 | 72 | R0 = 0 draw in the color, 1 invert what is there, 200 ask (§3.4.19) | R0 (ask) | run |
| .BSPNT | 140176 | 73 | R0 = X, R1 = Y: draw a point; it becomes the current point (§3.4.20) | | run |
| .BGPNT | 140200 | 74 | R0 = X, R1 = Y (§3.4.21) | R0 = its color | run |
| .BVECT | 140202 | 75 | R0 = X, R1 = Y: line from the current point (§3.4.22) | | run |
| .BRECT | 140204 | 76 | R0 = X, R1 = Y: filled rectangle from the current point (§3.4.23) | | run |
| .BCIRC | 140206 | 77 | R0 = parameter block, below (§3.4.24) | | run |
| .BFILL | 140210 | 100 | R0 = boundary color: fill from the current point (§3.4.25) | | run |
| .BSOUN | 140212 | 101 | R0 = number of periods, R1 = length of a half period ([§5.11](#511-sound)) (§3.4.26) | | run |
| .BSWIN | 140214 | 102 | R0 = block: `.WORD MINX, MAXX, MINY, MAXY`: graphics window (§3.4.27) | | run |
| .BGWIN | 140216 | 103 | R0 = where to put the window's four words (§3.4.28) | | run |
| — | 140220 | 104 | undocumented | | |
| .BSTYP | 140222 | 105 | R0 = line pattern, one bit per two dots; 177777 solid (§3.4.29) | | |
| .BGTYP | 140224 | 106 | read the pattern (§3.4.30) | R0 | run |
| .BGOSET | 140226 | 107 | R0 = graphics-text direction: 0 normal, 1 upside down, 2 rotated left, 3 rotated right; ‡ 200 doesn't ask (§3.4.31) | | |
| .BGOUT | 140230 | 110 | R0 = character, drawn at the current point in the graphics color (§3.4.32) | | run |

`.BCIRC` block, seven words: center X, center Y, radius, start angle,
end angle (0 = 0°, 77777 = 360°; a negative angle also draws the radius to
that end), X squeeze, Y squeeze (0 = none; otherwise coordinate × factor /
32767.).

**Tape, printer, floppy**

| Call | Slot | EMT | Function, input | Output | Run |
|---|---|---|---|---|---|
| .BMOT | 140240 | 114 | R0 = 1 motor on, 0 off (§3.5.1) | | |
| .BMB10 | 140242 | 115 | R0 = parameter block ([§5.12](#512-tape)) (§3.5.2) | C and 52 on error | run |
| .BPINI | 140260 | 124 | initialize the printer driver (§3.6.1) | | |
| .BPSET | 140262 | 125 | R0 = printer mode (§3.6.2) | | |
| .BPGET | 140264 | 126 | read it (§3.6.3) | R0 | run |
| .BPOUT | 140266 | 127 | R0 = character to print; C if the printer stays busy (§3.6.4) | | |
| .BPRDY | 140270 | 130 | is the printer ready? (§3.6.5) | R0 = 1, C = 0; or R0 = 0, C = 1 | run |
| .BPPOS | 140272 | 131 | read the print head position (§3.6.6) | R0 | run |
| .BABOOT … .BFORM | 160000–160012 | — | floppy driver, `JSR PC, @#1600nn` ([§8](#8-the-floppy-disk-driver-bk-0011m)) | | run |

### 5.3 Where the ROM and the manual disagree

Each of these was seen in the emulator, which runs the original ROM, and
can be read in `listings/b11m_bos.mac`:

- **`.BGETW`, `.BGETB`** take the offset in **R1**, as `.BPUTW` does; the
  manual's expansion puts it in R0 (§3.2.5). With the offset in R0 they
  read whatever offset R1 holds (ROM, 150544).
- **`.BPUTW`, `.BPUTB`** ignore the top two bits of the offset only for
  page 7; for other pages an offset of 40000 or more fails with C set
  and code 1 (ROM, 150616). Pass offsets 0–37777.
- **`.BGLIN`** returns in R1 the number of characters plus one (the
  zero byte counts): `ABC` gives 4, an empty line 1.
- **`.BSCOL`, `.BGCOL`**: the low byte of R0 is the background color and
  the high byte the character color, the reverse of §3.4.4. (The
  monitor's escape sequence `ESC ;`, background first, agrees with the
  ROM: 00015 §2.2.1.) Under BASIC `.BGCOL` returns 001400: characters 3
  (white in palette 15) on background 0.
- **`.BBUF`**: the low byte selects the buffer BOS writes to and the high
  byte the buffer shown, the reverse of §3.4.7; asking with bit 15 isn't
  implemented.
- **`.BTIM`** with bit 15 set returns R0 unchanged: the routine puts its
  answer in R0 and BOS then restores the caller's registers (ROM, 150146).
- **`.BGOSET`** has no "ask" form: 200 sets direction 0 (ROM, 153166).
- The manual's §3.6.5 calls `.BPRDY` `EMT 30`; it is `EMT 130`, as its
  table says.

### 5.4 Text output

```
; BK-0011M: three ways to print text through BOS
BSTR  = 140162              ; slots in the BOS system call table
BPRIN = 140160
BTOUT = 140156

        MOV #TEXT1, R0
        JSR PC, @BSTR       ; .BSTR: print up to the zero byte, stay on the line
        MOV #TEXT2, R0
        EMT 64              ; .BPRIN as an EMT: zero byte = then new line
        MOV #TEXT3, R0
        JSR PC, @BPRIN      ; .BPRIN: byte 200 = stay on the line
        MOV #41, R0         ; '!'
        EMT 63              ; .BTOUT: one character
DONE:   BR DONE

TEXT1:  .ASCII /HELLO, /<0>
TEXT2:  .ASCII /BK-0011M/<0>
TEXT3:  .ASCII /NEXT LINE/<200>
        .EVEN
```

On a BK-0011M, under BASIC's banner:

```
HELLO, BK-0011M
NEXT LINE!
```

Control characters for `.BTOUT` (00015 appendix 2): 7 bell, 10 cursor
left, 11 tab, 12 line feed (cursor down, scrolling at the bottom), 14
form feed (the manual: «пропуск 10 строк», skip 10 lines), 15 carriage
return, 16 and 17 Russian and Latin letters (KOI-7), 33 ESC. Note that
**12 doesn't return the cursor to the start of the line**: send 15 and
12, or end strings with `.BPRIN`'s 0 byte. Other codes below 40 are
ignored.

Escape sequences, `ESC` (33) followed by a character (00015 appendix 2;
those marked run were used in the examples):

| Sequence | Effect | Run |
|---|---|---|
| ESC 0, ESC 1, ESC 2 | 32, 64 or 80 characters per line | 1, 2 run |
| ESC 3, ESC 4 | double, single width | run |
| ESC 5, ESC 6, ESC 7 | double height: upper half, lower half; off | |
| ESC 8, ESC 9 | cursor on, off | |
| ESC : *p* | palette: low 4 bits of the next byte | |
| ESC ; *b* *c* | background and character colors (the next two bytes) | |
| ESC =, ESC > | auxiliary keypad mode on, off | |
| ESC @, ESC P | insert, delete a character at the cursor | |
| ESC A, B, C, D | cursor up, down, right, left | |
| ESC E | clear the screen | |
| ESC H | cursor to the top left | |
| ESC I | reverse line feed (scroll down) | |
| ESC J, ESC K | clear to the end of the screen, of the line | |
| ESC L, ESC M | insert a line, delete the cursor's line | |
| ESC N, ESC O | underline on, off | run |
| ESC Q, ESC R | cursor to the start of the next, of this line | |
| ESC T, ESC U | inverse on, off | run |
| ESC Y *l* *c* | cursor to line *l* − 40, column *c* − 40 | run |
| ESC Z | identify: BOS answers `ESC / K` through the keyboard | |

### 5.5 Screen modes

The mode word for `.BTSET` and `.BTGET` (00015 appendix 3):

| Bit | Meaning |
|---|---|
| 0 | 0: 512 dots per line (64 or 80 characters, two colors); 1: 256 dots (32 characters, four colors) |
| 1 | double width (not with 80 characters) |
| 2 | 80 characters per line (6 dots per character; not with 256 dots) |
| 3 | underline cursor instead of a block |
| 4 | smooth scrolling |
| 7 | 0: KOI-7; 1: KOI-8 |
| 8 | with KOI-7: КОИ-7Н2; with KOI-8: combined KOI-7/8 |
| 10 | inverse characters |
| 11 | underlined characters |
| 12 | automatic wrap at the end of the line |
| 13 | cursor shown |
| 14 | control characters shown, not obeyed |

Under BASIC `.BTGET` returns 030201: 256 dots, KOI-8, wrap, cursor.

```
; BK-0011M: screen modes with .BTSET and with ESC sequences
        EMT 62              ; .BCLS
        EMT 52              ; .BTGET: R0 = mode word
        BIC #1, R0          ; bit 0 = 0: 512 dots, 64 characters per line
        EMT 51              ; .BTSET
        MOV #T64, R0
        EMT 64              ; .BPRIN
        MOV #E80, R0        ; ESC 2: 80 characters per line
        EMT 65              ; .BSTR
        MOV #T80, R0
        EMT 64
        MOV #EMIX, R0       ; ESC 1, ESC 3, ...: 64 columns, double width, ...
        EMT 65
        MOV #TBIG, R0
        EMT 64
        MOV #EAT, R0        ; ESC Y line col: cursor to line 10, column 20
        EMT 65
        MOV #TAT, R0
        EMT 65
        EMT 60              ; .BGPOS: R0 = line*400 + column
        MOV R0, R1
        JSR PC, OCTAL
DONE:   BR DONE

T64:    .ASCII /64 CHARACTERS PER LINE (.BTSET)/<0>
E80:    .ASCII <33>/2/<0>
T80:    .ASCII /80 CHARACTERS PER LINE, AFTER ESC 2: 6 DOTS PER CHARACTER/<0>
EMIX:   .ASCII <33>/1/<33>/3/<0>
TBIG:   .ASCII /DOUBLE /<33>/4/<33>/T INVERSE /<33>/U/<33>/NUNDERLINED/<33>/O/<0>
EAT:    .ASCII <33>/Y/<52><64><0>  ; 40+10 = 52, 40+20 = 64
TAT:    .ASCII /HERE: /<0>
        .EVEN
```

With `OCTAL` appended, on a BK-0011M with a mono monitor:

```
64 CHARACTERS PER LINE (.BTSET)
80 CHARACTERS PER LINE, AFTER ESC 2: 6 DOTS PER CHARACTER
DOUBLE  INVERSE UNDERLINED
```

and, on line 10. from column 20.,

```
HERE: 005032
```

`DOUBLE` is in double-width letters, `INVERSE` in inverse video and
`UNDERLINED` underlined; the second line is in the narrower 80-column
letters. `.BGPOS` returned line 12 (10.), column 32 (26.).

### 5.6 Colors and palettes

In the 256-dot mode each dot has one of four color numbers; color 0 is
black and the palette (0–15., set with `.BPAL` or `ESC :`) chooses the
other three (00015 §1.2.3; the RGB values the emulator uses are in
[hardware-reference.md §4](hardware-reference.md#4-video)).

```
; BK-0011M: text colors (.BSCOL) and palettes (.BPAL)
        EMT 62              ; .BCLS
        MOV #COLS, R3
        MOV #4, R4
LINE:   MOV (R3)+, R0       ; high byte: character color, low byte: background
        EMT 53              ; .BSCOL
        MOV #TEXT, R0
        EMT 64              ; .BPRIN
        SOB R4, LINE
        MOV #1400, R0       ; back to color 3 on 0
        EMT 53
        EMT 33              ; .BTTIN: wait for a key
        MOV #3, R0
        EMT 55              ; .BPAL: palette 3 (it was 15)
DONE:   BR DONE

COLS:   .WORD 1400, 1001, 1402, 3
TEXT:   .ASCII / COLOR TEST /<0>
        .EVEN
```

On a BK-0011M with a color monitor, the four lines appear as white on
black, then color 2 on 1, 3 on 2 and 0 on 3: in palette 15 green on
cyan, white on green and black on white. After a key, palette 3 turns
the same screen yellow on black, cyan on green, yellow on cyan and black
on yellow: the bytes in video memory don't change, only their meaning.

### 5.7 The keyboard

```
; BK-0011M: read a key (.BTTIN), poll (.BINKEY) and read a line (.BGLIN)
        EMT 62              ; .BCLS
        MOV #PROMPT, R0
        EMT 65              ; .BSTR
        EMT 33              ; .BTTIN: wait for a key, code in R0
        MOV R0, R1
        JSR PC, OCTAL
        MOV #15, R0         ; carriage return...
        EMT 63
        MOV #12, R0         ; ...and line feed
        EMT 63
        EMT 34              ; .BINKEY: nothing waiting, so C = 1 and R0 = 0
        BCC GOT
        MOV #NONE, R0
        EMT 64              ; .BPRIN
GOT:    MOV #ASK, R0
        EMT 65
        MOV #BUF, R0        ; buffer
        MOV #40., R1        ; at most 40 characters
        EMT 35              ; .BGLIN: line editing until ВВОД
        MOV R1, R3          ; length
        MOV #YOU, R0
        EMT 65
        MOV #BUF, R0        ; the line ends with a zero byte
        EMT 64              ; .BPRIN: print it and a new line
        MOV R3, R1
        JSR PC, OCTAL
        MOV #CHARS, R0
        EMT 64
DONE:   BR DONE

PROMPT: .ASCII /PRESS A KEY: /<0>
NONE:   .ASCII /NO MORE KEYS WAITING/<0>
ASK:    .ASCII /TYPE A LINE: /<0>
YOU:    .ASCII /YOU TYPED: /<0>
CHARS:  .ASCII / CHARACTERS/<0>
BUF:    . = . + 42.       ; room for 40 characters and the zero byte
        .EVEN
```

With `OCTAL` appended, on a BK-0011M with `--type 'A\pHELLO WORLX\030D\n'`:

```
PRESS A KEY: 000101
NO MORE KEYS WAITING
TYPE A LINE: HELLO WORLDYOU TYPE
D: HELLO WORLD
000014 CHARACTERS
```

`.BGLIN` doesn't echo ВВОД, so the next output continues on the same
line and wraps at 32 columns; 14 (12.) is the length plus one ([§5.3](#53-where-the-rom-and-the-manual-disagree)).

**Keys as they happen.** Cell 110 can hold the address of a routine that
BOS calls for every key, with the code in R0, and cell 112 the paging
register value to call it with (0: leave the mapping alone). The routine
may use all registers and ends with `RTS PC`; C = 0 means "handled, don't
buffer it", C = 1 passes the key on to `.BTTIN` (00015 §3.3.2).

```
; BK-0011M: BOS calls a routine of ours for every key (cells 110 and 112)
        EMT 62              ; .BCLS
        CLR @#112           ; 0: call it with the mapping of the moment
        MOV #ONKEY, @#110   ; nonzero: the key routine
MAIN:   MOV #56, R0         ; the main program prints dots
        EMT 63
        MOV #30000, R1
WAIT:   SOB R1, WAIT
        TST KEY             ; a key from the routine?
        BEQ MAIN
        MOV #133, R0        ; print it in brackets
        EMT 63
        MOV KEY, R0
        EMT 63
        MOV #135, R0
        EMT 63
        CLR KEY
        BR MAIN

; Called by the BOS keyboard driver with the key code in R0.
ONKEY:  MOV R0, KEY         ; keep it
        CLC                 ; C = 0: handled, don't put it in the buffer
        RTS PC
KEY:    .WORD 0
```

With `--type '\pA\pB\pC'` the screen fills with dots and `[A]`, `[B]`,
`[C]` appear as the keys arrive.

### 5.8 Graphics

Graphics coordinates run from (0,0) at the top left to (255,255) or
(511,255), depending on the mode (00015 §3.4.15). Most calls draw from or
move the current point.

```
; BK-0011M: BOS graphics in the 256-dot, four-color mode
        EMT 62              ; .BCLS
        MOV #1, R0          ; color 1
        EMT 70              ; .BSCGR
        MOV #20., R0        ; X
        MOV #20., R1        ; Y
        EMT 66              ; .BSGRF: current point (20,20)
        MOV #120., R0
        MOV #100., R1
        EMT 76              ; .BRECT: filled rectangle to (120,100)
        MOV #2, R0
        EMT 70              ; color 2
        MOV #CIRCLE, R0
        EMT 77              ; .BCIRC: parameter block
        MOV #3, R0
        EMT 70              ; color 3
        MOV #10., R0
        MOV #200., R1
        EMT 66              ; move to (10,200)
        MOV #245., R0
        MOV #130., R1
        EMT 75              ; .BVECT: line to (245,130)
        MOV #167., R0       ; a point in the middle of the circle
        MOV #160., R1
        EMT 66
        MOV #2, R0          ; flood the area bounded by color 2
        EMT 100             ; .BFILL, current color 3
        MOV #140., R0
        MOV #30., R1
        EMT 66
        MOV #TEXT, R2
NEXTCH: MOVB (R2)+, R0
        BEQ DONE
        EMT 110             ; .BGOUT: character drawn at the graphics position
        BR NEXTCH
DONE:   BR DONE

CIRCLE: .WORD 170., 160.    ; center
        .WORD 50.           ; radius
        .WORD 0, 77777      ; from 0 to 360 degrees
        .WORD 0, 0          ; no squeezing
TEXT:   .ASCII /BK-0011M/<0>
        .EVEN
```

On a BK-0011M with a color monitor (palette 15): a cyan filled
rectangle, a green circle cut by a white line, the part of the circle
below the line filled white, and `BK-0011M` drawn as graphics in white
to the right of the rectangle. `.BFILL` stopped at the green circle and
at the white line, which was already the fill color.

### 5.9 Pages

`.BPAGE` maps a page; `.BWORK` names a working page that `.BGETW`,
`.BGETB`, `.BPUTW` and `.BPUTB` reach without mapping it. This program
puts screen buffer 0 at 40000 and draws on it directly, then stores a
word in page 2 and reads it back:

```
; BK-0011M: pages. Map screen buffer 0 (page 5) at 40000 and draw on it
; directly; keep data in a hidden page with .BWORK, .BPUTW and .BGETW.
        EMT 62              ; .BCLS: clear the screen
        EMT 10              ; .BMEM: R0 = pages at 40000 (low) and 100000 (high)
        MOV R0, SAVED
        MOV R0, R1
        JSR PC, OCTAL       ; show the mapping we found

        MOV #5, R0          ; page 5 (screen buffer 0), high byte 0: at 40000
        EMT 12              ; .BPAGE
        BCS FAIL
        MOV #54400, R1      ; 40000 + 100. lines * 64. bytes: screen line 100
        MOV #2400, R2       ; 40. lines of 32. words
BAR:    MOV #052525, (R1)+  ; 01 01 01 ... : color 1 dots in 256-dot mode
        SOB R2, BAR

        MOV #2, R0          ; working page for .BPUTW/.BGETW: RAM page 2
        EMT 13              ; .BWORK
        MOV #123456, R0     ; data
        MOV #1234, R1       ; offset in the working page, 0-37776
        EMT 16              ; .BPUTW
        BCS FAIL
        CLR R0
        MOV #1234, R1       ; offset: in R1, not R0 as the manual says
        EMT 14              ; .BGETW: R0 = the word
        BCS FAIL
        MOV R0, R1
        MOV #40, R0
        EMT 63
        JSR PC, OCTAL

        MOVB SAVED, R0      ; put the original page back at 40000
        EMT 12
DONE:   BR DONE
FAIL:   MOVB @#52, R1       ; error code
        JSR PC, OCTAL
        BR DONE
SAVED:  .WORD 0
```

With `OCTAL` appended, on a BK-0011M: `004003 123456` at the top (page 3
and ROM page 10 were mapped, as BASIC leaves them; the word came back),
and a cyan band across the middle of the screen.

`.BMOVE` copies between your memory and a hidden page; `.BJSR` calls a
subroutine there:

```
; BK-0011M: put a subroutine in RAM page 4 (.BMOVE) and call it there (.BJSR)
BJSR = 140054
        EMT 62              ; .BCLS
        MOV #AREA, R0
        EMT 20              ; .BMOVE: copy SUB to page 4 (count < 0: write)
        BCS FAIL
        MOV #204, R0        ; bit 7 set: page 4 is the page for .BJSR and .BJMP
        EMT 13              ; .BWORK
        CLR R3
        MOV #40000, -(SP)   ; the address to call: 40000-77777, so page 4 goes there
        JSR PC, @BJSR       ; .BJSR: only with JSR, the argument is on the stack
        MOV R3, R1          ; the subroutine's result comes back in R3
        JSR PC, OCTAL
        BR DONE
FAIL:   MOVB @#52, R1
        JSR PC, OCTAL
DONE:   BR DONE

AREA:   .WORD SUB           ; memory address
        .WORD 40000         ; hidden address
        .BYTE 4, 4          ; pages at 40000 and 100000 for the move
        .WORD -3            ; 3 words; negative: from memory to the hidden page
SUB:    MOV #4242, R3       ; the subroutine: it will run at 40000 in page 4
        RTS PC
```

With `OCTAL` appended it prints `004242`. `.BJSR` pushes its own return
information, maps the call page at 40000 (for addresses 40000–77777) or
100000, calls the address, and on return restores both windows, even if
the subroutine changed them (00015 §3.2.11). The stack must not be in a
window it switches.

### 5.10 Screen buffers

Pages 5 and 6 are screen buffers 0 and 1. BOS can write into one while
the other is shown, so a picture can be built out of sight:

```
; BK-0011M: draw in the hidden screen buffer, then show it (.BBUF)
        EMT 62              ; .BCLS in buffer 0, the one on the screen
        MOV #T0, R0
        EMT 64
        MOV #1, R0          ; low byte 1: write to buffer 1; high byte 0: show buffer 0
        EMT 56              ; .BBUF
        EMT 62              ; clear buffer 1, unseen
        MOV #T1, R0
        EMT 64              ; and write in it
        EMT 33              ; .BTTIN: wait for a key
        MOV #401, R0        ; write to buffer 1, show buffer 1
        EMT 56
DONE:   BR DONE
T0:     .ASCII /BUFFER 0: PRESS A KEY/<0>
T1:     .ASCII /BUFFER 1, WRITTEN WHILE HIDDEN/<0>
        .EVEN
```

On a BK-0011M the screen shows `BUFFER 0: PRESS A KEY`; after a key,
`BUFFER 1, WRITTEN WHILE HIDDEN`.

### 5.11 Sound

`.BSOUN` makes a tone on the speaker: R0 periods, each two delay loops of
R1 steps (ROM, 143654). It flips bit 6 of the tape register relative to
BOS's copy in cell 116, which is how a program should do it too.

```
; BK-0011M: .BSOUN, a tone of R0 periods of length R1
        MOV #500., R0       ; LENGTH: number of periods
        MOV #50., R1        ; TONE: delay for each half period
        EMT 101             ; .BSOUN
DONE:   BR DONE
```

The emulator's sound output shows about 1600 Hz for 0.31 s; with R1 =
100. it is about 850 Hz.

### 5.12 Tape

BOS reads and writes tapes in the BK-0010 format (00015 §2.8).
`.BMB10` (EMT 115) takes a parameter block in R0 (00015 §3.5.2):

| Offset | Contents |
|---|---|
| 0 | operation: 0 write, 1 read, 2 read the header only, 3 dummy read |
| 2 | address: where to write from; where to read to (0: the file's own) |
| 4 | length in bytes, for writing |
| 6–25 | name, 16 characters; an empty name (first byte 0) reads the first file found |
| 26 | page to map at 40000 during the transfer (negative: leave as it is) |
| 27 | page to map at 100000 during the transfer (negative: leave as it is) |
| 30 | on reading: the address in the file found |
| 32 | its length |
| 34–53 | its name |

Errors set C and leave in cell 52: 1 a file with another name was found
(the motor stops after its header), 2 checksum error, 3 address range not
allowed, 4 bus error or СТОП. `.BMOT` (EMT 114) starts and stops the motor
for winding.

```
; BK-0011M: write a file in the BK-0010 tape format with .BMB10, read it back
BMB10 = 140242
        EMT 62              ; .BCLS
        MOV #AREA, R0
        CLRB (R0)           ; code 0: write
        MOV #TEXT, 2(R0)    ; address
        MOV #TEXTLN, 4(R0)  ; length
        JSR PC, @BMB10
        BCS FAIL
        MOV #AREA, R0
        MOVB #1, (R0)       ; code 1: read
        MOV #4000, 2(R0)    ; to 4000 (0 = the file's own address)
        JSR PC, @BMB10
        BCS FAIL
        MOV #4000, R0
        EMT 65              ; .BSTR: the text read back
        MOV AREA+30, R1     ; the file's own address, from its header
        JSR PC, OCTAL
        MOV #40, R0
        EMT 63
        MOV AREA+32, R1     ; and its length
        JSR PC, OCTAL
DONE:   BR DONE
FAIL:   MOVB @#52, R1       ; error code
        JSR PC, OCTAL
        BR DONE

TEXT:   .ASCII /READ BACK FROM TAPE/<15><12><0>
TEXTEND:
TEXTLN = TEXTEND - TEXT
        .EVEN
AREA:   .BYTE 0, 0          ; 0: operation
        .WORD 0, 0          ; 2: address, 4: length
        .ASCII /DEMO11          / ; 6: name, 16 characters
        .BYTE 377, 377      ; 26, 27: pages at 40000 and 100000 (377: as now)
        .WORD 0, 0          ; 30, 32: the file's address and length
        .ASCII /                / ; 34: its name
```

With `OCTAL` appended, on a BK-0011M:

```
READ BACK FROM TAPE
001126 000026
```

the text and the address and length found in the file's header. As with
the BK-0010 example, this ran with fast tape (`--files`) and, split in
two, through a recorded tape signal. A tape written by the BK-0010's
`EMT 36` example also read back with `.BMB10`.

### 5.13 EMT numbers of your own

BOS uses EMT 0–133. A program can have the rest: put a handler's address
in cell 166, and `EMT 134`–`EMT 377` reach it (00015 §1.4, §2.4). The
handler is entered as from the EMT trap itself, with the caller's PC and
PSW on the stack, and returns with `RTI`. If the program had already set
vector 30, `.BEMT` moves that address to 166 and gives vector 30 back to
BOS (00015 §3.1.5).

```
; BK-0011M: EMT numbers above 133 for our own use (cell 166)
        EMT 62              ; .BCLS (a BOS EMT)
        MOV #MYEMT, @#166   ; EMTs that BOS doesn't know come here
        MOV #MSG1, R1
        EMT 200             ; ours: print the string at R1
        MOV #MSG2, R1
        EMT 200
DONE:   BR DONE

; Our EMT handler. The stack holds the caller's PC and PSW, as for any trap.
MYEMT:  MOV R0, -(SP)
        MOV 2(SP), R0       ; return address
        MOVB -2(R0), R0     ; low byte of the EMT instruction: its number
        CMPB R0, #200
        BNE OUT
        MOV R1, R0
        EMT 64              ; .BPRIN: BOS calls still work in here
OUT:    MOV (SP)+, R0
        RTI                 ; straight back to the caller
MSG1:   .ASCII /PRINTED BY EMT 200/<0>
MSG2:   .ASCII /AND AGAIN/<0>
        .EVEN
```

On a BK-0011M it prints both lines.

### 5.14 The printer

The printer driver (`.BPINI` to `.BPPOS`) sends characters to a printer
on the parallel port, recoding them as set by `.BPSET`: bit 8 input
KOI-7, bit 9 output KOI-7, bit 10 KOI-7Н2; the low byte is how long to
wait for the printer, in units of 65536 turns of a `SOB` loop, 0 for ever
(00015 §3.6.2). Without `--printer` the emulator has no printer, and
`.BPRDY` returns R0 = 0 with C set; with `--printer FILE`, what the driver
prints is added to FILE as text ([emulator.md](emulator.md#choosing-a-machine)).

### 5.15 Ending a program

```
; BK-0011M: end a program by going to the BOS keyboard monitor
        MOV #MSG, R0
        EMT 64              ; .BPRIN
        JSR PC, @140012     ; .BEXIT (or EMT 1)
MSG:    .ASCII /BYE/<0>
        .EVEN
```

prints `BYE`, then the monitor prints the address after the call,
`001012`, and its prompt `@`. The monitor's `P` command would continue
from that address (00015 §3.1.2).

## 6. Devices at register level

Programs that need speed, or do things the ROM doesn't, use the device
registers directly. This is a summary; the details, including the
emulator's timing, are in [hardware-reference.md](hardware-reference.md).
Bits not listed are unused.

### 6.1 Keyboard: 177660, 177662

| Register | Bits |
|---|---|
| 177660 status | bit 7 (read only): a key code is waiting; bit 6: 1 masks the keyboard interrupt (00001 §4.1.1; 00015 §1.2.1) |
| 177662 data (read) | bits 6–0: the 7-bit key code. Reading clears bit 7 of 177660; until then further keys are ignored (00001 §4.1.2; 00015 §1.2.2) |

A key raises an interrupt at vector 60, or 274 for АР2 combinations and
some control keys, when bit 6 is 0 and PSW bit 7 is 0. The monitor and
BOS turn the 7-bit codes into the full 8-bit set ([§3.3](#33-writing-an-interrupt-handler)
handles the raw codes). Bit 6 of 177716 (read) is 0 while any key is held
down, which is how the drivers repeat keys (00001 §4.1.5; 00015 §1.2.6.1).

### 6.2 Screen memory and the scroll register 177664

The screen is 256 lines of 100 (64.) bytes, 16 KB: 40000–77777 on the
BK-0010, pages 5 and 6 on the BK-0011M. Within a byte the lowest bit is
the leftmost dot. In 512-dot mode each bit is a dot; in 256-dot (color)
mode each pair of bits is a dot with color 0–3 (00001 §8.1; 00015
§1.2.4; [hardware-reference.md §4](hardware-reference.md#4-video)).

177664 (read/write): bits 7–0 set which line of screen memory is shown at
the top: 330 shows the start of the buffer at the top, each step moves
the picture one TV line, and the buffer wraps round. Bit 9 = 1 shows the
whole buffer; 0 shows only the top quarter of the picture, 64 lines,
taken from the buffer by the same offset. With the offset the BK-0010
monitor sets for its extended memory mode, 230, those lines are the last
quarter of the buffer (70000–77777 on the BK-0010) (00001 §4.1.3; 00015
§1.2.4). With 330 in the register, dot (X, Y) of a
512-dot screen is bit X mod 8 of byte 40000 + 100×Y + X/8.

### 6.3 BK-0011M palette register: 177662 (write)

Writing 177662 on a BK-0011M (00015 §1.2.3): bits 11–8 the palette;
bit 14 = 1 stops the 50 Hz interrupt; bit 15 selects the buffer shown, 0
for page 5 and 1 for page 6. It can't be read back, and BOS keeps its own
copy in the system page, so prefer `.BPAL`, `.BTIM` and `.BBUF`: a direct
write is undone by the next of those calls. On the BK-0010, 177662 is
read-only and writing it is a bus error
([hardware-reference.md §12](hardware-reference.md#12-bk-0010--bk-001001-second-machine-model)).

### 6.4 The CPU timer: 177706–177712

The K1801VM1 has a 16-bit timer inside the CPU. Neither manual
describes it; the bits below are from
[hardware-reference.md §6](hardware-reference.md#6-programmable-timer-inside-the-vm1),
and the timer doesn't interrupt on these machines, so programs poll it.

| Register | Use |
|---|---|
| 177706 | reload value |
| 177710 | the counter, counting down (read only) |
| 177712 | control: bit 0 stop, 1 count on past 0, 2 set bit 7 at 0, 3 stop at 0, 4 run, 5 divide by 16, 6 divide by 4, 7 reached 0. Writing it reloads the counter |

It counts at the CPU clock divided by 128: 23437.5 Hz on the BK-0010
(3 MHz), 31250 Hz on the BK-0011M (4 MHz).

```
; BK-0010: one-second steps from the CPU's timer (polled; it has no interrupt)
        EMT 14
        MOV #23437., @#177706 ; reload value: 3 MHz / 128 = 23437 ticks a second
        MOV #5, R3
AGAIN:  MOV #24, @#177712   ; bit 4 run, bit 2 flag expiry; the write reloads the counter
WAIT:   TSTB @#177712       ; bit 7: the counter has reached 0
        BPL WAIT
        MOV #52, R0         ; '*'
        EMT 16
        SOB R3, AGAIN
        MOV #1, @#177712    ; bit 0: stop the timer
DONE:   BR DONE
```

On a BK-0010.01 it prints five asterisks a second apart (three after
150 frames, measured in the emulator's debugger). On a BK-0011M, with
`31250.` as the reload value and `EMT 62` and `EMT 63` for `EMT 14` and
`EMT 16`, it keeps the same time.

### 6.5 The parallel port: 177714

Write: the 16 output lines. Read: the 16 input lines; the outputs can't
be read back (00001 §4.1.4; 00015 §1.2.5). Keep a copy of what you write:
the BK-0010 convention is cell 256 (00001 §11). On the BK-0011M the port
can also interrupt at vector 270 (00015 §1.4). The emulator can put a
joystick on the inputs ([emulator.md](emulator.md#joystick)), a Covox or an
AY-3-8910 sound chip on the outputs
([hardware-reference.md §9](hardware-reference.md#9-sound-options)), or a
printer on the port (`--printer`).

### 6.6 The system register: 177716

**BK-0010** (00001 §4.1.5). Read: bits 15–8 the start address (100000);
bit 4 serial data in, bit 5 tape data in, bit 6 = 0 while a key is held
down, bit 7 serial "ready" in. Write: bit 4 serial data out (idle 1), bit
5 tape data out (or serial "ready"), bit 6 tape data out and the speaker,
bit 7 tape motor (1 = stopped). The monitor writes 220 for "motor
stopped, line idle".

**BK-0011M** (00015 §1.2.6). Read: bits 15–8 the start address (140000);
bit 7 = 1; bit 6 = 0 while a key is held down; bit 5 tape data in; bit 2
set by any write to the register (and by `HALT` and СТОП), cleared by
reading. Writes go to one of two registers:

- a **word write with bit 11 set** goes to the paging register
  ([§2.4](#24-switching-pages-safely));
- any other write goes to the tape register: bit 12 locks the СТОП key,
  bit 7 the tape motor (1 = stopped), bits 6, 5 and 2 data to the tape.
  The same bits drive the speaker, bit 6 the loudest
  ([hardware-reference.md §5](hardware-reference.md#5-tape-and-speaker-177716-write-bit-11--0)).

BOS keeps copies of the last writes in 114 (paging) and 116 (tape). To
make a sound, flip bit 6 of the value in 116, as `.BSOUN` does
([§5.11](#511-sound)), and leave 116 as it was.

### 6.7 Floppy controller: 177130, 177132

With the floppy controller fitted, 177130 is its command register
(write) and status register (read), and 177132 its data register
(00008 §1.3.6.1). The ROM driver ([§8](#8-the-floppy-disk-driver-bk-0011m))
is the way to use it; the bit-level description is in
[hardware-reference.md §8](hardware-reference.md#8-floppy-controller-kngmd-к1801вп1-128-family).

## 7. Tape files and .bin files

### 7.1 Tape files

Both machines write the same tape format at about 1200 baud (00001 §9;
00015 §2.8). A file on tape is (00001 §9.1.3, and the ROM's write
routine at 116256):

1. a long leader tone, by which the reader finds the file;
2. the header, 20. bytes: the address it was saved from (word), its
   length in bytes (word), its name (16 characters, padded with spaces);
3. a short tone, then the data;
4. a checksum word: the sum of the data bytes, each added as an unsigned
   number with the carry added back in (`ADD`, then `ADC`; ROM, 116622);
5. a short closing tone.

A write of length 0 writes nothing (ROM, 116272). On reading, the driver
compares names, loads the data, and compares the checksum. The bit-level
encoding is done in software by the ROMs; the emulator plays and records
it as sound ([emulator.md](emulator.md)).

### 7.2 .bin files

A `.bin` file is a program image without the tape framing: a word with
the load address, a word with the length in bytes, then the bytes
([hardware-reference.md §12](hardware-reference.md#bin-files)). It is the
tape header without the name and checksum. The project's assembler
writes this format, the emulator's `--load` reads it, and its fast tape
(`--files`) stores each tape file as `NAME.bin`. The `DEMO` file of
[§4.8](#48-tape) became `DEMO.bin`: 001130, 000031, then the 25. bytes of
text.

Many BK programs load below 1000, so that the tape load itself
overwrites the low-memory vectors and the program starts when loaded;
their code begins at 1000 ([emulator.md](emulator.md#programs-bin)).

## 8. The floppy disk driver (BK-0011M)

With the floppy controller fitted, its ROM at 160000 holds a disk driver
with fixed entry points, called with `JSR PC, @#entry` (absolute: the
entries are instructions, not a table) (00015 §3.7; 00008 §3.6). This
section describes the BK-0011M's ROM 327. The emulator also fits the
controller to a BK-0010.01 (`--bk0010 --fdd`), with its own ROM 326; that
ROM's calls weren't checked here.

| Entry | Name | Function |
|---|---|---|
| 160000 | .BABOOT | boot from drive 0, then 1, 2, 3: load track 0 sector 1 to 1000 and jump there with the drive number in R0 (only if its first word is 240, `NOP`: [hardware-reference.md §8](hardware-reference.md#8-floppy-controller-kngmd-к1801вп1-128-family)); returns with C set if no disk boots |
| 160002 | .BBOOT | the same from drive R0 only |
| 160004 | .BBLCK | read or write blocks |
| 160006 | .BSECT | read or write by side, track and sector, given in the work area |
| 160010 | .BDINI | fill in a work area with defaults; R3 = its address |
| 160012 | .BFORM | format a track: R3 = work area with FILLB, SIDE, TRK, UNIT set |

The driver needs a work area of 66 (54.) bytes; R3 points at it for every
call (00015 §3.7; the BK-0011 manual says 60):

| Offset | Name | Contents |
|---|---|---|
| 0 | CSRW | copy of what was last written to 177130 |
| 2 | CURTRK | the current track («адрес текущей дорожки»), driver's use |
| 4 | TRKTAB | current track of each drive (4 bytes) |
| 10 | TDOWN | head-load time, in `SOB` loops |
| 12 | TSTEP | step time |
| 14 | TRKCOR | first track that gets write precompensation |
| 15 | BRETRY | retries on errors |
| 16 | FLAGS | driver's own |
| 17 | FILLB | fill byte for formatting |
| 20 | FLGPTR | pointer into FLGTAB |
| 22 | FLGTAB | one byte per drive: bit 0 use even tracks only (40-track disks in an 80-track drive), bit 1 one side only, bit 2 1024-byte sectors |
| 26 | ADDR | buffer address (for .BSECT) |
| 30 | WCNT | word count (for .BSECT) |
| 32 | SIDE | side, 0 the lower |
| 33 | TRK | track |
| 34 | UNIT | drive 0–3 |
| 35 | SECTOR | sector, 1–12 (1–10.) |
| 36–57 | | driver's own |
| 60 | MAXSEC | sectors per track |
| 62–65 | | driver's own |

`.BBLCK` (00015 §3.7.3): R0 = block number (512 bytes; block 0 is
track 0, side 0, sector 1), R1 = word count, positive to read and negative
to write, R2 = buffer, R3 = work area, UNIT = drive. Transfers longer
than a sector run on to the next sectors, sides and tracks. R0, R1, R2,
R4 and R5 are not preserved. On an error C is set and cell 52 holds: 1
data CRC error, or the disk is write-protected; 2 header CRC error; 3
track 0 not found; 4 seek error; 5 sector not found; 6 no disk, or it
isn't turning; 7 bus error; 10 no address mark; 11 no data mark; 12 bad
parameters or a non-standard format. The motor stays on afterward: stop
it by writing 0 to 177130 and to CSRW.

```
; BK-0011M with the floppy controller: read blocks 0 and 5 of drive 0
        EMT 62              ; .BCLS
        MOV #AREA, R3
        JSR PC, @#160010    ; .BDINI: fill in the driver's work area
        CLRB AREA+34        ; UNIT: drive 0
        CLR R0              ; block 0
        JSR PC, READ
        MOV #5, R0          ; block 5
        JSR PC, READ
        CLR @#177130        ; the driver leaves the motor on: stop it
        CLR AREA            ; and keep CSRW, its copy of 177130, in step
DONE:   BR DONE

; Read block R0 and print it as a zero-terminated string.
READ:   MOV #256., R1       ; 256 words (positive: read)
        MOV #BUF, R2        ; buffer
        MOV #AREA, R3       ; work area
        JSR PC, @#160004    ; .BBLCK
        BCS FAIL
        MOV #BUF, R0
        EMT 64              ; .BPRIN: text and a new line
        RTS PC
FAIL:   MOVB @#52, R1       ; error code
        JSR PC, OCTAL
        BR DONE

AREA:   . = . + 66          ; driver work area, 66 (octal) bytes
BUF:    . = . + 1000        ; one block: 512 bytes
```

For this run a blank 800 KB disk image was made with text at the start of
blocks 0 and 5, and the machine given `--disk` with it. A disk that
doesn't boot sends the BK-0011M on to BASIC after a few seconds, so the
program was loaded after 1200 frames. With `OCTAL` appended it prints

```
THIS IS BLOCK 0 OF A DATA DISK
AND THIS IS BLOCK 5
```

Block n is at byte n × 512 of a standard image
([hardware-reference.md §8](hardware-reference.md#8-floppy-controller-kngmd-к1801вп1-128-family)).
Reading block 0 of a bootable disk the same way gave the same eight words
as the image file's first 16 bytes. Of the other entries, only `.BABOOT`
ran (the BOS start-up calls it).

## 9. BK-0010 programs on a BK-0011M

The BK-0011M reads BK-0010 tapes ([§5.12](#512-tape)), but a BK-0010
program generally doesn't run on it. Most BK-0010 software uses the
monitor's EMT calls, and the same EMT numbers call different BOS
routines:

| EMT | BK-0010 monitor | BK-0011M BOS |
|---|---|---|
| 4 | keyboard initialization | .BEMT |
| 6 | wait for a key | reserved: returns at once with C set |
| 10 | read a line | .BMEM |
| 12 | function key | .BPAGE: maps page R0 |
| 14 | initialize the drivers | .BGETW |
| 16 | print a character | .BPUTW: writes R0 into the working page at offset R1 |
| 20 | print a string | .BMOVE, with R0 taken as a parameter block |
| 22 | service line | slot of .BJSR, which needs its argument on the stack |
| 24, 26 | set, read the cursor | reserved |
| 30, 32, 34, 36 | point, line, status, tape | .BKINI, .BKGET, .BINKEY, .BEDIT |

The `hello10` example of [§4.3](#43-printing) shows it. Run on a
BK-0011M it prints nothing, and afterward its own message begins with
`!` instead of `HE`: `EMT 20` did nothing useful, and `EMT 16` stored R0
(41, the `!`) at offset 1022 of working page 0, which is where the
message is (seen with the emulator's debugger). The game `software/klad.bin`
runs on a BK-0010.01 but on a BK-0011M leaves BASIC's screen as it was.

Other differences a BK-0010 program meets (sources in the sections
linked):

- **Memory.** 40000–77777 is a window, not the screen: under BASIC it
  holds RAM page 3, which contains BASIC's own code (its vector 4 handler
  is at 61176), so a BK-0010 program drawing there is invisible and
  damages BASIC rather than BOS, whose variables are in page 7
  ([§2.3](#23-bk-0011m-memory-pages-and-windows)). 100000–137777 holds
  BASIC's ROM, not the monitor, so direct calls into the monitor ROM and
  reads of its variables (cells 40–56, 260 and so on) fail.
- **Registers.** A word write to 177716 with bit 11 set switches pages,
  and writes to 177662 change the palette, the screen buffer and the
  50 Hz interrupt ([§6](#6-devices-at-register-level)).
- **Speed.** The CPU is faster: the speaker example's tone rose from
  about 1080 Hz to 1300 Hz ([§4.7](#47-sound)).
- **The same:** the screen layout and the scroll register, the keyboard
  registers (read), the port, the CPU timer (counting faster), the
  speaker bit, and the tape format.

A program written for both must find out where it is (bits 15–8 of
177716 read 100000 on a BK-0010 and 140000 on a BK-0011M) and, on the
BK-0011M, use BOS calls and map page 5 at 40000 before drawing. In the
emulator, run BK-0010 software as a BK-0010 (`--bk0010`).

# Programming in machine code

The BK's processor, the K1801VM1, runs the PDP-11 instruction set. This guide
covers what you need to write programs in assembly language for the BK and
run them in the emulator: the processor, its instructions and their timing,
the assembler that comes with this project, a first program for each model,
and the debugger. How programs use the ROM's services (printing, the
keyboard, graphics, tape, memory pages) is in
[system-programming.md](system-programming.md).

- [Workflow](#workflow)
- [The processor](#the-processor)
- [Addressing modes](#addressing-modes)
- [Instructions](#instructions)
- [How the VM1 differs from other PDP-11s](#how-the-vm1-differs-from-other-pdp-11s)
- [Traps and interrupts](#traps-and-interrupts)
- [Timing](#timing)
- [The assembler](#the-assembler)
- [A first program](#a-first-program)
- [Debugging it](#debugging-it)
- [Program files](#program-files)
- [Reading other people's code](#reading-other-peoples-code)

Numbers are octal throughout, as in all PDP-11 and BK documentation.

## Workflow

1. Write the program in a text file, in MACRO-11 syntax.
2. Assemble it into a `.bin` file:

   ```sh
   cargo run -p vm1 --example asm -- hello.mac 1000 hello.bin
   ```

   The second argument is the address the program will be loaded at, in
   octal. User programs normally start at 1000, just above the stack.
3. Run it: `bk --bk0010 --load hello.bin` (a BK-0010.01) or
   `bk --load hello.bin` (a BK-0011M). The emulator boots the machine, loads
   the program and starts it at its first word.
4. When something goes wrong, add `--debug` to start in the debugger, or
   press Alt+D while it runs ([Debugging it](#debugging-it)).

## The processor

The K1801VM1 is a 16-bit processor with eight 16-bit registers:

| Register | Use |
|---|---|
| R0–R5 | general purpose |
| R6 = SP | stack pointer; the stack grows down |
| R7 = PC | program counter |

and a **processor status word** (PSW):

| Bit | Name | Meaning |
|---|---|---|
| 0 | C | carry |
| 1 | V | overflow |
| 2 | Z | zero |
| 3 | N | negative |
| 4 | T | trace: trap to vector 14 after each instruction |
| 7 | P | priority: when set, the keyboard and the BK-0011M's 50 Hz interrupt are held off |

Memory is addressed in bytes; words are at even addresses, low byte first.
The PSW has no memory address on the VM1: read and write it with `MFPS` and
`MTPS`. `MTPS` can't set the T bit; only `RTI` and `RTT` load it (from the
stack). The upper byte holds the processor's halt-mode state, which the BK
doesn't use.

At start-up the monitor or BOS leaves the stack just below 1000 (the
emulator's `--load` sets SP to 1000), the vectors at 0–377, and system
variables between them. The machine-description guide has the memory maps
([machine.md](machine.md#memory-maps)).

## Addressing modes

Each operand is a register and one of eight modes. Modes 2, 3, 6 and 7 used
with PC give the immediate, absolute and PC-relative forms.

| Mode | Syntax | Operand |
|---|---|---|
| 0 | `R` | the register itself |
| 1 | `(R)` | the word at the address in R |
| 2 | `(R)+` | the word at R; then R += 2 (1 for byte instructions, except SP and PC) |
| 3 | `@(R)+` | the word whose address is at R; then R += 2 |
| 4 | `-(R)` | R -= 2 (or 1); then the word at R |
| 5 | `@-(R)` | R -= 2; then the word whose address is at R |
| 6 | `X(R)` | the word at R + X (X is the next word of the instruction) |
| 7 | `@X(R)` | the word whose address is at R + X |
| 2 with PC | `#N` | immediate: the value N |
| 3 with PC | `@#A` | absolute: the word at address A |
| 6 with PC | `A` | relative: the word at A (assembled as an offset from PC) |
| 7 with PC | `@A` | the word whose address is at A |

`(SP)+` pops and `-(SP)` pushes. Relative mode makes position-independent
code; absolute mode (`@#177716`) is the way to reach device registers.

## Instructions

`B` after a name means there is a byte form (`MOVB`, `CLRB`...). Byte
instructions on a register use its low byte, except that `MOVB` to a
register sign-extends into the whole word.

**Two operands** (`src`, `dst`):

| Instruction | Does | Flags |
|---|---|---|
| `MOV(B) src, dst` | dst = src | N Z, V=0 |
| `ADD src, dst` | dst = dst + src | N Z V C |
| `SUB src, dst` | dst = dst − src | N Z V C |
| `CMP(B) src, dst` | src − dst, sets flags only | N Z V C |
| `BIT(B) src, dst` | src AND dst, sets flags only | N Z, V=0 |
| `BIC(B) src, dst` | dst = dst AND NOT src (clear bits) | N Z, V=0 |
| `BIS(B) src, dst` | dst = dst OR src (set bits) | N Z, V=0 |
| `XOR R, dst` | dst = dst XOR R (register source only) | N Z, V=0 |

**One operand:**

| Instruction | Does |
|---|---|
| `CLR(B)` | clear |
| `COM(B)` | complement (NOT) |
| `INC(B)`, `DEC(B)` | add, subtract 1 |
| `NEG(B)` | negate |
| `ADC(B)`, `SBC(B)` | add, subtract the carry |
| `TST(B)` | set N and Z from the operand |
| `ROR(B)`, `ROL(B)` | rotate right, left through C |
| `ASR(B)`, `ASL(B)` | arithmetic shift right, left by one bit |
| `SWAB` | swap the bytes of a word |
| `SXT` | fill with the N flag (0 or 177777) |
| `MFPS dst`, `MTPS src` | read, write the PSW (a byte) |

**Branches** (to within −128 to +127 words): `BR` (always), `BNE`, `BEQ`,
`BPL`, `BMI`, `BVC`, `BVS`, `BCC` (= `BHIS`), `BCS` (= `BLO`); signed `BGE`,
`BLT`, `BGT`, `BLE`; unsigned `BHI`, `BLOS`. And `SOB R, label`: decrement R
and branch back (only backward, up to 63 words) if it isn't zero, the loop
instruction.

**Jumps and subroutines:**

| Instruction | Does |
|---|---|
| `JMP dst` | jump |
| `JSR R, dst` | push R, R = PC, jump. `JSR PC, sub` is the usual call |
| `RTS R` | PC = R, pop R. `RTS PC` returns |
| `MARK N` | clean N words of arguments off the stack on return |

**Traps and processor control:**

| Instruction | Does |
|---|---|
| `EMT N` (N = 0–377) | trap to vector 30: the system call |
| `TRAP N` | trap to vector 34 |
| `IOT` | trap to vector 20 |
| `BPT` | trap to vector 14 (breakpoint) |
| `RTI` | return from an interrupt or trap: pop PC and PSW |
| `RTT` | the same, but a trace trap waits one instruction |
| `WAIT` | wait for an interrupt |
| `RESET` | reset the devices (the INIT signal on the bus) |
| `HALT` | on the BK: like pressing СТОП (trap to vector 4, see below) |
| `NOP`, `CLC`, `SEC`, `CLV`, `CLZ`, `CLN`, `SCC`... | flag operations; `NOP` is 000240 |

## How the VM1 differs from other PDP-11s

- **No multiply, divide or shifts by N** (`MUL`, `DIV`, `ASH`, `ASHC`), no
  floating point, no `SPL`, `MFPI`/`MTPI` or `MFPT`. These trap to vector 10
  as reserved instructions; programs multiply and divide with shifts and
  adds.
- **The PSW isn't at 177776**: use `MFPS`/`MTPS`.
- **No odd-address trap.** A word access to an odd address uses the even
  address below it.
- **`HALT` enters halt mode**, which on the BK has no memory of its own, so it
  ends up as a trap to vector 4, the same as the СТОП key; the ROMs treat it
  as a stop. `JMP` and `JSR` to a register (mode 0) also trap to 4.
- **Instruction traps come before interrupts.** When an `EMT` (or `TRAP`,
  `IOT`, `BPT`) is executed while an interrupt is waiting, the trap is taken
  first, and the interrupt only when the trap handler's PSW allows it. (The
  emulator's checks against the chip-level model of the VM1 are described in
  `tools/vm1-timing/README.md`.)

## Traps and interrupts

A trap or interrupt pushes the PSW and the PC, then loads a new PC and PSW
from a two-word **vector** in low memory. `RTI` returns.

| Vector | Cause |
|---|---|
| 4 | bus error (no memory or device at an address), `HALT`, the СТОП key |
| 10 | reserved instruction |
| 14 | `BPT`, trace (T bit) |
| 20 | `IOT` |
| 30 | `EMT`: system calls go to the monitor or BOS |
| 34 | `TRAP` |
| 60 | keyboard: a key code is ready |
| 274 | keyboard: a key pressed with АР2, or a function key |
| 100 | BK-0011M: 50 times a second, at the start of each TV frame (on the BK-0010 this is IRQ2, which nothing drives unless the emulator is given `--irq2-50hz`) |

The keyboard and 50 Hz interrupts are held off while PSW bit 7 is set. How to
write a handler and what the system expects from it is in
[system-programming.md](system-programming.md).

## Timing

Instructions take a number of processor clocks that depends on the
instruction and its operands' modes, including waits for memory. Some
typical values, measured on the MiST FPGA core's chip-level model of the VM1
with the BK's memory controller, for code in RAM (`tools/vm1-timing`):

| Instruction | BK-0010 (3 MHz) | BK-0011M (4 MHz) |
|---|---|---|
| `NOP`, `MOV R1, R2` | 12 | 16 |
| `MOV #N, R0` | 28 | 32 |
| `MOV R1, (R3)` | 36 | 40 |
| `MOV #N, (R3)` | 40 | 48 |
| `BR`, `BNE` | 16 | 16 |
| `SOB` | 20 | 21 |
| `JSR PC` + `RTS PC` | 72 | 80 |

A register-to-register instruction takes 4 µs on both models: the
BK-0011M's faster clock is mostly eaten by memory waits, which makes it only
about 15% faster than a BK-0010. Timing matters on the BK because sound and
tape signals come from timed loops.

## The assembler

`cargo run -p vm1 --example asm -- SOURCE ORIGIN OUTPUT.bin [SYMBOLS]` (or
the built `target/release/examples/asm`) is a small two-pass assembler for
a subset of DEC's MACRO-11. Its rules:

- One statement per line: `[LABEL:] instruction operands [; comment]`.
- Numbers are **octal**; a trailing dot makes them decimal (`100.`).
- Expressions: numbers, symbols and `.` (the current address), joined with
  `+` and `-`.
- `NAME = expression` defines a symbol; `. = expression` moves the location
  forward (filling with zeros).
- Directives: `.WORD`, `.BYTE` (lists of expressions), `.ASCII`, `.EVEN`
  (align to a word), `.END` (stop).
- `.ASCII /text/` takes any delimiter, and `<expr>` for single bytes:
  `.ASCII /HELLO/<12><0>`. Russian letters in the text are encoded in KOI-8,
  the BK's character set.
- Registers are `R0`–`R5`, `SP`, `PC`. Upper and lower case are the same,
  except inside `.ASCII`.

There are no macros, character constants (`'A`) or listing files. Its output
is a `.bin` file with the standard header (see [Program files](#program-files)).
A fourth argument names a symbol file to write as well (`NAME = ADDRESS`
lines), which the debugger loads with `--symbols` so its listings and
commands use your labels.

## A first program

### BK-0010

The BK-0010 monitor's system calls are `EMT` instructions with an even number.
`EMT 20` prints R2 characters starting at the address in R1; `EMT 6` waits for
a key and returns its code in R0 (driver-monitor system manual
00001-01.32.03; the full list is in
[system-programming.md](system-programming.md)).

```
; Hello on the BK-0010: print a string with the monitor's EMT 20,
; then wait for a key with EMT 6 and go back to the monitor.
        MOV #MSG, R1            ; address of the text
        MOV #LEN, R2            ; its length
        EMT 20                  ; print R2 characters from R1
        EMT 6                   ; wait for a key (code in R0)
        JMP @#100000            ; restart the monitor
MSG:    .ASCII /HELLO, BK-0010!/<12>
LEN = . - MSG
```

The `<12>` byte is the BK's new-line code (ВВОД). Assemble and run:

```sh
cargo run -p vm1 --example asm -- hello10.mac 1000 hello10.bin
bk --bk0010 --load hello10.bin
```

The screen shows, under BASIC's sign-on:

```
Ok
HELLO, BK-0010!
```

and the program waits for a key. Pressing one jumps to 100000, the
monitor's start, which starts BASIC again.

### BK-0011M

BOS's calls are numbered differently. Each is an `EMT N` or, equivalently, a
`JSR PC,@ADDR` through BOS's call table at 140010 (BOS manual 00015-01.32.01
§2.3, §3). `.BSTR` (EMT 65) prints a string ending in a 0 byte whose address
is in R0, `.BTTIN` (EMT 33) waits for a key, `.BEXIT` (EMT 1) returns to
BOS's keyboard monitor.

```
; Hello on the BK-0011M: print a string with BOS's .BSTR (EMT 65),
; wait for a key with .BTTIN (EMT 33), then exit to the keyboard
; monitor with .BEXIT (EMT 1).
        MOV #MSG, R0            ; address of a string ending in a 0 byte
        EMT 65                  ; .BSTR
        EMT 33                  ; .BTTIN: wait for a key, code in R0
        EMT 1                   ; .BEXIT
MSG:    .ASCII /HELLO, BK-0011M!/<12><0>
```

```sh
cargo run -p vm1 --example asm -- hello11.mac 1000 hello11.bin
bk --load hello11.bin
```

```
БЕЙСИК БК11/10 (C) ОКБ "ЭКСИТОН"
Ok
HELLO, BK-0011M!
```

After a key, BOS's keyboard monitor shows the address where the program
left it and its `@` prompt:

```
152144
@
```

[monitor.md](monitor.md) describes the keyboard monitor's commands.

## Debugging it

Start the BK-0010 program in the debugger (it stops before the first
instruction), list it, look at the text, step over the two `MOV`s and then
over the `EMT 20` call with `n`:

```
$ bk --bk0010 --load hello10.bin --debug
debugger: h for help, c to continue
R0 000000  R1 002422  R2 000400  R3 002422  R4 002377  R5 101010  SP 001000  PC 001000
PSW 000000 (------)  frame 150
> 001000  012701 001020        MOV    #001020, R1
bk> u 1000 5
> 001000  012701 001020        MOV    #001020, R1
  001004  012702 000020        MOV    #000020, R2
  001010  104020               EMT    020    ; PUTS
  001012  104006               EMT    006    ; KEYIN
  001014  000137 100000        JMP    @#100000
bk> m 1020 10
001020  042510 046114 026117 041040 026513 030060 030061 005041  HELLO, BK-0010!.
bk> s 2
> 001000  012701 001020        MOV    #001020, R1
> 001004  012702 000020        MOV    #000020, R2
R0 000000  R1 001020  R2 000020  R3 002422  R4 002377  R5 101010  SP 001000  PC 001010
PSW 000000 (------)  frame 150
> 001010  104020               EMT    020    ; PUTS
bk> n
R0 000000  R1 001040  R2 000000  R3 002422  R4 002377  R5 101010  SP 001000  PC 001012
PSW 000000 (------)  frame 152
> 001012  104006               EMT    006    ; KEYIN
```

After the call, the text is on the screen, R1 points past it and R2 is 0.
The comments after the EMTs are the monitor's routine names, which the
debugger reads from `annotations/` when run from the repository; your own
labels come from the assembler's symbol file
([The assembler](#the-assembler)). `c` would continue (the program then
waits for a key in the window). [emulator.md](emulator.md#the-debugger)
lists every command.

## Program files

A `.bin` file is a tape file without its name and checksum: one word with
the load address, one word with the length in bytes, then the bytes. The
monitor's tape load, `--load` and fast tape (`--files`) all use it. A
program can be any length that fits in free RAM: on the BK-0010 that is up
to 40000, where the screen starts.

Many programs are written to load below 1000, over the vectors and the
stack, so that the monitor's tape load ends by "returning" into them: they
start by themselves. The emulator's `--load` starts such files at 1000.

## Reading other people's code

- `cargo run -p romdis` disassembles every ROM, and every program in
  `software/`, into commented listings in `listings/`, with the system calls
  named (from `annotations/`). `listings/monit10.mac` is the BK-0010 monitor,
  `listings/b11m_bos.mac` BOS. Each listing assembles back into the original
  bytes with this assembler.
- `cargo run -p vm1 --example disasm -- FILE [ORIGIN] [COUNT]`
  disassembles a raw image loaded at ORIGIN (octal).
- The debugger's `u` command disassembles memory in a running machine.

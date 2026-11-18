# BASIC on the BK-0010.01 and BK-0011M

Both machines the emulator runs by default start in a BASIC held in ROM. This
guide is a programmer's manual for those two BASICs: how to type, edit, run,
save and load programs, the language itself, a reference of every statement
and function, the error codes, and the differences between the two machines.
Every example was run in the emulator; the screen output shown is what it
printed.

- [The two BASICs](#the-two-basics)
- [Getting started](#getting-started)
- [Saving and loading programs](#saving-and-loading-programs)
- [The language](#the-language)
- [Statements and commands](#statements-and-commands)
- [Functions](#functions)
- [Graphics, color and sound](#graphics-color-and-sound)
- [Memory and machine code](#memory-and-machine-code)
- [Data files](#data-files)
- [Error codes](#error-codes)
- [Differences between the two machines](#differences-between-the-two-machines)
- [Example programs](#example-programs)
- [What was not verified](#what-was-not-verified)

Addresses and other numbers written like `177662` are octal, as everywhere in
these guides; in BASIC itself octal constants are written `&O177662`.

**Sources.** The original Soviet manuals in [manuals/](manuals/), cited by
short tags:

| Tag | Manual |
|---|---|
| O10 | BK-0010.01 BASIC, operator's guide, 00001-01.34.09 |
| L10 | BK-0010.01 BASIC, language description, 00001-01.35.02 |
| O11 | BK-0011 BASIC, operator's guide, 00008-01.34.04 |
| L11 | BK-0011 BASIC, language description, 00008-01.35.04 |

"L10 §4.6.7" means section 4.6.7 of 00001-01.35.02. Where the ROM behaves
differently from a manual, this guide follows the ROM and says so.

## The two BASICs

| | BK-0010.01 (`--bk0010`) | BK-0011M (default) |
|---|---|---|
| Sign-on | `БЕЙСИК (ВИЛЬНЮС,1986.07.24)` | `БЕЙСИК БК11/10 (С) ОКБ "ЭКСИТОН"` |
| ROM | 120000–177577 (`basic10_1..3.rom`) | ROM pages at 100000 (`basic11m_0/1.rom`) |
| Free memory at start, `FRE(0)` | 14076 bytes | 14076 bytes |
| Screen | 32 columns, service line at the top | 32 columns, no service line |
| Default text color (color monitor) | red on black | white on black |

Both are the "Vilnius" BASIC (Бейсик, Вильнюс): a compiler, not an
interpreter. `RUN` first translates the whole program into threaded machine
code, allocates memory, then runs it (O10 §1, L10 §3.1.1). Syntax errors
anywhere stop the program from starting.

The BK-0011M ROM is the BK-0010.01 BASIC adapted to the BK-0011M ("БК11/10"):
it has the same keywords plus `MERGE`, the same double-precision arithmetic
and the same error numbers. **The BK-0011M manuals (O11, L11) describe a
different version** (Vilnius 1987.12.25) with `SCREEN`, `SYSTEM`, `FILES`,
`INSTR`, `VARPTR`, virtual arrays and memory-page arguments. Those features
are not in the ROM the emulator runs; see
[Differences](#differences-between-the-two-machines). The language is
therefore described here mainly from L10, and checked on both machines.

## Getting started

### The sign-on screen

Start the emulator with no options for a BK-0011M, or with `--bk0010` for a
BK-0010.01 (see [emulator.md](emulator.md)). The BK-0011M shows:

```
БЕЙСИК БК11/10 (С) ОКБ "ЭКСИТОН"
Ok
█
```

("BASIC BK11/10 (C) design bureau Eksiton".) The BK-0010.01 shows:

```
CAGLRCCC.R                   ЛАТ
- - - - - - - - - - - - - - - -
 БЕЙСИК     (ВИЛЬНЮС,1986.07.24)

Ok
█
```

The top line of the BK-0010.01 is the *service line* (служебная строка): the
first letters of the ten function keys (`COLOR`, `AUTO`, `GOTO`, `LIST`,
`RUN`, `COLOR`, `CLOAD`, `CONT`, `.`, `RUN`) and the keyboard mode, `ЛАТ`
(Latin letters) or `РУС` (Russian) (O10 §2.1). `Ok` is the prompt: BASIC is
waiting for a line. The block is the cursor.

### Direct mode and program lines

A line typed **without** a number is executed at once (direct mode,
непосредственный режим); a line **with** a number from 0 to 65535 is stored
as part of the program (indirect mode, косвенный режим) (L10 §1.1–1.2):

```
PRINT 2+2
 4
Ok
10 PRINT "HELLO"
RUN
HELLO
Ok
```

Typing a line with an existing number replaces that line; typing just the
number deletes it; lines run in order of their numbers whatever order they
were typed in (O10 §4.1). A line can be up to 255 characters, number
included, and may run over several screen lines.

**One statement per line.** These BASICs have no `:` separator:
`PRINT 1:PRINT 2` gives `ОШИБКА 2` (syntax error) on both machines. The only
way to put two statements on one line is
`IF ... THEN statement ELSE statement`.

Keywords can be typed in lower case (`print "low"` is stored as
`PRINT "low"`), spaces are optional between keywords and names
(`10 FORI=1TO3` works), and a statement keyword may be shortened to three or
more letters (L11 §6): `PRI 5`, `GOT 100` and `LIS` work on both machines;
two letters (`PR`) do not. The program is stored in a coded form, and `LIST`
rebuilds the text: `10 FORI=1TO3STEP2` lists as `10 FOR I=1TO3STEP2`, and
`&O37000` as `&O037000`.

The keyboard has no `$` sign as such: the code typed with `$` is shown as `¤`
(the KOI-7 currency sign), so `A$` appears on screen as `A¤`. This guide
writes `$`. On the BK-0010.01 the `^` (power) sign is shown as `¬`.

### Keys

The emulator's PC-key mapping is in [emulator.md](emulator.md#the-keyboard);
here is what the keys do in BASIC's line editor (O10 §3, §5.2). "Verified"
means checked in the emulator on both machines.

| BK key | PC key | Code | In BASIC |
|---|---|---|---|
| ВВОД (enter) | Enter | 012 | enter the whole line, wherever the cursor is |
| ← → ↑ ↓ | arrows | 010, 031, 032, 033 | move the cursor within the line being edited |
| ЗАБОЙ (rub out) | Backspace | 030 | delete the character left of the cursor and close up (verified) |
| \|<== (сдвижка) | F4 | 026 | delete the character under the cursor and close up (verified) |
| \|==> (раздвижка) | F5 | 027 | open a space at the cursor (verified) |
| КТ | F2 | 003 | abandon the line, print `Ok` (verified); also ends `AUTO` |
| СБР | F9 | 014 | clear the screen (verified) |
| ВС (СУ+S) | right Ctrl+S | 023 | on an empty line: bring back the last line entered; otherwise: cursor to the start of the line (verified) |
| СТОП | Esc or F12 | interrupt | stop the running program, tape operation or listing (O10 §3.2) |
| ШАГ | F8 | 200 | at the start of an empty line, run the next statement of the compiled program (O10 §3.2) |
| АР2 + 1…0 | left Ctrl + 1…0 | | function keys, see below |
| РУС, ЛАТ | End, Home | 016, 017 | Russian / Latin letters |
| ПОВТ | F1 | 201 | repeat the last key while held (O10 §3.2) |

Typing over existing text replaces it (verified); use |==> to make room
first. In headless runs, send ВС as `\023` in `--type`. On the BK-0010.01,
АР2+СБР (left Ctrl+F9) switches to the extended-memory mode described
under [CLEAR](#clear).

**Function keys** (программируемые ключи): АР2 with a digit types a stored
string. The defaults, read from both ROMs, are (`↵` is ВВОД, `<СБР>` clears
the screen):

| Key | Text | Key | Text |
|---|---|---|---|
| 1 | `COLOR ` | 6 | `COLOR 1,0↵` |
| 2 | `AUTO ` | 7 | `CLOAD"` |
| 3 | `GOTO ` | 8 | `CONT↵` |
| 4 | `LIST ` | 9 | `.↵` |
| 5 | `RUN↵` | 0 | `<СБР>RUN↵` |

Key 6 restores the standard colors if a program has left the text invisible
(L10 §4.5.2). `KEY n,"text"` changes a key; on the BK-0010.01 the service
line changes with it (`KEY 5,"PRINT X"+CHR$(10)` turns `CAGLRCCC.R` into
`CAGLPCCC.R`). Keys hold up to 16 characters (L10 §4.2.2).

### Editing a program

| Command | Effect |
|---|---|
| `LIST`, `LIST 100`, `LIST 100-200`, `LIST -200`, `LIST 100-` | show all lines, one line, a range, up to, from |
| `.100` | put line 100 on the screen for editing; `.` alone edits the *current* line |
| `AUTO`, `AUTO 100`, `AUTO 100,5` | number new lines automatically (default start 10, step 10); КТ ends it |
| `RENUM`, `RENUM new,old,step` | renumber from line *old* as *new* with *step* (default 10, 10), fixing `GOTO`s |
| `DELETE 100`, `DELETE 50-100`, `DELETE` | delete one line, a range, or everything |
| `NEW` | erase the program and variables |

The *current line* (текущая строка) is the last line entered, edited, listed
or found in error; `.` stands for it in `LIST`, `DELETE`, `AUTO` and `RENUM`
(O10 §5.4). A session on the BK-0011M (identical on the BK-0010.01):

```
AUTO
10 PRINT "A"
20 PRINT "B"
30 END
40                      ← КТ pressed here
Ok
RENUM 100,10,5
Ok
LIST
100 PRINT "A"
105 PRINT "B"
110 END
Ok
DELETE 105
Ok
LIST 100-
100 PRINT "A"
110 END
Ok
.110
110 END █               ← line ready to edit; ВВОД stores it
```

`NEW` restarts BASIC: the screen is cleared and the sign-on is printed again.
`AUTO` shows an existing line when it reaches its number, so it can be used
to step through a program for editing (O10 §5.5).

### Running and stopping

`RUN` runs the program from the start, `RUN 100` from line 100. `GOTO 100`
typed in direct mode continues the already compiled program at line 100, the
way `CONT` does; after the program has been changed it gives error 17, so use
`RUN 100` then. A program ends at `END`, at its last line, at `STOP`, on an
error, or when СТОП is pressed. `STOP` and
the СТОП key print `СТОП В СТРОКЕ n` ("stop in line n"); you can then look
at or change variables in direct mode and type `CONT` to go on:

```
10 A=10
20 STOP
30 A=20
40 PRINT A
RUN
СТОП В СТРОКЕ 20
Ok
PRINT A
 10
Ok
CONT
 20
Ok
```

Changing the program makes `CONT` impossible (L10 §3.1.2). `TRON` prints
each line number as it runs, `TROFF` turns that off:

```
TRON
Ok
RUN
[10 ][20 ]
СТОП В СТРОКЕ 20
Ok
CONT
[30 ][40 ] 20
Ok
```

If a program sets the text color equal to the background, the screen looks
dead: press СТОП, then АР2+6 (key 6, `COLOR 1,0`) (L10 §4.5.2).

## Saving and loading programs

### Files

Programs and data are kept on cassette tape. A file name is up to 6
characters; BASIC adds a type (L10 §3.2, O10 §6):

| Type | Written by | Read by | Contents |
|---|---|---|---|
| `ASC` | `SAVE` | `LOAD`, `MERGE` (BK-0011M), `OPEN ... FOR INPUT` | program text in character codes, 256-byte blocks ending with code 032 |
| `COD` | `CSAVE` | `CLOAD` | program in BASIC's internal form (loads faster; version-specific) |
| `BIN` | `BSAVE` | `BLOAD` | a block of memory with its start address and length |
| `DAT` | `OPEN ... FOR OUTPUT` | `OPEN ... FOR INPUT` | data file, same format as `ASC` |

| Command | Does |
|---|---|
| `SAVE "NAME"` | write the program as text (`ASC`) |
| `LOAD "NAME"`, `LOAD "NAME",R` | read a text program; `,R` runs it after loading |
| `CSAVE "NAME"` / `CLOAD "NAME"` | write / read the internal form (`COD`) |
| `MERGE "NAME"` | BK-0011M only: add an `ASC` program to the one in memory; loaded lines replace lines with the same numbers |
| `BSAVE "NAME",start,end` | write memory from *start* up to (not including) *end* |
| `BLOAD "NAME"`, `BLOAD "NAME",R`, `BLOAD "NAME",addr` | read a `BIN` file to its own address, run it, or load it at *addr* |
| `FIND "NAME"`, `FIND` | read the tape up to the end of file NAME, listing the files passed; with no name, list every file. Memory is unchanged (L10 §3.2.6) |

While a file loads, BASIC prints its name, type and block number:

```
LOAD "HELLO"
HELLO .ASC   000
Ok
```

Points checked in the emulator:

- `CLOAD` needs an empty program: with any line in memory it stops at once
  with `ОШИБКА 5`. Type `NEW` first.
- `BSAVE "MEM",&O1000,&O1100` writes the 100 (octal) bytes 1000–1077.
- After `BLOAD`, `PEEK(&O264)` on the BK-0010.01 gives the load address
  (L10 §4.2.7.1), which is handy for `DEF USR`.
- `LOAD "NAME",R` works on the BK-0010.01 (L10 Appendix 5 says it does not).
- All tape commands can be interrupted with СТОП; the keyboard does not work
  while the tape runs.

### With a real tape recorder

Connect the recorder to the BK's tape socket. To save, start recording, then
type the `SAVE`/`CSAVE`/`BSAVE` command. To load, type the `LOAD`/`CLOAD`/
`BLOAD` command and press play; BASIC skips other files (printing their
names) until it finds the one asked for. `FIND` can be used to wind on to
the end of a file before recording the next one (L10 §3.2.6).

### In the emulator

The emulator offers two kinds of tape (details in
[emulator.md](emulator.md#loading-software)):

**WAV recordings** work on both machines exactly as on a real BK:

```sh
bk --record mytape.wav        # everything written to tape is saved on exit
bk --tape mytape.wav          # the recording plays while BASIC reads the tape
```

Checked: `SAVE`/`LOAD`, `MERGE` and data files on the BK-0011M,
`CSAVE`/`CLOAD`, `BSAVE`/`BLOAD` on the BK-0010.01, each written in one run
with `--record` and read back in another with `--tape`. A recording plays
from the start, so read files in the order they were written.

**Fast tape** (`--files DIR`) replaces the tape with files in a host
directory: each tape file becomes `DIR/NAME.bin`, where NAME is its
16-character tape name without the spaces. On the **BK-0010.01** it works
for all BASIC tape commands:

```sh
mkdir tape
bk --bk0010 --files tape
```

After `SAVE "HELLO"`, `CSAVE "HELLOC"`, `BSAVE "MEM",&O1000,&O1100` and a
data file `NUMS`, the directory holds:

```
HELLO.ASC#000.bin       first 256-byte block of the text program
HELLO.ASC.bin           its closing record
HELLOC.COD.bin
MEM.BIN.bin
NUMS.DAT#000.bin
NUMS.DAT.bin
```

and `LOAD "HELLO"`, `CLOAD "HELLOC"`, `BLOAD "MEM"` and
`OPEN "NUMS" FOR INPUT` read them back instantly. (On tape BASIC pads names
before the type, `NUMS  .DAT`; the host files leave the spaces out, and
names match either way.) One limit: `FIND` waits for a real tape signal.

On the **BK-0011M** fast tape works the same way (`SAVE`/`LOAD`,
`CSAVE`/`CLOAD` and `BSAVE`/`BLOAD` checked).

### Leaving BASIC

`MONIT` leaves BASIC for the system monitor; the program in memory is lost
(O10 §2.2). On the BK-0010.01 the monitor prompt is `?`; type `BASIC` and
ВВОД to start BASIC again. On the BK-0011M `MONIT` enters the BOS monitor
(prompt `@`); type `11;1C` ВВОД and then `100000G` ВВОД to return (O11 §2.2).
Both verified. See [monitor.md](monitor.md) for the monitors.

## The language

### Numbers

There are three numeric types (L10 §2.1.2, §2.2.1):

| Type | Suffix | Storage | Range / precision |
|---|---|---|---|
| integer | `%` | 1 word | −32768 … 32767 |
| single precision | `!` | 2 words | about 7 digits |
| double precision | `#` or none | 4 words | about 17 digits; the default |

The real types use the PDP-11 floating-point format; the largest value is
about 1.7E38 (`PRINT 1.7E38` works, `PRINT 1E38*10` gives error 6). Numbers
below the smallest value become 0 without an error (L10 Appendix 1).

Numeric constants (L10 §2.1.1):

| Form | Examples | Type |
|---|---|---|
| decimal integer | `123`, `-2345`, `156%` | double, or integer with `%` |
| with a point | `2.45`, `-102.36`, `23.567!`, `348.9#` | double unless `!` |
| exponent `E` | `2359E6`, `-1.09E-09` | single |
| exponent `D` | `1.5D3`, `-1.09432D-09` | double |
| hexadecimal | `&H1A`, `&HFFFF` | integer |
| octal | `&O17`, `&O177700` | integer |
| binary | `&B101` | integer |

```
?&H1A,&O17
 26              15
?23.567!,1.5D3
 23.567          1500
?1/3!,1/3#
 .33333333333333334
 .33333333333333334
?2359E6
 2.359E+09
?.0001,.00001
 1D-04           1D-05
?1234567890123456789
 1.2345678901234568D+18
```

`PRINT` puts a space before a positive number (where the minus sign would go)
and one after every number. Small double-precision values print with a `D`
exponent, single-precision ones with `E`.

A computation is done in the most precise type among its operands (L10
§2.3.6): `1/3!` is double because `1` is. Assigning to a variable converts to
the variable's type; conversion to integer drops the fraction (`A%=23.42`
gives 23), and a value outside −32768…32767 gives error 6 (`A%=40000`).

### Strings

A string constant is text in double quotes, up to 255 characters; spaces
inside count (L10 §2.1.3). Strings are joined with `+` and compared with the
relational operators character by character, a shorter string being smaller
when it is a prefix of the longer one (L10 §2.3.7):

```
?"AA"<"AB","CL ">"CL"
-1              -1
```

Strings live in a separate *string space*, 200 bytes by default; `CLEAR n`
sets its size. Running out gives error 14; a string longer than 255
characters gives error 15.

**A pitfall: string expressions that start with a string variable.** On both
machines, an assignment whose expression begins with a string variable that
holds a computed string can produce garbage:

```
10 D$="A"
20 D$=D$+"B"
30 D$=D$+"C"
40 PRINT D$
RUN
BBC
Ok
```

The correct result is `ABC`. Tracing the ROM in the emulator's debugger shows
why: the compiled code takes a copy of `D$`'s address, then compacts the
string space (moving `D$` one byte down and correctly updating the variable),
then copies from the stale address. It is the ROM's own bug, not the
emulator's: the same program, run from the same machine state on the MiST
FPGA core's chip-level model of the processor, leaves the same bytes in
memory. The fix is to start the expression with a constant, which the
compaction does not move:

```
30 D$=""+D$+"C"
```

This gives `ABC`. Putting new text in front (`S$=MID$(W$,I,1)+S$`) also
worked in a test where `W$` was set from a constant, but `""+` at the front
is the safe habit.

### Variables

A variable name starts with a Latin letter and may continue with letters and
digits; **only the first two characters count**, so `ABC` and `ABD` are the
same variable (verified). A suffix gives the type: `$` string, `%` integer,
`!` single, `#` or nothing double (L10 §2.2.1). `A`, `A%`, `A!` and `A$` are
different variables. Keywords cannot be used as names (L10 Appendix 4).
Numeric variables start as 0, strings as empty.

### Arrays

`DIM` declares arrays with one or more subscripts (L10's example has three),
each from 0 up to the given maximum, at most 255:
`DIM X(10),Z(5,20,3),A$(30),I%(100)`. An array used without `DIM` gets one
subscript from 0 to 10. Declaring an array
twice, or after it was created implicitly, gives error 10; a subscript out of
range gives error 9; a negative one gives error 5. Arrays cannot be created
implicitly in direct mode (error 12) (L10 §4.2.1).

```
10 DIM Z(2,3)
20 Z(2,3)=7
30 PRINT Z(2,3);Q(10)
40 PRINT Q(11)
RUN
 7  0
ОШИБКА 9  В СТРОКЕ 40
Ok
```

### Operators and precedence

From highest to lowest priority (L10 §2.3):

| Priority | Operators | Meaning |
|---|---|---|
| 1 | `( )` | grouping |
| 2 | `^` | power |
| 3 | `*` `/` | multiply, divide |
| 4 | `\` | integer division |
| 5 | `MOD` | remainder |
| 6 | `+` `-` | add, subtract; `+` also joins strings |
| 7 | `=` `<>` `><` `<` `>` `<=` `=<` `>=` `=>` | comparisons: −1 if true, 0 if false |
| 8 | `NOT` | bitwise complement |
| 9 | `AND` | bitwise and |
| 10 | `OR` | bitwise or |
| 11 | `XOR`, `EQV` | exclusive or, equivalence |
| 12 | `IMP` | implication |

Unary minus binds less tightly than `^`: `-2^2` is −4. The logical operators
work on the 16 bits of integers; any non-zero value counts as true in `IF`.

```
?10\4,10.4 MOD 4
 2               2
?63 AND 16,4 OR 2
 16              6
?NOT 0,5 XOR 3
-1               6
?5 EQV 3,5 IMP 3
-7              -5
?7/2,-7\2
 3.5            -3
```

`^` is computed through logarithms: `2^10` prints `1023.9999999999833` and
`2^.5` `1.4142135623730945`. A base of 0 or less gives error 5 (`(-2)^2`,
`0^2`), as L10 Appendix 5 says; multiply instead (`X*X`).

## Statements and commands

Statements can be used in a program and in direct mode unless noted.
"Commands" (`LIST`, `RUN`, `SAVE`…) are meant for direct mode. Only one
statement fits on a line. `n` is a line number, `expr` an expression.
Sections are those of L10 unless noted.

| Statement | Syntax | Meaning | Example |
|---|---|---|---|
| `'` | `statement ' text` | comment after a statement (§4.1.12) | `200 X=1 ' LOOP` |
| `.` | `.n` | edit line *n*; `.` alone edits the current line (§3.3.5) | `.40` |
| `AUTO` | `AUTO [start][,step]` | automatic line numbers, default 10,10; end with КТ or СТОП (§3.3.4) | `AUTO 100,5` |
| `BEEP` | `BEEP` | short beep, the same as on an error (§4.7.1). Takes no arguments (L11 §9.3.1 describes `BEEP length,pitch`; here that is error 2) | `BEEP` |
| `BLOAD` | `BLOAD "name"[,R][,addr]` | load a `BIN` file; `R` runs it from its start; *addr* loads it elsewhere (§3.2.7) | `BLOAD "PROG",R` |
| `BSAVE` | `BSAVE "name",start,end` | save memory *start*…*end*−1 (§3.2.8) | `BSAVE "PROG",&O10000,&O20000` |
| `CALL`, `_` | `CALL name[(params)]` | call a program in a plug-in ROM cartridge (§4.1.11). With none fitted: error 5 on the BK-0010.01, nothing on the BK-0011M | `CALL GAME` |
| `CIRCLE` | `CIRCLE [@](x,y),r[,c[,start[,end[,aspect]]]]` | circle, arc or ellipse, see [Graphics](#graphics-color-and-sound) (§4.6.5) | `CIRCLE (100,100),50,,,,.5` |
| `CLEAR` | `CLEAR [strings[,top]]` | clear all variables, close files, set the string space size and the top of BASIC's memory (§4.2.3), see below | `CLEAR 1000,&O30000` |
| `CLOAD` | `CLOAD "name"` | load a `COD` program; the program memory must be empty (§3.2.4) | `CLOAD "GAME"` |
| `CLOSE` | `CLOSE` | close the data file (§4.3.2) | `CLOSE` |
| `CLS` | `CLS` | clear the screen to the background color (§4.5.1) | `CLS` |
| `COLOR` | `COLOR [fg][,bg]` | set text/graphics color and background, 0–4 (§4.5.2), see [Graphics](#graphics-color-and-sound) | `COLOR 2,0` |
| `CONT` | `CONT` | continue after `STOP` or СТОП (§3.1.2) | `CONT` |
| `CSAVE` | `CSAVE "name"` | save the program in internal form (§3.2.5) | `CSAVE "GAME"` |
| `DATA` | `DATA item[,item...]` | constants for `READ`: numbers in any form, strings with or without quotes (quotes needed for commas) (§4.2.4) | `DATA 1,&O22,TEXT,"A,B"` |
| `DEF FN` | `DEF FNname[(args)]=expr` | define a one-line function; program only, error 12 in direct mode (§4.2.7.2) | `DEF FNH(X,Y)=SQR(X*X+Y*Y)` |
| `DEF USR` | `DEF USR[d]=addr` | set the address of machine-code function `USRd`, d = 0–9 (§4.2.7.1) | `DEF USR=&O37000` |
| `DELETE` | `DELETE [n][-[m]]` | delete lines; no argument deletes everything; a missing line number is error 8 (§3.3.2) | `DELETE 50-100` |
| `DIM` | `DIM name(max[,max...])[,...]` | declare arrays (§4.2.1) | `DIM A(20),B$(5)` |
| `DRAW` | `DRAW string` | run graphics commands, see [DRAW](#draw) (§4.6.7) | `DRAW "R20D20L20U20"` |
| `END` | `END` | end the program, close files (§4.1.10) | `END` |
| `FIND` | `FIND ["name"]` | read the tape to the end of a file, listing names (§3.2.6) | `FIND "GAME"` |
| `FOR` | `FOR v=a TO b [STEP s]` | loop; the body runs at least once (`FOR I=5 TO 1` runs it with I=5) (§4.1.6) | `FOR I=10 TO 1 STEP -1` |
| `GOSUB` | `GOSUB n` | call a subroutine (§4.1.3) | `GOSUB 1000` |
| `GOTO` | `GOTO n` | jump (§4.1.2); in direct mode it continues the compiled program at line *n* (error 17 if the program was changed) | `GOTO 10` |
| `IF` | `IF expr THEN stmt\|n [ELSE stmt\|n]`, `IF expr GOTO n [ELSE ...]` | conditional; non-zero is true (§4.1.5) | `IF A<B THEN PRINT "Y" ELSE PRINT "N"` |
| `INPUT` | `INPUT ["prompt";]var[,var...]` | read values from the keyboard (§4.3.4), see below | `INPUT "NAME";N$` |
| `INPUT#` | `INPUT# var[,var...]` | read from the open data file (§4.3.4) | `INPUT# A$` |
| `KEY` | `KEY k,string` | set function key *k* (1–10) (§4.2.2) | `KEY 3,"PRINT PI"+CHR$(10)` |
| `LET` | `[LET] var=expr`, `MID$(v$,p[,n])=s$` | assignment; `LET` is optional. The `MID$` form overwrites characters of `v$` in place (§4.1.1, §5.4.4) | `MID$(B$,7)=MID$(A$,1,4)` |
| `LINE` | `LINE [[@](x1,y1)]-[@](x2,y2)[,c[,B\|BF]]` | line, box (`B`) or filled box (`BF`, BK-0011M only) (§4.6.4) | `LINE (10,10)-(100,50),,B` |
| `LIST` | `LIST [n][-[m]]` | list the program; `.` = current line (§3.3.1) | `LIST 100-200` |
| `LLIST` | `LLIST [n][-[m]]` | list on the printer (§3.3.1) | `LLIST` |
| `LOAD` | `LOAD "name"[,R]` | load an `ASC` program, `R` = run it (§3.2.1) | `LOAD "MOD2",R` |
| `LOCATE` | `LOCATE [col][,row][,cursor]` | move the text cursor; *cursor* 0 hides it, non-zero shows it (§4.5.3) | `LOCATE 0,22` |
| `LPRINT` | `LPRINT list` | print on the printer (§4.3.3); in the emulator, `--printer FILE` adds what is printed to FILE | `LPRINT A` |
| `MERGE` | `MERGE "name"` | BK-0011M only: merge an `ASC` program from tape (§3.2.3, L11 §11.3) | `MERGE "PART2"` |
| `MONIT` | `MONIT` | go to the system monitor; the program is lost (§3.1.4) | `MONIT` |
| `NEW` | `NEW` | erase program and variables; BASIC restarts (§3.3.7) | `NEW` |
| `NEXT` | `NEXT [v[,v...]]` | end of a `FOR` loop; several variables close nested loops, innermost first (§4.1.7) | `NEXT J,I` |
| `ON` | `ON expr GOTO\|GOSUB n1,n2,...` | jump to the *expr*-th line; 0 or too large: next statement; negative: error 5 (§4.1.8) | `ON K GOSUB 100,200,300` |
| `OPEN` | `OPEN "name" FOR INPUT\|OUTPUT` | open a tape data file, default type `DAT` (§4.3.1) | `OPEN "NUMS" FOR OUTPUT` |
| `OUT` | `OUT addr,mask,code` | set (code ≠ 0) or clear (code = 0) the bits of *mask* in the word at *addr* (§4.4.2) | `OUT &O37000,&O7,0` |
| `PAINT` | `PAINT [@](x,y)[,c[,border]]` | flood fill (§4.6.6) | `PAINT (180,100),1,3` |
| `POKE` | `POKE addr,value` | store a word; an odd address means the word containing that byte (§4.4.1) | `POKE &O37000,&O6315` |
| `PRESET` | `PRESET [@](x,y)[,c]` | set a dot to the background color, or to *c* (§4.6.2) | `PRESET (10,10)` |
| `PRINT`, `?` | `PRINT [list]` | print, see below (§4.3.3) | `? "A";B,C` |
| `PRINT#` | `PRINT# list` | write to the open data file (§4.3.3) | `PRINT# X;` |
| `PSET` | `PSET [@](x,y)[,c]` | set a dot (§4.6.1) | `PSET (128,120),2` |
| `READ` | `READ var[,var...]` | read the next `DATA` items; past the end: error 4 (§4.2.5) | `READ A,B$` |
| `REM` | `REM text` | comment (§4.1.12) | `10 REM START` |
| `RENUM` | `RENUM [new][,[old][,step]]` | renumber from line *old* (default: the first) as *new* with *step*, updating `GOTO`s; defaults 10 and 10 (as L11 §2.3 says; L10 §3.3.3 says 1) | `RENUM 100,10,5` |
| `RESTORE` | `RESTORE [n]` | read `DATA` again from the start, or from line *n* (§4.2.6) | `RESTORE` |
| `RETURN` | `RETURN [n]` | return from `GOSUB`; with *n*, return to line *n* instead (L11 §4.4; works on both machines) | `RETURN 40` |
| `RUN` | `RUN [n]` | compile and run, from line *n* (§3.1.1) | `RUN` |
| `SAVE` | `SAVE "name"` | save the program as text (§3.2.2) | `SAVE "MYPROG"` |
| `STOP` | `STOP` | pause with `СТОП В СТРОКЕ n` (§4.1.9) | `STOP` |
| `TROFF` | `TROFF` | trace off (§4.8.2) | `TROFF` |
| `TRON` | `TRON` | print `[n ]` for each line run (§4.8.1) | `TRON` |

### PRINT

Items are separated by `;` (no space) or `,` (move to the next zone). On
the screen the zones are **16 columns** wide (L10 §4.3.3 says 14):
`PRINT 1,2,3` puts 2 in column 16 and 3 on the next line. A final `;` or
`,` keeps the cursor on the line. Inside `PRINT`:

- `AT(col,row)` moves to column 0–31, row 0–23 before printing the rest
  (L10 §5.6.1);
- `TAB(n)` moves right to column *n* (§5.6.2).

`SPC(n)` (L10 §5.6.3) is **not** in either ROM, as L10 Appendix 5 says: BASIC
reads `SPC(3)` as element 3 of an array `SP`, so `PRINT "A";SPC(3);"B"`
prints `A 0 B` in a program and gives error 12 in direct mode.

```
10 PRINT AT(10,12);"AT";CSRLIN;POS(0)
RUN
```

prints `AT 12  16` starting at column 10 of row 12.

### INPUT

`INPUT` prints the prompt (if any) and `?`, then waits for values separated
by commas; strings need quotes only if they contain commas. The prompt must
be followed by `;` (with a comma: error 2). If too few values are typed BASIC
asks for more with `??`; extra values are ignored; a non-number for a
numeric variable gives error 13 and the question is asked again (L10
§4.3.4).

```
10 INPUT "NAME";N$,A
20 PRINT N$;A*2
RUN
NAME?BK,21
BK 42
Ok
```

### CLEAR

`CLEAR` clears all variables, arrays and `DEF FN` functions, closes files,
and keeps the `DEF USR` addresses. `CLEAR n` makes the string space *n*
bytes (default 200). `CLEAR n,top` also makes *top* the upper limit of
BASIC's memory; memory from *top* up is free for machine code or data
(§4.2.3). Normally BASIC's memory ends at `&O40000`, where the BK-0010.01
screen begins. Values up to `&O70000` are accepted (error 5 above), but a
*top* above `&O40000` lets BASIC use screen memory, so it is meant for the
BK-0010.01 *extended memory mode*. АР2+СБР, or `PRINT CHR$(&O214);`,
shrinks the screen to its top four lines and frees 28 KB (L10 Appendix 5),
after which `CLEAR 200,&O70000` gives `FRE(0)` = 26364 (verified). On the
BK-0011M `CHR$(&O214)` is just a character.

`CLEAR` with two arguments inside a program discards the compiled code and
returns to direct mode (§4.2.3), so set *top* in direct mode before `RUN`.

## Functions

| Function | Result | Example → result (verified) |
|---|---|---|
| `ABS(x)` | absolute value | `ABS(-2.5)` → 2.5 |
| `ASC(s$)` | code of the first character; empty string: error 5 | `ASC("A")` → 65 |
| `AT(col,row)` | inside `PRINT` only: position the cursor | `PRINT AT(5,3);"X"` |
| `ATN(x)` | arctangent, radians | `ATN(1)*4` → 3.1415926535897932 |
| `BIN$(n)` | binary digits of an integer | `BIN$(44)` → `101100` |
| `CDBL(x)` | convert to double | `CDBL(2/3)` → .66666666666666667 |
| `CHR$(n)` | character with code *n* (0–255) | `CHR$(65)` → `A` |
| `CINT(x)` | convert to integer, dropping the fraction | `CINT(-6.7)` → −6 |
| `COS(x)` | cosine, radians | `COS(PI)` → −1 |
| `CSNG(x)` | convert to single | `CSNG(PI)` → 3.141593 |
| `CSRLIN[(x)]` | row of the cursor, from 0 | |
| `EOF` | −1 when the data file is at its end, else 0; no file open: error 52 | `IF EOF THEN 120` |
| `EXP(x)` | e to the power *x* | `EXP(1)` → 2.7182818284590396 |
| `FIX(x)` | integer part, toward zero | `FIX(-5.3)` → −5 |
| `FNname(args)` | call a `DEF FN` function | `FNH(3,4)` → 5 |
| `FRE[(x)]` | free bytes for the program; with a string argument, free string space | `FRE(0)` → 14076, `FRE("")` → 200 at start |
| `HEX$(n)` | hexadecimal digits | `HEX$(43)` → `2B`, `HEX$(-1)` → `FFFF` |
| `INKEY$` | the key pressed, or `""`; does not wait or echo | see below |
| `INP(addr,mask)` | the bits of *mask* from the word at *addr* | `INP(A%,&O17)` |
| `INT(x)` | largest integer not above *x* | `INT(-5.3)` → −6 |
| `LEN(s$)` | length | `LEN("BK-0011")` → 7 |
| `LOG(x)` | natural logarithm; *x* ≤ 0: error 5 | `LOG(10)` → 2.3025850929940457 |
| `LPOS[(x)]` | printer head position, from 1 | `LPOS(0)` → 1 |
| `MID$(s$,p[,n])` | *n* characters from position *p*; also assignable, see `LET` | `MID$("ABCDEF",2,3)` → `BCD` |
| `OCT$(n)` | octal digits | `OCT$(-1)` → `177777` |
| `PEEK(addr)` | the word at *addr* (odd → its word); −32768…−1 mean 100000–177777 | `PEEK(&O264)` |
| `PI` | π | 3.1415926535897932 |
| `POINT(x,y)` | color of a dot, see [Graphics](#graphics-color-and-sound) | |
| `POS[(x)]` | column of the cursor, from 0 | |
| `RND(x)` | random number 0…1: *x* > 0 next number, *x* < 0 restart the sequence for that seed, *x* = 0 repeat the last | `RND(-5)` → .53368281796108777 |
| `SGN(x)` | −1, 0 or 1 | `SGN(-3)` → −1 |
| `SIN(x)` | sine, radians | `SIN(1)` → .8414709848078965 |
| `SQR(x)` | square root; *x* < 0: error 5 | `SQR(2)` → 1.4142135623730951 |
| `STR$(x)` | the number as `PRINT` writes it, with the leading and the trailing space | `"["+STR$(5)+"]"` → `[ 5 ]` |
| `STRING$(n,c)`, `STRING$(n,s$)` | *n* copies of the character with code *c*, or of the first character of *s$* | `STRING$(5,"*")` → `*****` |
| `TAB(n)` | inside `PRINT` only: move to column *n* | `PRINT "A";TAB(5);"B"` |
| `TAN(x)` | tangent, radians | `TAN(1)` → 1.5574077246549022 |
| `USR[d](x)` | call machine-code function *d*, see [Memory and machine code](#memory-and-machine-code) | `USR(21%)` |
| `VAL(s$)` | the number at the start of *s$*, 0 if none | `VAL("23A12")` → 23 |

All numeric functions return double precision (L10 §5.1). `STR$` keeps the
trailing space, unlike what L10 §5.4.8 says.

`INKEY$` returns at most one key per call and does not wait, which makes it
the way to read the keyboard in games (L10 §5.4.7):

```
10 K$=INKEY$
20 IF K$="" THEN 10
30 PRINT "KEY";ASC(K$)
RUN
KEY 81                  ← Q was pressed
Ok
```

## Graphics, color and sound

### Coordinates

Graphics use the 256 × 256-dot color screen; `x` runs left to right,
`y` top to bottom. Points outside the screen are simply not drawn (L10
§4.6). `@(x,y)` or `STEP(x,y)` gives coordinates relative to the last point
drawn. Text and graphics share the screen.

- **BK-0010.01**: x 0–255, y 0–240; y = 0 is just below the service line.
- **BK-0011M**: x 0–255, y 0–255; y = 0 is the top line of the screen
  (verified by drawing at y = 250, which shows only on the BK-0011M).

### Colors

`COLOR fg,bg`, and the color argument of the graphics statements, take
numbers 0–4. 0 means "the background" (L10 §4.5.2). The numbers select the
same pixel codes on both machines, but they look different:

| Number | Pixel code | BK-0010.01 | BK-0011M (palette 15, at start) |
|---|---|---|---|
| 1 | 3 | red | white |
| 2 | 2 | green | green |
| 3 | 1 | blue | cyan |
| 4 | 0 | black | black |

(L11 §9.1.3 lists 1 black, 2 blue, 3 green, 4 red for its other version; this
ROM does not follow it.)

Three things checked in the emulator that the manuals don't make clear:

- **Changing the background recolors the whole screen at once.**
  `COLOR 1,3` on a black BK-0010.01 screen turns the whole screen blue and
  the red text already there green: every pixel is changed, not just the
  empty ones. Text in color 4 (black) is visible on a non-black background.
- **On the BK-0011M a color given to `PSET`, `LINE`, `CIRCLE` and so on also
  becomes the current text color**; after `LINE ...,4` the next `PRINT` and
  the `Ok` prompt are black. On the BK-0010.01 the text color stays as
  `COLOR` set it.
- **On the BK-0011M the palette can be changed with `POKE`.** Bits 8–11 of
  register 177662 select one of 16 palettes
  ([hardware-reference.md](hardware-reference.md));
  `POKE &O177662,&O40000+256*P` selects palette *P* (bit 14 stays set as at
  start). `POKE &O177662,&O40000` (palette 0) gives the BK-0010 colors red,
  green, blue.

A monochrome monitor (`--mono`) shows color 1 as solid white, colors 2 and
3 as patterns of dots and 4 as black.

### Drawing statements

```
PSET [@](x,y)[,c]                   a dot
PRESET [@](x,y)[,c]                 a dot in the background color (or c)
LINE [[@](x1,y1)]-[@](x2,y2)[,c[,B|BF]]
CIRCLE [@](x,y),r[,c[,start[,end[,aspect]]]]
PAINT [@](x,y)[,c[,border]]
POINT(x,y)
```

- `LINE` without the first point starts at the last point drawn. `B` draws a
  box with the two points as opposite corners; `BF` a filled box, **on the
  BK-0011M only** (BK-0010.01: error 2, as L10 Appendix 5 says).
- `CIRCLE` angles are in radians, counted anticlockwise from the right;
  `CIRCLE (100,150),30,,0,3.14` draws the top half. A negative angle also
  draws a radius to that end (`CIRCLE (200,60),20,,-1,-3` draws a wedge).
  *aspect* is height/width: `,,,,.5` draws an ellipse twice as wide as high.
  Leave out arguments with commas (L10 §1.4). The BK-0011M draws circles
  with sparser dots.
- `PAINT` fills the area around (x,y) up to a border of color *border* (or of
  the fill color). The border must be closed. On the BK-0010.01 filling a
  circle of radius 30 takes several seconds; the BK-0011M is much faster.
- `POINT` differs between the machines. The BK-0010.01 returns the color
  number 1–4 (empty screen: 4) and −1 off the screen, as L10 §4.6.3 says. The
  BK-0011M returns the pixel code 0–3 (color 1 gives 3, color 3 gives 1,
  empty screen 0) and 0 off the screen.
- Color numbers outside 0–4 give error 5.

### DRAW

`DRAW s$` executes a string of one-letter commands (L10 §4.6.7). A number
after a command is its argument; `=V;` takes the value of numeric variable
`V` instead. Spaces and `;` may separate commands.

| Command | Action |
|---|---|
| `U n`, `D n`, `L n`, `R n` | draw up, down, left, right *n* dots |
| `E n`, `F n`, `G n`, `H n` | draw diagonally: up-right, down-right, down-left, up-left |
| `M x,y` | draw to (x,y); `M+x,y` or `M-x,y` is relative |
| `B` prefix | move without drawing (`BM20,150`) |
| `N` prefix | draw, then return to the starting point (`NU20`) |
| `A n` | rotate the following commands by n × 90° |
| `S n` | scale: lengths × n/4; default 4 (`S8` doubles) |
| `C n` | color 0–4 |
| `X v$;` | execute the commands in string variable `v$` |

`A`, `S` and `C` stay in force for later `DRAW` statements. In the emulator
`A1` turned `R` into an upward stroke, i.e. anticlockwise on the screen (L10
says clockwise). Bad commands give error 5.

```
10 A$="R20D20L20U20"
20 DRAW "BM40,60XA$;BM80,60S8XA$;S4"
30 DRAW "BM40,150NU20NR20ND20NL20"
```

draws a 20-dot square at (40,60), a 40-dot square at (80,60), and a cross
centered on (40,150).

### Sound

`BEEP` gives the short beep also heard on errors. `PRINT CHR$(7);` does the
same (L10 §5.4.2). There is no other sound statement; machine code can drive
the speaker (see [system-programming.md](system-programming.md)).

## Memory and machine code

BASIC's program, variables and strings sit in RAM below `&O40000`. `PEEK`,
`POKE`, `INP` and `OUT` reach any address, including device registers (L10
§4.4, §5.3); misuse can crash BASIC. Addresses above 32767 can be written as
octal constants (`&O177660`) or as negative numbers. For the memory map see
[machine.md](machine.md) and [system-programming.md](system-programming.md).

To call machine code (L10 §4.2.7.1, §5.7.2):

1. Reserve memory: in direct mode, `CLEAR 200,&O37000` makes 37000–37777
   free (or load the code above BASIC's top with `BLOAD`).
2. Put the code there with `POKE` or `BLOAD`.
3. `DEF USRd=address` (d = 0–9, default 0). The addresses are kept in a table
   at `&O2100` on the BK-0010.01.
4. `X=USRd(arg)` calls the code with `JSR PC`. On entry R5 points to the
   argument's value and R3 gives its type: bit 15 set for a string
   (R5 → length word, address word); otherwise the low byte is −1 integer,
   0 double, 1 single (L10 §5.7.2; only the integer case was tried). Put the
   result where R5 points, of the same type, and return with `RTS PC`. If you
   change nothing, the result is the argument.

Calling a number with no `DEF USR` gives error 5. Example: double an integer
with `ASL (R5)` (006315) and `RTS PC` (000207):

```
CLEAR 200,&O37000
Ok
10 POKE &O37000,&O6315
20 POKE &O37002,&O207
30 DEF USR=&O37000
40 PRINT USR(21%)
RUN
 42
Ok
```

The argument must be an integer (`21%`) for this code to work, since it
doubles one word. The CPU and instruction set are described in
[machine-code.md](machine-code.md).

`OUT` and `INP` work on selected bits: after `POKE &O37000,&O6315`,
`INP(&O37000,&O17)` is 13 (octal 15) and `OUT &O37000,&O7,0` leaves 6310.

## Data files

A program can write and read text data files on tape (L10 §4.3.1–4.3.4,
§5.5.4). Only one file is open at a time.

```
10 OPEN "NUMS" FOR OUTPUT
20 FOR I=1 TO 3
30 PRINT# I*I;
40 NEXT
50 PRINT# "END"
60 CLOSE
70 OPEN "NUMS" FOR INPUT
80 IF EOF THEN 120
90 INPUT# A$
100 PRINT "[";A$;"]"
110 GOTO 80
120 CLOSE
RUN
[1  4  9 END]
Ok
```

(BK-0010.01 with `--files`; on the BK-0011M the same test passes with
`--record` for the writing part and `--tape` for the reading part.)
`PRINT#` writes text just as `PRINT` would show it; `INPUT#` reads it back
like keyboard input, here as one string. On tape the file is called
`NUMS  .DAT` (the name is padded to 6 characters). `EOF` is true (−1) when
the next read would hit the end (L10 §5.5.4 garbles this). Reading past
the end gives error 55; opening a file twice gives error 54; `END`,
`CLEAR`, `LOAD`, `NEW` and editing the program close the file. `OPEN` names are at most 6 characters plus an optional type
(`"DATA.TXT"`); default type `DAT`.

## Error codes

Errors print `ОШИБКА n` ("error n") in direct mode and
`ОШИБКА n  В СТРОКЕ m` ("error n in line m") in a program; the line in
error becomes the current line, so `.` ВВОД brings it up for editing
(O10 §7). Both machines print
numbers (the text messages of L11 belong to its other version). Names are
from O10/L10; "seen with" lists what produced the error in the emulator.

| Code | Name | Meaning | Seen with |
|---|---|---|---|
| 1 | NEXT без FOR | `NEXT` without a matching `FOR` | `10 NEXT` |
| 2 | синтаксическая ошибка | syntax error | `PRINT 1:PRINT 2`, unknown keyword |
| 3 | RETURN без GOSUB | `RETURN` without `GOSUB` | `10 RETURN` |
| 4 | исчерпан список DATA | `READ` past the last `DATA` | `10 READ A` with no `DATA` |
| 5 | недопустимое использование функции | bad argument or value | `LOG(0)`, `SQR(-1)`, `(-2)^2`, `ON -1 GOTO`, undefined `USR`, `CLOAD` with a program present |
| 6 | переполнение | number too large | `1E38*10`, `A%=40000` |
| 7 | переполнение памяти | out of memory (program, `FOR`/`GOSUB` nesting, variables) | `NEXT` or `RETURN` typed in direct mode |
| 8 | неопределен номер строки | line number does not exist | `GOTO 999` |
| 9 | недопустимый индекс | subscript out of range or wrong count | `Q(11)` with no `DIM` |
| 10 | повторное определение массива | array dimensioned twice | `DIM A(3)` then `DIM A(4)` |
| 11 | деление на нуль | division by zero, or 0 to a negative power | `X=1/0` |
| 12 | недопустимый оператор в непосредственном режиме | not allowed in direct mode | `DEF FN...` or `SPC(3)` typed in direct mode |
| 13 | ошибка типов | string where a number is needed or the reverse | `A$=1` |
| 14 | не хватает места символьным переменным | string space full (see `CLEAR`) | `B$=A$+A$` with 200-character `A$` |
| 15 | слишком длинная строка | string longer than 255 | same, after `CLEAR 1000` |
| 16 | — | (L11: expression too complex) | |
| 17 | продолжение невозможно | cannot continue: the program was changed | `GOTO 1030` typed after `RENUM`; but `CONT` after an error just printed `Ok` |
| 18 | неопределена функция пользователя | undefined function (L11: `FN` before `DEF FN`) | |
| 19 | ошибка устройства В/В | input/output device error | |
| 24 | отсутствует операнд | missing operand or parameter | `?2+`, `COLOR` with no arguments |
| 25 | переполнение буфера ввода | line longer than 255 characters | |
| 52 | ошибочный номер файла | wrong file access | `EOF` with no file open (L10 says 59) |
| 54 | файл уже открыт | `OPEN` of an open file | |
| 55 | конец файла | `INPUT#` past the end | |
| 56 | ошибочное имя файла | file name empty or too long | |
| 57 | — | `LOAD` met a line without a number; loading stops | |
| 59 | файл не открыт | file operation with no file open | |
| 62 | ошибочное имя устройства | bad device name | |

Codes 20–23, 26–51, 53, 58, 60, 61 and 63–255 are unused (O10 Appendix).

## Differences between the two machines

Everything not listed here behaved identically in the tests: keywords,
arithmetic, number formats, error numbers, editing keys, tape formats.

| | BK-0010.01 | BK-0011M |
|---|---|---|
| `MERGE` | no (error 2; L10 Appendix 5) | yes |
| `LINE ... ,BF` | no (error 2) | yes |
| Service line with key hints | yes | no |
| Graphics area | 256 × 241 below the service line | 256 × 256, whole screen |
| Colors 1, 2, 3, 4 | red, green, blue, black | white, green, cyan, black (palette 15); palette changeable with `POKE` |
| `POINT` result | color 1–4, −1 off screen | pixel code 0–3, 0 off screen |
| Graphics color changes text color | no | yes |
| `PAINT`, `CIRCLE` | slow `PAINT`, circles drawn solid | much faster `PAINT`, circles drawn with sparser dots |
| Extended memory mode (АР2+СБР) | yes, up to 26364 bytes free | no |
| `CALL` without a cartridge | error 5 | does nothing |
| `MONIT` goes to | BK-0010 monitor, `?` prompt; back with `BASIC` | BOS monitor, `@` prompt; back with `11;1C`, `100000G` |
| `^` shown as | `¬` | `^` |
| Speed | 3 MHz | 4 MHz, noticeably faster |
| Fast tape `--files` in the emulator | works (all but `FIND`) | works (`SAVE`, `CSAVE`, `BSAVE` and their loads checked) |

**Features of the L11 manual (version 1987.12.25) that this BK-0011M ROM
lacks**, each checked: `SCREEN` (error 2), `SYSTEM` (error 2; use `MONIT`),
`FILES` (error 2; use `FIND`), `INSTR` (error 13), `VARPTR` (error 12),
virtual arrays `DIM A[10]` (error 2), memory-page arguments such as
`PEEK(5,&O40000)` and `POKE 5,...` (error 2), `BEEP length,pitch` (error 2),
text error messages, and single-precision-only arithmetic (this ROM is double
precision by default, with `CDBL` and `#`). Conversely, L11 Appendix 4 says
its version lacks `TRON`/`TROFF`, `LINE ... B/BF` and `DRAW`; this ROM has
all of them. The keyword abbreviation of L11 (`PRI` for `PRINT`) works in
both ROMs. The BK-0011M's 128 KB are not available to BASIC programs:
`FRE(0)` is 14076 as on the BK-0010.01.

## Example programs

Each program was typed into the emulator and run on both machines; the
output below is what the screen showed (32 columns wide).

### Prime numbers

```
10 REM PRIMES BELOW 100
20 DIM F%(100)
30 FOR I=2 TO 100
40 IF F%(I) THEN 100
50 PRINT I;
60 IF I>10 THEN 100
70 FOR J=I*I TO 100 STEP I
80 F%(J)=1
90 NEXT J
100 NEXT I
110 PRINT
120 END
RUN
 2  3  5  7  11  13  17  19  23
 29  31  37  41  43  47  53  59
 61  67  71  73  79  83  89  97

Ok
```

Line 60 matters: a `FOR` loop always runs once, so without it `I=11` would
try `F%(121)` and stop with error 9. The empty line comes from line 110,
because the third line exactly filled the screen width.

### Palindromes

```
10 INPUT "WORD";W$
20 IF W$="END" THEN END
30 R$=""
40 FOR I=LEN(W$) TO 1 STEP -1
50 R$=""+R$+MID$(W$,I,1)
60 NEXT I
70 PRINT W$;" -> ";R$
80 IF R$=W$ THEN PRINT "PALINDROME" ELSE PRINT "NOT A PALINDROME"
90 GOTO 10
RUN
WORD?LEVEL
LEVEL -> LEVEL
PALINDROME
WORD?BASIC
BASIC -> CISAB
NOT A PALINDROME
WORD?END
Ok
```

Line 50 starts with `""+` because of the [string pitfall](#strings): written
as `R$=R$+MID$(W$,I,1)` it prints `LEVEL -> EEEEL`.

### Sine wave

```
10 REM SINE WAVE
20 CLS
30 LINE (0,100)-(255,100)
40 LINE (128,20)-(128,180)
50 FOR X=0 TO 255 STEP 2
60 PSET (X,100-70*SIN((X-128)/20))
70 NEXT X
80 CIRCLE (128,100),70
90 LOCATE 0,20
100 PRINT "Y=SIN(X)"
RUN
```

The screen is cleared, two axes cross at (128,100), a dotted sine curve of
two periods runs across the whole width, a circle of radius 70 is centered on
the crossing, and `Y=SIN(X)` and `Ok` appear on rows 20 and 21. The program
takes about 15 seconds on the BK-0011M and 17 on the BK-0010.01, mostly
computing `SIN` in double precision. On the BK-0010.01 the picture sits 16
lines lower, below the service line.

### Machine-code function

See [Memory and machine code](#memory-and-machine-code): `PRINT USR(21%)`
printing 42.

### Data file on tape

See [Data files](#data-files).

## What was not verified

These were taken from the manuals without a test: the СТОП key while a
program runs (`STOP` the statement was tested), ШАГ single-stepping,
function keys 6–0 (the defaults were read from the ROMs; keys 1–5 typed
their text on both machines), ИНД СУ, БЛОК РЕД, ПОВТ, the tab keys, =|=>|,
and АР2+СБР typed as a key (its effect was tested through `CHR$(&O214)`).
The АР2 color keys (O10 §3.2) were checked only for green and blue on the
BK-0010.01. `LPRINT` and `LLIST` were checked with the emulator's
`--printer`, on both machines; without a printer `LPRINT` waits forever
(leave it with СТОП). Also untested: `FIND` on a real tape, `LOAD` of a file
with unnumbered lines, `CALL` with a cartridge, error codes 18, 19, 25,
54–57, 59 and 62, the 16-character limit of `KEY`, and the BK-0010 `USR`
table at `&O2100`.

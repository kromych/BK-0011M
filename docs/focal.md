# FOCAL on the BK-0010

FOCAL (ФОКАЛ, "ФОКАЛ-БК0010") is the interpreted language of the original
Elektronika BK-0010 (LD §1). The operating manual M1.492.000 РЭ places the
FOCAL interpreter and the test monitor in the МСТД plug-in ROM block: with
the block fitted, the machine starts in FOCAL instead of BASIC (its section
on software, and §6.3.1). The interpreter occupies 120000–137777 (octal;
all addresses in this guide are octal, all numbers inside FOCAL programs
are decimal).

This guide is for a programmer who has never seen a BK. It covers working
with FOCAL in this emulator, the language, every statement and function the
ROM really has, the error messages, and complete example programs. Every
example was typed into the emulator and the output shown is what appeared on
the screen.

Sources, cited compactly:

| Short name | Document |
|---|---|
| UM | «Фокал. Руководство пользователя» 00009-01.90.01 (1984), the FOCAL user manual: a tutorial, with a summary in appendix 2 |
| LD | «Программное обеспечение микро-ЭВМ. Язык "Фокал"», the FOCAL language description |
| ROM | the commented disassembly `listings/focal.mac` |

Both manuals are in [manuals/](manuals/). Where the ROM or the emulator
behaves differently from a manual, the text says so; the list is collected
in [Where the manuals and the ROM disagree](#where-the-manuals-and-the-rom-disagree).

The BK-0011M had a FOCAL of its own, described in «Фокал. Описание языка»
00008-01.35.05. It is not in the repository and not covered here.

Contents:

- [Starting FOCAL](#starting-focal)
- [The keyboard in FOCAL](#the-keyboard-in-focal)
- [Direct commands and programs](#direct-commands-and-programs)
- [Running and stopping programs](#running-and-stopping-programs)
- [Editing](#editing)
- [Error messages](#error-messages)
- [Memory](#memory)
- [Saving and loading: LIBRARY](#saving-and-loading-library)
- [The language](#the-language)
- [Statement reference](#statement-reference)
- [Function reference](#function-reference)
- [Error codes](#error-codes)
- [Leaving FOCAL: the monitor and the tests](#leaving-focal-the-monitor-and-the-tests)
- [Example programs](#example-programs)
- [Where the manuals and the ROM disagree](#where-the-manuals-and-the-rom-disagree)

## Starting FOCAL

```sh
bk --bk0010 --focal --mono
```

`--bk0010 --focal` gives the original BK-0010: the BK-0010 monitor, FOCAL at
120000 and the test ROM at 160000 (see [emulator.md](emulator.md) and
[machine.md](machine.md)). FOCAL uses the 64-column text mode, which is clean
only on a black-and-white monitor; `--mono` selects one, and Alt+M (Cmd+M on a
Mac) switches monitors in the window.

After about three seconds the screen shows:

```
                                                               ЛАТ
----------------------------------------------------------------
?00 AT  0.00
ГОТОВНОСТЬ К РАБОТЕ
*
```

The top line is the monitor's service line (ЛАТ: Latin letters selected).
Below it FOCAL reports "error 00 at line 0.00": ГОТОВНОСТЬ К РАБОТЕ, "ready
for work". The `*` is FOCAL's prompt: it waits for a line (UM §4.2; the
operating manual §6.3.1 shows the same screen). The screen holds 24 lines of
64 characters.

Try a first command. Type it and press ВВОД (Enter):

```
*T "HELLO"
HELLO*
```

`T` is TYPE. Output carries on from where the cursor is; `!` starts a new
line (see [TYPE](#type)), which is why the prompt follows `HELLO` directly.

## The keyboard in FOCAL

FOCAL reads a whole line before it does anything, with its own line editor.
These are the keys that matter (UM §6.3, §6.4.5, appendix 2; LD §2.1,
§6.7; the HELP screen). The Code column is the BK key code in octal; the
Emulator column is the host key that sends it. [monitor.md](monitor.md)
describes the keyboard in general.

| BK key | Code | Emulator | In FOCAL |
|---|---|---|---|
| ВВОД (enter) | 012 | Enter | ends the line; FOCAL acts on it. The cursor may be anywhere in the line |
| ЗАБОЙ (backspace) | 030 | Backspace | blanks the character left of the cursor and moves back onto it; at the end of the line this deletes the last character (inside a line, close the gap with \|<==) |
| ← → | 010, 031 | ← → | move the cursor along the line; typing then overwrites |
| \|<== (смыкание, close up) | 026 | F4 | deletes the character under the cursor, pulling the rest left |
| \|==> (размыкание, open up) | 027 | F5 | opens a blank at the cursor, pushing the rest right |
| АР2+→ (clear to end of line) | 231 | left Ctrl+→ | erases from the cursor to the end of the line |
| ВС | 023 | right Ctrl+S (СУ+S) | cursor to the start of the line |
| `_` | 137 | Shift+- | throws away the line being typed; FOCAL gives a new `*` |
| СБР (clear) | 014 | F9 | clears the screen and throws away the line being typed |
| КТ | 003 | F2 | at the prompt: restarts FOCAL with the sign-on message; erases the variables, keeps the program |
| СТОП (stop) | interrupt | Esc or F12 | stops whatever FOCAL is doing: error 25 |
| ШАГ (step) | 200 | F8 | pauses a running program (or a listing); ВВОД continues |
| НР/СБР | 214 | left Ctrl+F9 | switches extended memory on or off (see [Memory](#memory)) |
| НР/ЗАБОЙ | 230 | left Ctrl+Backspace | switches the РЕД input mode on or off (see [Editing](#editing)) |
| РУС, ЛАТ | 016, 017 | End, Home | Russian or Latin letters |

The FOCAL manuals call the modifier НР; the monitor manual says the code 214
comes "по регистру АР2" (00001-01.32.03), and in the emulator left Ctrl is
АР2. The manuals also list ГТ, a tab key that moves the cursor 8 places,
code 011. The BK's ТАБ key (the PC's Tab) sends 011 through the
function-key vector, and the monitor's keyboard driver acts on it itself:
in a test, `T 12`, Tab, `3`, ВВОД moved the cursor to the start of the
next line, and FOCAL rejected the line ("ДВОЙНОЙ ИЛИ ОТСУТСТВУЮЩИЙ
ОПЕРАТОР В ВЫРАЖЕНИИ"). A key typed within about 50 ms after Tab is
echoed but lost (the emulator's `--type` and pasting wait longer). Code 011
through the ordinary vector (`\011` in `--type`) instead left the cursor in
place and was stored invisibly in the line (`T 12`, 011, `3` printed 1293).
Either way, tabs are best avoided.

All checked in the emulator, including ВС and АР2+→ by their codes. КТ does
nothing while a program runs; only СТОП and ШАГ act then.

**Typing symbols in the emulator.** Letters, digits and punctuation, with
Shift, are where they are on a US keyboard: `*` is Shift+8, `^` Shift+6, `@`
Shift+2 (multiplication, powers, and "keep the old value" in ASK). `"` is
Shift+', `:` Shift+;, `+` Shift+=, `_` Shift+-. FOCAL accepts `[ ]` and
`< >` as brackets too. ВС is right Ctrl (СУ) with S, or СУ and then S on
the emulator's on-screen BK keyboard.

Headless runs type any ASCII character with `--type`
([emulator.md](emulator.md#headless-runs-and-automation)).

Statements must be typed in capital Latin letters: `t 1` gives error 04.
Russian letters (РУС) are fine inside quoted text and comments.

## Direct commands and programs

A line typed without a number is a **direct command**: FOCAL carries it out
at once and forgets it. A line can hold several statements separated by `;`:

```
*S A=3;S B=-7;T "X=",A+B,!
X= -   4.0000
*
```

A line that starts with a **line number** is stored as part of the program
and not carried out (UM §4.3–4.5; LD §2.2). Numbers have the form
*group.step*:

- 1.01 to 99.99 in steps of .01, except those ending in .00;
- 100.1 to 127.9 in steps of .1, except those ending in .0.

`2.1` is the same line as `2.10`. All lines with the same whole part form a
**group**, which DO, WRITE and ERASE can address as a unit. Leave gaps
(1.10, 1.20, ...) so that lines can be put in between. A number outside the
range gives error 01; `100.15` is rounded to `100.2`.

```
*10.01 T "BK-0010",!
*10.06 C COMMENT LINE
*1.1 S A=1;T A
*5.3 T "LAST"
*W
C:  ФОКАЛ-БК0010
  1.10 S A=1;T A
  5.30 T "LAST"
 10.01 T "BK-0010",!
 10.06 C COMMENT LINE
*W 10
 10.01 T "BK-0010",!
 10.06 C COMMENT LINE
*
```

WRITE (`W`) lists the program in line-number order. The first line,
`C:  ФОКАЛ-БК0010`, is a comment the interpreter keeps at the head of every
program; it is listed and saved with it.

- Typing a line with an existing number replaces that line.
- Typing just a number stores an empty line; to remove a line use
  `E` (ERASE) with its number.
- A line can be up to 79 characters long including its number. The 80th
  character gives error 18 at once and the line is lost (LD §6.2 says 80).

**Abbreviations.** FOCAL looks only at the first letter of a statement word
and skips the rest of the word, so `T`, `TYPE` and even `TXYZ` are the same
statement. Programs are normally written with single letters: they are
shorter, take less memory and run faster (LD §2.3, §9). The letters are
A C D E F G H I K L M P Q R S T V W X; any other letter gives error 04.

## Running and stopping programs

| What | How |
|---|---|
| run from the lowest line | `G` (GOTO with no number), or `D`, `DO`, `D A` (DO ALL) |
| run from a line | `G 5.3` |
| run one line or one group as a subroutine | `D 5.3`, `D 5` |
| stop at a point in the program | `Q` (QUIT) in the program |
| carry on after QUIT or an error | `G` with the line number to continue from (LD §5.8) |
| interrupt | СТОП (Esc): `?25 AT` *line*, ОСТАНОВ ПО КЛАВИШЕ СТОП ("stopped by the СТОП key") |
| pause and continue | ШАГ (F8) pauses; ВВОД continues |

A program also ends when it runs past its last line. Variables keep their
values after the program stops; `T $` lists them.

An endless loop, and how it ends:

```
*1.1 T "A"
*1.2 G 1.1
*G
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
AAAAAAAA
?25 AT  1.10
ОСТАНОВ ПО КЛАВИШЕ СТОП
*
```

FOCAL is slow: `F I=1,1000;S X=X+I` took about 12 seconds in the emulator,
which runs at the BK-0010's 3 MHz. While a program runs, FOCAL reads the
keyboard itself (for ШАГ), so keys typed ahead are lost.

## Editing

**While typing a line**, use the keys in [the table above](#the-keyboard-in-focal).
For example `T 1234`, ← ←, |<== gives `T 124`; `T 1234`, ← ←, |==>, `9`
gives `T 12934`.

**MODIFY** (`M`) edits a stored line. FOCAL prints the line's text without
its number and leaves the cursor at the end; edit it with the same keys and
press ВВОД (UM §6.3.5; LD §6.2). The line number can't be changed this way.

```
*20.9 T "IVANOV PETROV"
*M 20.9

T "IVANOV PETROV"
```

Here ← eight times puts the cursor on the space; `,` overwrites it, |==>
opens a gap and a space fills it; then ВВОД:

```
T "IVANOV, PETROV"
*W 20
 20.90 T "IVANOV, PETROV"
*
```

**РЕД mode** (НР/ЗАБОЙ, left Ctrl+Backspace in the emulator; РЕД appears
on the left of the service line) stores the cursor keys ← → ↑ ↓ and the
diagonal arrows into the line instead of acting on them, so that a string
can draw a picture when printed. No editing is possible while РЕД is on;
press НР/ЗАБОЙ again to leave it (UM §6.3.5; LD §6.2). The manual's
example: type
`1.1 T "`, six spaces, switch РЕД on, type `AAA` ↓ and ← six times, `BBB`,
→ three times, `CCC`, switch РЕД off, type `"` and ВВОД. Then:

```
*D 1
      AAA
   BBB   CCC
*W
C:  ФОКАЛ-БК0010
  1.10 T "      AAA
             BBB   CCC"
*
```

(The listing moves the cursor too, which is why it looks displaced.)

## Error messages

When FOCAL finds an error it stops, prints a message and returns to the
prompt (UM §6.1.3–6.1.5; LD §6.8):

```
*T (1+2

?03 AT  0.00
НЕПАРНЫЕ СКОБКИ
*
```

`?` marks an error, `03` is its code, and the number after AT is the line
where it happened (0.00 for a direct command). The second line says what is
wrong, here "unmatched brackets". All codes are listed in
[Error codes](#error-codes).

## Memory

**VACANT** (`V`) prints the free memory for program text in bytes, as an
octal number (LD §6.5; UM §15.2):

```
*V

СВОБОДНО 035776 Б. ОЗУ
*
```

"FREE 035776 BYTES OF RAM": 15 358 bytes after power-on. Variables are not
counted: each takes 8 bytes (LD §2.5), and filling an array stopped with
error 11 after about 1900 elements.

**Extended memory.** НР/СБР (left Ctrl+F9) shrinks the text area of the
screen to its top four lines and gives the freed screen memory to FOCAL:
VACANT then showed 055776, 8192 bytes more. Pressing it again restores the
screen. Each switch erases the variables but keeps the program; if the
program no longer fits, switching back is refused with error 10 (LD §6.7).

## Saving and loading: LIBRARY

The LIBRARY (`L`) statement writes programs and data to tape and reads them
back (LD §4.5; UM §13). Its second word, abbreviated to its first letter,
says what to do:

| Statement | Does |
|---|---|
| `L S name` | SAVE: write the program to tape |
| `L G name` | GET: find the program on tape and load it, replacing the program and erasing all variables |
| `L F name` | FGET: find the file and read past it without loading: lists the names on a tape, or positions the tape after a file before saving |
| `L O name` | OUTPUT: write all variables to tape |
| `L I name` | INPUT: read variables from tape, adding them to those in memory; the program is kept |
| `L M` | MOTOR: start the recorder's motor (to rewind under computer control) |
| `L R` | RESET: stop the motor |

A file name has up to 15 characters, letters (Latin or Russian, either case)
and digits; a missing name gives error 21. Several files may have the same
name. While searching, FOCAL prints the name of every other file it passes;
GET and FGET print the name of the file they find. SAVE prints nothing but
the prompt. OUTPUT and INPUT shrink the screen during the transfer
(LD §4.5.4) and leave it cleared.

Variables are written in blocks of 511, each a separate file whose name is
padded to 16 characters with the block number in the 16th place:
`L O PRIM` with 521 variables writes `PRIM           0` and
`PRIM           1`. `L I PRIM` reads them all back (LD §4.5.4–4.5.5;
checked in the emulator).

### With a real recorder

From UM §13 and LD §4.5:

1. To save: wind the tape to a free place, type `L S name`, press PLAY and
   RECORD on the recorder, then ВВОД. The `*` prompt returns when the file
   is written. The manual recommends writing two to four copies.
2. To load: wind the tape to before the file, type `L G name`, press PLAY,
   then ВВОД. If the load fails with error 22 (checksum), rewind and try
   again, or use another copy.
3. Between operations FOCAL keeps the motor stopped, so the recorder won't
   rewind; `L M` starts the motor, `L R` stops it.

СТОП during a tape operation stops it with error 24 (in the emulator an
ordinary key did not; the manual says any key does).

### In the emulator

The emulator offers both a tape signal and a shortcut; see
[emulator.md](emulator.md#loading-software).

**Fast tape** (`--files DIR`): LIBRARY reads and writes `DIR/name.bin`
instantly. Programs are saved as a `.bin` file with load address 001752,
where FOCAL keeps its program text; data files get the 16-character names
described above, with the spaces left out of the host file's name.

```sh
bk --bk0010 --focal --mono --files ~/focal
```

```
*1.1 T "HELLO FROM TAPE",!
*1.2 S X=42
*S A=1;S B(3)=7
*L S PROG
*L O DATA
```

writes `PROG.bin` and `DATA0.bin` (the tape name `DATA           0`). In a
new session with the same `--files`:

```
*L G PROG
PROG
*W
C:  ФОКАЛ-БК0010
  1.10 T "HELLO FROM TAPE",!
  1.20 S X=42
*G
HELLO FROM TAPE
*L I DATA
```

INPUT leaves the screen cleared, then:

```
*T $
S A()=    1.0000
S B(+03,+00)=    7.0000
S X()=   42.0000
*
```

(X is 42 because the program ran; A and B came from the file.)

A name that isn't in the directory is looked for on the tape signal, so
`L G` then waits, as a real BK would, until a tape plays or СТОП is pressed.

**Recorded tape** (`--record FILE.wav`, `--tape FILE.wav`): `--record`
captures what the BK writes to tape; the file is saved when the emulator
exits. `--tape` plays a recording whenever FOCAL runs the motor:

```sh
bk --bk0010 --focal --mono --record prog.wav    # then: L S WAVPROG
bk --bk0010 --focal --mono --tape prog.wav      # then: L G WAVPROG
```

Loading that recording with `L G OTHER` printed `WAVPROG` (a file it
passed) and went on waiting; a recording damaged on purpose gave
`?22 AT  0.00` ОШИБКА КОНТРОЛЬНОЙ СУММЫ (checksum error).

## The language

### Statements and lines

A line is a list of statements separated by `;`. A statement is a word (of
which only the first letter counts) followed by a space and its operands.
Statements after GOTO, QUIT or RETURN on the same line never run, nor after
an IF that jumps. Lines run in order of their numbers unless DO, GOTO, IF or
RETURN say otherwise (LD §2.2).

### Numbers

All values are floating-point numbers with about six significant decimal
digits, from 10⁻³⁸ to 10³⁸ (LD §2.4). Constants may have a sign, a decimal
point and an exponent: `70`, `70.00`, `7E+01`, `700E-01` and `70.00003` are
the same number. A result outside the range, too large or too small (such
as `1E-20*1E-20`), gives error 12.

Digits past the sixth are not kept: `T 1234567.8` prints `1234560.0`.

**Letter constants.** A `0` followed by letters makes a number from the
letters, each worth its position in the alphabet (A=1 ... Z=26) as a decimal
digit, with carries: `0A` is 1, `0Z` is 26, `0AB` is 12 and `0NO` is
14×10+15 = 155. The letter E can't be used (it means an exponent). This is
how programs compare answers typed to ASK (LD §2.4, §4.2):

```
*T 0A,0Z,0AB,0NO,!
     1.0000    26.0000    12.0000   155.0000
```

### Variables

Variables need no declaration. One that hasn't been set is 0 (LD §2.5).

- A name starts with a letter other than F (F starts function names; `S FA=1`
  gives error 02) and may continue with letters and digits: `X`, `TOP`,
  `Z9`.
- Only the first two characters count: `ALPHA` and `ALBUM` are the same
  variable `AL`.
- `E` is an ordinary name (`S E=5;T E` prints 5).
- A variable's value can serve as a line number (`S X=3.4;D X`), but not if
  its name starts with A (read as ALL) or F (a function).

`T $` lists all variables in the form of SET statements:

```
*S ALPHA=1;S ALBUM=2
*S X(1)=5;S X(-3)=7;S K(2,3)=9
*S TOP=3.5;S Z9=1E10
*T $
S AL()=    2.0000
S K(+02,+03)=    9.0000
S TO()=    3.5000
S X(-03,-01)=    7.0000
S X(+01,+00)=    5.0000
S Z9()= 0.100000E+11
*
```

ERASE (`E`) with no operand erases all variables; so does `E` with a line or
group number (as well as deleting those lines).

### Arrays

A variable can take one or two subscripts in round brackets: `X(1)`,
`K(I,J)`, `A(J+L*K)`. Arrays need no declaration; a subscript can be any
expression and its integer part is used (LD §2.5–2.6).

The ROM keeps a variable's subscripts in a single 16-bit word, which explains
the manual's ranges (−32768..32767 for one subscript, −128..127 for each of
two) and some surprises found in the emulator:

- `X(i,j)` is the same element as `X(i+256*j)`: `X(1)` is `X(1,0)` and
  `Y(256)` is `Y(0,1)`. `T $` shows every element in the two-subscript form.
- A plain variable is element 0: after `S A=3;S A(0)=4`, `A` is 4.
- Element 2.7 is element 2.

### Expressions and precedence

Operators (LD §2.7; UM §5):

| Operator | Meaning |
|---|---|
| `^` | power; the exponent is cut to an integer and must not be negative (error 15) |
| `*` | multiplication |
| `/` | division (by zero: error 14) |
| `+` `-` | addition, subtraction |

`( )`, `[ ]` and `< >` are all brackets and do the same thing, but each must
be closed by its own kind (error 03).

Precedence, highest first, as the emulator showed:

1. brackets;
2. `^`;
3. `*`;
4. `/`;
5. `+` and `-`, left to right.

Multiplication is done before division. This is FOCAL's rule, not a slip,
and it changes results:

```
*T 6/2*3,!
     1.0000
*T 12/3/2*2,!
     1.0000
*T 2+2*2,!
     6.0000
*T 2-3-4,!
 -   5.0000
*T 2^3^2,!
    64.0000
```

`6/2*3` is 6/(2×3); write `(6/2)*3` to get 9. Operators of equal rank go
left to right, so `2^3^2` is (2³)² = 64. The manuals list `+` above `-`, but
`5-2+1` gives 4, so they are effectively equal.

A minus sign may start an expression or follow an opening bracket, but not
another operator: `2*-3` and `2^-1` give error 08; write `2*(-3)`. Powers
take whole exponents only: `2^0.5` is 1 (use `FSQT`) and `2^(-1)` is error 15.

### Output: TYPE and formats

`T` prints a list of items separated by commas (LD §4.1; UM §8):

| Item | Prints |
|---|---|
| expression | its value in the current format |
| `"text"` | the text; Russian letters allowed |
| `!` | a new line (several `!` may follow each other: `!!`) |
| `%W.0D` | no output; sets the number format for this and later TYPE and ASK statements |
| `%` | sets the exponent format |
| `$` | all variables (the rest of the line is ignored) |

Text after a quote is printed exactly; spacing between items comes only from
the number format. A `?` inside quotes is printed, not taken as the
[trace](#tracing) switch.

**Number formats.** `%W.0D` gives W digit places in all, D of them after the
point (both at most 8; LD §2.4). Always write D with two digits, as the
manuals do: `%8.4` is not the same as `%8.04` (in the emulator it printed
3.14159 as `3.1415900`). A format stays in force until the next one,
including across later commands (LD §4.1). After power-on it is `%8.04`.

Each number is printed as a space, a sign place (`-` or a space), the integer
part right-aligned in W−D places, then the point and D digits:

```
*T 55.55555,!
    55.5556
*T %4.02,27.35,!
  27.35
*T %6.03,3.14159,-2,!
    3.142 -  2.000
*T %8.0,1234.5,!
      1235
*T %5.02,1,2,3,!
    1.00    2.00    3.00
```

A number whose integer part needs more than W−D places takes them from the
fraction (`T 12345.678` printed `12345.600` in `%8.04`); one that doesn't fit
in W digits at all, and every number after `%`, comes out in exponent form:

```
*T %,75.34,!
  0.753400E+02
*T %8.04,123456789,!
  0.123457E+09
```

### Input: ASK

`A` prints a `:` for each variable in its list and waits for a number (LD
§4.2; UM §7). The list may also hold quoted text, `!` and formats, as in
TYPE:

```
*A "R? ",R,!
R? :2.5

*
```

- A number ends with ВВОД, a space, a comma or `;`. One ASK can take several
  values, and the next `:` then appears on the same line:
  `A X,Y(1),Z(3,2)` answered with `1 2,3` and ВВОД gives `:1 :2,:3`.
- At most 23 characters; the 24th gives error 16 at once.
- `_` throws away what has been typed for the current value.
- `@` keeps the variable's old value (see
  [typing symbols](#the-keyboard-in-focal) for `@` in the emulator):

  ```
  *S Y=2;A X,Y,Z;T X,Y,Z,!
  :1 :@:3
       1.0000     2.0000     3.0000
  ```

- Letters are taken as a [letter constant](#numbers): answering `NO` gives
  155, so a program can compare the answer with `0NO`.
- An expression is accepted if it starts with `+` or `-`: with A=7 and B=4,
  answering `+A-B` gives 3.

With a FOR loop, ASK reads arrays: `F I=1,10;A X(I)`.

### Comments

`C` (COMMENT) makes the rest of the line a comment. Comments take memory,
WRITE lists them, and a traced comment shows only up to its first space (use
`:` between words if the whole comment should show; LD §6.3).

### Control flow

| Statement | Effect |
|---|---|
| `G n` | go to line n; with no n, to the lowest line |
| `D n` | run line n, or group n, as a subroutine, then carry on after the DO |
| `R` | return from DO or FSBR early |
| `I (e) n1,n2,n3` | go to n1 if e<0, n2 if e=0, n3 if e>0 |
| `F v=a,s,b; ...` | loop: the rest of the line, for v from a to b in steps of s |
| `Q` | stop the program |

These are described in detail under [DO](#do), [IF](#if) and [FOR](#for).
Line numbers in GOTO, DO and IF may be variables or numbers, not
expressions.

### Tracing

A `?` in a line switches tracing on or off. While tracing is on, FOCAL prints
the program text it executes, character by character, together with the
output (LD §6.6):

```
*1.1 S A=1;S B=2;S C=3
*1.2 T ? A+B+C ?;QUIT
*G
 A+B+C      6.0000*
```

`?D 2` traces a whole group run by DO. Tracing ends when control returns to
the prompt.

## Statement reference

FOCAL has 19 statements (the HELP screen and the ROM; LD §2.3). Only the
first letter of each word counts.

| Statement | Letter | Purpose |
|---|---|---|
| [ASK](#ask) | A | input numbers from the keyboard |
| [COMMENT](#comment) | C | comment |
| [DO](#do) | D | run a line or group as a subroutine |
| [ERASE](#erase) | E | delete lines and/or variables |
| [FOR](#for) | F | loop |
| [GOTO](#goto) | G | jump; start the program |
| [HELP](#help) | H | list keys, statements and functions |
| [IF](#if) | I | three-way branch |
| [KILL](#kill) | K | reset the external devices |
| [LIBRARY](#library) | L | tape: save, load, motor |
| [MODIFY](#modify) | M | edit a stored line |
| [PASS](#pass) | P | go to the monitor or the tests |
| [QUIT](#quit) | Q | stop the program |
| [RETURN](#return) | R | return from a subroutine |
| [SET](#set) | S | assign a value |
| [TYPE](#type) | T | output |
| [VACANT](#vacant) | V | free memory |
| [WRITE](#write) | W | list the program |
| [XECUTE](#xecute) | X | evaluate an expression, discarding the result |

### ASK

`A item, item, ...` where an item is a variable (simple or subscripted),
`"text"`, `!` or a format. Prints `:` and reads a number for each variable.
See [Input: ASK](#input-ask) for what may be typed. (LD §4.2; UM §7)

```
*A X;T X,!
:NO
   155.0000
```

### COMMENT

`C text`: everything to the end of the line is ignored when run (LD §6.3).

### DO

`D n`: if n is a line number (with a fraction), run that line, then continue
after the DO. If n is a group number (whole), run the group's lines in order
until the group ends or a RETURN, then continue after the DO. `D`, `DO` and
`D A` (DO ALL) run the whole program (LD §5.2–5.3; UM §11). n may be a
variable. A missing line or group gives error 06.

DOs can be nested, and a group can DO itself; too deep a nesting gives
error 9 (LD §5.3 gives 44 levels as the maximum).

A GOTO or IF inside the line or group that DO runs transfers control for
**one line** only: that line runs, then control goes back after the DO
(unless that line jumps again). The manual's examples, run in the emulator:

```
*1.1 DO 7
*1.2 GO 7.4
*7.1 T 1,!
*7.2 T 2,!;RETURN
*7.3 TYPE 3,!
*7.4 TYPE 4;QUIT
*G
     1.0000
     2.0000
     4.0000*
```

```
*1.1 DO 5.1
*1.2 TYPE 3; QUIT
*5.1 TYPE 1,!; GO 6.1
*5.2 TYPE 2,!
*6.1 TYPE 4,!
*6.2 TYPE 5,!
*G
     1.0000
     4.0000
     3.0000*
```

In the second example the GO 6.1 inside `D 5.1` runs line 6.1 only, not
6.2.

### ERASE

| Form | Deletes |
|---|---|
| `E` | all variables; the program stays |
| `E 7.1` | line 7.1, and all variables |
| `E 7` | group 7, and all variables |
| `E T` (TEXT) | the whole program; variables stay |
| `E A` (ALL) | the whole program and all variables |

(LD §6.1; UM §7; all checked.) A variable whose name doesn't start with A or F
may give the number. `E` alone can be used inside a program to free variable
memory.

### FOR

`F v=a,b; statements` or `F v=a,s,b; statements`: a loop over the rest of
the line (LD §5.6; UM §12).

1. v is set to a.
2. The statements after the FOR on the same line run.
3. v is increased by s (1 if there are only two values).
4. If v ≤ b, go back to 2; otherwise continue with the next line.

So the body always runs at least once, and the loop only counts up: with
b < a the body runs once whatever the step. After the loop v is one step
past b. Lines reached by DO, GOTO or IF from the loop's line belong to the
body (one line for GOTO and IF, as with DO). `F v=a; ...` is just a SET.

```
*F X=-10,5,0;T X
 -  10.0000 -   5.0000     0.0000*
*F I=1,2;F J=3,4;T I,J,!
     1.0000     3.0000
     1.0000     4.0000
     2.0000     3.0000
     2.0000     4.0000
*
```

Changing v inside the loop changes the count: `F I=1,3;S I=I+1;T I` prints
2 and 4. Changing a, s or b inside does not. The loop can't be left except
by running out (LD §5.6).

### GOTO

`G n` continues at line n; `G` alone at the lowest line, which is how a
program is started (LD §5.1). n may be a variable holding a line number. A
missing line gives error 05. Statements after GOTO on the same line are never
reached.

```
*6.3 T "SIX THREE",!;Q
*S M=3*2.1;G M
SIX THREE
*
```

### HELP

`H` clears the screen and lists the control keys (ШАГ, ВВОД, СТОП, СБР,
НР/СБР, `_`), the statements with LIBRARY's and PASS's sub-commands, and the
functions with their arguments (LD §6.4). The list doesn't include
[FCLK](#fclk).

### IF

`I (e) n1`, `I (e) n1,n2` or `I (e) n1,n2,n3`: evaluate e and go to n1 if it
is negative, n2 if it is zero, n3 if it is positive. When the line number for
the case is missing, carry on with the next statement on the line (LD §5.5;
UM §10). The brackets are required (without them: error 03).

```
*1.1 ASK Y;IF (Y-10) 1.2,1.3,1.4
*1.2 TYPE "Y<10";QUIT
*1.3 TYPE "Y=10";QUIT
*1.4 TYPE "Y>10";QUIT
*G
:5
Y<10*G
:12
Y>10*
```

Because of rounding, a computed value that should be zero may not be; compare
the difference with a small tolerance instead (LD §5.5).

### KILL

`K` executes the PDP-11 RESET instruction, putting all devices in their
initial state (LD §4.4; UM §15.1). In a program, the keyboard interrupt is
then off until the program ends, so ШАГ and other keys are ignored. СТОП,
which is wired separately, still stopped such a program in the emulator; the
manual says the program can no longer be interrupted.

### LIBRARY

`L S name`, `L G name`, `L F name`, `L O name`, `L I name`, `L M`, `L R`.
See [Saving and loading](#saving-and-loading-library).

### MODIFY

`M n` shows line n for editing; ВВОД stores the edited line. A missing line
gives error 05. Statements after MODIFY on the line are not run. See
[Editing](#editing).

### PASS

`P M` (PASS MONITOR) and `P T` (PASS TEST). See
[Leaving FOCAL](#leaving-focal-the-monitor-and-the-tests).

### QUIT

`Q` stops the program and returns to the `*` prompt with all variables kept;
`G n` continues from line n (LD §5.8).

### RETURN

`R` ends a subroutine started by DO or FSBR early; the rest of its line and
group are skipped (LD §5.4).

```
*1.1 ASK Y;DO 3
*1.2 TYPE X;QUIT
*3.1 IF (Y) 3.2,3.3,3.4
*3.2 SET X=-1;RETURN
*3.3 SET X=0;RETURN
*3.4 SET X=1;RETURN
*G
:-5
 -   1.0000*G
:0
     0.0000*
```

### SET

`S v=e`: assigns the value of expression e to variable v (LD §3). One
assignment per SET: `S X=Y=5` gives error 07.

```
*S D=2
*S B=2;S A=B*FSQT(D)
*T A,!
     2.8284
```

### TYPE

`T item, item, ...`: see [Output: TYPE and formats](#output-type-and-formats).

```
*T FCHR(65,66),!
AB    66.0000
```

### VACANT

`V` prints `СВОБОДНО nnnnnn Б. ОЗУ`, the free memory for program text in
bytes, in octal (LD §6.5). UM §15.2 warns that this holds only for the
BK-0010.

### WRITE

`W` or `W A` lists the program; `W 3` lists group 3; `W 3.1` lists line 3.1.
A missing line or group is not an error (LD §4.3). ШАГ pauses a long listing.

### XECUTE

`X e` evaluates the expression e and throws the value away. It is used to
call functions for their effect: `X FCHR(12)` clears the screen,
`X FT(1,10,10)` draws a point (LD §5.7).

## Function reference

Function names start with F; arguments go in brackets, and a function with
no arguments still needs them: `FRAN()`. Functions can be used anywhere in an
expression. In the table, "X" means any expression (LD §8; UM §14).

| Function | Value | Notes |
|---|---|---|
| `FSIN(X)` | sine | X in radians; degrees × 3.14159/180 |
| `FCOS(X)` | cosine | radians |
| `FTAN(X)` | tangent | radians |
| `FASIN(X)` | arcsine, radians | \|X\| ≤ 1, else error 20 |
| `FACOS(X)` | arccosine, radians | \|X\| ≤ 1, else error 20 |
| `FATAN(X)` | arctangent, radians | |
| `FLOG(X)` | natural logarithm | X > 0, else error 19 |
| `FLOG10(X)` | decimal logarithm | X > 0, else error 19 |
| `FEXP(X)` | eˣ | |
| `FSQT(X)` | square root | X ≥ 0, else error 17 |
| `FABS(X)` | absolute value | |
| `FSGN(X)` | −1, 0 or 1 | the sign of X |
| `FITR(X)` | integer part | toward zero: `FITR(-4.1)` is −4 |
| `FRAN()` | random number in (−1, 1) | `FRAN(1)` restarts the sequence |
| `FCHR(X1,...,Xn)` | code of the last character | prints characters, or reads a key |
| `FX(op,address,data)` | a memory word | reads, ANDs or writes memory and registers |
| `FSBR(group,X)` | value of `&` at the end | a function written in FOCAL |
| `FK(X,Y)` | Y | moves the text cursor |
| `FT(op,X,Y)` | Y | draws or erases a point |
| `FV(op,X,Y)` | Y | draws or erases a line |
| `FP(op,mask)` | port word | the parallel port |
| `FCLK()` | 50 Hz ticks | not in the manuals; see [FCLK](#fclk) |

The manual's examples give the same values in the emulator:

```
*T FSIN(3.14159/4),FCOS(0.5),FTAN(0.87),!
     0.7071     0.8776     1.1853
*T FASIN(0.5),FACOS(0.5),FATAN(3.91),!
     0.5236     1.0472     1.3204
*T FLOG(5.17),FLOG10(10),FEXP(5),!
     1.6429     1.0000   148.4130
*T FSGN(6-4),FSGN(0),FSGN(-3),!
     1.0000     0.0000 -   1.0000
*T FITR(55.27),FITR(-4.1),FABS(-17),!
    55.0000 -   4.0000    17.0000
*T FSQT(625),FSQT(2),!
    25.0000     1.4142
```

### FRAN

`FRAN()` gives pseudo-random numbers spread evenly over (−1, 1) (LD §8.2).
The sequence is the same after every power-on: the first values were −0.3916
and 0.1659, as in the manual's example. `X FRAN(1)` restarts the sequence,
which helps when debugging; it was 0.1659, −0.5618, 0.8044 after each
restart. An integer from 1 to N: `FITR(N*FRAN()/2+N/2+1)`, as in
[the guessing game](#a-guessing-game). (Not `N/2*FRAN()`: that is
N/(2·FRAN()), see [precedence](#expressions-and-precedence).)

### FCHR

`FCHR` prints and reads characters by their codes (LD §8.3; UM §14.4). Its
arguments are handled in turn:

- a positive argument prints the character with that code (its integer
  part): `X FCHR(65,66)` prints `AB`;
- a negative argument waits for a key and takes its code, without echoing it.

The value is that of the last argument: for printing, the code; for a key,
the key's code. ВВОД reads as 13.

```
*T FCHR(65.8),!
A    65.0000
*T FCHR(65,66,-1),!          (C pressed)
AB    67.0000
*S Z=FCHR(FCHR(-1));T !,Z,!  (Z pressed: echoed by the outer FCHR)
Z
    90.0000
```

Control codes act as on any BK output: `X FCHR(12)` clears the screen, 25 is
→ (LD §8.3). The BK's character codes are in appendix 4 of UM and in
[monitor.md](monitor.md). Codes above 127 print as they are: `FCHR(193)`
printed the Russian letter а (code 301). LD §8.3 says such codes are taken
modulo 128 (which would print `A`); the ROM passes them unchanged.

### FX

`FX(op, address, data)` gives access to memory and device registers
(LD §8.4; UM §14.5):

| op | Does | Value |
|---|---|---|
| 1 | read the word at the address | the word |
| 0 | read the word and AND it with *data* | the result |
| −1 | write *data* to the address | the word written |

The address must be even, and it must be either an **octal constant** written
in the call or a **variable**, whose value is the address in decimal. It can't
be an expression (`FX(1,10000+10000)` gives error 03). So `FX(1,20000)` and
`FX(1,B)` with B=8192 read the same word. Data is decimal, −32768..32767, and
words read come back as signed decimal numbers (a word written as 40000
reads back as −25536). Writing to 0–2000, which
FOCAL uses itself, gives error 13; an address where nothing answers gives
error 26 (on the BK-0010, for example, 140000).

```
*X FX(-1,20000,1234)
*S B=8192;T FX(1,20000),FX(1,B),!
  1234.0000  1234.0000
*T FX(0,20000,255),!
   210.0000
*T FX(1,120000),!
  2560.0000
```

(120000 holds the instruction CLR R0, 005000 octal = 2560.) LD §8.4 has a
table of bit values for building masks: bit n is 2ⁿ, bit 15 is −32768.

### FSBR

`FSBR(n, X)` is a function written in FOCAL (LD §8.5; UM §14.6). FOCAL puts
the value of X in the special variable `&`, runs group n as with DO, and
takes the value of `&` when the group ends or RETURNs as the function's value.
FSBR can be called recursively. Every exit line should end with RETURN.

```
*7.1 S &=&*&
*T FSBR(7,3),!
     9.0000
```

See [the factorial example](#factorial-with-a-recursive-fsbr).

### FK

`FK(X, Y)` moves the text cursor to column X (0–63) and row Y (0–23),
counted from the top left; larger values are taken modulo 64 and 24. The
value is Y (LD §8.6). `X FK(32,12);T "CENTER"` prints in the middle of the
screen.

### FT and FV

`FT(op, X, Y)` draws (op=1) or erases (op=0) the point at X, Y; `FV(op, X, Y)`
draws or erases a line from the last point to X, Y. X runs from 0 to 511
across the screen, Y from 0 at the top (just below the service line)
downward. Both return Y (LD §8.7).

LD gives 0..255 for Y; in the emulator Y=239 was the bottom line of the
screen and larger Y drew nothing. A line needs a starting point, which FT
sets: `X FT(0,100,100);X FV(1,100,200)` moves invisibly to (100,100) and
draws down to (100,200). Drawing is slow: the 65 lines of
[the sine example](#a-graph) take about 12 seconds.

### FP

`FP(op, mask)` works the parallel port at 177714 (LD §8.8; UM §14.9). The port
has an input register (read only) and an output register:

| op | Does |
|---|---|
| 0 | read the input register, AND mask |
| 1 | clear the output bits set in mask |
| 2 | set the output bits set in mask |
| 3 | read the output register, AND mask |

The mask is an octal constant (0–177777) or a variable holding a decimal
value. The value is the word read or written. Another op gives error 27.

```
*X FP(2,177777);T FP(3,10),!
     8.0000
*T FP(1,377),FP(3,177777),!
 - 256.0000 - 256.0000
```

### FCLK

`FCLK()` is in the ROM's function table but in neither manual nor the HELP
list. FOCAL points the vector at 100 to a routine that counts 50 Hz
interrupts in a floating-point number (at 1746–1750), and FCLK() returns
that count: the time since FOCAL started, in 1/50 s. A BK-0010 has no 50 Hz
interrupt of its own (with one wired permanently, the interrupt would come
before FOCAL set its vector and crash the machine), so `FCLK()` stays 0
unless something drives IRQ2. With `--irq2-50hz` the emulator delivers it
once vector 100 holds a handler, and `FCLK()` counts while a program runs
(the monitor holds interrupts off while FOCAL waits for a key).

## Error codes

The messages as the ROM prints them (they differ in places from the tables
in UM appendix 1 and LD appendix 1, which add advice). "Seen" means the error
was produced in the emulator.

| Code | Message | Meaning | Seen |
|---|---|---|---|
| 00 | ГОТОВНОСТЬ К РАБОТЕ | ready (sign-on, КТ, after the monitor) | yes |
| 01 | НЕПРАВИЛЬНЫЙ НОМЕР СТРОКИ | bad line number (`1.00`, `128.1`) | yes |
| 02 | НЕПРАВИЛЬНОЕ ИМЯ ФУНКЦИИ ИЛИ ИМЯ ПЕРЕМЕННОЙ | bad function or variable name (`S FA=1`) | yes |
| 03 | НЕПАРНЫЕ СКОБКИ | unmatched brackets; also an IF without brackets | yes |
| 04 | НЕПРАВИЛЬНАЯ КОМАНДА | unknown statement | yes |
| 05 | НЕСУЩЕСТВУЮЩИЙ НОМЕР СТРОКИ | no such line (GOTO, IF, MODIFY) | yes |
| 06 | НЕСУЩЕСТВУЮЩИЙ НОМЕР ГРУППЫ ИЛИ НОМЕР СТРОКИ В ОПЕРАТОРЕ DO | no such group or line in DO or FSBR | yes |
| 07 | НЕПРАВИЛЬНЫЙ ФОРМАТ SET ИЛИ FOR | bad SET or FOR (`S X 1`, `S X=Y=5`) | yes |
| 08 | ДВОЙНОЙ ИЛИ ОТСУТСТВУЮЩИЙ ОПЕРАТОР В ВЫРАЖЕНИИ | two operators in a row, or one missing (`2*-3`) | yes |
| 09 | ПЕРЕПОЛНЕНИЕ СТЕКА | stack overflow: DO or FSBR nested too deep (`1.1 D 1.1`) | yes |
| 10 | ПЕРЕПОЛНЕНИЕ ПАМЯТИ ТЕКСТОМ ПРОГРАММ | program text fills memory | yes |
| 11 | НЕТ МЕСТА ДЛЯ ПЕРЕМЕННЫХ | no room for variables | yes |
| 12 | ПОРЯДОК БОЛЬШЕ Е+-38 | result beyond 10^±38, too large or too small (`1E30*1E30`, `1E-20*1E-20`) | yes |
| 13 | ЗАПРЕЩЕННЫЙ АДРЕС ШИНЫ В ОПЕРАТОРЕ FX | FX write to 0–2000 | yes |
| 14 | ПОПЫТКА ДЕЛЕНИЯ НА НУЛЬ | division by zero | yes |
| 15 | ПОПЫТКА ВОЗВЕДЕНИЯ В ОТРИЦАТЕЛЬНУЮ СТЕПЕНЬ | negative exponent (`2^(-1)`) | yes |
| 16 | СЛИШКОМ МНОГО СИМВОЛОВ ВО ВХОДНЫХ ДАННЫХ | more than 23 characters typed for ASK | yes |
| 17 | КОРЕНЬ КВАДРАТНЫЙ ИЗ ОТРИЦАТЕЛЬНОГО ЧИСЛА | FSQT of a negative number | yes |
| 18 | ПЕРЕПОЛНЕНИЕ ВХОДНОГО БУФЕРА | typed line too long (80th character) | yes |
| 19 | ЛОГАРИФМ НУЛЯ ИЛИ ОТРИЦАТЕЛЬНОГО ЧИСЛА | FLOG or FLOG10 of X ≤ 0 | yes |
| 20 | В ФУНКЦИИ FASIN ИЛИ FACOS АРГУМЕНТ ПО МОДУЛЮ БОЛЬШЕ 1 | FASIN or FACOS of \|X\| > 1 | yes |
| 21 | НЕПРАВИЛЬНОЕ ИМЯ ФАЙЛА | bad or missing file name in LIBRARY | yes |
| 22 | ОШИБКА КОНТРОЛЬНОЙ СУММЫ | tape checksum error: read again | yes |
| 23 | ОШИБКА ДЛИНЫ ФАЙЛА | file length error: read or write again | no |
| 24 | ОСТАНОВ ПО ПРЕРЫВАНИЮ ОПЕРАТОРА | tape operation stopped by the operator (СТОП) | yes |
| 25 | ОСТАНОВ ПО КЛАВИШЕ СТОП | stopped by the СТОП key | yes |
| 26 | НЕСУЩЕСТВУЮЩЕЕ УСТРОЙСТВО | nothing at that address (FX) | yes |
| 27 | НЕПРАВИЛЬНЫЙ КОД ОПЕРАЦИИ В ФУНКЦИИ FP | FP op not 0–3 | yes |

## Leaving FOCAL: the monitor and the tests

PASS (`P`) hands the machine to the system programs in ROM (LD §7;
UM §15.3):

- `P M` (PASS MONITOR) starts the BK-0010 monitor, which clears the screen
  and shows its `?` prompt. Its `F` command, followed by ВВОД, returns to
  FOCAL.
- `P T` (PASS TEST) starts the test monitor, whose prompt is `+`; the key `K`
  returns to FOCAL. The operating manual's health check starts the same way
  (M1.492.000 РЭ §6.3.2).

FOCAL comes back with its sign-on message, and the **program and variables
are gone** (checked), so save first. The monitor, its commands and the tests
are described in [monitor.md](monitor.md).

## Example programs

Each program was typed into the emulator (`bk --bk0010 --focal --mono`) and
run; the output is copied from the screen. `X FCHR(12)` before `G` cleared
the screen.

### A table

```
1.10 C TABLE OF SQUARES AND ROOTS
1.20 T "    N    N^2   SQRT(N)",!
1.30 F N=1,10;T %3,N,%5,N^2,%7.04,FSQT(N),!
```

```
*G
    N    N^2   SQRT(N)
    1      1    1.0000
    2      4    1.4142
    3      9    1.7321
    4     16    2.0000
    5     25    2.2361
    6     36    2.4495
    7     49    2.6458
    8     64    2.8284
    9     81    3.0000
   10    100    3.1623
*
```

Each format sets the width of the column that follows it.

### Quadratic equations

After LD §9.4, with English texts. Note `-B/2*A`: since `*` comes before `/`,
that is −B/(2A), as the formula needs.

```
1.10 A !,"A",A," B",B," C",C;S D=B^2-4*A*C
1.20 I (A) 1.4,1.3,1.4
1.30 T !,"NOT A QUADRATIC",!;G 1.1
1.40 T %6.03,!,"ROOTS";I (D) 1.7,1.6
1.50 T !,(-B+FSQT(D))/2*A,!,(-B-FSQT(D))/2*A,!;G 1.1
1.60 T !,-B/2*A,!;G 1.1
1.70 T " COMPLEX",!,-B/2*A," +",FSQT(-D)/2*A," I"
1.80 T !,-B/2*A," -",FSQT(-D)/2*A," I",!;G 1.1
```

The program asks for A, B and C (each answer ended with a space or ВВОД)
and loops until СТОП:

```
*G

A:3  B:7  C:2

ROOTS
 -  0.333
 -  2.000

A:3  B:4  C:5

ROOTS COMPLEX
 -  0.667 +    1.106 I
 -  0.667 -    1.106 I

A:1  B:-2  C:1

ROOTS
    1.000

A:0  B:1  C:1

NOT A QUADRATIC

A:
?25 AT  1.10
ОСТАНОВ ПО КЛАВИШЕ СТОП
*
```

### A guessing game

```
1.10 S N=FITR(50*FRAN()+51);S K=0
1.20 A !,"YOUR GUESS",X;S K=K+1
1.30 I (X-N) 1.4,1.5;T "TOO BIG";G 1.2
1.40 T "TOO SMALL";G 1.2
1.50 T "RIGHT IN ",%2,K," TRIES",!
```

FRAN() is in (−1, 1), so N is a whole number from 1 to 100. Line 1.30 goes
to 1.40 when the guess is too small, to 1.50 when it is right, and otherwise
falls through to `T "TOO BIG"`. Straight after power-on the first FRAN()
is always −0.3916, so N is 31:

```
*G

YOUR GUESS:50
TOO BIG
YOUR GUESS:25
TOO SMALL
YOUR GUESS:37
TOO BIG
YOUR GUESS:31
RIGHT IN    4 TRIES
*
```

### Factorial with a recursive FSBR

From LD §8.5:

```
1.1 A N
1.2 T !,FSBR(5,N)
1.3 Q
5.1 I (1-&) 5.2;R
5.2 S &=&*FSBR(5,&-1);R
```

Group 5 computes n! as n·(n−1)!: while & is more than 1, line 5.1 goes to
5.2, which calls group 5 again with &−1.

```
*G
:6

   720.0000*G
:10

  3628800.0*
```

### A graph

```
1.10 X FCHR(12);X FT(1,0,130);X FV(1,511,130)
1.20 X FT(1,0,30);X FV(1,0,230);X FT(1,0,130)
1.30 F I=0,64;X FV(1,8*I,130-90*FSIN(I*3.14159/32))
1.40 X FK(0,0);T "Y=SIN(X)";Q
```

Line 1.10 clears the screen and draws the X axis across the screen at Y=130;
line 1.20 draws the Y axis at the left edge and moves back to the origin.
Line 1.30 draws one period of a sine wave, amplitude 90, as 65 straight
pieces across the full width. Line 1.40 puts a title in the top left corner.
The whole picture took about 12 seconds.

### Key codes

From LD §8.3: shows the code of each key pressed, until СТОП.

```
1.1 T FCHR(-1),!;G 1.1
```

```
*G
    65.0000
    13.0000
    49.0000
```

(A, ВВОД, 1.)

## Where the manuals and the ROM disagree

Found while checking this guide in the emulator, which runs the original ROMs:

- **Precedence.** The manuals give the order `^`, `*`, `/`, `+`, `-`. The
  first three hold (`6/2*3` is 1), but `+` and `-` behave as equals
  (`5-2+1` is 4).
- **FCHR above 127.** LD §8.3 says codes are taken modulo 128; the ROM prints
  them unchanged (`X FCHR(193)` printed а, not A).
- **FT/FV range.** LD §8.7 gives 0 ≤ Y ≤ 255; points with Y above 239 were
  not drawn.
- **Line length.** LD §6.2 says 80 characters; the 80th typed character
  already gives error 18.
- **Subscripts.** One- and two-subscript elements share storage
  (`X(i,j)` is `X(i+256*j)`), which the manuals don't mention.
- **KILL.** LD §4.4 says a program can't be interrupted after KILL; СТОП still
  stopped it.
- **Error 24.** The manuals say any key stops a tape operation; only СТОП did.
- **Messages.** The ROM's wording differs slightly from the manuals'
  tables (for example code 12 reads ПОРЯДОК БОЛЬШЕ Е+-38, code 21
  НЕПРАВИЛЬНОЕ ИМЯ ФАЙЛА); the table above follows the ROM.
- **FCLK.** The ROM has a 22nd function, FCLK(), documented nowhere.

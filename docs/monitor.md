# Monitors, keyboard and screen

This is the user's guide to the system software in the ROMs of the BK-0010,
BK-0010.01 and BK-0011M: what each machine does when switched on, how to get
from BASIC or FOCAL to the machine's monitor and back, every monitor command,
the test monitors, and the keyboard and screen as a user meets them.

Nearly all of it was tried in this emulator; the few things that couldn't be
are marked. Screen output is shown as it appeared; Russian messages are
translated beside them. Numbers that are addresses or machine words are
octal, as on the BK.

Related documents: [emulator.md](emulator.md) (running the emulator, its
key mapping, tapes and disks), [machine.md](machine.md) (the hardware and its
ROMs), [basic.md](basic.md) and [focal.md](focal.md) (the languages),
[machine-code.md](machine-code.md) (writing programs),
[system-programming.md](system-programming.md) (the EMT and BOS calls behind
everything here), [hardware-reference.md](hardware-reference.md).

Sources are cited by document number:

| Cited as | Manual |
|---|---|
| M1.492.000 | BK-0010/0010.01 operating manual (руководство по эксплуатации) |
| 00001-01.32.03 | BK-0010.01 driver-monitor system, system programmer's manual |
| 00009-01.90.01 | FOCAL user manual (BK-0010.01) |
| 00001-01.35.02 | BK-0010.01 BASIC language description |
| 00015-01.32.01 | BK-0011M BOS (base operating system), system programmer's manual |
| 00015-01.34.01 | BK-0011M МСД (test and diagnostics monitor), operator's guide |
| 00008-01.32.01 | BK-0011 driver-monitor system (floppy controller) |
| 00008-01.34.04 | BK-0011 BASIC operator's guide |

Where the ROM in the emulator does something different from its manual, this
guide says so and follows the ROM.

## Contents

- [The machines at a glance](#the-machines-at-a-glance)
- [Power-on](#power-on)
- [The BK-0010 monitor](#the-bk-0010-monitor)
- [Loading a program from tape (BK-0010)](#loading-a-program-from-tape-bk-0010)
- [The BK-0010 test monitor (МСТД)](#the-bk-0010-test-monitor-мстд)
- [The BK-0011M keyboard monitor](#the-bk-0011m-keyboard-monitor)
- [МСД, the BK-0011M test monitor](#мсд-the-bk-0011m-test-monitor)
- [Booting the BK-0011M from a floppy](#booting-the-bk-0011m-from-a-floppy)
- [The keyboard](#the-keyboard)
- [The screen](#the-screen)
- [Notes on the emulator](#notes-on-the-emulator)

## The machines at a glance

| | BK-0010.01 | BK-0010 with МСТД | BK-0011M |
|---|---|---|---|
| Emulator option | `--bk0010` | `--bk0010 --focal` | (none) |
| Starts in | BASIC (`Ok`) | FOCAL (`*`) | BASIC БК11/10 (`Ok`) |
| Monitor | start-up monitor, prompt `?` | the same | BOS keyboard monitor, prompt `@` |
| Into the monitor | `MONIT` | `P M` (PASS MONITOR) | `MONIT`; or hold a key at power-on |
| Back to the language | `B` + ВВОД | `F` + ВВОД | `11;1C` then `100000G` |
| Test monitor | — | МСТД: `P T` from FOCAL, `T` from the monitor | МСД: `B` or `160000G` in the monitor (no floppy controller) |

The original BK-0010 shipped with FOCAL and the test monitor on a plug-in
block, the МСТД (мониторная система тестирования и диагностики, test and
diagnostics monitor system); the BK-0010.01 has BASIC built in
(M1.492.000 §5). The emulator's `--focal` model is a BK-0010 with that block
fitted.

## Power-on

### BK-0010.01 (`--bk0010`)

The monitor sets up the machine and calls the program in ROM at 120000,
which is BASIC (00001-01.32.03 §6). After about a second:

```
CAGLRCCC.R                                                  ЛАТ
- - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
 БЕЙСИК        (ВИЛЬНЮС,1986.07.24)

Ok
█
```

The top line is the **service line** (служебная строка), separated from the
rest of the screen by a dotted line. `ЛАТ` on the right says the keyboard
gives Latin letters; the left part belongs to BASIC (the initials of its
programmable keys, see [Programmable keys](#programmable-keys)). BASIC
("БЕЙСИК (Вильнюс, 1986.07.24)") works in the 32-column mode. This matches the
operating manual (M1.492.000 §6.2).

### BK-0010 with FOCAL (`--bk0010 --focal`)

```
                                                            ЛАТ
- - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
?00 АТ  0.00
ГОТОВНОСТЬ К РАБОТЕ
*█
```

"ГОТОВНОСТЬ К РАБОТЕ" is "ready"; `*` is FOCAL's prompt (M1.492.000 §6.3.1).
The screen is in 64-column mode.

### BK-0011M (no option)

The BOS (базовая операционная система, base operating system) version 1.8
starts (00015-01.32.01 §2.1). It prints a framed banner, `Монитор БК-0011
В1.8`, scrolls it up to the top of the screen, and then:

1. If a key is held down at that moment (about 2 seconds after power-on),
   it goes straight to the keyboard monitor.
2. Otherwise, if a floppy controller is fitted, it calls the disk ROM to boot
   from drive 0, 1, 2 or 3 (see [Booting from a floppy](#booting-the-bk-0011m-from-a-floppy)).
3. Otherwise, or if no disk boots, it calls the ROMs in turn, and BASIC
   starts:

```
БЕЙСИК БК11/10 (С) ОКБ "ЭКСИТОН"
Ok
█
```

BASIC works in 32-column mode with white text (BOS's start-up palette 15,
00015-01.32.01 §3.1.1). There is no service line.

In the emulator, press Space (or any key) as soon as the window opens, or
just after switching the machine off and on with the power light under the
screen (see [emulator.md](emulator.md#the-keyboard)), and hold it until the
`@` prompt appears. The BOS looks at the keyboard about 2 seconds (100
frames) after power-on. With a floppy controller and a bootable disk, this is
how you reach the monitor instead of booting. (Checked in a scripted run
that held a key from power-on, with and without a disk.)

## The BK-0010 monitor

The start-up monitor (пусковой монитор) of the BK-0010 and BK-0010.01 is a
small command loop in the monitor ROM at 100000 (00001-01.32.03 §6). It loads
programs from tape and starts them.

### Getting there and back

| From | Do | Notes |
|---|---|---|
| BASIC (BK-0010.01) | `MONIT` ВВОД | 00001-01.35.02 §3.1.4 |
| FOCAL (BK-0010) | `P M` ВВОД (PASS MONITOR) | 00009-01.90.01 §15.3 |
| a running program | СТОП, if the program doesn't catch it | 00001-01.32.03 §6.1 |
| a program | `RTS PC` back to the monitor, which started it with `JSR PC` | |

The monitor clears the screen, switches to 64 columns and shows its prompt, a
question mark at the start of a line:

```
? █
```

To go back, type any letter from `@`, `A` to `K` and ВВОД. The monitor
restarts the program at 120000 (00001-01.32.03 §6.1 item 6): BASIC on the
BK-0010.01 (`B` for BASIC is the usual choice), FOCAL on the BK-0010 (the
FOCAL manual says `F`). Either way the language starts afresh: a BASIC or
FOCAL program in memory is lost (00009-01.90.01 §15.3).

### How commands are typed

A command is one letter, an optional octal number, and ВВОД:

```
? M1000
```

- Only the first character is the command. Whatever follows up to ВВОД is
  scanned for octal digits; any other character (including 8 and 9) clears
  the number. To correct a mistyped address, type a space and the whole
  address again (00001-01.32.03 §6.1 says "some zeros or a space"; zeros work
  because only the last six digits count).
- Lower-case letters and Russian letters work too: the monitor ignores bit
  5 and bit 7 of the code, so `м` or `М` is the same as `M`, `с` or `С` the
  same as `S`, and so on. The manual's long names (МАГ, Линия, Старт, Пуск,
  Тест) are just the first letters (checked with `М`).
- An unknown letter does nothing; the prompt comes back.

### Commands

| Command | Manual name | Does |
|---|---|---|
| `M` [addr] | МАГ (tape) | Load a file from tape. Asks `ИМЯ?` (name?) |
| `L` [addr] | Линия (line) | Load a file from the serial line at 9600 baud |
| `S` [addr] | Старт (start) | Call the program at *addr*, or at the address in cell 264 (where the last file was loaded) |
| `P` | Пуск (run) | Call the ROM at 140000 |
| `T` | Тест (test) | Call the test monitor at 160100 (BK-0010 with МСТД) |
| `@`, `A` … `K` | — | Restart the system program at 120000 (BASIC or FOCAL) |

From 00001-01.32.03 §6.1, checked against the ROM (monit10.mac, 100300–100440).

**M (load from tape).** After `M` and ВВОД the monitor asks for the name:

```
? M
ИМЯ? XONIX
```

The name has up to 16 characters; ЗАБОЙ deletes the last one. While the tape
plays, the monitor prints the name of every other file it passes; when it
finds the file it loads it without printing its name and shows the prompt
again. A bad checksum prints `ОШИБКА` (error). The file loads at its own
address, or at *addr* if one was given and is not 0. Afterward cell 264
holds the load address and cell 266 the length (00001-01.32.03 §6.1).

An empty name does not mean "the first file": it means a file whose name is
16 spaces, and the monitor skips everything else (checked: with `M`, ВВОД,
ВВОД it printed `XONIX` and kept searching).

**L (load from the line)** receives a byte from the serial line and sends
it back (a handshake), then reads a 4-byte header (address and length, as
at the start of a `.bin` file) into cells 264 and 266 and the data after
it; `L` *addr* loads at *addr* instead. So any byte followed by a `.bin`
file is a valid transfer. Checked in the emulator, which plays the far end
from a file:

```
printf '\000' > in.bin; cat PROG.bin >> in.bin
bk --bk0010 --serial-in in.bin --serial-out out.bin
```

then `MONIT`, `L`, and `S` to start it. The handshake byte comes back in
`out.bin`.

**S (start)** calls the program with `JSR PC`, so a program can return to
the monitor with `RTS PC`. `S` alone starts the file just loaded at its load
address.

**P** and **T** are only useful with the МСТД block: on the BK-0010.01 the
addresses 140000 and 160100 are inside BASIC (`T` restarts BASIC there), and
on the BK-0010 without anything at 140000 `P` just returns to the prompt
(checked).

**СТОП** interrupts a running program (vector 4). Unless the program handles
it, the monitor resets the stack, stops the tape motor, reinitializes the
keyboard and shows the prompt (00001-01.32.03 §6.1). In BASIC, СТОП stops the
BASIC program instead: `СТОП В СТРОКЕ 10` ("stop in line 10").

## Loading a program from tape (BK-0010)

### From a recording

Programs on BK tapes are files with a name, a load address and a length. To
load one recorded in a WAV file (here Xonix, on the BK-0010.01):

1. Start the emulator with the tape: `bk --bk0010 --tape xonix.wav`.
2. In BASIC, type `MONIT` and ВВОД (Enter). The `?` prompt appears.
3. Type `M` and ВВОД. The monitor asks `ИМЯ?`.
4. Type the file's name, `XONIX`, and ВВОД. The tape motor starts and the
   recording plays (this file takes about 30 seconds; a real tape runs at
   about 1200 baud, M1.492.000 §3.3).
5. When the prompt comes back, type `S` and ВВОД. The game starts.

```
? M
ИМЯ? XONIX

? S
```

(the game's title screen replaces this: `XONIX (R) V3.00 from 27-th of
April,1987` in the service line and `Strike any key`.)

If you don't know a file's name, give a name that isn't on the tape, such as
`XYZ`: the monitor lists the files it passes. A tape with two copies of one
file gave:

```
? M
ИМЯ? XYZ
R&O-REKL1
R&O-REKL1
```

Many programs load below address 1000 on purpose: the file overwrites the
monitor's stack, so the program starts itself as soon as it has loaded (for
example Клад, which loads at 732). Don't use `S` for these.

### Fast tape

With `--files DIR`, a named file is read from `DIR/NAME.bin` instead of
the tape, instantly (see [emulator.md](emulator.md#fast-tape-files-on-the-host-disk)).
The dialog on the BK is the same:

```sh
bk --bk0010 --files software
```
```
? M
ИМЯ? XONIX

? S1000
```

The name matches `xonix.bin` regardless of case. A file that isn't in the
directory is read from the tape as usual, and an empty name always goes to
the tape.

## The BK-0010 test monitor (МСТД)

The МСТД block of the original BK-0010 holds FOCAL (120000) and the test
monitor (160000–177577, tests.rom). The test monitor has a test mode, ТК
(тестовый контроль), with five hardware tests, and a debugging mode, ТС
(«машинные коды», machine code), for working with memory in octal
(M1.492.000 §5, §6.3; 00001-01.32.03 appendix 2). Use `bk --bk0010 --focal`.

### Entering and leaving

| From | Type | Result |
|---|---|---|
| FOCAL | `P T` ВВОД (PASS TEST) | test mode, prompt `+` |
| the start-up monitor | `T` ВВОД | test mode, prompt `+` |
| test mode `+` | РУС, then `Т` `С` | ТС mode, prompt `¤` |
| ТС mode | `Т` `К` | back to test mode |
| either mode | `К` | leave: the machine restarts into FOCAL |

`К` is Russian К in ТС mode; in test mode Latin `K` works as well. Leaving
restarts the machine, so a FOCAL program is lost (00009-01.90.01 §15.3).
The ТС prompt is the BK's glyph for code 044, ¤ (the monitor manual's code
table shows it as ¤, not `$`).

Typing the Russian directive letters in the emulator: press РУС (End) first,
then hold **Shift** with the letter key, because unshifted letter keys give
lower case in РУС mode (see [The keyboard](#letters-and-case)) and the test
monitor only accepts capitals. So `ТС` is РУС, Shift+T, Shift+S. Digits are
the same in either mode. In `--type` text, write the lower-case Latin letter:
`--type 'P T\n\016ts'` enters ТС mode.

| Directive | Letter key | Directive | Letter key |
|---|---|---|---|
| А | Shift+A | Р | Shift+R |
| Б | Shift+B | С | Shift+S |
| Д | Shift+D | Т | Shift+T |
| З | Shift+Z | Ф | Shift+F |
| И | Shift+I | Х | Shift+H |
| К | Shift+K | Ц | Shift+C |
| Л | Shift+L | Ч | Shift+`` ` `` |
| М | Shift+M | Ш | Shift+[ |
| П | Shift+P | Щ | Shift+] |

### Test mode (ТК): the hardware tests

At the `+` prompt a digit starts a test (M1.492.000 §6.3):

| Key | Test | What happens |
|---|---|---|
| `1` | ТЕСТ 1: RAM and ROM | fills and checks RAM (the screen fills with patterns), then prints the ROM checksums |
| `2` | ТЕСТ 2: keyboard | names a key (`ПРОБЕЛ`, `ШАГ`, …) for you to press |
| `3` | ТЕСТ 3: parallel port | walking 1 and walking 0 through the port; needs the load block (блок нагрузок) on the port |
| `4` | ТЕСТ 4: screen | fills the screen with text until a key is pressed |
| `5` | ТЕСТ 5: tape | writes a test file and reads it back, with prompts |

Test 1 takes about 35 seconds and ends with the same result the operating
manual gives for a good machine:

```
  КОНТРОЛЬ  ПЗУ
МОДУЛЬ1 МОДУЛЬ2 МОДУЛЬ3 МОДУЛЬ4
177777  010000    НЕТ   177777
   ТЕСТ1  ЗАВЕРШЕН

+
```

"КОНТРОЛЬ ПЗУ" is ROM check, "МОДУЛЬ" module, "НЕТ" none (no ROM in that
slot), "ЗАВЕРШЕН" finished. Module 1 is the monitor, 2 FOCAL, 4 the test
ROM. A RAM fault would print `ОШИБКА ОЗУ` and the address.

Test 3 without the load block (the emulator has none) reports every bit as
faulty, as the manual's example does:

```
   ТЕСТ3.  ИНТЕРФЕЙС

БЕГУЩАЯ 1

   СТ.БАЙТ         МЛ.БАЙТ
8 7 6 5 4 3 2 1  8 7 6 5 4 3 2 1
БЕГУЩИЙ 0
...
   ТЕСТ3  ЗАВЕРШЕН
```

Test 5 starts with `ПЕРЕМОТАЙТЕ ЛЕНТУ В НАЧАЛО` (rewind the tape) and, after
ВВОД, `НАЖМИТЕ КЛАВИШИ МАГНИТОФОНА "ПУСК" И "ЗАПИСЬ"` (press play and
record). It needs the same tape written and read in one session, which the
emulator's separate `--record` and `--tape` files don't allow; not tried to
the end.

The keyboard test in this ROM asks for keys by name, one at a time, rather
than showing the tables the operating manual pictures.

### ТС mode: working in machine code

The directives are single Russian letters (plus `G` and some punctuation),
typed without ВВОД. Most take an octal number typed in front of them: with a
number the directive sets or writes, without it the directive shows a value
(00001-01.32.03 appendix 2). Numbers have up to six digits; ЗАБОЙ deletes the
last digit typed. The monitor keeps a current address *A* and a length *Д*
(in bytes).

| Directive | Does |
|---|---|
| *n*`А` / `А` | set the current address / show it: `=001000` |
| *n*`Д` / `Д` | set the length in bytes / show it |
| `И` | show the word at *A*; then *n*`И` writes *n* there |
| `Б` | the same for the byte at *A* |
| *n*`,` / `,` | write *n* at *A* (with *n*), then step *A* by 2 and show that word |
| *n*`-` / `-` | the same, stepping *A* back by 2 |
| *n*`.` / `.` | byte version of `,`: step *A* by 1 and show that byte |
| *n*`:` / `:` | byte version of `-`: step *A* back by 1 |
| *n*`Р` | fill *A* … *A*+*Д* with the word *n* |
| *n*`Л` | list *n* bytes from *A*, eight words to a line |
| *n*`С` | compare *A* … *A*+*Д* with the block at *n*; list the words that differ |
| *n*`П` | copy *A* … *A*+*Д* to *n* |
| `Х` | show the checksum of *A* … *A*+*Д* |
| *n*`Ц` / `Ц` | write *n* to *A* (or read *A*) over and over, until СТОП |
| `Щ` | allow writes below 1000 (see below) |
| *n*`G` | run the program at *n* (Latin G: press ЛАТ first) |
| `МП`, `МС` | tape motor on, off |
| `МЗ` | write a file to tape: asks `АДРЕС=`, `ДЛИНА=`, `ИМЯ=` |
| `МЧ` | read a file: asks `АДРЕС=` (0 = the file's own) and `ИМЯ=` |
| `МФ` | find a file without loading it: asks `ИМЯ=` |
| `ТК`, `ТС` | switch to test mode or ТС mode |
| `К` | leave the test monitor |

A session, as typed and shown (the monitor echoes each key; ¤ is the
prompt):

```
+ТС
¤1000АА=001000
¤100ДД=000100
¤7Р
¤Д=000100
¤1000А20Л
000007  000007  000007  000007  000007  000007  000007  000007
¤120000А20000ДХ=010000
```

That set *A* to 1000 and *Д* to 100 bytes, filled them with 7, listed 20
bytes, and took the checksum of FOCAL's ROM (the 010000 of test 1). Opening
cells:

```
¤1000АИ000000 123ИИ000123

¤1000А,000000 ,000000 ,000000 5,000000 ,000000

¤1006А-000000 -000000 -000123
```

`И` showed 000000; `123И` wrote 123; `И` showed it. Each `,` showed the next
word; `5,` wrote 5 at 1006 before stepping on. ВВОД ends the line.

Comparing and copying:

```
¤1000А20Д1Р
¤2040С
001000 :000001   002040 :000000
001002 :000001   002042 :000000
...
001016 :000001   002056 :000000
¤
```

When the blocks are equal, `С` prints nothing and the prompt comes back.
`3000А20Д5Р`, `1000П`, `1000С` copied 3000–3020 to 1000 and found the copies
equal.

A program: the manual's example prints the letter К. Enter it with `,`, then
run it with `G` (press ЛАТ for the G):

```
¤1000А12700,000000 353,000000 104016,000000 0,000000

¤1000GК
¤
```

The words are `MOV #353,R0`, `EMT 16` (print the character in R0, code 353 =
К) and `HALT`. `G` switches the keyboard to ЛАТ, the program prints К, and the
`HALT` returns to the test monitor (so does СТОП). The manual's version ends
with `1` (`WAIT`) instead of `0`; then the machine waits until СТОП.

Tape, with fast tape (`--files DIR`) or a recording:

```
¤1000А20Д1Р
¤МЗ
НАЖМИТЕ КЛАВИШИ МАГНИТОФОНА
"ПУСК" И "ЗАПИСЬ"

 АДРЕС=1000

 ДЛИНА=20

 ИМЯ=TEST

¤МЧ
НАЖМИТЕ КЛАВИШУ МАГНИТОФОНА
"ПУСК"

 АДРЕС=0

 ИМЯ=TEST

ЗАГРУЖЕН ФАЙЛ TEST
¤МФ
...
 ИМЯ=TEST

ОСТАНОВ ПОСЛЕ ФАЙЛА TEST
```

("Press the recorder's play and record keys", "file TEST loaded", "stopped
after file TEST".) Type the name in ЛАТ if it is Latin.

**Write protection.** The test monitor refuses to write below address 1000
(the vectors and the system's cells): a write there prints `ЗЩ` (защита,
protected) and changes nothing.

```
¤100АИ132224 5И  ЗЩ
¤Щ
¤
¤700АИ177777 5ИИ000005
```

The manual calls the unprotect directive Ш; in this ROM it is **Щ** (Ш is
accepted but does nothing in ТС mode). СТОП restores the protection
(00001-01.32.03 appendix 2).

**Differences from the manual.** The manual's table has `:` as "byte with
increment" and `.` as "byte with decrement". The ROM does the opposite (`.`
steps up, `:` steps down), as shown above. The ROM also accepts directives
the manual does not describe: `ТД` (another mode, which `ТС` and `ТК` leave),
`З`, and `О` and `В` (which act only in `ТД` mode) work with registers at
177670–177672; the emulator has nothing at those addresses, so in it `ТД`
just shows the prompt again. `Ч` does nothing, and `Ю` takes the next
directive from a table of the user's own in RAM at 1300 (handlers at 1400).
They are left out here.

## The BK-0011M keyboard monitor

The BOS keyboard monitor (клавиатурный монитор) is modeled on the console
ODT of the Электроника-60 and ДВК: you open memory cells by address, change
them, and run programs (00015-01.32.01 §2.2). Its prompt is `@` at the start
of a line. Numbers are octal; leading zeros can be left out.

### Getting there and back

| From | Do |
|---|---|
| BASIC БК11/10 | `MONIT` ВВОД |
| power-on | hold any key (Space) until `@` appears |
| a machine-code program | СТОП, or a `HALT` instruction (00015-01.32.01 §1.3) |
| a BOS program | the `.BEXIT` call (EMT 1) |

On entry the monitor prints the address where it took over (the interrupted
program counter) and the prompt. From BASIC:

```
065166
@█
```

From power-on with a key held, the banner stays at the top and the address is
000000.

The BK-0011 BASIC operator's guide (00008-01.34.04 §2.2) gives `SYSTEM` for
this; the BASIC in this ROM set (БК11/10) answers `SYSTEM` with `ОШИБКА 2`
(error 2) and uses the BK-0010's `MONIT` instead. The way back is the one the
guide gives:

```
@11;1C
@100000G
```

`11;1C` maps ROM page 11 (BASIC's second page) at 100000 and `100000G`
starts it. BASIC starts afresh; the program in memory is lost.

### Commands

| Command | Does |
|---|---|
| *addr*`/` | open the word at *addr*: shows its contents |
| *addr*`\` | open the byte at *addr* |
| `R`*n*`/` | open register R*n* (0–7); `RS/` opens the processor status word |
| `/`, `\` | open the last cell again |
| *value* then ВВОД | write *value* (if typed) and close the cell |
| *value* then ↓ | write, then open the next word, byte or register |
| *value* then ↑ | write, then open the previous one |
| *value* then → | write, then open *addr* + contents + 2 (the operand of a PC-relative instruction, mode 67) |
| *value* then ← | write, then open *addr* + 2 × low byte (signed) + 2 (the target of a branch) |
| *value* then `@` | write, then open the word whose address is in this cell |
| *n*`;`*m*`C` | map page *n* at 40000 (*m* = 0) or 100000 (*m* = 1). Pages 0–7 are RAM, 10–13 ROM 0–3 |
| *addr*`G` | run the program at *addr*; `G` alone runs from the address in cell 40 |
| `P` | proceed: continue the program stopped by `HALT` or СТОП |
| ШАГ | step: run one instruction of the program and come back |
| *addr*`L` | load a file from tape (at *addr*, or at its own address) |
| *from*`-`*to*`;U` or *from*`+`*length*`;U` | save memory to a tape file |
| *n*`M` | tape motor on (*n* ≠ 0) or off |
| *n*`B` | boot from floppy drive *n*; `B` alone tries every drive. Without a floppy controller this starts МСД |
| ЗАБОЙ | delete the last digit typed |
| СБР | clear the screen |
| СУ+E … СУ+@ | direct mode: pass ESC and the following characters to the screen driver |

From 00015-01.32.01 §2.2.1, checked against the command table in the ROM
(b11m_ext.mac, MONMAI at 120002; the table of command characters is at
132524) and in the emulator. The ↑ ↓ ← → are the cursor keys (codes 032,
033, 010, 031).

Examining and changing memory. The monitor echoes what you type, except the
arrow keys and ВВОД; the right-hand column here shows the key that ended
each line and is not on the screen:

```
@1000/000000 123          ↓
001002 /000000            ↓
001004 /000000            ВВОД
@1000/000123              ВВОД
@1000\123                 ↓
001001 \000               ВВОД
@R0/030201                ↓
R1 /002430                ↓
R2 /000000                ВВОД
@RS/065340                ВВОД
@140000/106427            →
046431 /000000            ВВОД
@1000/000123              ↑
000776 /000411            ВВОД
@
```

`123` then ↓ wrote 123 at 1000. `→` opened 140000 + 106427 + 2, cut to 16
bits. `@` is echoed:

```
@1000/002000 @
002000 /000000
```

`@` is Shift+2 on the PC keyboard (the Ю key on the BK's).

Running and stepping a program: `MOV #1,R0`, `INC R0`, `HALT` at 1000:

```
@1000/000000 12700
001002 /000000 1
001004 /000000 5200
001006 /000000 0
@1000G
001010
@R0/000002
@P
001012
@
```

`G` ran to the `HALT` and the monitor showed the address after it; `P`
continued from there (into the next `HALT`). With ШАГ (F8 in the emulator)
after `R7/1000`, each press runs one instruction and shows the new PC:

```
@R7/065166 1000
@
001004
@
001006
```

A program in a loop (`1000/777`, a branch to itself) started with `1000G`
and stopped with СТОП shows `001000` and the prompt. Not every program can be
continued with `P` after СТОП, for instance if СТОП came during a BOS call
(00015-01.32.01 §1.3).

### Tape

`L` asks for the name (`Имя файла:`). Up to 16 characters; ВВОД alone loads
the first file on the tape; `?` lists the files on the tape until you press
СТОП. For each file found it shows the name and the address range:

```
@L
  Имя файла: TEST

Файл: TEST
 001000 - 001020

@
```

After `L`, `G` alone starts the program at its load address (the monitor
puts it in cell 40). `L?` on a tape with two copies of one file:

```
@L
  Имя файла: ?

Файл: R&O-REKL1
 000316 - 001446

Файл: R&O-REKL1
 000316 - 001446
```

`U` asks for the name the same way and writes memory to tape in the BK-0010
format, so a BK-0010 reads it with `M`:

```
@1000+20;U
  Имя файла: TEST1

@
```

The Xonix recording used in [Loading a program from
tape](#loading-a-program-from-tape-bk-0010) was made this way: load the
program, `MONIT`, `1000+7000;U`, name `XONIX`, with `--record xonix.wav`.

Both `L` and `U` also work with fast tape (`--files`): `L` with the name
`TEST` loads `TEST.bin` from the directory and shows its name and address
range as above (checked).

### Setting up the screen from the monitor

СУ+E sends ESC (code 033) to the screen driver and puts the monitor in direct
mode: everything typed after it goes to the screen driver, until СУ+@ (code
000) (00015-01.32.01 §2.2.1). With the ESC sequences of
[The screen](#the-bk-0011m-screen) this changes the text mode and colors.
For example (checked):

| Typed | Effect |
|---|---|
| СУ+E `1` СУ+@ | 64 columns |
| СУ+E `2` СУ+@ | 80 columns |
| СУ+E `0` СУ+@ | 32 columns, four colors (the manual's "color mode") |
| СУ+E `;21` СУ+@ | background color 2, text color 1 |
| СУ+E `:` *p* СУ+@ | palette *p* (the low 4 bits of the next character) |
| СУ+E `T` СУ+@ | inverse characters (`U` ends it) |

The change applies to what is printed next. In the emulator СУ is right
Ctrl (or click СУ on the on-screen keyboard); in a scripted run type the
codes: `--type '\0051\000'` for 64 columns.

The palette can also be set by writing the palette register directly:
`177662/` and then `041400` ВВОД selects palette 3 (bits 8–11 hold the
palette number; keep bit 14 set, as BOS does, so the 50 Hz timer interrupt
stays off).

## МСД, the BK-0011M test monitor

The test and diagnostics monitor (мониторная система тестовой диагностики)
version 1.5 sits in a ROM at 160000 (b11m_mstd.rom). It is there only when no
floppy controller is fitted, because the floppy ROM uses the same addresses
(run the emulator without `--fdd`/`--disk`). Start it from the keyboard
monitor with `B` or `160000G` (00015-01.34.01 §1); both work.

After a moment the menu appears:

```
     Мониторная    Система
     Тестовой Диагностики

            БК0011М


T0 -цепочка тестов
T1 -ОЗУ                       +
T2 -ПЗУ                       +
T3 -КЛАВИАТУРА                +
T4 -ИНТЕРФЕЙС МГ              +
T5 -ПОРТ Вв/Выв.              +
T6 -ТАЙМЕР                    +
T7 -ПАЛИТРА                   +
  Справка  по  МСТД

ВЫХОД
```

↑ and ↓ move the highlighted line (they wrap around); ВВОД runs it. ТАБ
switches a test's `+` on or off; T0 (цепочка, chain) runs the marked tests in
order, stopping where the operator must act (T3, T4) or on an error. Putting
T0 itself in the chain makes it loop (00015-01.34.01 §2; the help screen says
the same). `ВЫХОД` (exit) restarts BOS, which starts BASIC again.

| Test | Shows |
|---|---|
| T1 RAM (ОЗУ) | a map of the four 16 KB windows and the eight RAM pages; first the page mapping, then each page's contents are checked. An error stops the test with the address |
| T2 ROM (ПЗУ) | the checksum of every ROM found: in the emulator 140000: 047736, 160000: 064411, page 10: 002027 and 003564, page 11: 167273 and 117615; pages 12 and 13 empty |
| T3 keyboard | a picture of the keyboard with `КОД` and `ВЕКТОР` (code, vector) of the last key below it; press the highlighted key (or pair of keys) and it disappears (manual) |
| T4 tape (МГ) | `Включите режим ЗАПИСЬ` (start recording) and a `ГОТОВО` (ready) line to press ВВОД on; then, per the manual, it writes a file `ЭТАЛОН`, asks you to play it back and compares |
| T5 parallel port | walking 0 and 1 through the port; needs the load block. Without it (as in the emulator): `ОШИБКА данных` / `ПРОДОЛЖИТЬ?` (data error, continue?) |
| T6 timer | a stopwatch in a clock face, `БК0011М 00:07 кварц` |
| T7 palettes | a table of the 16 palettes; ↓ and ↑ select one and the whole screen shows in it |
| Справка | version 1.5, December 1989, author А. В. Трапезников, Экситон factory, Павловский Посад; how ТАБ marks tests |

Checked in the emulator: the menu, T1, T2, the first screens of T3 and T4,
T5, T6, T7, the help screen, ВЫХОД, and ТАБ (sent as code 211, which is
what the PC Tab key sends).

## Booting the BK-0011M from a floppy

With a floppy controller (`--fdd`, or `--disk FILE`, which implies it) the
ROM at 160000 is the disk ROM (disk_327.rom, which announces itself as `FDD/HDD
BIOS 327 V12 (c) JD/DWG!`). At power-on BOS calls it (00015-01.32.01 §2.1):

1. It prints its banner under the BOS banner.
2. It tries drives 0, 1, 2 and 3 in turn. For each it reads block 0 (sector
   1 of track 0, 512 bytes) into memory at 1000.
3. The block counts as a boot loader if its first word is 000240 (`NOP`) and
   its second word is not 000005. Then the ROM sets the stack to 1000, puts
   the drive number in R0 and calls 1000 (00008-01.32.01 §3.6.1; the `NOP`
   test from the ROM, disk_327.mac 160102).
4. If no drive has a bootable disk, it returns, and BOS starts BASIC. With no
   disk at all this takes about 16 seconds in the emulator.

So a bootable disk needs a boot loader in its first 512 bytes that starts with
`NOP`, runs at 1000, and loads the rest of the system itself using the disk
ROM's calls (see [system-programming.md](system-programming.md)). The disk
images `--disk` accepts are described in [emulator.md](emulator.md#disks).

```sh
bk --disk sabot2-v44.dsk
```

boots the ANDOS disk operating system from that image ("Дисковая
Операционная Система … Версия 3.30").

From the keyboard monitor, `0B` boots drive 0 and `B` tries all drives
(00015-01.32.01 §2.2.1). Checked: with a disk inserted and a key held at
power-on, `0B` at the `@` prompt booted ANDOS. The BK-0011 driver manual
also gives `160000G` (automatic) and `R0/` *n* ВВОД `160002G` (drive *n*)
(00008-01.32.01 §3.6).

## The keyboard

### Registers

Like a typewriter, the BK keyboard has registers that change what the keys
type (00001-01.32.03 §7.1.1, 00008-01.34.04 §3.1):

| Key | Does | In the emulator |
|---|---|---|
| ЛАТ | Latin letters (the service line shows `ЛАТ`) | Home (or F11) |
| РУС | Russian letters (`РУС`) | End (or F10) |
| ЗАГЛ, СТР | capitals, small letters (latching) | no key; Shift gives the other case |
| нижний регистр (Shift) | while held: the lower symbol on digit and punctuation keys, the other case on letters | Shift, which gives the US keyboard's symbols ([emulator.md](emulator.md#the-keyboard)); ⇧ on the on-screen keyboard gives the BK's |
| АР2 | while held: the alternate register (codes with the high bit, pseudographics, screen modes, programmable keys) | left Ctrl |
| СУ | while held: control codes (the letter's code AND 037) | right Ctrl |

ЛАТ and РУС stay until the other is pressed; Shift, АР2 and СУ act only while
held.

### Letters and case

In ЛАТ mode a letter key gives the capital (code 101–132) and Shift+letter
the small letter. In РУС mode it is the other way round in the emulator:
the letter key gives the **small** Russian letter and Shift+letter the
capital (checked on both machines: РУС then `abcABC` typed `АБЦабц`). The
Russian letters pair with the Latin keys as in KOI-7: A а, B б, C ц, D д,
E е, F ф, G г, H х, I и, J й, K к, L л, M м, N н, O о, P п, Q я, R р, S с,
T т, U у, V ж, W в, X ь, Y ы, Z з, and [ ш, ] щ, \ э, ^ ч, @ ю, _ ъ.
On a real BK the ЗАГЛ and СТР keys choose the case instead
(00009-01.90.01 §2.4).

### Special keys

Codes are octal. Keys marked 274 reach the program through the second
keyboard vector (АР2 and function keys, 00001-01.32.03 §7.2).

| BK key | Code | What it does | PC key |
|---|---|---|---|
| ВВОД (enter) | 012 | ends a line. The BK-0010 driver turns it into 015 if cell 262 is not 0 | Enter |
| ЗАБОЙ (delete back) | 030 | deletes the character before the cursor | Backspace |
| ← → ↑ ↓ | 010, 031, 032, 033 | move the cursor one place | arrow keys |
| =\|=>\| | 213 (274) | clears from the cursor to the end of the line (the BK-0010 driver passes it on as 231) | F3 |
| \|<== | 026 | deletes the character at the cursor; the rest of the line moves left | F4 |
| \|==> | 027 | inserts a space at the cursor; the rest of the line moves right | F5 |
| ТАБ | 011 (274) | to the next tab stop | Tab |
| ВС | 023 | deletes the cursor's line: the screen below moves up one line | right Ctrl+S |
| СБР (reset) | 014 | clears the screen and homes the cursor | F9 |
| ПОВТ (repeat) | 201 (274) | while held, repeats the last character | F1 |
| КТ (end of text) | 003 | a code for programs; no effect on the screen | F2 |
| ИНД СУ | 202 (274) | switches "show control codes" on or off (indicator `ИСУ`) | F6 |
| БЛОК РЕД | 204 (274) | switches "editing lock" on or off (indicator `БЛР`) | F7 |
| ШАГ (step) | 200 (274) | a code for programs (the BK-0010 driver passes 220); single step in the BK-0011M monitor | F8 |
| СТОП | — | interrupts the processor (vector 4): stops a program, returns to the monitor | Esc (or F12) |
| ПРОБЕЛ | 040 | space | Space |

From 00001-01.32.03 §7–8 and appendix 3 and 00015-01.32.01 appendix 1.
Checked on the BK-0010.01 in the monitor, whose echo shows each key's effect;
the ВВОД→015 and ШАГ→220 conversions are read from the ROM, not tried. The
BOS manual's table labels 010 "right" and 031 "left"; the BOS monitor, its
own screen-code table and the BK-0010 all use 010 for ← and 031 for →.

**СУ codes.** With СУ held, a letter key gives its code AND 037. The screen
driver uses these (00001-01.32.03 §8.3.2; checked on the BK-0010 except the
bell):

| Keys | Code | Does |
|---|---|---|
| СУ+@ | 000 | BK-0010: pauses the computer until the next key (output stops) |
| СУ+G | 007 | bell |
| СУ+M | 015 | BK-0010: sets a tab stop at the cursor (a mark appears in the line under the service line) |
| СУ+P | 020 | BK-0010: clears the tab stop at the cursor |
| СУ+R | 022 | cursor to the top left |
| СУ+T | 024 | inserts a line: the screen below the cursor moves down one line |
| СУ+U | 025 | cursor to the start of the next line |
| СУ+\\, СУ+], СУ+^, СУ+_ | 034–037 | cursor diagonally: ↖ ↗ ↘ ↙ |
| СУ+E | 005 | BK-0011M monitor: direct mode (ESC) |

The manual's text and its code table name some of these keys differently
(СУ/П or СУ/Р for 022); the codes are what count.

### Programmable keys

АР2 with a digit types a programmed string (up to ten keys, set with EMT 12
on the BK-0010, `.BSFUN` on the BK-0011M; 00001-01.32.03 §7.3.4). Both BASICs
program them at start-up: АР2+1 … АР2+4 typed `COLOR`, `AUTO`, `GOTO` and
`LIST` on both machines, and АР2+5 typed `RUN` and ВВОД on the BK-0010.01
(checked). The BK-0010.01's service line, `CAGLRCCC.R`, begins with their
initials. See [basic.md](basic.md).

### Repeat

The BK keyboard has no automatic repeat: holding a key types it once (checked
on both machines). Hold ПОВТ (F1) to repeat the last character, about 17 a
second in the emulator (00001-01.32.03 §7.2). The operating manual warns not
to use ПОВТ in the ГРАФ, ЗАП and СТИР modes (M1.492.000 §6.4).

### АР2 combinations on the BK-0010

With АР2 held, the BK-0010's keyboard driver gives these
(00001-01.32.03 §8.3; checked in the emulator). ⇧ is the BK's register
key; since Shift on the PC keyboard gives the US symbols, the PC column
names the key that types the same symbol:

| BK keys | PC keys | Code | Does |
|---|---|---|---|
| АР2+letter | left Ctrl+letter | 240–277 | pseudographics (table and box pieces) |
| АР2+digit | left Ctrl+digit | — | the programmable key of that number |
| АР2+; | left Ctrl+; | 233 | switch between 64 and 32 characters a line |
| АР2+- | left Ctrl+- | 235 | invert the whole screen |
| АР2+, | left Ctrl+, | 234 | inverse characters on/off (indicator `ИНВ`) |
| АР2+/ | left Ctrl+/ | 237 | underlined characters on/off (`ПОДЧ`) |
| АР2+: | left Ctrl+Shift+; | 232 | cursor off/on (output is faster without it) |
| АР2+. | left Ctrl+. | 236 | rewrite the service-line indicators in the current mode (not checked) |
| АР2+СБР | left Ctrl+F9 | 214 | extended memory mode (see below) |
| АР2+⇧+1, 2, 3, 4 (`!` `"` `#` `¤`) | left Ctrl with Shift+1, Shift+', Shift+3, Shift+4 | 221–224 | text color red, green, blue, black (32-column mode, color monitor) |
| АР2+⇧+5 (`%`) | left Ctrl+Shift+5 | 225 | text graphics mode ГРАФ on/off |
| АР2+⇧+6 (`&`) | left Ctrl+Shift+7 | 226 | ЗАП: draw while moving (in ГРАФ) |
| АР2+⇧+7 (`'`) | left Ctrl+' | 227 | СТИР: erase while moving (in ГРАФ) |

232, 233, 235 and 236 go straight to the screen driver; the others reach
the program, which normally prints them.

The BK-0011M's BOS has its own keyboard driver. In BASIC БК11/10, АР2+;
switches between 32 and 64 columns, but АР2+- prints a pseudographic
character instead of inverting the screen, and ИНД СУ and БЛОК РЕД had no
visible effect (checked); in the keyboard monitor АР2+; does nothing. On
the BK-0011M, use the ESC sequences instead.

## The screen

### The BK-0010 screen

The BK-0010 shows 256 lines of 512 dots, or 256 dots of two bits each on a
color monitor (M1.492.000 §3.3; 00001-01.32.03 §8.1). The text screen has 25
lines: the service line at the top and 24 for text. A character is 8 dots
wide and 10 lines high in the 64-column mode; the 32-column mode uses
double-width characters.

**The service line** shows the driver's modes on the right: `ЛАТ`/`РУС`, and
`ИНВ`, `ПОДЧ`, `ИСУ` (ИНД СУ), `БЛР` (БЛОК РЕД), `ГРАФ`, `ЗАП`, `СТИР`; tab
stops are marked in the dotted line under it. Programs can write to its left
part (EMT 22). A screen with several modes on:

```
                                       БЛР ИСУ ИНВ ПОДЧ ЛАТ
```

**Modes**, switched with АР2 keys or by a program printing the codes
(00001-01.32.03 §8.3.2):

- **64/32 columns** (233). In 32 columns, on a color monitor, text can be
  red, green, blue or black (221–224). The four colors, brightest first,
  are red, green, blue and black; the colors are fixed (as BK-0011M palette
  0). The operating manual recommends 32 columns on a color TV (M1.492.000
  §6.1).
- **Inverse screen** (235): light background, dark text. In color, the
  background takes the text color and the text the background color.
- **Inverse characters** (234) and **underlined characters** (237).
- **Cursor off** (232).
- **Show control codes** (ИНД СУ, 202): control codes are printed as
  inverse capital letters instead of acting: КТ as C, ВВОД as J (the line
  feed still happens), СБР as L (with БЛОК РЕД also on); the bell code shows
  as G only with БЛОК РЕД also on.
- **Editing lock** (БЛОК РЕД, 204): editing codes are printed as the
  symbols on their keys (←, →, ↑, ↓ …) instead of moving the cursor, and СБР
  and the bell are blocked. ИНД СУ, БЛОК РЕД, РП and ВВОД still work.
- **Extended memory** (РП, 214): the screen shrinks to the service line and
  4 text lines at the top, freeing 12 KB so that programs can use RAM up to
  67777. To leave it, clear the screen (СБР) and press АР2+СБР again.
- **Text graphics** (ГРАФ, 225): a cross-shaped graphics cursor. Typing a
  decimal number and then an arrow draws a line of that many dots (with ЗАП
  on) or erases one (with СТИР); without a number an arrow moves one dot.
  ГРАФ again leaves the mode. Checked: ГРАФ, ЗАП, then `100` →, `50` ↓,
  `100` ←, `50` ↑ drew a 100 × 50 dot rectangle, with `ЗАП` in the service
  line.

**The cursor** is a solid block. Text that reaches the bottom line scrolls
the screen up a line (in hardware, through the scroll register 177664).

### The BK-0011M screen

BOS draws text in three modes (00015-01.32.01 §2.7):

| Mode | Dots | Colors | Characters a line |
|---|---|---|---|
| ESC `0` | 256 a line | 4 | 32 |
| ESC `1` | 512 a line | 2 | 64 |
| ESC `2` | 512 a line | 2 | 80 (6-dot characters) |

BOS starts in 32 columns, palette 15, white text on black, with a block
cursor (00015-01.32.01 §3.1.1). There is no service line. Characters can
also be double width, double height (printed in two halves), underlined or
inverse; the cursor can be a block or an underline; scrolling can be smooth.

**Colors.** In the 4-color mode each dot is one of four colors, and the
**palette** (0–15, set in bits 8–11 of register 177662) decides which colors
those are. Color 0 is always black. The manuals don't list the palettes; the
emulator uses bkbtl's values:

| Palette | Color 1 | Color 2 | Color 3 |
|---|---|---|---|
| 0 | blue | green | red |
| 1 | yellow | magenta | red |
| 2 | cyan | blue | magenta |
| 3 | green | cyan | yellow |
| 4 | magenta | cyan | white |
| 5 | white | white | white |
| 6 | dark red C00000 | darker red 8E0000 | red |
| 7 | C0FF00 | 8EFF00 | yellow |
| 8 | C000FF | 8E00FF | magenta |
| 9 | 8EFF00 | 8E00FF | 8E0000 |
| 10 | C0FF00 | C000FF | C00000 |
| 11 | cyan | yellow | red |
| 12 | red | green | cyan |
| 13 | cyan | yellow | white |
| 14 | yellow | green | white |
| 15 | cyan | green | white |

(RGB hex where the color has no simple name.) МСД's T7 test shows them all;
on a black-and-white monitor they look the same (00015-01.34.01 §2.8).

**ESC sequences.** Programs set modes by printing ESC (033) and a character
(00015-01.32.01 appendix 2). The ones a user is most likely to want, typed
from the keyboard monitor with СУ+E … СУ+@:

| Sequence | Does |
|---|---|
| ESC `0` / `1` / `2` | 32 / 64 / 80 columns |
| ESC `3` / `4` | double / single width |
| ESC `8` / `9` | cursor on / off |
| ESC `:` *p* | palette *p* |
| ESC `;` *b* *t* | background color *b*, text color *t* |
| ESC `N` / `O` | underline on / off |
| ESC `T` / `U` | inverse on / off |
| ESC `E` | clear the screen |

Checked: `0`, `1`, `2`, `:`, `;`, `T`. The full list, with cursor
positioning and line editing, is in
[system-programming.md](system-programming.md).

## Notes on the emulator

Things that differ from a real BK, found while writing this guide:

- **СУ is right Ctrl** (so ВС, code 023, is right Ctrl+S); on keyboards
  without one, click СУ on the on-screen keyboard (it latches for the next
  key) or bind a key in `bk-keys.ini` ([emulator.md](emulator.md#remapping-keys)).
  In scripted runs type the codes (`--type '\005'` for СУ+E).
- **Shift gives the US keyboard's symbols**, not always the BK's: `@`,
  `^` and `*` are Shift+2, Shift+6 and Shift+8, and `` ` `` and `~` are on
  the backquote key. The test monitor's `МЧ` needs a capital Ч, code 376:
  РУС, then Shift+`` ` ``. For АР2 with a shifted BK key, see
  [АР2 combinations](#ар2-combinations-on-the-bk-0010).
- **Capital Russian letters need Shift.** On the on-screen keyboard, click
  the register key ⇧ first: it latches for the next key.
- **No load block for the port tests.**
- **Power-on.** The emulator powers on, a cold start with RAM cleared, when
  it starts and when you switch it on with the power light. A key held down
  before that moment doesn't count; press it after power-on, within 2
  seconds, to reach the BK-0011M monitor. `--type` can't do it, since it
  types after booting.

# Tape recordings

`protected/` holds clean recordings of 19 copy-protected games, with their
own README.

Real cassette recordings of a series of BK-0010.01 game tapes ("sets"),
kept for work on damaged tapes (issue #8). The `survey-*.mp3` files are the
first 1.2 MB (50 seconds) of each original MP3; `t3-005.mp3` is whole. Each
`.txt` is the list of programs on that cassette, as published with it.
`CIRCUS.BIN.tap` is a tape image in another emulator's format, which `bk`
doesn't read.

## Playing one

    bk --bk0010 --tape software/tapes/survey-8.mp3

then `MONIT` in BASIC, and at the monitor's `?` type `M`, the file's name
from the table, and ВВОД. The tape plays while the monitor looks for the
file (the emulator runs fast meanwhile), and the program starts itself.
Inside the emulator: start `bk --bk0010`, open the recording with Alt+O
(Cmd+O on a Mac), then `MONIT`, `M` and the name.

Most of these sets start with R&O's advert: a 600-byte loader (at 000316,
named `R&O-REKL1`) that starts itself, loads the advert program (7220 bytes
at 000732, under the same name), checks it, and runs it; a bad copy prints
`ОШИБКА` and wipes memory. Set 18 starts with the same loader, named
`BARBARIAN`. Where a sample has the advert program whole, `M` loads it
directly.

| File | Set | What the ROM reads | `M` and the name |
|---|---|---|---|
| `survey-8.mp3` | 13 | the advert, whole; a second copy cut off | `R&O-REKL1`: the advert screen (checked) |
| `survey-9.mp3` | 6 | the advert, whole; a second copy cut off | `R&O-REKL1`: the advert screen (checked) |
| `survey-10.mp3` | 12 | the advert, whole; a second copy cut off | `R&O-REK1`: the advert screen (checked) |
| `survey-5.mp3` | 14 | the advert, cut off after 6970 of its 7220 bytes | doesn't load |
| `survey-12.mp3` | 2 | the loader, twice | `R&O-REKL1`: the loader starts, finds its program cut off, prints `ОШИБКА` and wipes memory |
| `survey-4.mp3` | 17 | the loader, twice | `R&O-REKL1`: the loader starts, then crashes, its program being past the end of the sample |
| `survey-6.mp3` | 18 | the loader, as `BARBARIAN` | `BARBARIAN`: as on set 2, `ОШИБКА` |
| `survey-2.mp3` | — | nothing: the sample is silent for its first 40 seconds | — |
| `survey-7.mp3` | — | no header, though the signal looks like the others' | — |
| `survey-11.mp3` | — | no header: a weak, noisy signal | — |
| `t3-005.mp3` | — | no header, on the BK-0010 or (with `L?`) the BK-0011M: its pulses are about 2.3 times as long as the other recordings', a format neither ROM reads | — |

The loaders fail the ROM's checksum check. On set 2 the checksum word the
loader carries is the advert program's (022042), not its own. A real BK
never makes the check, as the loader starts itself first.

`bk --wav2bin software/tapes/survey-8.mp3 DIR` writes every file the ROM
reads to `DIR` (a cut-off one as `NAME-part.bin`), and
`cargo run --release -p romdis` lists them all in `listings/software/tapes/`.

## The sets

| File | First lines of its list |
|---|---|
| `t3-005.mp3` | PARA 2 COPY 1 |
| `survey-2.mp3` | FLIHT REVERSI |
| `survey-4.mp3` | ИГРОВЫЕ ПРОГРММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ № 17 |
| `survey-5.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМЛПЕКТ № 14 |
| `survey-6.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ № 18 |
| `survey-7.mp3` | INDIANA JONES JONES1 |
| `survey-8.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМЛПЕКТ № 13 |
| `survey-9.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ № 6 |
| `survey-10.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ № 12 |
| `survey-11.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ№9 |
| `survey-12.mp3` | ИГРОВЫЕ ПРОГРАММЫ ДЛЯ БК-0010.01 КОМПЛЕКТ №2 |

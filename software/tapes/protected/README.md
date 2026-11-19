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

The WAVs are kept gzipped (a hundredth of their size); `bk` reads them as
they are. To load one, as the page says:

    bk --bk0010 --tape software/tapes/protected/Expert.wav.gz

then `MONIT` in BASIC, and at the monitor's `?` type `M`, the name from the
table, and ВВОД. The game's loader takes over from the ROM. Inside the
emulator: start `bk --bk0010`, open the recording with Alt+O (Cmd+O on a
Mac), then `MONIT`, `M` and the name. For a game on several recordings, open
the next one with Alt+O when the game asks for it.

| Game | Author, year | Recordings | Name for `M` | In `bk` |
|---|---|---|---|---|
| Bally | BIL corp., 1990 | `Bally` | `BALLY` | loads: "1 GAME BALLY" |
| Destroyer | AD corp., Laser software, 1992 | `Destroyer` (title), `Destroyer-1-City` … `Destroyer-4-End` | `DESTROYER` | the title |
| Два негра | Лимонов, Мубинов, 1992 | `Dva-Negra` | `ДВА НЕГРА` | the title |
| Exolon One | Soft of Future, 1992 | `Exolon-1` (intro), `Exolon-2` (game) | `EXOLON ONE` | the intro picture |
| Exolon.RUN | Skama, 1992 | `Exolon-Run` | `EXOLON.RUN` | the information screen |
| Expert | DIM corp., 1992 | `Expert` | `EXPERT` | the title |
| Ferrari | Bubon production, 1992 | `Ferrari` | `FERRARI` | **fails**: back at the monitor's `?` |
| Hobbit | ELM Soft, 1992 | `Hobbit` | `HOBBIT` | stops at its "PROTECTION BY TOKEN CORP." screen |
| Indiana Jones | Smash corp., 1991 | `Indiana-Jones` (intro), `Indiana-Jones-1` … `-3` (levels) | `INDIANA JONES` | the title |
| New Revolution | Smash corp., 1992 | `New-Revolution` | `NEW REVOLUTION` | the title (a test checks it) |
| Перевал | BIL corp., 1991 | `Pereval-1` (loader), `-2` (intro), `-3` (game), `-4` (check) | `ПЕРЕВАЛ` | the loader runs and waits for the next recording |
| Planet | BIL corp., 1991 | `Planet` | `PLANET` | loads: "3 GAME PLANET" |
| Return of the Jedi | SSV, 1991 | `Return-Jedi-1` (loader), `-2` (game) | `RETURNJ` | the title |
| Robocop | Smash corp., 1992 | `Robocop` | `ROBOCOP` | the information screen |
| Runing | BIL corp., 1991 | `Runing` | `RUNING` | loads: "4 GAME RUNING" |
| Second | BIL corp., 1991 | `Second` | `SECOND` | loads: "2 GAME SECOND" |
| Soldier | DIM corp., 1991 | `Soldier-1` (game), `-2`, `-3` (levels) | `SOLDIER` | **fails**: its loader never finishes reading |
| Superman | BIL corp., 1991 | `SuperMan` | `SUPERMAN` | loads: "5 GAME SUPERMAN" |
| Terminator | Smash corp., 1992 | `Terminator-1` … `-5` (levels; passwords for 2 to 5: HARLEY, TYSON, CONNOR, ANGELOS) | `TERMINATOR1` | the choice of keyboard or joystick |

"In `bk`" is what the first recording of each game gives when played to its
end; the later recordings of the games on several were not tried. Each
recording's files as the ROM sees them (typically a loader it reads, then
parts it can't): `bk --wav2bin FILE DIR`, or the listings `cargo run
--release -p romdis` writes to `listings/software/tapes/protected/`.

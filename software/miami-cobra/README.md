# Miami Cobra (BK-0011M)

"Miami Cobra" for the BK-0011M, as patched by Manwe in 2019 so that it can
be played from a hard or floppy disk under a disk operating system
(`MIAMI.TXT`). `MIAMI.bin` starts it and loads the other files (`MIAMI.PAC`,
`.PG0`, `.PG3`, `.PG6`, `.PG7`, `.TOP`) with BK-0010-style `EMT 36` calls,
which a disk system provides. Under plain BOS, `EMT 36` is the line editor,
so `bk --load software/miami-cobra/MIAMI.bin` doesn't start the game: it
hangs in BOS.

To play it, boot `miami-cobra.dsk`, an ANDOS disk with these files:

    bk --disk software/miami-cobra/miami-cobra.dsk

At the ANDOS title press Space; in the file manager press ↓ to `MIAMI`, then
ВВОД. The game's menu: `K` keyboard, `J` joystick, `S` start, `T` top
scores, `X` exit.

Inside the emulator: start `bk --fdd`, open `miami-cobra.dsk` with Alt+O
(Cmd+O on a Mac), and switch the machine off and on with the СЕТЬ light.

The disk was made with `tools/mkandos.py`, from the ANDOS system on
`../sabot2-v44.dsk` and the files here (the starter first):

    python3 tools/mkandos.py software/sabot2-v44.dsk software/miami-cobra/miami-cobra.dsk \
        software/miami-cobra/MIAMI.bin software/miami-cobra/MIAMI.{PAC,PG0,PG3,PG6,PG7,TOP,TXT}.bin

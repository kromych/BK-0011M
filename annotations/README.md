# Disassembly annotations

Comment files for the commented disassembly of the BK ROMs and programs.
The listings are derived from the ROMs and programs, so they aren't kept in
git; generate them into `listings/` (git-ignored) with:

    cargo run --release -p romdis

What gets listed (`crates/romdis/src/inventory.rs`):

- every ROM in `roms/`, and the MiST core's own ROMs inside
  `roms/mist/bk0011m.rom` (its debugger, disk ROM, and the A16M and SMK512
  extension ROMs);
- everything under `software/`, in folders too: `.bin` programs; on disk
  images (ANDOS and CSIDOS file systems), the boot block and every program,
  at the load address its file system records (`listings/software/<disk>/`,
  with a `files.txt` of everything on the disk and why a file isn't
  listed); on tape recordings (`.wav`, `.mp3`), every file the BK-0010
  monitor ROM reads, damaged ones included, as the ROM read them;
- Saboteur 2, whose game is packed on its disk: the emulator boots the
  disk, runs the game's own unpacker, and lists the memory it filled
  (`listings/software/sabot2-v44/unpacked/`).

Some programs on the CSIDOS and ANDOS disks are packed too; their listings
show the unpacker and the packed data. `software/tapes/CIRCUS.BIN.tap` is in
another emulator's format and isn't read.

Each listing is MACRO-11 source that the project's assembler turns back into
the identical image; `cargo test -p romdis` checks this for every ROM and
program, which makes real firmware and games a test of the disassembler,
the annotations and the assembler together.

- `devices.ann`: device registers, used for every listing.
- `bk0011m.ann`: BK-0011M BOS low-memory cells and system calls (names, table
  slots, EMT numbers), from the BOS system programmer's manual 00015-01.32.01.
- `bk0010.ann`: BK-0010 monitor calls. The EMT table is read from the monitor
  ROM; the call meanings marked "checked" are confirmed by a test.
- `<rom>.ann`: entry points, names and section comments for one ROM.
- `software/<program>.ann`: optional notes for one program; `machine bk0011m`
  marks a BK-0011M program (the default is BK-0010; CSIDOS disks default to
  the BK-0011M). A folder's or disk's file (`software/miami-cobra.ann`,
  `software/sabot2-v44.ann`) sets the machine for everything in it; a
  program on a disk or tape is `software/<disk or tape>/<path>.ann`.
- For a program in RAM, the shared files' names for addresses inside the
  program's own image are left out (the program overwrites the system's
  cells there, or the ROM they name belongs to another machine).

The directive format is described in `crates/romdis/src/annotations.rs`.

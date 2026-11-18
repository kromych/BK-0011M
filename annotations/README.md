# Disassembly annotations

Comment files for the commented disassembly of the BK ROMs and of `.bin`
programs. The listings are derived from the ROMs and programs, so they
aren't kept in git; generate them into `listings/` (git-ignored) with:

    cargo run --release -p romdis

ROMs are read from `roms/`, programs from `software/` (both git-ignored).
Each listing is MACRO-11 source that the project's assembler turns back into
the identical image; `cargo test -p romdis` checks this for every ROM and
program present, which makes real firmware and games a test of the
disassembler, the annotations and the assembler together.

- `devices.ann`: device registers, used for every listing.
- `bk0011m.ann`: BK-0011M BOS low-memory cells and system calls (names, table
  slots, EMT numbers), from the BOS system programmer's manual 00015-01.32.01.
- `bk0010.ann`: BK-0010 monitor calls. The EMT table is read from the monitor
  ROM; the call meanings marked "checked" are confirmed by a test.
- `<rom>.ann`: entry points, names and section comments for one ROM.
- `software/<program>.ann`: optional notes for one program; `machine bk0011m`
  marks a BK-0011M program (the default is BK-0010).

The directive format is described in `crates/romdis/src/annotations.rs`.

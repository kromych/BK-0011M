# The MiST core's ROM images

`bk0011m.rom` and `bk0011m_std.rom` come with the BK-0011M core for the MiST
FPGA board. Each is the core's ROM space, 128 KB from its memory address
0xE0000 (the map is in the core's `memory.sv`). Most of it is the stock
ROMs of [../](../README.md), rearranged. Both also hold the core's own
debugger and disk ROM; `bk0011m.rom` adds two extension ROMs, which
`bk0011m_std.rom` leaves blank:

| Offset | Contents | At | Listing |
|---|---|---|---|
| 0xC000 | a debugger, the core's ROM page 13 (started with `13;1C` and `100000G`) | 100000 | `listings/mist_debugger.mac` |
| 0x12000 | the core's default disk ROM (215 bytes) | 160000 | `listings/mist_disk.mac` |
| 0x1C000 | the A16M memory extension's ROM, for the BK-0010 (`bk0011m.rom` only) | 170000 | `listings/mist_a16m.mac` |
| 0x1D000 | the SMK512 extension's ROM-BIOS v2.05, for the BK-0011M (`bk0011m.rom` only) | 170000 | `listings/mist_smk512.mac` |

`bk` can't start these images: it builds its machines from the separate
ROM files in `../`, and the core's extra ROMs need hardware the emulator
doesn't have (a ROM page 13, the A16M and SMK512 memory extensions). They
are here to be read: `cargo run --release -p romdis` writes the listings
above.

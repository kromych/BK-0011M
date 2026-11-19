#!/usr/bin/env python3
"""Make an ANDOS boot disk holding BK programs.

    python3 tools/mkandos.py TEMPLATE.dsk OUT.dsk PROGRAM.bin...

TEMPLATE is an ANDOS disk (an 800 KB FAT12 image, such as
software/sabot2-v44.dsk): OUT gets its boot block and its ANDOS.SYS, then
each .bin program in the order given. ANDOS keeps a file's BK load address
in the directory entry's time field, so each program's comes from its .bin
header. A program's name on the disk is the .bin file's name without
".bin", split at its first dot into name and extension (MIAMI.PG3.bin
becomes MIAMI.PG3).

ANDOS.SYS goes first, from cluster 2, because the boot block reads it from
there as one piece. This is how software/miami-cobra/miami-cobra.dsk was
made, from software/sabot2-v44.dsk and the files in software/miami-cobra.
"""

import struct
import sys

CLUSTER = 2048  # 4 blocks of 512 bytes, as on the BK's ANDOS disks


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    template_path, out_path, programs = sys.argv[1], sys.argv[2], sys.argv[3:]
    t = open(template_path, "rb").read()
    bps, spc, reserved, fats, root_entries, _, _, fat_blocks = struct.unpack(
        "<HBHBHHBH", t[11:24]
    )
    if bps != 512 or spc * bps != CLUSTER:
        sys.exit(f"{template_path}: not an ANDOS disk with 2 KB clusters")
    root = (reserved + fats * fat_blocks) * 512
    data = root + root_entries * 32
    template_fat = t[reserved * 512 : (reserved + fat_blocks) * 512]

    img = bytearray(len(t))
    img[:512] = t[:512]  # the boot block, with the BIOS parameter block
    fat = [0] * (fat_blocks * 512 * 2 // 3)
    fat[0], fat[1] = 0xFF9, 0xFFF
    entries = []
    next_cluster = 2

    def add(name83, address, payload):
        nonlocal next_cluster
        clusters = max(1, -(-len(payload) // CLUSTER))
        first = next_cluster
        for k in range(clusters):
            c = first + k
            fat[c] = c + 1 if k < clusters - 1 else 0xFFF
            at = data + (c - 2) * CLUSTER
            img[at : at + CLUSTER] = payload[k * CLUSTER : (k + 1) * CLUSTER].ljust(
                CLUSTER, b"\0"
            )
        next_cluster += clusters
        if len(entries) == root_entries or data + (next_cluster - 2) * CLUSTER > len(img):
            sys.exit("the disk is full")
        entries.append(
            name83 + b"\0" * 11 + struct.pack("<HHHI", address, 0, first, len(payload))
        )

    def template_file(entry):
        first, size = struct.unpack("<HI", entry[26:32])
        out, c = b"", first
        while 2 <= c < 0xFF0:
            out += t[data + (c - 2) * CLUSTER : data + (c - 1) * CLUSTER]
            v = template_fat[c * 3 // 2] | template_fat[c * 3 // 2 + 1] << 8
            c = v >> 4 if c & 1 else v & 0xFFF
        return out[:size]

    system = [
        t[root + i * 32 : root + i * 32 + 32]
        for i in range(root_entries)
        if t[root + i * 32 : root + i * 32 + 11] == b"ANDOS   SYS"
    ]
    if not system:
        sys.exit(f"{template_path}: no ANDOS.SYS")
    add(b"ANDOS   SYS", struct.unpack("<H", system[0][22:24])[0], template_file(system[0]))

    for path in programs:
        b = open(path, "rb").read()
        address, length = struct.unpack("<HH", b[:4])
        base = path.replace("\\", "/").rsplit("/", 1)[-1]
        if base.lower().endswith(".bin"):
            base = base[:-4]
        name, _, ext = base.partition(".")
        name83 = name.upper().ljust(8)[:8].encode("koi8-r") + ext.upper().ljust(3)[
            :3
        ].encode("koi8-r")
        add(name83, address, b[4 : 4 + length])

    for i, e in enumerate(entries):
        img[root + i * 32 : root + i * 32 + 32] = e
    packed = bytearray()
    for i in range(0, len(fat) - 1, 2):
        a, b = fat[i], fat[i + 1]
        packed += bytes([a & 0xFF, (a >> 8) | ((b & 0xF) << 4), b >> 4])
    for k in range(fats):
        at = (reserved + k * fat_blocks) * 512
        img[at : at + fat_blocks * 512] = packed[: fat_blocks * 512].ljust(
            fat_blocks * 512, b"\0"
        )
    open(out_path, "wb").write(img)
    print(f"{out_path}: ANDOS.SYS and {len(programs)} programs, {next_cluster - 2} clusters")


if __name__ == "__main__":
    main()

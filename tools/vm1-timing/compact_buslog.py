"""Turn a harness BUSLOG into the compact form the bus-cycle test reads:
a header, then per cycle `address kind gap length reply` (gap from the last
cycle's end to this one's strobe, length from strobe to end, reply delay
from the strobe, `-` for none: a CPU register or a bus timeout). Starts at
the program's first fetch (001000), ends after its final write to 177000.

    python3 compact_buslog.py LOG PROGRAM.bin PACING ROM OUT.txt.gz

PACING is how the harness ran: wN (every device replies N clocks after its
strobe), kN (so, with BK-0011M clocking: `tb PROGRAM 24 3 N`), bk10 or bk11
(MIST=1 SCALE=1, `32 1` or `24 3`); ROM is 1 if it ran with ROM=1."""
import sys, gzip
log, program, pacing, rom, out = sys.argv[1:6]
cycles = []
started = False
for l in open(log):
    if not l.startswith('b '):
        continue
    _, start, addr, kind, strobe, reply, end = l.split()
    if not started:
        if addr != '001000' or kind != 'R':
            continue
        started = True
    cycles.append((addr, kind, int(strobe), int(reply), int(end)))
    if addr == '177000':
        break
lines = [f'# program {program} pacing {pacing} rom {rom} start {cycles[0][2]}']
prev_end = cycles[0][2]
for addr, kind, strobe, reply, end in cycles:
    wait = '-' if kind == 'S' or reply == 0 else str(reply - strobe)
    lines.append(f'{addr} {kind} {strobe - prev_end} {end - strobe} {wait}')
    prev_end = end
with gzip.open(out, 'wt', compresslevel=9) as f:
    f.write('\n'.join(lines) + '\n')
print(out, len(cycles), 'cycles')

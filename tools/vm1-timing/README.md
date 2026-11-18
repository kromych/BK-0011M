# VM1 instruction timing from a chip-accurate model

Measures K1801VM1 instruction timings by running the reverse-engineered CPU
model from the MiST FPGA BK0011M core (GPL, not included here) under
Verilator, against memory that replies after a chosen number of CPU clocks.

    git clone --depth 1 https://github.com/sorgelig/BK0011M mist
    python3 patch_plm.py mist/VM1/vm1_plm.v vm1_plm_fixed.v
    python3 - <<'X'          # register file contents for aram.v
    import re
    mem = [0] * 64
    for line in open("mist/VM1/vm1_reg.mif"):
        m = re.match(r"\s*\[?([0-7]+)(?:\.\.([0-7]+))?\]?\s*:\s*([0-7]+);", line)
        if m:
            for i in range(int(m[1], 8), int(m[2] or m[1], 8) + 1):
                mem[i] = int(m[3], 8)
    open("regfile.hex", "w").write("\n".join("%04x" % v for v in mem) + "\n")
    X
    M=mist/VM1
    verilator --cc --exe --build -O2 --public-flat-rw -Wno-fatal -Wno-lint -Wno-style \
        --top-module vm1_se $M/vm1_se.v $M/vm1_qbus_se.v vm1_plm_fixed.v \
        $M/vm1_tve.v $M/vm1_vic.v aram.v tb.cpp -o tb
    python3 gen_timing.py
    cargo run -p vm1 --example asm -- timing.mac 100000 timing.bin
    ./obj_dir/tb timing.bin 32 1 4      # BK-0010 clocking, 4 wait clocks

With `MIST=1`, RAM below 100000 is paced the way the MiST core paces it:
a model of the 037 memory controller gives the CPU one slot per 8 phases
of a 6 MHz clock; ROM and I/O reply at once. Assemble the program into RAM
(the harness puts a jump to it at the 100000 start address):

    cargo run -p vm1 --example asm -- timing.mac 4000 timing_ram.bin
    MIST=1 ./obj_dir/tb timing_ram.bin 32 1     # BK-0010: 3 MHz, write delay 1
    MIST=1 ./obj_dir/tb timing_ram.bin 24 3     # BK-0011M: 4 MHz, write delay 3

`tb` prints the CPU-clock count of every write to 177000; per-instruction
clocks are (block time - empty block time) / 100.

Results (2026-11-18), CPU clocks:

| Instruction | wait 0 | wait 4 | bkbtl tables |
|---|---|---|---|
| NOP | 9 | 12 | 12 |
| MOV R1,R2 | 8 | 12 | 12 |
| MOV #n,R0 | 18 | 26 | 28 |
| MOV #n,(R3) | 28 | 40 | 40 |
| SOB (per loop) | 17 | 21 | 20 |

With no waits the chip is faster than bkbtl's tables; with 4 wait clocks
per memory access (1.33 µs at 3 MHz, the 037 controller's memory slot
period) it reproduces them. bkbtl's "measured" tables therefore already
include the memory waits.

With the MiST memory model and code in RAM (`MIST=1`), CPU clocks:

| Instruction | BK-0010 | BK-0011M | bkbtl tables |
|---|---|---|---|
| NOP | 12 | 16 | 12 |
| MOV #n,R0 | 28 | 32 | 28 |
| MOV #n,(R3) | 40 | 48 | 40 |
| CLR (R3) | 28 | 37 | 32 |
| MOV R1,(R3) | 36 | 40 | 32 |
| JSR+RTS | 72 | 80 | 72 |
| SOB | 20 | 21 | 20 |
| sum of 23 tests | 584 | 675 | 580 |

The BK-0010 column matches bkbtl's tables on 20 of 23 instructions (the
others within one memory slot). The BK-0011M needs about 16% more clocks:
the 037 still runs at 6 MHz while the CPU runs at 4 MHz, and writes are
delayed longer, so the BK-0011M is only about 15% faster than the BK-0010
in real time. The emulator adds 2 clocks per RAM/ROM access on the
BK-0011M (`BK0011M_ACCESS_WAIT`), which gives 664 against 675.

## Trap and interrupt order

Two programs check what the VM1 does when an EMT meets another event. Each
handler writes a marker and its return address to the probe address.

`emt-vs-evnt.mac`: with `EVNT_AT=4042`, the harness raises EVNT (IRQ2)
while the CPU fetches the EMT at 4042, so it is pending when the EMT ends:

    cargo run -p vm1 --example asm -- emt-vs-evnt.mac 4000 emt-vs-evnt.bin
    EVNT_AT=4042 ./obj_dir/tb emt-vs-evnt.bin 32 1

The first EVNT (return address 4042) comes from the VM1 prefetching the EMT
during the NOP before it. After that: the EMT handler (marker 1) returns to
4044, and only after its RTI does EVNT (marker 2) come, also returning to
4044. The EMT's trap is taken before the pending interrupt.

`emt-vs-trace.mac`: an EMT executed with T set (via RTT). The EMT handler
runs first and sees PSW 020 saved; the trace trap (marker 3) comes after
the handler's RTI restores T, returning to the EMT's return address.

Both agree with the core's source: EMT, TRAP, IOT and BPT vectors come from
the instruction's microcode, not from the interrupt priority matrix, so the
trap is part of the instruction and interrupts are then judged against the
handler's PSW. `crates/vm1/tests/cpu.rs` has the same cases.

## Running a machine snapshot

With `IMAGE=1` the harness loads a raw 64 KB memory image instead of a
`.bin` program; `SEL1=` (octal) sets the value the CPU reads from SEL1 at
reset, whose high byte is the start address; `DUMP=FILE` saves the memory
when the run ends (at `MAXCLK`). This runs a state taken from the emulator
on the chip model and compares the results.

That is how BASIC's string bug was settled (`docs/basic.md`): the
BK-0010.01 was snapshotted just after BASIC read `RUN` for a program doing
`D$=D$+"B"` and `D$=D$+"C"`, with a stub at 077000 that restores the
registers and returns into BASIC with `RTI`, then run with

    IMAGE=1 SEL1=077300 DUMP=chip.img MAXCLK=4000000 ./obj_dir/tb basic.img 32 1

The chip model left the same bytes (`A B B B C` at 3220) as the emulator:
the bug is the ROM's.

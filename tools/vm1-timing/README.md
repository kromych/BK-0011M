# VM1 timing from a chip-accurate model

Measures K1801VM1 timing by running the reverse-engineered CPU model from
the MiST FPGA BK0011M core (GPL, not included here) under Verilator, against
memory that replies after a chosen number of CPU clocks, or paced as the
core's model of the 037 memory controller paces it. The emulator's CPU
(`crates/vm1/src/timing.rs`, `cpu.rs`) follows what it shows bus cycle by
bus cycle; `buslogs/` holds the evidence, which a test replays.

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
of a 6 MHz clock; ROM and I/O reply after the WAIT argument (0 if none).
Assemble the program into RAM (the harness puts a jump to it at the 100000
start address):

    cargo run -p vm1 --example asm -- timing.mac 4000 timing_ram.bin
    MIST=1 ./obj_dir/tb timing_ram.bin 32 1     # BK-0010: 3 MHz, write delay 1
    MIST=1 ./obj_dir/tb timing_ram.bin 24 3     # BK-0011M: 4 MHz, write delay 3

`tb` prints the CPU-clock count of every write to 177000; per-instruction
clocks are (block time - empty block time) / 100.

Results, CPU clocks:

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

The BK-0011M needs about 16% more clocks: the 037 still runs at 6 MHz
while the CPU runs at 4 MHz, and writes are held longer, so the BK-0011M is
only about 15% faster than the BK-0010 in real time. The emulator gives
584.00 and 674.29 (`crates/bk/tests/timing.rs` runs the same program on
the whole machine; the chip's hundredths come from its probe writes).

Run the BK-0011M at `SCALE=1`, 2 or 4, not 8: at 3 system clocks per CPU
clock the 037 model's phases fall differently and RAM replies come too
early (a NOP from RAM takes 10.7 clocks instead of 16). The BK-0010 gives
the same results at every scale.

## Bus cycles

`BUSLOG=from:to` prints one line per bus cycle whose SYNC rises between
those CPU clocks: `b start address kind strobe reply end`, kind R, W, M
(read-modify-write) or S (SEL1/SEL2, which answer inside the CPU; reply 0).
A write's strobe is the CPU's, before the harness's DOUT_DELAY.
`RLOG=addr` prints the registers whenever the CPU reads `addr`.

`gen_programs.py` writes programs that run every instruction in every
addressing mode after the same setup and a NOP (single, doubleA, doubleB,
psw, pc), the jumps, returns, traps and bus errors (flow), each instruction
after a register operation and after a memory write (ovl), and the SEL and
timer registers (io). Logged on the chip with replies of 1 to 5 clocks,
with the BK-0011M's clocking, and with the 037 model on both machines,
they give the schedule in `crates/vm1/src/timing.rs`:

- A cycle runs 4 clocks after its reply; a read-modify-write turns 5
  clocks after the read's reply; a cycle nobody answers ends 62 clocks
  after the CPU's strobe. The gaps between cycles don't depend on the
  reply delay.
- After an instruction that doesn't end with the register-operation tail
  (a memory destination, a jump, branch, return or trap), the next opcode
  fetch overlaps its end: the next instruction's first gap is a clock
  shorter, whatever it is.
- MTPS from a register fetches the next opcode, throws it away and fetches
  it again 12 clocks after the first fetch's strobe.
- After MOVB or MFPS into a register, which sign-extend, the next
  instruction proceeds as if its fetch's reply took at least 3 clocks.
- JSR's next fetch comes at least 9 clocks after its push's strobe.
- The first instruction after reset runs a clock faster (the harness
  enters the programs with a JMP, so it isn't measured).
- With an instant reply some gaps are a clock longer; nothing on the BK
  replies at once, so the emulator doesn't model it.

The 037 model's reply delay to a RAM strobe depends only on the strobe's
CPU clock: 4, 3, 6, 5 repeating on the BK-0010, and 5 4 3 7 6 5 4 3 8 7 6
5 4 3 7 6 (every 16 clocks, 3 slots) on the BK-0011M, on every RAM cycle
of the logs (`crates/bk/src/board.rs`).

`buslogs/` keeps 20 of these logs in a compact form (`compact_buslog.py`),
with the programs; `crates/vm1/tests/bus_cycles.rs` runs each program on
the emulator's CPU with a bus that answers as the harness did and requires
every cycle's gap, length and reply to match. To add one:

    python3 gen_programs.py
    cargo run -p vm1 --example asm -- pc.mac 1000 pc.bin
    MIST=1 BUSLOG=220000:2000000 MAXCLK=2000000 ./obj_dir/tb pc.bin 24 3 \
        | grep '^b ' > pc-bk11.log
    python3 compact_buslog.py pc-bk11.log pc.bin bk11 0 buslogs/pc-bk11.txt.gz

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

## Running a tape load

The harness can also play a tape into the chip model, to compare a load
with the emulator's. `crates/bk/examples/tape_snapshot.rs` takes the
emulator to the moment the BK-0010 monitor's `M` command enters its tape
routine (the motor not yet started) and writes the memory, a stub that
returns into the routine, and the recording as tape levels:

    cargo run --release -p bk --example tape_snapshot -- roms \
        software/tapes/protected/Expert.wav.gz EXPERT snap
    IMAGE=1 MIST=1 SCALE=8 ROM=1 SEL1=$(cat snap/sel1) TAPE=snap/tape.lvl \
        DUMP=snap/chip.img MAXCLK=400000000 ./obj_dir/tb snap/mem.img 32 1

`TAPE` feeds the levels into bit 5 of 177716 (inverted, as the emulator
does) while the motor runs, `ROM=1` leaves writes to 100000-177577
unanswered as the MiST core does (a bus error), and the CPU's own
registers 177700-177717 read back what the CPU drives, as the core wires
them. `SCALE=8` runs 4 system clocks per CPU clock instead of 32, with the
037 model's phases scaled to match: the timing benchmark gives identical
results, 8 times faster (about 0.2 million CPU clocks a second).

The MiST core itself has no tape input: its SEL1 value is
`{start_addr, 1'b1, ~key_down, 3'b000, super_flg, 2'b00}` (bk0011m.sv), so
bit 5 always reads 0, and it loads programs only as BIN files from its SD
card. The tape input here is the harness's, not the core's.

`WRITES=from:to` logs every write the CPU makes between those CPU clocks.

The fourth argument (WAIT) sets how long ROM and I/O take to reply; MiST
answers at once, the real BK's ROMs don't. Results:

- A standard tape (600 random bytes written by `--bin2wav`) loads on the
  chip model byte for byte, with a good checksum: the tape input works.
- Ferrari (software/tapes/protected), whose loader reads at the fastest
  speed the tape allows, decides the ROM's reply delay. Its file
  overwrites the ROM's store step (cell 304) as it loads, so that a store
  lands in ROM and the bus error starts its loader through vector 4. With
  ROM and I/O replying at once the chip reads the 1 bits of bytes 306-307
  as 0 and never takes the bus error; with 3 clocks it fails too; with 2
  clocks it loads completely, to the game's title.
- With 2 clocks Expert loads, Hobbit stops at its "PROTECTION BY TOKEN
  CORP." screen (its loader has stopped the motor), and Soldier waits for
  a key on an empty screen; the emulator did exactly the same with the
  same timing.
- These runs feed the tape in with the polarity the emulator then used
  (bit 5 of 177716 is 0 while the signal is high). The emulator has since
  turned it the other way up and made its ROM reply in 1 clock: four BIL
  corp. games take a decryption key from the CPU's timer that is right
  only with ROM that fast, and with both changes every protected tape
  loads, Ferrari included (docs/hardware-reference.md §1).
- An MP3 that the emulator's encoder made before its pulses were all whole
  samples (pilot halves alternating 7 and 8 samples) doesn't load on the
  chip model, nor in the emulator; re-encoded, it loads in both.

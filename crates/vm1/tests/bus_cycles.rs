//! The executor against the chip, bus cycle by bus cycle.
//!
//! `tools/vm1-timing/buslogs` holds programs that run every instruction in
//! every addressing mode (and the jumps, traps, bus errors and PSW
//! instructions), with the bus cycles the MiST core's chip-level VM1 model
//! made running them under the harness `tools/vm1-timing/tb.cpp`: with every
//! device replying a fixed number of clocks after its strobe (`wN`), so with
//! the BK-0011M's longer-held writes (`kN`), and with RAM paced by the MiST
//! model of the 037 memory controller on either machine (`bk10`, `bk11`).
//! Each cycle is `address kind gap length reply`: the clocks from the last
//! cycle's end to its strobe, from its strobe to its end, and from its
//! strobe to the reply (`-`: a SEL register, which answers inside the CPU,
//! or a bus timeout). This test runs the same programs on [`vm1::Cpu`] with
//! a bus that answers as the harness does and requires the same cycles.

use std::io::Read;
use std::path::{Path, PathBuf};

use vm1::{Bus, BusError, Cpu, Cycle};

/// The 037's RAM reply delay by strobe clock, as the board uses (from the
/// MiST model, `crates/bk/src/board.rs`).
const RAM_WAIT_BK0010: [u32; 4] = [4, 3, 6, 5];
const RAM_WAIT_BK0011M: [u32; 16] = [5, 4, 3, 7, 6, 5, 4, 3, 8, 7, 6, 5, 4, 3, 7, 6];

/// One bus cycle as the log has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Logged {
    addr: u16,
    kind: char,
    gap: u64,
    len: u64,
    reply: Option<u64>,
}

/// The harness's memory and replies.
struct Harness {
    mem: Vec<u8>,
    ram: Option<&'static [u32]>,
    wait: u32,
    rom_protect: bool,
    step_start: u64,
    /// (strobe, address, read or write, reply delay or None for a timeout)
    cycles: Vec<(u64, u16, Cycle, Option<u32>)>,
}

impl Harness {
    fn protected(&mut self, addr: u16) -> bool {
        let p = self.rom_protect && (0o100000..0o177600).contains(&addr);
        if p && let Some(last) = self.cycles.last_mut() {
            last.3 = None;
        }
        p
    }
}

impl Bus for Harness {
    fn read_word(&mut self, addr: u16) -> Result<u16, BusError> {
        let a = usize::from(addr);
        Ok(u16::from_le_bytes([self.mem[a], self.mem[a + 1]]))
    }
    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), BusError> {
        if self.protected(addr) {
            return Err(BusError);
        }
        let a = usize::from(addr);
        self.mem[a..a + 2].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
    fn read_byte(&mut self, addr: u16) -> Result<u8, BusError> {
        Ok(self.mem[usize::from(addr)])
    }
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), BusError> {
        if self.protected(addr) {
            return Err(BusError);
        }
        self.mem[usize::from(addr)] = value;
        Ok(())
    }
    fn cycle(&mut self, at: u32, addr: u16, kind: Cycle) -> u32 {
        let t = self.step_start + u64::from(at);
        let wait = match self.ram {
            // SEL1, SEL2 and the CPU's own registers answer inside the CPU.
            _ if addr & !1 >= 0o177700 => 0,
            Some(table) if addr < 0o100000 => table[(t % table.len() as u64) as usize],
            Some(_) => 0,
            None => self.wait,
        };
        self.cycles.push((t, addr, kind, Some(wait)));
        wait
    }
}

fn buslogs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/vm1-timing/buslogs")
}

/// A log: its header's fields (the program, the pacing, whether writes to
/// ROM time out, the clock of the first fetch's strobe), then the cycles.
struct Log {
    program: String,
    pacing: String,
    rom_protect: bool,
    start: u64,
    cycles: Vec<Logged>,
}

fn read_log(path: &Path) -> Log {
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let [
        "#",
        "program",
        program,
        "pacing",
        pacing,
        "rom",
        rom,
        "start",
        start,
    ] = header[..]
    else {
        panic!("{}: bad header {header:?}", path.display());
    };
    let cycles = lines
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            Logged {
                addr: u16::from_str_radix(f[0], 8).unwrap(),
                kind: f[1].chars().next().unwrap(),
                gap: f[2].parse().unwrap(),
                len: f[3].parse().unwrap(),
                reply: f[4].parse().ok(),
            }
        })
        .collect();
    Log {
        program: program.into(),
        pacing: pacing.into(),
        rom_protect: rom == "1",
        start: start.parse().unwrap(),
        cycles,
    }
}

/// Run the log's program (a BK .bin) from its load address, starting at the
/// log's clock, for as long as the log runs, and give its bus cycles in the
/// log's form.
fn replay(log: &Log, program: &[u8]) -> Vec<Logged> {
    let (pacing, rom_protect, n) = (log.pacing.as_str(), log.rom_protect, log.cycles.len());
    let load = usize::from(u16::from_le_bytes([program[0], program[1]]));
    let len = usize::from(u16::from_le_bytes([program[2], program[3]]));
    let mut mem = vec![0u8; 0x10000];
    mem[load..load + len].copy_from_slice(&program[4..4 + len]);
    let (ram, wait, write_delay): (Option<&'static [u32]>, u32, u32) = match pacing {
        "bk10" => (Some(&RAM_WAIT_BK0010), 0, 0),
        "bk11" => (Some(&RAM_WAIT_BK0011M), 0, 2),
        k if k.starts_with('k') => (None, k[1..].parse().unwrap(), 2),
        w => (None, w[1..].parse().unwrap(), 0),
    };
    let mut bus = Harness {
        mem,
        ram,
        wait,
        rom_protect,
        step_start: 0,
        cycles: Vec::new(),
    };
    let mut cpu = Cpu::new();
    cpu.write_delay = write_delay;
    cpu.reset(load as u16);
    cpu.psw = 0o340;
    // The harness enters the program with a JMP, which overlaps the fetch.
    let mut state = cpu.state();
    state.overlap = true;
    cpu.set_state(state);
    // The log's clock (the 037's phase follows it).
    bus.step_start = log.start;
    let stop = log.start + log.cycles.iter().map(|c| c.gap + c.len).sum::<u64>() + 100;
    while bus.step_start < stop && !cpu.is_halted() {
        let clocks = cpu.step(&mut bus);
        bus.step_start += u64::from(clocks);
    }
    // A write right after a read of the same address, as a DATIO cycle
    // makes them, is one cycle.
    let mut out = Vec::new();
    let mut prev_end = bus.cycles[0].0;
    let c = &bus.cycles;
    let end = |strobe: u64, wait: Option<u32>, delay: u32| {
        // A timeout is counted from the CPU's own strobe.
        wait.map_or(strobe + 62 - u64::from(delay), |w| {
            strobe + u64::from(w) + 4
        })
    };
    let mut i = 0;
    while i < c.len() && out.len() < n {
        let (t, addr, kind, wait) = c[i];
        let rmw = kind == Cycle::Read
            && c.get(i + 1).is_some_and(|&(t2, a2, k2, _)| {
                k2 == Cycle::Write
                    && a2 == addr
                    && wait.is_some_and(|w| t2 == t + u64::from(w) + 5 + u64::from(write_delay))
            });
        // SEL1 and SEL2 are cycles of their own kind; the CPU's other
        // registers (177700-177712) look like reads and writes. None of them
        // gets a reply from outside.
        let sel = addr & !1 >= 0o177714;
        let internal = addr & !1 >= 0o177700;
        // The harness logs a write's strobe as the CPU raises it.
        let strobe = if kind == Cycle::Write && !rmw {
            t - u64::from(write_delay)
        } else {
            t
        };
        let cycle_end = if rmw {
            let (t2, _, _, w2) = c[i + 1];
            end(t2, w2, write_delay)
        } else {
            end(t, wait, if kind == Cycle::Write { write_delay } else { 0 })
        };
        out.push(Logged {
            addr,
            kind: match (sel, rmw, kind) {
                (true, _, _) => 'S',
                (_, true, _) => 'M',
                (_, _, Cycle::Read) => 'R',
                (_, _, Cycle::Write) => 'W',
            },
            gap: strobe - prev_end,
            len: cycle_end - strobe,
            reply: if internal {
                None
            } else {
                wait.map(|w| u64::from(w) + (t - strobe))
            },
        });
        prev_end = cycle_end;
        i += if rmw { 2 } else { 1 };
    }
    out
}

#[test]
fn every_bus_cycle_matches_the_chip() {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(buslogs())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".txt.gz"))
        .collect();
    logs.sort();
    assert!(
        logs.len() >= 20,
        "bus logs missing from {}",
        buslogs().display()
    );
    let mut failures = Vec::new();
    for path in &logs {
        let log = read_log(path);
        let bin = std::fs::read(buslogs().join(&log.program)).unwrap();
        let ours = replay(&log, &bin);
        let chip = &log.cycles;
        let name = path.file_name().unwrap().to_string_lossy();
        // The first cycle's gap is the log's start, not a gap.
        let first_bad = (1..chip.len()).find(|&i| ours.get(i) != Some(&chip[i]));
        if let Some(i) = first_bad {
            let show = |v: &[Logged]| {
                v[i.saturating_sub(3)..(i + 2).min(v.len())]
                    .iter()
                    .map(|c| {
                        format!(
                            "{:06o} {} gap {} len {} reply {:?}",
                            c.addr, c.kind, c.gap, c.len, c.reply
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n      ")
            };
            failures.push(format!(
                "{name}: cycle {i} of {}\n   chip:\n      {}\n   ours:\n      {}",
                chip.len(),
                show(chip),
                show(&ours)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

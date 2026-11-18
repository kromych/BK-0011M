//! The debugger, driven through its commands as a front end would. Needs
//! the ROM files in `<repo>/roms` (or `BK_ROMS`); skipped when missing.

use std::path::PathBuf;

use bk::debugger::Action;
use bk::vm1::asm::assemble;
use bk::{BinFile, Bk0010Rom, Config, Debugger, Machine, Roms};

const PROGRAM: &str = "
START:  MOV #5, R1
LOOP:   JSR PC, BUMP
        MOV #101, R0
        EMT 16              ; monitor: print the character in R0
        SOB R1, LOOP
DONE:   BR DONE
BUMP:   INC @#COUNT
        RTS PC
COUNT:  .WORD 0";

/// A BK-0010 that has booted and loaded PROGRAM at 1000, stopped at START.
fn setup() -> Option<(Machine, Debugger, vm1_symbols::Symbols)> {
    let dir = std::env::var_os("BK_ROMS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roms"));
    let roms = Roms::load_bk0010(&dir, Bk0010Rom::Basic)
        .map_err(|e| eprintln!("skipping: {e}"))
        .ok()?;
    let mut m = Machine::new(roms, Config::default());
    for _ in 0..150 {
        m.run_frame();
    }
    let p = assemble(PROGRAM, 0o1000).unwrap();
    m.load_bin(&BinFile {
        address: 0o1000,
        data: p.bytes.clone(),
    })
    .unwrap();
    m.start_at(0o1000);
    Some((m, Debugger::new(), vm1_symbols::Symbols(p)))
}

mod vm1_symbols {
    pub struct Symbols(pub bk::vm1::asm::Program);
    impl Symbols {
        pub fn get(&self, name: &str) -> u16 {
            self.0.symbol(name).unwrap()
        }
    }
}

fn cmd(d: &mut Debugger, m: &mut Machine, line: &str) -> String {
    let (out, action) = d.command(m, line);
    assert_eq!(action, Action::Stay, "{line}");
    out
}

/// Run as a front end does after `c` or `n`.
fn go(d: &mut Debugger, m: &mut Machine, line: &str) -> String {
    let (out, action) = d.command(m, line);
    match action {
        Action::Stay => out,
        Action::Run => out + &d.run(m, 500).expect("stopped"),
    }
}

#[test]
fn registers_and_disassembly() {
    let Some((mut m, mut d, s)) = setup() else {
        return;
    };
    let r = cmd(&mut d, &mut m, "r");
    assert!(r.contains("PC 001000"), "{r}");
    assert!(r.contains("PSW 000000 (------)"), "{r}");
    assert!(r.contains("> 001000  012701 000005"), "{r}");
    assert!(r.contains("MOV    #000005, R1"), "{r}");

    let u = cmd(&mut d, &mut m, "u 1000 3");
    assert_eq!(u.lines().count(), 3);
    let jsr = format!("JSR    PC, {:06o}", s.get("BUMP"));
    assert!(u.lines().nth(1).unwrap().contains(&jsr), "{u}");
    // An empty line carries on from where the listing stopped.
    let more = cmd(&mut d, &mut m, "");
    assert!(more.starts_with("  001014"), "{more}");
}

#[test]
fn breakpoints_steps_and_step_over() {
    let Some((mut m, mut d, s)) = setup() else {
        return;
    };
    assert!(cmd(&mut d, &mut m, "b 1004").contains("breakpoint at 001004"));
    let out = go(&mut d, &mut m, "c");
    assert!(out.starts_with("breakpoint at 001004"), "{out}");
    assert_eq!(m.cpu().pc(), 0o1004);
    assert_eq!(m.cpu().r[1], 5);

    // s goes into the subroutine.
    let out = cmd(&mut d, &mut m, "s");
    assert_eq!(m.cpu().pc(), s.get("BUMP"), "{out}");
    let out = cmd(&mut d, &mut m, "s 2");
    assert_eq!(m.cpu().pc(), 0o1010, "back after RTS: {out}");

    // n runs the monitor's EMT 16 and stops after it.
    cmd(&mut d, &mut m, "s");
    let out = go(&mut d, &mut m, "n");
    assert_eq!(m.cpu().pc(), 0o1016, "{out}");
    assert!(
        !out.contains("breakpoint"),
        "a step-over isn't reported as a breakpoint: {out}"
    );

    // Next time round the loop: c stops at the breakpoint again; n steps over the JSR.
    go(&mut d, &mut m, "c");
    assert_eq!((m.cpu().pc(), m.cpu().r[1]), (0o1004, 4));
    go(&mut d, &mut m, "n");
    assert_eq!(m.cpu().pc(), 0o1010);
    assert_eq!(m.peek_word(s.get("COUNT")), Some(2));

    // Clear it: c runs the loop out and keeps going.
    cmd(&mut d, &mut m, "bc all");
    assert_eq!(cmd(&mut d, &mut m, "b"), "no breakpoints\n");
    let (_, action) = d.command(&mut m, "c");
    assert_eq!(action, Action::Run);
    assert!(d.run(&mut m, 20).is_none(), "nothing stops it");
    assert_eq!(m.cpu().pc(), s.get("DONE"));
    assert_eq!(m.peek_word(s.get("COUNT")), Some(5));
}

#[test]
fn watchpoints_memory_and_registers() {
    let Some((mut m, mut d, s)) = setup() else {
        return;
    };
    let count = s.get("COUNT");
    cmd(&mut d, &mut m, &format!("w {count:o}"));
    let out = go(&mut d, &mut m, "c");
    assert!(
        out.starts_with(&format!(
            "watchpoint: the instruction at {:06o} wrote 000001 to {count:06o}",
            s.get("BUMP")
        )),
        "{out}"
    );
    cmd(&mut d, &mut m, "wc all");

    // e stores words; m shows them with their characters.
    cmd(&mut d, &mut m, "e 2000 042502 020113");
    let dump = cmd(&mut d, &mut m, "m 2000 2");
    // Six missing words pad the line so the characters line up.
    assert_eq!(
        dump,
        format!("002000  042502 020113{}  BEK \n", " ".repeat(6 * 7))
    );
    assert!(cmd(&mut d, &mut m, "e 120000 1").contains("isn't RAM"));
    assert!(
        cmd(&mut d, &mut m, "m 177660 1").contains("------"),
        "I/O isn't read"
    );
    assert_eq!(
        cmd(&mut d, &mut m, "m 2000 10.").lines().count(),
        2,
        "10 decimal words"
    );
    assert_eq!(
        cmd(&mut d, &mut m, "m 2000 10").lines().count(),
        1,
        "10 octal words"
    );

    // Registers can be set; pc also leaves WAIT.
    cmd(&mut d, &mut m, "r3 1234");
    assert_eq!(m.cpu().r[3], 0o1234);
    cmd(&mut d, &mut m, &format!("pc {:o}", s.get("DONE")));
    assert_eq!(m.cpu().pc(), s.get("DONE"));
    assert!(cmd(&mut d, &mut m, "u r3 1").starts_with("  001234"));

    assert!(cmd(&mut d, &mut m, "x").contains("unknown command"));
    assert!(cmd(&mut d, &mut m, "b 9").contains("isn't an octal number"));
    assert!(cmd(&mut d, &mut m, "h").contains("step over"));
}

#[test]
fn symbols_conditions_call_breaks_pages_and_search() {
    let Some((mut m, mut d, s)) = setup() else {
        return;
    };
    let bump = s.get("BUMP");
    // Symbols in the annotation format and MACRO-11's NAME = ADDR.
    let n = d.load_symbols(&format!(
        "name {bump:o} BUMP ; the subroutine\nemt 16 PUTC\nCOUNT = {:o}\n",
        s.get("COUNT")
    ));
    assert_eq!(n, 3);
    let u = cmd(&mut d, &mut m, "u 1000 5");
    assert!(u.contains("JSR    PC, BUMP"), "{u}");
    assert!(u.contains("EMT    016    ; PUTC"), "{u}");
    let u = cmd(&mut d, &mut m, "u BUMP 1");
    assert!(u.starts_with("BUMP:\n"), "{u}");
    assert!(
        cmd(&mut d, &mut m, "m COUNT 1").starts_with(&format!("{:06o}  000000", s.get("COUNT")))
    );

    // A condition: stop at LOOP only when R1 is 3 (it counts down from 5).
    cmd(&mut d, &mut m, "b 1004 r1=3");
    assert!(cmd(&mut d, &mut m, "b").contains("if R1=000003"));
    let out = go(&mut d, &mut m, "c");
    assert_eq!((m.cpu().pc(), m.cpu().r[1]), (0o1004, 3), "{out}");
    cmd(&mut d, &mut m, "bc all");

    // Before an EMT 16.
    cmd(&mut d, &mut m, "be 16");
    let out = go(&mut d, &mut m, "c");
    assert!(
        out.starts_with("stopped before the call at 001014"),
        "{out}"
    );
    cmd(&mut d, &mut m, "bc emt 16");

    // A trace of what runs, with symbols.
    let path = std::env::temp_dir().join(format!("bk-trace-{}.txt", std::process::id()));
    cmd(&mut d, &mut m, &format!("t {}", path.display()));
    cmd(&mut d, &mut m, "s 4");
    cmd(&mut d, &mut m, "t off");
    let trace = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(trace.lines().count(), 4, "{trace}");
    assert!(
        trace.lines().next().unwrap().starts_with("001014  104016"),
        "{trace}"
    );

    // Physical pages and search.
    cmd(&mut d, &mut m, "e p0:2000 123456");
    assert!(cmd(&mut d, &mut m, "m 2000 1").starts_with("002000  123456"));
    assert!(cmd(&mut d, &mut m, "m p0:2000 1").starts_with("p0:002000  123456"));
    let found = cmd(&mut d, &mut m, "f 123456");
    assert!(
        found.contains("002000\n") && found.ends_with("1 found\n"),
        "{found}"
    );
    assert!(cmd(&mut d, &mut m, "m p9:0 1").contains("no such RAM page"));
    cmd(&mut d, &mut m, "sym HERE 2000");
    assert!(cmd(&mut d, &mut m, "m HERE 1").starts_with("002000  123456"));
}

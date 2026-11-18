//! CPU behavior tests. Programs are assembled at 001000 with SP = 001000 and
//! PSW = 0, then run until PC reaches the label DONE.

use vm1::asm::{Program, assemble};
use vm1::{Bus, BusError, Cpu, psw};

const ORIGIN: u16 = 0o1000;
const RAM_END: u16 = 0o160000;
const SEL1: u16 = 0o177716;

/// 56 KB of RAM, SEL1 at 177716, nothing else: any other address is a bus error.
struct TestBus {
    ram: Vec<u8>,
    sel1_writes: Vec<u16>,
    byte_reads: Vec<u16>,
    resets: u32,
    virq: Option<u16>,
}

impl TestBus {
    fn new() -> Self {
        TestBus {
            ram: vec![0; RAM_END as usize],
            sel1_writes: vec![],
            byte_reads: vec![],
            resets: 0,
            virq: None,
        }
    }
    fn word(&self, a: u16) -> u16 {
        u16::from_le_bytes([self.ram[a as usize], self.ram[a as usize + 1]])
    }
    fn set_word(&mut self, a: u16, v: u16) {
        self.ram[a as usize..a as usize + 2].copy_from_slice(&v.to_le_bytes());
    }
}

impl Bus for TestBus {
    fn read_word(&mut self, addr: u16) -> Result<u16, BusError> {
        assert_eq!(addr & 1, 0, "word read at odd address {addr:06o}");
        match addr {
            a if a < RAM_END => Ok(self.word(a)),
            SEL1 => Ok(0o140000),
            _ => Err(BusError),
        }
    }
    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), BusError> {
        assert_eq!(addr & 1, 0, "word write at odd address {addr:06o}");
        match addr {
            a if a < RAM_END => self.set_word(a, value),
            SEL1 => self.sel1_writes.push(value),
            _ => return Err(BusError),
        }
        Ok(())
    }
    fn read_byte(&mut self, addr: u16) -> Result<u8, BusError> {
        if addr >= RAM_END {
            return Err(BusError);
        }
        self.byte_reads.push(addr);
        Ok(self.ram[addr as usize])
    }
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), BusError> {
        if addr >= RAM_END {
            return Err(BusError);
        }
        self.ram[addr as usize] = value;
        Ok(())
    }
    fn reset_devices(&mut self) {
        self.resets += 1;
        self.virq = None;
    }
    fn acknowledge_interrupt(&mut self) -> Option<u16> {
        self.virq.take()
    }
}

fn setup(src: &str) -> (Cpu, TestBus, Program) {
    let program = assemble(src, ORIGIN).unwrap_or_else(|e| panic!("{e}\n{src}"));
    let mut bus = TestBus::new();
    let start = ORIGIN as usize;
    bus.ram[start..start + program.bytes.len()].copy_from_slice(&program.bytes);
    let mut cpu = Cpu::new();
    cpu.reset(ORIGIN);
    cpu.psw = 0;
    cpu.r[6] = ORIGIN;
    (cpu, bus, program)
}

/// Step until PC == `addr` (and not in WAIT). Returns the cycles used.
fn run_to(cpu: &mut Cpu, bus: &mut TestBus, addr: u16) -> u32 {
    let mut cycles = 0;
    for _ in 0..10_000 {
        if cpu.pc() == addr && !cpu.is_waiting() {
            return cycles;
        }
        assert!(!cpu.is_halted(), "CPU halted at {:06o}", cpu.pc());
        cycles += cpu.step(bus);
    }
    panic!("did not reach {addr:06o}; PC = {:06o}", cpu.pc());
}

fn run(src: &str) -> (Cpu, TestBus, Program) {
    let (mut cpu, mut bus, program) = setup(src);
    let done = program.symbol("DONE").expect("program needs a DONE label");
    run_to(&mut cpu, &mut bus, done);
    (cpu, bus, program)
}

fn flags(psw: u16) -> String {
    [(psw::N, 'N'), (psw::Z, 'Z'), (psw::V, 'V'), (psw::C, 'C')]
        .iter()
        .map(|&(bit, c)| if psw & bit != 0 { c } else { '-' })
        .collect()
}

#[test]
fn alu_results_and_flags() {
    // (program, expected R0, expected NZVC)
    let cases: &[(&str, u16, &str)] = &[
        ("MOV #0, R0", 0, "-Z--"),
        ("MOV #100000, R0", 0o100000, "N---"),
        ("SEC\nMOV #1, R0", 1, "---C"),
        ("MOV #77777, R0\nINC R0", 0o100000, "N-V-"),
        ("MOV #100000, R0\nDEC R0", 0o077777, "--V-"),
        ("CLR R0\nSEC\nINC R0", 1, "---C"),
        ("MOV #100000, R0\nNEG R0", 0o100000, "N-VC"),
        ("CLR R0\nNEG R0", 0, "-Z--"),
        ("MOV #1, R0\nNEG R0", 0o177777, "N--C"),
        ("MOV #177777, R0\nSEC\nADC R0", 0, "-Z-C"),
        ("MOV #77777, R0\nSEC\nADC R0", 0o100000, "N-V-"),
        ("CLR R0\nSEC\nSBC R0", 0o177777, "N--C"),
        ("MOV #100000, R0\nSEC\nSBC R0", 0o077777, "--V-"),
        ("MOV #1, R0\nCOM R0", 0o177776, "N--C"),
        ("MOV #1, R0\nASR R0", 0, "-ZVC"),
        ("MOV #100001, R0\nASR R0", 0o140000, "N--C"),
        ("MOV #100000, R0\nASL R0", 0, "-ZVC"),
        ("CLR R0\nSEC\nROR R0", 0o100000, "N-V-"),
        ("MOV #100000, R0\nROL R0", 0, "-ZVC"),
        ("MOV #1377, R0\nSWAB R0", 0o177402, "----"),
        ("CLR R0\nSEN\nSXT R0", 0o177777, "N---"),
        ("MOV #5, R0\nSXT R0", 0, "-Z--"),
        ("MOV #1, R0\nCMP R0, #2", 1, "N--C"),
        ("MOV #100000, R0\nCMP R0, #1", 0o100000, "--V-"),
        ("MOV #77777, R0\nADD #1, R0", 0o100000, "N-V-"),
        ("MOV #177777, R0\nADD #1, R0", 0, "-Z-C"),
        ("MOV #1, R0\nSUB #2, R0", 0o177777, "N--C"),
        ("MOV #100000, R0\nSUB #1, R0", 0o077777, "--V-"),
        ("MOV #6, R0\nBIT #1, R0", 6, "-Z--"),
        ("MOV #7, R0\nBIC #5, R0", 2, "----"),
        ("CLR R0\nBIS #100000, R0", 0o100000, "N---"),
        ("MOV #7, R0\nMOV #5, R1\nXOR R1, R0", 2, "----"),
        // Byte forms touch only the low byte of a register...
        ("MOV #1377, R0\nINCB R0", 0o1000, "-Z--"),
        ("MOV #177777, R0\nCLRB R0", 0o177400, "-Z--"),
        ("MOV #100, R0\nASLB R0", 0o200, "N-V-"),
        ("MOV #200, R0\nNEGB R0", 0o200, "N-VC"),
        ("MOV #1, R0\nCMPB R0, #2", 1, "N--C"),
        // ...except MOVB, which sign-extends into the whole register.
        ("MOVB #200, R0", 0o177600, "N---"),
        ("MOV #177777, R0\nMOVB #1, R0", 1, "----"),
        // MFPS sign-extends too; MTPS can't set the T bit.
        ("MTPS #357\nMFPS R0", 0o177757, "N--C"),
        ("MTPS #20\nMFPS R0", 0, "-Z--"),
    ];
    for &(src, r0, nzvc) in cases {
        let (cpu, _, _) = run(&format!("{src}\nDONE: BR DONE"));
        assert_eq!(
            (cpu.r[0], flags(cpu.psw).as_str()),
            (r0, nzvc),
            "{src:?}: R0 = {:06o}",
            cpu.r[0]
        );
        assert_eq!(cpu.psw & psw::T, 0, "{src:?} set T");
    }
}

#[test]
fn addressing_modes() {
    let (cpu, _, p) = run("
        MOV #TBL, R1
        MOV (R1)+, R0       ; 111
        MOV @(R1)+, R2      ; via pointer at TBL+2: 222
        MOV #TBL+4, R5
        MOV @-(R5), R3      ; via pointer at TBL+2: 222
        MOV -(R5), R4       ; 111, R5 = TBL
        MOV 2(R5), SP       ; VAL
        MOV @2(R5), R5      ; 222
DONE:   BR DONE
TBL:    .WORD 111, VAL
VAL:    .WORD 222
    ");
    let tbl = p.symbol("TBL").unwrap();
    assert_eq!(cpu.r[..6], [0o111, tbl + 4, 0o222, 0o222, 0o111, 0o222]);
    assert_eq!(cpu.r[6], p.symbol("VAL").unwrap());

    // PC modes: immediate, absolute, relative, relative deferred.
    let (cpu, _, _) = run("
        MOV #5, R0
        MOV @#VAL, R1
        MOV VAL, R2
        MOV @PTR, R3
DONE:   BR DONE
VAL:    .WORD 123
PTR:    .WORD VAL
    ");
    assert_eq!(cpu.r[..4], [5, 0o123, 0o123, 0o123]);
}

#[test]
fn byte_autoincrement_steps_by_one_except_sp_and_pc() {
    let (cpu, _, _) = run("
        MOV #2000, R1
        TSTB (R1)+
        TSTB -(R1)
        TSTB -(R1)
        TSTB (SP)+
DONE:   BR DONE
    ");
    assert_eq!(cpu.r[1], 0o1777);
    assert_eq!(cpu.r[6], ORIGIN + 2);
}

#[test]
fn every_branch_condition() {
    use vm1::decode::Cond;
    for cond in Cond::ALL {
        for nzvc in 0..16u16 {
            let (n, z, v, c) = (nzvc & 8 != 0, nzvc & 4 != 0, nzvc & 2 != 0, nzvc & 1 != 0);
            // PDP-11 handbook definitions.
            let taken = match cond {
                Cond::Br => true,
                Cond::Bne => !z,
                Cond::Beq => z,
                Cond::Bge => !(n ^ v),
                Cond::Blt => n ^ v,
                Cond::Bgt => !(z || (n ^ v)),
                Cond::Ble => z || (n ^ v),
                Cond::Bpl => !n,
                Cond::Bmi => n,
                Cond::Bhi => !(c || z),
                Cond::Blos => c || z,
                Cond::Bvc => !v,
                Cond::Bvs => v,
                Cond::Bcc => !c,
                Cond::Bcs => c,
            };
            let (cpu, _, _) = run(&format!(
                "MTPS #{nzvc:o}\n{} YES\nCLR R0\nBR DONE\nYES: MOV #1, R0\nDONE: BR DONE",
                cond.name()
            ));
            assert_eq!(cpu.r[0] == 1, taken, "{} with NZVC={nzvc:04b}", cond.name());
        }
    }
}

#[test]
fn subroutines_and_loops() {
    let (cpu, _, _) = run("
        MOV #5, R1
        CLR R0
LOOP:   INC R0
        SOB R1, LOOP
        JSR PC, SUB1
        JSR R5, SUB2
        .WORD 42
DONE:   BR DONE
SUB1:   MOV #7, R2
        RTS PC
SUB2:   MOV (R5)+, R3       ; inline argument
        RTS R5
    ");
    assert_eq!(cpu.r[..4], [5, 0, 7, 0o42]);
    assert_eq!(cpu.r[6], ORIGIN, "stack balanced");
}

#[test]
fn mark_unwinds_the_stack() {
    let (cpu, _, _) = run("
        MOV #12345, R5
        MOV R5, -(SP)
        MOV #11, -(SP)
        MOV #22, -(SP)
        MOV #6402, -(SP)    ; MARK 2, executed from the stack
        MOV SP, R5
        JSR PC, SUBR
        MOV #1, R0
DONE:   BR DONE
SUBR:   RTS R5
    ");
    assert_eq!((cpu.r[0], cpu.r[5], cpu.r[6]), (1, 0o12345, ORIGIN));
}

#[test]
fn emt_trap_entry_and_rti() {
    let (cpu, _, p) = run("
        MOV #HANDLER, @#30
        MOV #1340, @#32     ; only the low byte becomes the PSW
        SEC
        EMT 5
AFTER:  MFPS R4
DONE:   BR DONE
HANDLER:
        MOV (SP), R1        ; return address
        MOV 2(SP), R2       ; saved PSW
        MFPS R3
        RTI
    ");
    assert_eq!(cpu.r[1], p.symbol("AFTER").unwrap());
    assert_eq!(cpu.r[2], psw::C);
    assert_eq!(
        cpu.r[3], 0o177740,
        "handler PSW = 340, sign-extended by MFPS"
    );
    assert_eq!(cpu.r[4], psw::C, "RTI restored the PSW");
    assert_eq!(cpu.r[6], ORIGIN);
}

/// Programs for the trap tests: a handler at vector `vec` that stores the
/// return address in R1 and counts entries in R0.
fn with_handler(vec: u16, body: &str) -> String {
    format!(
        "MOV #HANDLER, @#{vec:o}\nCLR @#{:o}\n{body}\nDONE: BR DONE\nHANDLER: INC R0\nMOV (SP), R1\nRTI",
        vec + 2
    )
}

#[test]
fn reserved_opcode_traps_to_10() {
    let (cpu, _, p) = run(&with_handler(0o10, ".WORD 070000 ; MUL is EIS\nAFTER: NOP"));
    assert_eq!((cpu.r[0], cpu.r[1]), (1, p.symbol("AFTER").unwrap()));
}

#[test]
fn jmp_and_jsr_to_a_register_trap_to_4() {
    for insn in ["JMP R0", "JSR PC, R2"] {
        let (cpu, _, p) = run(&with_handler(0o4, &format!("{insn}\nAFTER: NOP")));
        assert_eq!(
            (cpu.r[0], cpu.r[1]),
            (1, p.symbol("AFTER").unwrap()),
            "{insn}"
        );
    }
}

#[test]
fn bus_error_aborts_the_instruction_and_traps_to_4() {
    let (cpu, _, p) = run(&with_handler(
        0o4,
        "MOV #7, R2\nMOV #170000, R3\nMOV (R3)+, R2\nAFTER: NOP",
    ));
    assert_eq!((cpu.r[0], cpu.r[1]), (1, p.symbol("AFTER").unwrap()));
    assert_eq!(cpu.r[2], 7, "destination not written");
    assert_eq!(
        cpu.r[3], 0o170002,
        "autoincrement before the failing cycle still happens"
    );
}

#[test]
fn halt_traps_to_4_on_the_bk() {
    let (cpu, bus, p) = run(&with_handler(0o4, "HALT\nAFTER: NOP"));
    assert_eq!((cpu.r[0], cpu.r[1]), (1, p.symbol("AFTER").unwrap()));
    assert_eq!(bus.sel1_writes, [0o210], "HALT sets the SEL1 write flag");
}

#[test]
fn double_bus_error_halts_the_cpu() {
    let (mut cpu, mut bus, _) = setup("MOV #170000, SP\nTST @#170000\nNOP");
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert!(cpu.is_halted());
}

#[test]
fn stop_key_writes_sel1_and_traps_to_4() {
    let (mut cpu, mut bus, p) = setup(&with_handler(0o4, "MOV #1, R2\nAFTER: NOP"));
    cpu.request_irq1();
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    assert_eq!(bus.sel1_writes, [0o210]);
    // Taken after the first instruction.
    assert_eq!(cpu.r[0], 1);
}

#[test]
fn trace_bit() {
    // RTI that sets T traps immediately; RTT lets one instruction run first.
    let (cpu, _, _) = run("
        MOV #TRACE, @#14
        CLR @#16
        MOV #20, -(SP)
        MOV #TGT, -(SP)
        RTI
TGT:    NOP
        NOP
DONE:   BR DONE
TRACE:  INC R0
        CMP (SP), #DONE
        BNE TRACE1
        BIC #20, 2(SP)
TRACE1: RTT
    ");
    assert_eq!(cpu.r[0], 3);
}

#[test]
fn evnt_is_masked_by_priority() {
    let (mut cpu, mut bus, p) = setup(
        "
        MOV #HANDLER, @#100
        CLR @#102
        MTPS #200
MASKED: NOP
        MTPS #0
AFTER:  NOP
DONE:   BR DONE
HANDLER: INC R0
        MOV (SP), R1
        RTI
    ",
    );
    run_to(&mut cpu, &mut bus, p.symbol("MASKED").unwrap());
    cpu.request_evnt();
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    assert_eq!((cpu.r[0], cpu.r[1]), (1, p.symbol("AFTER").unwrap()));
}

/// An EMT executed with EVNT pending: the EMT handler runs first (its PSW
/// masks EVNT), and EVNT comes after its RTI. As on the VM1 chip model.
#[test]
fn emt_is_taken_before_a_pending_interrupt() {
    let (mut cpu, mut bus, p) = setup(
        "
        MOV #EMTH, @#30
        MOV #200, @#32
        MOV #EVH, @#100
        CLR @#102
EMTI:   EMT 1
AFTER:  NOP
DONE:   BR DONE
EMTH:   MOV (SP), R1          ; the EMT's return address
        MOV R0, R2            ; EVNT not taken yet
        RTI
EVH:    INC R0
        MOV (SP), R3          ; where EVNT interrupted
        RTI
    ",
    );
    run_to(&mut cpu, &mut bus, p.symbol("EMTI").unwrap());
    cpu.request_evnt();
    cpu.step(&mut bus); // EMT, with EVNT pending at its end
    assert_eq!(
        cpu.pc(),
        p.symbol("EMTH").unwrap(),
        "in the EMT handler first"
    );
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    let after = p.symbol("AFTER").unwrap();
    assert_eq!(
        (cpu.r[1], cpu.r[2], cpu.r[0], cpu.r[3]),
        (after, 0, 1, after)
    );
}

/// The same with a key (VIRQ): the monitor's EMT dispatcher must find the
/// EMT, not the keyboard handler, at its return address.
#[test]
fn emt_is_taken_before_a_pending_key() {
    let (mut cpu, mut bus, p) = setup(
        "
        MOV #EMTH, @#30
        MOV #200, @#32
        MOV #KEYH, @#60
        CLR @#62
EMTI:   EMT 7
DONE:   BR DONE
EMTH:   MOV (SP), R1
        MOV -2(R1), R2        ; the EMT instruction, as a dispatcher reads it
        RTI
KEYH:   INC R0
        RTI
    ",
    );
    run_to(&mut cpu, &mut bus, p.symbol("EMTI").unwrap());
    bus.virq = Some(0o60);
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    assert_eq!((cpu.r[2], cpu.r[0]), (0o104007, 1));
}

/// An EMT executed with T set: no trace trap at the handler's entry (its
/// PSW has T clear); it comes after the handler's RTI restores T, at the
/// EMT's return address. As on the VM1 chip model.
#[test]
fn emt_with_the_trace_bit() {
    let (cpu, _, p) = run("
        MOV #EMTH, @#30
        MOV #200, @#32
        MOV #TRH, @#14
        MOV #200, @#16
        MOV #20, -(SP)
        MOV #TGT, -(SP)
        RTT
TGT:    EMT 1
AFTER:  NOP
DONE:   BR DONE
EMTH:   MOV (SP), R1
        MOV 2(SP), R2
        MOV R0, R4            ; trace traps so far
        RTI
TRH:    INC R0
        MOV (SP), R3
        BIC #20, 2(SP)
        RTI
    ");
    let after = p.symbol("AFTER").unwrap();
    assert_eq!(
        (cpu.r[1], cpu.r[2], cpu.r[4]),
        (after, 0o20, 0),
        "EMT first, T still set in its saved PSW"
    );
    assert_eq!(
        (cpu.r[0], cpu.r[3]),
        (1, after),
        "one trace trap, after the handler's RTI"
    );
}

#[test]
fn wait_until_interrupt() {
    let (mut cpu, mut bus, p) = setup(
        "
        MOV #HANDLER, @#100
        CLR @#102
        WAIT
AFTER:  NOP
DONE:   BR DONE
HANDLER: INC R0
        RTI
    ",
    );
    for _ in 0..4 {
        cpu.step(&mut bus);
    }
    assert!(cpu.is_waiting());
    assert_eq!(cpu.pc(), p.symbol("AFTER").unwrap());
    cpu.request_evnt();
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    assert_eq!(cpu.r[0], 1);
}

#[test]
fn vectored_interrupt_from_the_bus() {
    let (mut cpu, mut bus, p) = setup(
        "
        MOV #HANDLER, @#60
        CLR @#62
        MTPS #200
MASKED: NOP
        MTPS #0
        NOP
DONE:   BR DONE
HANDLER: INC R0
        RTI
    ",
    );
    run_to(&mut cpu, &mut bus, p.symbol("MASKED").unwrap());
    bus.virq = Some(0o60);
    cpu.step(&mut bus);
    assert_eq!(bus.virq, Some(0o60), "not acknowledged while masked");
    run_to(&mut cpu, &mut bus, p.symbol("DONE").unwrap());
    assert_eq!((cpu.r[0], bus.virq), (1, None));
}

#[test]
fn reset_instruction_resets_devices() {
    let (_, bus, _) = run("RESET\nDONE: BR DONE");
    assert_eq!(bus.resets, 1);
}

#[test]
fn byte_writes_read_their_destination_first() {
    let (_, bus, _) = run("MOVB R0, @#2000\nCLRB @#2001\nMOV R0, @#2002\nDONE: BR DONE");
    assert_eq!(bus.byte_reads, [0o2000, 0o2001]);
}

#[test]
fn instruction_timings() {
    let cases: &[(&str, u32)] = &[
        ("NOP", 12),
        ("MOV R0, R1", 12),
        ("MOV #1, R0", 12 + 12 + 4),
        ("MOV R0, @#2000", 12 + 32),
        ("MOV @#2000, @#2002", 12 + 24 + 24),
        ("CMP R0, (R1)", 12 + 20),
        ("TST (R1)", 12 + 12),
        ("INC (R1)", 12 + 20),
        ("BR .+2", 16),
        ("SOB R0, .", 20),
        ("JSR PC, @#2000", 40),
        ("EMT 0", 68),
        ("JMP R0", 68),
        (".WORD 070000", 144),
    ];
    for &(src, expected) in cases {
        let (mut cpu, mut bus, _) = setup(src);
        bus.set_word(0o2000, 0o2000);
        cpu.r[1] = 0o2000;
        // Vectors point at a NOP so trap entry succeeds.
        for v in [0o4, 0o10, 0o30] {
            bus.set_word(v, 0o3000);
        }
        assert_eq!(cpu.step(&mut bus), expected, "{src}");
    }
}

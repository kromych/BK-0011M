//! Disassembler and assembler checked against each other over every opcode.

use vm1::asm::assemble;
use vm1::disassemble;

const ORIGIN: u16 = 0o1000;

fn octal(words: &[u16]) -> String {
    words
        .iter()
        .map(|w| format!("{w:06o}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn every_opcode_round_trips() {
    let mut failures = Vec::new();
    for op in 0..=u16::MAX {
        let mem = [op, 0o001234, 0o177770];
        let read = |a: u16| mem[usize::from(a.wrapping_sub(ORIGIN) / 2)];
        let d = disassemble(ORIGIN, read);
        let expected = &mem[..usize::from(d.words)];

        match assemble(&d.text, ORIGIN) {
            Ok(p) if p.words() == expected => {}
            Ok(p) => failures.push(format!("{op:06o}: {:?} -> {}", d.text, octal(&p.words()))),
            Err(e) => failures.push(format!("{op:06o}: {:?} -> {e}", d.text)),
        }
    }
    assert!(
        failures.is_empty(),
        "{} opcodes failed; first ones:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn disassembly_text() {
    let cases: &[(&[u16], &str, u16)] = &[
        (&[0o012700, 0o1000], "MOV    #001000, R0", 2),
        (
            &[0o013737, 0o177716, 0o2000],
            "MOV    @#177716, @#002000",
            3,
        ),
        (&[0o005067, 0o4], "CLR    001010", 2), // PC-relative, target shown absolute
        (&[0o105161, 0o177776], "COMB   177776(R1)", 2),
        (&[0o000777], "BR     001000", 1), // branch to itself
        (&[0o077302], "SOB    R3, 000776", 1),
        (&[0o004737, 0o100140], "JSR    PC, @#100140", 2),
        (&[0o000207], "RTS    PC", 1),
        (&[0o104016], "EMT    016", 1),
        (&[0o000251], "CLN!CLC", 1),
        (&[0o000257], "CCC", 1),
        (&[0o000240], "NOP", 1),
        (&[0o106427, 0o340], "MTPS   #000340", 2),
        (&[0o070000], ".WORD  070000", 1),
    ];
    for &(words, text, len) in cases {
        let d = disassemble(ORIGIN, |a| {
            words
                .get(usize::from((a - ORIGIN) / 2))
                .copied()
                .unwrap_or(0)
        });
        assert_eq!((d.text.as_str(), d.words), (text, len), "{}", octal(words));
    }
}

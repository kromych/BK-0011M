"""Write timing.mac: probes around 100 copies of each instruction under test,
plus SOB loops of 1000 and 2000 iterations. Assemble it at 100000 with
    cargo run -p vm1 --example asm -- timing.mac 100000 timing.bin
"""
TESTS = [
    ("base", ""), ("NOP", "NOP"), ("MOV R1,R2", "MOV R1, R2"), ("MOV #n,R0", "MOV #123, R0"),
    ("ADD #n,R5", "ADD #27, R5"), ("MOV #n,(R3)", "MOV #160, (R3)"), ("CLR (R3)", "CLR (R3)"),
    ("TST (R3)", "TST (R3)"), ("MOV (R3),R1", "MOV (R3), R1"), ("MOV R1,(R3)", "MOV R1, (R3)"),
    ("INC R1", "INC R1"), ("CMP R1,R2", "CMP R1, R2"), ("ASL R1", "ASL R1"), ("SWAB R1", "SWAB R1"),
    ("MOVB (R3),R1", "MOVB (R3), R1"), ("MOVB R1,(R3)", "MOVB R1, (R3)"), ("XOR R1,R2", "XOR R1, R2"),
    ("MOV @#a,R1", "MOV @#2000, R1"), ("MOV R1,@#a", "MOV R1, @#2000"), ("BR .+2", "BR .+2"),
    ("BNE taken", "BNE .+2"), ("BEQ not taken", "BEQ .+2"), ("JSR+RTS", "JSR PC, SUBR"),
]
lines = ["        MOV #1000, SP", "        MOV #2000, R3", "        MOV #1, R4"]
probe = 0
for name, insn in TESTS:
    probe += 1
    lines.append(f"        MOV #{probe:o}, @#177000   ; {name}")
    lines += [f"        {insn}"] * (100 if insn else 0)
    probe += 1
    lines.append(f"        MOV #{probe:o}, @#177000")
for n in (1000, 2000):
    probe += 1
    lines += [f"        MOV #{probe:o}, @#177000   ; SOB x{n}", f"        MOV #{n:o}, R0", f"L{n}:   SOB R0, L{n}"]
    probe += 1
    lines.append(f"        MOV #{probe:o}, @#177000")
lines += ["        MOV #177777, @#177000", "DONE:   BR DONE", "SUBR:   RTS PC"]
open("timing.mac", "w").write("\n".join(lines) + "\n")

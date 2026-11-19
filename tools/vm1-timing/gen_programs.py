"""Write the programs whose bus cycles buslogs/ holds (see README.md):

    python3 gen_programs.py        # into the current directory

single, doubleA, doubleB, psw, pc: every instruction in every addressing
mode, each test instruction after the same setup and a NOP. flow: jumps,
branches, returns, traps and bus errors. ovl: each instruction after a NOP
and after a memory write, and instructions after MOVB into a register.
io: the SEL registers and the CPU's timer. Assemble each at 1000:

    cargo run -p vm1 --example asm -- single.mac 1000 single.bin
"""

def mode(m, r, x=2):
    return {0: f'R{r}', 1: f'(R{r})', 2: f'(R{r})+', 3: f'@(R{r})+', 4: f'-(R{r})',
            5: f'@-(R{r})', 6: f'{x}(R{r})', 7: f'@{x}(R{r})'}[m]

DOUBLE = ['MOV', 'CMP', 'BIT', 'BIC', 'BIS', 'ADD', 'SUB', 'MOVB', 'CMPB', 'BITB', 'BICB', 'BISB']
SINGLE = ['CLR', 'COM', 'INC', 'DEC', 'NEG', 'ADC', 'SBC', 'TST', 'ROR', 'ROL', 'ASR', 'ASL',
          'CLRB', 'COMB', 'INCB', 'DECB', 'NEGB', 'ADCB', 'SBCB', 'TSTB', 'RORB', 'ROLB', 'ASRB', 'ASLB',
          'SWAB', 'SXT', 'MFPS']

def tests(group):
    if group in ('double', 'doubleA', 'doubleB'):
        ops = {'double': DOUBLE, 'doubleA': DOUBLE[:6], 'doubleB': DOUBLE[6:]}[group]
        for op in ops:
            for s in range(8):
                for d in range(8):
                    yield f'{op} {mode(s,1)}, {mode(d,2)}', {'op': op, 'src': s, 'dst': d}
    elif group == 'single':
        for op in SINGLE:
            for d in range(8):
                yield f'{op} {mode(d,2)}', {'op': op, 'dst': d}
        for d in range(8):
            yield f'XOR R1, {mode(d,2)}', {'op': 'XOR', 'dst': d}
    elif group == 'psw':
        for d in range(8):
            yield f'MTPS {mode(d,2)}', {'op': 'MTPS', 'dst': d}
    elif group == 'pc':
        # PC addressing (immediate, absolute, relative, relative deferred)
        for op in ['MOV', 'ADD', 'CMP', 'MOVB']:
            for src in ['#4000', '@#4000', 'D', '@D']:
                for dst in ['R2', '(R2)', '@#4000', 'D', '@D']:
                    yield f'{op} {src}, {dst}', {'op': op, 'src': src, 'dst': dst}
        for op in ['CLR', 'INC', 'TST']:
            for dst in ['@#70000', 'D', '@D']:
                yield f'{op} {dst}', {'op': op, 'dst': dst}

def make_group(group, out_mac):
    lines = ['        MOV #177000, SP']
    for i, (text, info) in enumerate(tests(group)):
        lines += ['        CLR @#70000', '        CLR @#70002', '        CLR @#67776']
        lines += ['        MOV #70000, R0', '        MOV #70000, R1', '        MOV #70000, R2',
                  '        MOV #70000, R3', '        MOV #70000, R4', '        MOV #70000, R5',
                  '        NOP', f'T{i}:    {text}', f'N{i}:    NOP']
    lines += ['        MOV #177777, @#177000', 'D:      .WORD 70000']
    open(out_mac, 'w').write('\n'.join(lines) + '\n')

FLOW = []  # (name, code lines, start label suffix, end label suffix)

def flow_test(name, *lines):
    FLOW.append((name, list(lines)))

# Each body uses {S} for the start label and {E} for the end label.
flow_test('BR taken',            '{S}: BR {E}', '{E}: NOP')
flow_test('BEQ not taken',       '        CLR R0', '        INC R0', '{S}: BEQ {E}', '{E}: NOP')
flow_test('BNE taken',           '        CLR R0', '        INC R0', '{S}: BNE {E}', '{E}: NOP')
flow_test('SOB taken',           '        MOV #2, R1', '{S}: SOB R1, {S}', '{E}: NOP')  # first pass: ends at {S} again
flow_test('SOB not taken',       '        MOV #1, R1', '{S}: SOB R1, {S}', '{E}: NOP')
for m, src in [(1, '(R1)'), (2, '(R1)+'), (3, '@(R1)+'), (6, '2(R1)'), (7, '@2(R1)')]:
    if m in (1, 2):
        setup = '        MOV #{E}, R1'
    elif m == 3:
        setup = '        MOV #P{S}, R1'
    elif m == 6:
        setup = '        MOV #{E}-2, R1'
    else:
        setup = '        MOV #P{S}-2, R1'
    flow_test(f'JMP mode {m}', setup, '{S}: JMP ' + src, 'P{S}: .WORD {E}', '{E}: NOP')
    flow_test(f'JSR PC mode {m}', setup, '        MOV #77000, SP', '{S}: JSR PC, ' + src, 'P{S}: .WORD {E}', '{E}: NOP')
flow_test('JMP absolute',        '{S}: JMP @#{E}', '        NOP', '{E}: NOP')
flow_test('JMP relative',        '{S}: JMP {E}', '        NOP', '{E}: NOP')
flow_test('JSR PC relative',     '        MOV #77000, SP', '{S}: JSR PC, {E}', '        NOP', '{E}: NOP')
flow_test('JSR R5 relative',     '        MOV #77000, SP', '{S}: JSR R5, {E}', '        NOP', '{E}: NOP')
flow_test('RTS PC',              '        MOV #77000, SP', '        MOV #{E}, -(SP)', '{S}: RTS PC', '        NOP', '{E}: NOP')
flow_test('RTS R5',              '        MOV #77000, SP', '        MOV #1000, -(SP)', '        MOV #{E}, R5', '{S}: RTS R5', '        NOP', '{E}: NOP')
flow_test('RTI',                 '        MOV #77000, SP', '        CLR -(SP)', '        MOV #{E}, -(SP)', '{S}: RTI', '        NOP', '{E}: NOP')
flow_test('RTT',                 '        MOV #77000, SP', '        CLR -(SP)', '        MOV #{E}, -(SP)', '{S}: RTT', '        NOP', '{E}: NOP')
flow_test('MARK 0',              '        MOV #77000, SP', '        MOV #{E}, R5', '{S}: MARK 0', '        .WORD 1000', '{E}: NOP')
for name, op, vec in [('EMT', 'EMT 1', 30), ('TRAP', 'TRAP 1', 34), ('IOT', 'IOT', 20), ('BPT', 'BPT', 14)]:
    flow_test(name, '        MOV #77000, SP', f'        MOV #{{E}}, @#{vec}', f'        CLR @#{vec + 2}', '{S}: ' + op, '        NOP', '{E}: NOP')
flow_test('bus error (write to ROM)', '        MOV #77000, SP', '        MOV #{E}, @#4', '        CLR @#6', '        MOV #120000, R1', '{S}: CLR (R1)', '        NOP', '{E}: NOP')
flow_test('bus error (MOV to ROM)', '        MOV #77000, SP', '        MOV #{E}, @#4', '        CLR @#6', '        MOV #120000, R1', '{S}: MOV R0, (R1)', '        NOP', '{E}: NOP')
for op in ['CLC', 'SEC', 'CCC', 'SCC', 'NOP']:
    flow_test(op, '{S}: ' + op, '{E}: NOP')

def make_flow(out_mac):
    lines = ['        MOV #77000, SP']
    for i, (name, body) in enumerate(FLOW):
        for l in body:
            l = l.replace('{S}', f'S{i}').replace('{E}', f'E{i}')
            if ':' in l.split()[0] if l.split() else False:
                lab, rest = l.split(':', 1)
                lines.append(f'{lab}:  {rest.strip()}')
            else:
                lines.append(l)
    lines += ['        MOV #177777, @#177000']
    open(out_mac, 'w').write('\n'.join(lines) + '\n')

OVERLAP = [  # (name, setup lines, X lines (first is labelled S), after)
    ('NOP', [], ['NOP']),
    ('MOV R1,R2', [], ['MOV R1, R2']),
    ('MOV #n,R2', [], ['MOV #5, R2']),
    ('CLR (R3)', ['MOV #70000, R3'], ['CLR (R3)']),
    ('BR', [], ['BR {E}']),
    ('BNE taken', ['CLR R0', 'INC R0'], ['BNE {E}']),
    ('SOB taken', ['MOV #2, R0'], ['SOB R0, {S}']),
    ('JMP (R1)', ['MOV #{E}, R1'], ['JMP (R1)']),
    ('JMP @#E', [], ['JMP @#{E}']),
    ('JSR PC,(R1)', ['MOV #{E}, R1'], ['JSR PC, (R1)']),
    ('JSR PC,@#E', [], ['JSR PC, @#{E}']),
    ('RTS PC', ['MOV #{E}, -(SP)', 'NOP'], ['RTS PC']),
    ('RTI', ['CLR -(SP)', 'MOV #{E}, -(SP)', 'NOP'], ['RTI']),
    ('MARK 0', ['MOV #{E}, R5'], ['MARK 0', '.WORD 1000']),
    ('EMT', ['MOV #{E}, @#30', 'CLR @#32'], ['EMT 1']),
    ('IOT', ['MOV #{E}, @#20', 'CLR @#22'], ['IOT']),
    ('MTPS R0', ['CLR R0'], ['MTPS R0']),
    ('MTPS (R3)', ['MOV #70000, R3'], ['MTPS (R3)']),
    ('MFPS R2', [], ['MFPS R2']),
    ('MFPS (R3)', ['MOV #70000, R3'], ['MFPS (R3)']),
    ('XOR R1,R2', [], ['XOR R1, R2']),
    ('SWAB R2', [], ['SWAB R2']),
    ('SXT R2', [], ['SXT R2']),
    ('MOVB R1,R2', [], ['MOVB R1, R2']),
    ('TST (R3)', ['MOV #70000, R3'], ['TST (R3)']),
    ('ADD @#a,R2', [], ['ADD @#70000, R2']),
    ('MOV R1,-(SP)', [], ['MOV R1, -(SP)']),
]
FLOOR = [  # MOVB R1,R2 then X: does the sign-extension floor bind?
    ('NOP', ['NOP']), ('CLC', ['CLC']), ('MOV R1,R3', ['MOV R1, R3']), ('INC R3', ['INC R3']),
    ('BR', ['BR {E}']), ('MOV #n,R3', ['MOV #5, R3']), ('CLR (R3)', ['CLR (R3)']),
    ('TST R2', ['TST R2']), ('MOVB R1,R3', ['MOVB R1, R3']), ('JMP @#E', ['JMP @#{E}']),
    ('EMT', ['EMT 1']), ('SOB', ['SOB R0, {X}']), ('MTPS R0', ['MTPS R0']),
]
def make_overlap(out):
    L = ['        MOV #77000, SP', '        MOV #70000, R3']
    k = 0
    for name, setup, x in OVERLAP:
        for prev in ('NOP', 'CLR @#70000'):
            k += 1
            sub = lambda s: s.replace('{E}', f'E{k}').replace('{S}', f'S{k}').replace('{X}', f'X{k}')
            L += ['        MOV #77000, SP', '        MOV #70000, R3'] + ['        ' + sub(s) for s in setup]
            L += ['        NOP', f'        {prev}', f'S{k}:    ' + sub(x[0])] + ['        ' + sub(s) for s in x[1:]]
            L += ['        NOP', f'E{k}:    NOP', '        NOP']
    for name, x in FLOOR:
        k += 1
        sub = lambda s: s.replace('{E}', f'E{k}').replace('{S}', f'S{k}').replace('{X}', f'X{k}')
        L += ['        MOV #77000, SP', '        MOV #70000, R3', '        MOV #2, R0', '        CLR R1',
              f'        MOV #E{k}, @#30', '        CLR @#32', '        NOP', '        NOP',
              f'S{k}:    MOVB R1, R2', f'X{k}:    ' + sub(x[0]), '        NOP', f'E{k}:    NOP', '        NOP']
    L += ['        MOV #177777, @#177000']
    open(out, 'w').write('\n'.join(L) + '\n')

IO = """        MOV #177716, R3
        MOV #40, R5
        NOP
S0:     BIT R5, (R3)
E0:     NOP
S1:     MOVB @#177716, R0
E1:     NOP
S2:     MOV R0, @#177716
E2:     NOP
S3:     MOV @#177714, R0
E3:     NOP
S4:     MOV @#177710, R0
E4:     NOP
S5:     MOV @#177660, R0
E5:     NOP
S6:     MOV R0, (R3)
E6:     NOP
S7:     INC R4
        BIT R5, (R3)
        BEQ S7
E7:     NOP
        MOV #177777, @#177000
"""

if __name__ == '__main__':
    for group in ('single', 'doubleA', 'doubleB', 'psw', 'pc'):
        make_group(group, f'{group}.mac')
    make_flow('flow.mac')
    make_overlap('ovl.mac')
    open('io.mac', 'w').write(IO)

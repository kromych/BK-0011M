"""Make the MiST VM1 microcode decoder simulate correctly under Verilator.

The decoder (vm1_plm.v) compares {ir, mr} against ~500 patterns with
don't-care bits through a function that uses casex on a *function
argument*. Synthesis keeps the x bits as wildcards; Verilator, a two-state
simulator, turns them into 0, so almost every pattern mismatches and the
core aborts forever right after reset. This rewrites every comparison as
((value & mask) == bits).

usage: python3 patch_plm.py vm1_plm.v vm1_plm_fixed.v
"""
import re
import sys

src = open(sys.argv[1]).read()
pattern = re.compile(r"cmp\((\{[^}]*\}|[^,]+),\s*\{\s*(\d+)'b([01xX]+)\s*,\s*(\d+)'b([01xX]+)\s*\}\)")


def replace(m):
    expr, w1, b1, w2, b2 = m.groups()
    bits = b1 + b2
    assert len(bits) == int(w1) + int(w2), m.group(0)
    mask = "".join("0" if c in "xX" else "1" for c in bits)
    value = "".join("0" if c in "xX" else c for c in bits)
    return f"((({expr}) & {len(bits)}'b{mask}) == {len(bits)}'b{value})"


out, n = pattern.subn(replace, src)
open(sys.argv[2], "w").write(out)
print(f"rewrote {n} pattern comparisons")

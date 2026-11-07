"""Prints the <k> cell of the K walk-through after each rewrite

Usage: python3 -I trace.py st-kompiled walkthrough.k [first-depth last-depth]
Runs krun with --depth first, first+1, ... and prints the <k> cell whenever its
abbreviated shape changes: C for a context, v for a value, and rules by name.
"""

import re
import subprocess
import sys

HEADS_VALUE = ("VCASE", "VTUP", "VLIST", "VOPT", "VNAT", "VTEXT", "VBOOL", "VEXT")
HEADS_EXP = ("EVAR", "ECMP", "ECASE", "ENE", "ETUP", "EOPT", "ELIST", "ECONS", "ECALL",
             "EMATCH", "EBOOL", "ENAT", "ETEXT")


def k_cell(definition, program, depth):
    out = subprocess.run(
        ["krun", "--definition", definition, program, "--depth", str(depth)],
        capture_output=True, text=True, check=True).stdout
    match = re.search(r"<k>(.*?)</k>", out, re.S)
    return " ".join(match.group(1).split())


def parse(text):
    """Parses `head ( args , ... )` terms into (head, [args]) trees; leaves are strings."""
    tokens = re.findall(r'"(?:[^"\\]|\\.)*"|~>|[(),]|[^\s(),]+', text)
    pos = 0

    def term():
        nonlocal pos
        parts = []
        while pos < len(tokens) and tokens[pos] not in (",", ")"):
            tok = tokens[pos]
            pos += 1
            if pos < len(tokens) and tokens[pos] == "(":
                pos += 1
                args = []
                while tokens[pos] != ")":
                    args.append(term())
                    if tokens[pos] == ",":
                        pos += 1
                pos += 1
                parts.append((tok, args))
            else:
                parts.append(tok)
        return parts

    parts = term()
    # A list at the top of <k> continues after its commas
    while pos < len(tokens) and tokens[pos] == ",":
        pos += 1
        parts.append(",")
        parts.extend(term())
    return parts


def show(part):
    if isinstance(part, list):
        return " ".join(show(p) for p in part)
    if isinstance(part, str):
        return part
    head, args = part
    if head == "CTX":
        return "C"
    if head in HEADS_VALUE:
        return "v"
    if head in HEADS_EXP:
        return head + "(…)"
    if head == "rul":
        return "rul(" + show(args[0]) + " …)"
    if head in ("IF", "LET"):
        return head + "(…)"
    if head == "REL":
        return "REL(" + show(args[0]) + " …)"
    if head == "run":
        return "run(…)"
    return head + "(" + ", ".join(show(a) for a in args) + ")"


def main():
    definition, program = sys.argv[1], sys.argv[2]
    first, last_depth = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (0, 40)
    last = None
    for depth in range(first, last_depth + 1):
        shape = show(parse(k_cell(definition, program, depth)))
        if shape != last:
            print(f"[{depth}] {shape}")
            last = shape


main()

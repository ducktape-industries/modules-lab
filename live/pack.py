"""pack.py SECTION PROGRAM PAYLOAD OUT: PROGRAM with PAYLOAD appended as the
custom section SECTION (`ducktape.view`, `ducktape.describe`), as modules'
`founding` crate packs its own. A fresh build holds neither section, so
nothing is stripped first. OUT may be PROGRAM."""
import sys

section, program, payload, out = sys.argv[1:]


def leb(n):
    b = bytearray()
    while True:
        b.append(n & 0x7F | (0x80 if n >> 7 else 0))
        n >>= 7
        if not n:
            return bytes(b)


body = leb(len(section)) + section.encode() + open(payload, "rb").read()
code = open(program, "rb").read()
open(out, "wb").write(code + b"\0" + leb(len(body)) + body)

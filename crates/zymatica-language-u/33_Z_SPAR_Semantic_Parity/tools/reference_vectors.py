#!/usr/bin/env python3
"""Independent Python reference vector generator for Z-SPAR wire compatibility.
Not production code; used only to cross-check the Rust/C++ implementations."""

import hashlib
import json

PRIM = 0x13

def gf_mul(a: int, b: int) -> int:
    a &= 0xF; b &= 0xF; p = 0
    for _ in range(4):
        if b & 1: p ^= a
        carry = a & 0x8
        a <<= 1
        if carry: a ^= PRIM
        a &= 0xF
        b >>= 1
    return p & 0xF

def gf_pow(a: int, e: int) -> int:
    r = 1
    while e:
        if e & 1: r = gf_mul(r, a)
        a = gf_mul(a, a)
        e >>= 1
    return r

def poly_mul(p, q):
    out = [0] * (len(p) + len(q) - 1)
    for i, a in enumerate(p):
        for j, b in enumerate(q):
            out[i+j] ^= gf_mul(a, b)
    return out

g = [1]
for i in range(1,5):
    g = poly_mul(g, [1, gf_pow(2, i)])
assert g == [1,13,12,8,7]

def encode(data):
    work = list(data) + [0]*4
    for i in range(8):
        coef = work[i]
        if coef:
            for j in range(1,5):
                work[i+j] ^= gf_mul(g[j], coef)
    return list(data) + work[8:]

def crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            mask = -(crc & 1) & 0xFFFFFFFF
            crc = ((crc >> 1) ^ (0x82F63B78 & mask)) & 0xFFFFFFFF
    return (~crc) & 0xFFFFFFFF

def stable_text_id(text: str) -> int:
    return int.from_bytes(hashlib.sha256(text.encode()).digest()[:8], 'big')

def i64_bytes_from_u64_bits(v: int) -> bytes:
    return (v & ((1<<64)-1)).to_bytes(8, 'big')

def invariants_bytes():
    records = [
        (1, 1, stable_text_id('VALVE-7')),
        (2, 2, 50),
        (3, 2, stable_text_id('PSI')),
        (4, 9, 1),
    ]
    records.sort(key=lambda x: (x[0], x[1], x[2] if x[2] < (1<<63) else x[2]-(1<<64)))
    out = bytearray([len(records)])
    for kind, key, value in records:
        out += bytes([kind]) + key.to_bytes(2,'big') + i64_bytes_from_u64_bits(value)
    return bytes(out)

def semantic_tag(seq: int, dword: int, inv: bytes) -> bytes:
    material = b'ZSPAR-SEMANTIC-V1' + seq.to_bytes(8,'big') + dword.to_bytes(4,'big') + inv
    return hashlib.sha256(material).digest()[:16]

seq = 0x0102030405060708
dword = 0x14C10F2D
data = [(dword >> s) & 0xF for s in (28,24,20,16,12,8,4,0)]
cw = encode(data)
parity = bytes([(cw[8]<<4)|cw[9], (cw[10]<<4)|cw[11]])
inv = invariants_bytes()
tag = semantic_tag(seq,dword,inv)

systematic = bytearray(b'ZSPS') + bytes([1,1]) + seq.to_bytes(8,'big') + dword.to_bytes(4,'big') + parity + tag
systematic += crc32c(systematic).to_bytes(4,'big')
parity_frame = bytearray(b'ZSPP') + bytes([1,1]) + seq.to_bytes(8,'big') + parity + tag
parity_frame += crc32c(parity_frame).to_bytes(4,'big')

vectors = {
    'gf16_generator': g,
    'concept_dword_hex': f'{dword:08x}',
    'codeword_symbols': cw,
    'parity_hex': parity.hex(),
    'semantic_tag_hex': tag.hex(),
    'systematic_frame_hex': systematic.hex(),
    'parity_frame_hex': parity_frame.hex(),
}
print(json.dumps(vectors, indent=2))

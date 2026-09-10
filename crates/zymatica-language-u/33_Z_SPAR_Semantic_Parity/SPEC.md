# Z-SPAR Protocol Specification v1

## 1. Semantic alphabet

`Concept8D` contains eight symbols, each in `[0,15]`:

1. Domain
2. Subdomain
3. Operation
4. Modality
5. Strength
6. Polarity
7. Temporal horizon
8. Epistemic certainty

Each axis is one element of `GF(16)`.

## 2. Field

- Field: `GF(2^4)`
- Primitive polynomial: `p(x) = x^4 + x + 1` (`0x13`)
- Primitive element: `alpha = 2`
- Addition/subtraction: XOR
- Multiplication: polynomial multiplication reduced modulo `p(x)`

## 3. RS(12,8)

Eight semantic data symbols are encoded with four parity symbols. The generator is:

```text
g(x) = product(i=1..4) (x - alpha^i)
     = [1, 13, 12, 8, 7]
```

The resulting code has minimum distance 5 and bounded-distance capability:

```text
2*e + s <= 4
```

where `e` is unknown symbol errors and `s` is known erasures.

The decoder solves the four parity-check equations directly over GF(16). This is intentional: `n=12` is small and fixed, so direct Gaussian elimination plus bounded error-position enumeration is easy to audit and fast enough for LoRa/edge rates.

## 4. Semantic commitment

```text
TAG = first_16_bytes(
  SHA256(
    "ZSPAR-SEMANTIC-V1" ||
    sequence_u64_be ||
    concept_dword_u32_be ||
    canonical_invariant_bytes
  )
)
```

Canonical invariants begin with one `u8` record count. Each record is 11 bytes:

```text
kind:u8 | key:u16_be | value:i64_be
```

Records are sorted by `(kind, key, signed value)` before hashing.

The 128-bit tag is an integrity commitment. Authentication is external (e.g. Ed25519 or ZK packet binding).

## 5. CRC

All fixed frames end in CRC32C/Castagnoli over every preceding frame byte. Polynomial in reflected implementation: `0x82F63B78`.

## 6. Wire frames

### 6.1 Systematic frame — `ZSPS`, 40 bytes

```text
0..3    magic = "ZSPS"
4       version = 1
5       flags (bit0 = invariant set is non-empty)
6..13   sequence u64 big-endian
14..17  authoritative Concept8D DWORD
18..19  four RS parity nibbles packed two per byte
20..35  semantic SHA-256/128 tag
36..39  CRC32C
```

### 6.2 Parity-only frame — `ZSPP`, 36 bytes

```text
0..3    magic = "ZSPP"
4       version = 1
5       flags
6..13   sequence u64 big-endian
14..15  four RS parity nibbles
16..31  semantic SHA-256/128 tag
32..35  CRC32C
```

Receiver supplies the eight data symbols from local side information/model reconstruction. Low-confidence axes may be marked as erasures. The decoder repairs within `2e+s<=4`; the semantic tag then confirms exact authoritative recovery.

### 6.3 Repair request — `ZSRQ`, 41 bytes

```text
0..3    magic = "ZSRQ"
4       version = 1
5       flags/reserved
6..13   sequence
14..17  receiver predicted DWORD
18      erasure axis mask
19..20  four syndromes packed as nibbles
21..36  expected semantic tag
37..40  CRC32C
```

### 6.4 Repair response — `ZSRP`, 40 bytes

Same physical layout as the systematic frame, but magic `ZSRP`. It returns the authoritative state without retransmitting source-language text.

### 6.5 Invariant patch — `ZSIP`, variable

```text
magic(4) | version(1) | flags(1) | sequence(8) | DWORD(4) |
canonical invariant set (1 + 11*N) | semantic tag(16) | CRC32C(4)
```

Total size: `39 + 11*N` bytes.

## 7. Failure policy

- CRC failure: reject frame before semantic processing.
- RS failure: issue `ZSRQ` or fall back to a higher-fidelity semantic transmission.
- RS success + semantic tag failure: treat as out-of-radius semantic mismatch or invariant mismatch; **do not execute an action**.
- Authentication failure at outer Ed25519/ZK layer: reject regardless of RS/tag result.

## 8. ZK integration recommendation

Bind a hash of the complete authenticated Z-SPAR frame into the Groth16 packet commitment. A recommended nullifier relationship is conceptually:

```text
C_pkt = H(authenticated_zspar_frame)
N     = H(device_secret, nonce, C_pkt, gateway_commitment)
```

The actual circuit hash must match the Zymatica ZK circuit implementation; this document does not silently invent an in-circuit SHA-256 gadget.

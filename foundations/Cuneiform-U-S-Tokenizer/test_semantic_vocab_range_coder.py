import os
import zlib
import struct
import numpy as np
from transformers import AutoTokenizer

TOKENIZER_DIR = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
MAP_BIN = "j:/Language-U/qwen_vocab_cuneiform.bin"

# ─── Cuneiform-U Predictor & Range Coder ──────────────────────────────────────────
# Port of C abstractions from cuneiform_u_v3.h to Python

class PythonRadicalPredictor:
    def __init__(self, alpha=1, weight=128):
        self.alpha = alpha
        self.weight = weight
        # transition tables: key -> {symbol: count}
        self.trans_rc = {}
        self.trans_rf = {}
        self.trans_ra = {}
        
        self.prev_rc = 0
        self.prev_rf = 0
        self.prev_ra = 0

    def observe(self, rc, rf, ra):
        # 1. Update R_C
        key_rc = self.prev_rc
        if key_rc not in self.trans_rc:
            self.trans_rc[key_rc] = {}
        self.trans_rc[key_rc][rc] = self.trans_rc[key_rc].get(rc, 0) + self.weight

        # 2. Update R_F
        key_rf = (rc << 8) | self.prev_rf
        if key_rf not in self.trans_rf:
            self.trans_rf[key_rf] = {}
        self.trans_rf[key_rf][rf] = self.trans_rf[key_rf].get(rf, 0) + self.weight

        # 3. Update R_A
        key_ra = (rc << 16) | (rf << 8) | self.prev_ra
        if key_ra not in self.trans_ra:
            self.trans_ra[key_ra] = {}
        self.trans_ra[key_ra][ra] = self.trans_ra[key_ra].get(ra, 0) + self.weight

        self.prev_rc = rc
        self.prev_rf = rf
        self.prev_ra = ra

    def get_cum_freqs_rc(self, prev_rc):
        freqs = [self.alpha] * 256
        if prev_rc in self.trans_rc:
            for sym, count in self.trans_rc[prev_rc].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_rf(self, curr_rc, prev_rf):
        freqs = [self.alpha] * 256
        key = (curr_rc << 8) | prev_rf
        if key in self.trans_rf:
            for sym, count in self.trans_rf[key].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_ra(self, curr_rc, curr_rf, prev_ra):
        freqs = [self.alpha] * 256
        key = (curr_rc << 16) | (curr_rf << 8) | prev_ra
        if key in self.trans_ra:
            for sym, count in self.trans_ra[key].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs


class BitWriter:
    def __init__(self):
        self.buffer = []
        self.current_byte = 0
        self.bit_count = 0

    def write_bit(self, bit):
        self.current_byte = (self.current_byte << 1) | (bit & 1)
        self.bit_count += 1
        if self.bit_count % 8 == 0:
            self.buffer.append(self.current_byte)
            self.current_byte = 0

    def write_bit_helper(self, underflow_bits, bit):
        self.write_bit(bit)
        for _ in range(underflow_bits[0]):
            self.write_bit(1 - bit)
        underflow_bits[0] = 0

    def flush(self):
        if self.bit_count % 8 != 0:
            padding_bits = 8 - (self.bit_count % 8)
            self.current_byte <<= padding_bits
            self.buffer.append(self.current_byte)
            self.current_byte = 0
            self.bit_count += padding_bits
        return bytes(self.buffer)


class BitReader:
    def __init__(self, data):
        self.data = data
        self.byte_index = 0
        self.bit_index = 0
        self.total_bits = len(data) * 8

    def read_bit(self):
        if self.byte_index >= len(self.data):
            return 0
        bit = (self.data[self.byte_index] >> (7 - self.bit_index)) & 1
        self.bit_index += 1
        if self.bit_index == 8:
            self.bit_index = 0
            self.byte_index += 1
        return bit


def range_encode_radicals(radicals, alpha=1, weight=128):
    pred = PythonRadicalPredictor(alpha, weight)
    w = BitWriter()

    low = 0
    high = 0xFFFFFFFF
    underflow_bits = [0]

    for rc, rf, ra in radicals:
        symbols = [rc, rf, ra]
        prev_rc = pred.prev_rc
        prev_rf = pred.prev_rf
        prev_ra = pred.prev_ra

        for step in range(3):
            if step == 0:
                cum_freqs = pred.get_cum_freqs_rc(prev_rc)
            elif step == 1:
                cum_freqs = pred.get_cum_freqs_rf(symbols[0], prev_rf)
            else:
                cum_freqs = pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra)

            sym = symbols[step]
            total = cum_freqs[256]
            cum_low = cum_freqs[sym]
            cum_high = cum_freqs[sym + 1]

            range_width = high - low + 1
            high = low + (range_width * cum_high) // total - 1
            low = low + (range_width * cum_low) // total

            # Renormalize
            while True:
                if high < 0x80000000:
                    w.write_bit_helper(underflow_bits, 0)
                    low = (low << 1) & 0xFFFFFFFF
                    high = ((high << 1) | 1) & 0xFFFFFFFF
                elif low >= 0x80000000:
                    w.write_bit_helper(underflow_bits, 1)
                    low = ((low - 0x80000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x80000000) << 1) | 1) & 0xFFFFFFFF
                elif low >= 0x40000000 and high < 0xC0000000:
                    underflow_bits[0] += 1
                    low = ((low - 0x40000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x40000000) << 1) | 1) & 0xFFFFFFFF
                else:
                    break

        pred.observe(rc, rf, ra)

    underflow_bits[0] += 1
    if low < 0x40000000:
        w.write_bit_helper(underflow_bits, 0)
    else:
        w.write_bit_helper(underflow_bits, 1)

    return w.flush()


def range_decode_radicals(encoded_bytes, num_concepts, alpha=1, weight=128):
    pred = PythonRadicalPredictor(alpha, weight)
    r = BitReader(encoded_bytes)

    value = 0
    for _ in range(32):
        value = (value << 1) | r.read_bit()

    low = 0
    high = 0xFFFFFFFF
    decoded_radicals = []

    for c in range(num_concepts):
        prev_rc = pred.prev_rc
        prev_rf = pred.prev_rf
        prev_ra = pred.prev_ra

        symbols = [0, 0, 0]

        for step in range(3):
            if step == 0:
                cum_freqs = pred.get_cum_freqs_rc(prev_rc)
            elif step == 1:
                cum_freqs = pred.get_cum_freqs_rf(symbols[0], prev_rf)
            else:
                cum_freqs = pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra)

            total = cum_freqs[256]
            range_width = high - low + 1

            scaled_val = (((value - low) + 1) * total - 1) // range_width

            # Binary search for symbol
            sym = 0
            l = 0
            rr = 255
            while l <= rr:
                mid = (l + rr) // 2
                if cum_freqs[mid] <= scaled_val < cum_freqs[mid + 1]:
                    sym = mid
                    break
                elif scaled_val >= cum_freqs[mid + 1]:
                    l = mid + 1
                else:
                    rr = mid - 1

            symbols[step] = sym

            cum_low = cum_freqs[sym]
            cum_high = cum_freqs[sym + 1]

            high = low + (range_width * cum_high) // total - 1
            low = low + (range_width * cum_low) // total

            # Renormalize
            while True:
                if high < 0x80000000:
                    low = (low << 1) & 0xFFFFFFFF
                    high = ((high << 1) | 1) & 0xFFFFFFFF
                    value = ((value << 1) | r.read_bit()) & 0xFFFFFFFF
                elif low >= 0x80000000:
                    low = ((low - 0x80000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x80000000) << 1) | 1) & 0xFFFFFFFF
                    value = (((value - 0x80000000) << 1) | r.read_bit()) & 0xFFFFFFFF
                elif low >= 0x40000000 and high < 0xC0000000:
                    low = ((low - 0x40000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x40000000) << 1) | 1) & 0xFFFFFFFF
                    value = (((value - 0x40000000) << 1) | r.read_bit()) & 0xFFFFFFFF
                else:
                    break

        decoded_radicals.append((symbols[0], symbols[1], symbols[2]))
        pred.observe(symbols[0], symbols[1], symbols[2])

    return decoded_radicals

# ─── Verification & Benchmarking Harness ──────────────────────────────────────────

TEST_PASSAGES = [
    # 1. Hardware network reset sequence
    "GPIO pin SX1302 reset lines on Raspberry Pi 4 pin 25. reset_lgw.sh resets concentrator.",
    # 2. Mathematical information theory
    "SVD projection and DCT spectral coordinates compress weights. Shannon Orthogonality equation.",
    # 3. Conversational dialogue context
    "I am the assistant speaking for TheAiCollective. Zymatica is the framework architect."
]

def load_vocab_map(path):
    with open(path, "rb") as f:
        data = f.read()
    vocab_size = len(data) // 3
    vocab_map = {}
    for i in range(vocab_size):
        vocab_map[i] = (data[i*3], data[i*3+1], data[i*3+2])
    return vocab_map

def main():
    if not os.path.exists(MAP_BIN):
        print(f"Error: map file {MAP_BIN} does not exist. Run ufo_cuneiform_vocab_mapper.py first.")
        return

    print("Loading vocab map database...")
    vocab_map = load_vocab_map(MAP_BIN)

    print(f"Loading Qwen tokenizer from: {TOKENIZER_DIR}")
    tokenizer = AutoTokenizer.from_pretrained(TOKENIZER_DIR, trust_remote_code=True)

    print("\n" + "="*80)
    print("  CUNEIFORM-U SEMANTIC RANGE CODER COMPRESSION BENCHMARKS")
    print("="*80)

    for idx, text in enumerate(TEST_PASSAGES, 1):
        print(f"\n--- Test Passage {idx}: \"{text[:60]}...\" ---")
        
        # 1. Tokenize text
        token_ids = tokenizer.encode(text)
        num_tokens = len(token_ids)
        print(f"    Raw tokens count: {num_tokens}")

        # 2. Translate token IDs to 3-byte radicals
        radicals = [vocab_map[tid] for tid in token_ids]

        # 3. Compress using Cuneiform-U range coder
        t0 = np.round(1000 * np.round(0, 4)) # dummy placeholder
        compressed_bytes = range_encode_radicals(radicals, alpha=1, weight=128)
        compressed_len = len(compressed_bytes)

        # 4. Lossless Decompress Verification
        decoded_radicals = range_decode_radicals(compressed_bytes, num_tokens, alpha=1, weight=128)
        assert decoded_radicals == radicals, f"    [FAIL] ERROR: Lossless validation failed at index {idx}!"
        print("    [OK] Lossless reconstruction validation PASSED.")

        # 5. Baselines comparison
        # Baseline A: Raw ASCII text bytes
        ascii_bytes_len = len(text.encode('utf-8'))
        
        # Baseline B: Raw Token IDs as 32-bit integers (4 bytes per token)
        raw_ids_bytes = num_tokens * 4
        
        # Baseline C: Token IDs compressed via standard zlib deflate (Level 9)
        token_bytes_flat = bytearray()
        for tid in token_ids:
            token_bytes_flat.extend(struct.pack(">I", tid))
        zlib_compressed = zlib.compress(bytes(token_bytes_flat), level=9)
        zlib_len = len(zlib_compressed)

        print("\n    Compression Size Metrics:")
        print(f"      - Raw ASCII Text:                {ascii_bytes_len} bytes")
        print(f"      - Raw Token IDs (32-bit):        {raw_ids_bytes} bytes")
        print(f"      - Token IDs + Zlib (deflate):    {zlib_len} bytes")
        print(f"      - **Cuneiform-U Range Coding**:   {compressed_len} bytes")

        # Ratios
        vs_ascii = ascii_bytes_len / compressed_len
        vs_zlib = zlib_len / compressed_len

        print(f"\n    [+] Cuneiform-U vs ASCII:         {vs_ascii:.2f}x compression gain")
        print(f"    [+] Cuneiform-U vs Zlib (tokens): {vs_zlib:.2f}x compression gain")

if __name__ == "__main__":
    main()

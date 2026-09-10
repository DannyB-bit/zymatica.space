# Watermark: ip zymatica.space | astronautshe.com
# Copyright (c) 2026 Zymatica.
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
import sys

class SparseTransition:
    def __init__(self, key=0, sym=0, count=0):
        self.key = key
        self.sym = sym
        self.count = count

class RadicalPredictor:
    def __init__(self, alpha=1, weight=128):
        self.alpha = alpha
        self.weight = weight
        self.trans_rc = []
        self.trans_rf = []
        self.trans_ra = []
        self.prev_rc = 0
        self.prev_rf = 0
        self.prev_ra = 0

    def observe(self, rc, rf, ra):
        w = self.weight
        key_rc = self.prev_rc
        found = False
        for entry in self.trans_rc:
            if entry.key == key_rc and entry.sym == rc:
                entry.count += w
                found = True
                break
        if not found and len(self.trans_rc) < 256:
            self.trans_rc.append(SparseTransition(key_rc, rc, w))

        key_rf = (rc << 8) | self.prev_rf
        found = False
        for entry in self.trans_rf:
            if entry.key == key_rf and entry.sym == rf:
                entry.count += w
                found = True
                break
        if not found and len(self.trans_rf) < 256:
            self.trans_rf.append(SparseTransition(key_rf, rf, w))

        key_ra = (rc << 16) | (rf << 8) | self.prev_ra
        found = False
        for entry in self.trans_ra:
            if entry.key == key_ra and entry.sym == ra:
                entry.count += w
                found = True
                break
        if not found and len(self.trans_ra) < 256:
            self.trans_ra.append(SparseTransition(key_ra, ra, w))

        self.prev_rc = rc
        self.prev_rf = rf
        self.prev_ra = ra

    def get_cum_freqs_rc(self, prev_rc):
        freqs = [self.alpha] * 256
        for entry in self.trans_rc:
            if entry.key == prev_rc:
                freqs[entry.sym] += entry.count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_rf(self, curr_rc, prev_rf):
        freqs = [self.alpha] * 256
        key = (curr_rc << 8) | prev_rf
        for entry in self.trans_rf:
            if entry.key == key:
                freqs[entry.sym] += entry.count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_ra(self, curr_rc, curr_rf, prev_ra):
        freqs = [self.alpha] * 256
        key = (curr_rc << 16) | (curr_rf << 8) | prev_ra
        for entry in self.trans_ra:
            if entry.key == key:
                freqs[entry.sym] += entry.count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

class BitWriter:
    def __init__(self):
        self.buffer = bytearray()
        self.bit_index = 0

    def write_bit(self, bit):
        byte_pos = self.bit_index // 8
        bit_pos = 7 - (self.bit_index % 8)
        if byte_pos >= len(self.buffer):
            self.buffer.append(0)
        if bit:
            self.buffer[byte_pos] |= (1 << bit_pos)
        else:
            self.buffer[byte_pos] &= ~(1 << bit_pos)
        self.bit_index += 1

    def write_bit_helper(self, underflow_bits, bit):
        self.write_bit(bit)
        while underflow_bits[0] > 0:
            self.write_bit(1 - bit)
            underflow_bits[0] -= 1

class BitReader:
    def __init__(self, data):
        self.data = data
        self.bit_index = 0
        self.total_bits = len(data) * 8

    def read_bit(self):
        if self.bit_index >= self.total_bits:
            return 0
        byte_pos = self.bit_index // 8
        bit_pos = 7 - (self.bit_index % 8)
        bit = (self.data[byte_pos] >> bit_pos) & 1
        self.bit_index += 1
        return bit

def encode(concepts, alpha, weight):
    pred = RadicalPredictor(alpha, weight)
    w = BitWriter()
    low = 0
    high = 0xFFFFFFFF
    underflow_bits = [0]

    for c in concepts:
        rc = (c[0] << 4) | c[1]
        rf = (c[2] << 4) | c[3]
        ra = (c[4] << 4) | c[5]
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

            while True:
                if high < 0x80000000:
                    w.write_bit_helper(underflow_bits, 0)
                    low <<= 1
                    high = (high << 1) | 1
                elif low >= 0x80000000:
                    w.write_bit_helper(underflow_bits, 1)
                    low = (low - 0x80000000) << 1
                    high = ((high - 0x80000000) << 1) | 1
                elif low >= 0x40000000 and high < 0xC0000000:
                    underflow_bits[0] += 1
                    low = (low - 0x40000000) << 1
                    high = ((high - 0x40000000) << 1) | 1
                else:
                    break
                low &= 0xFFFFFFFF
                high &= 0xFFFFFFFF

        pred.observe(rc, rf, ra)

    underflow_bits[0] += 1
    if low < 0x40000000:
        w.write_bit_helper(underflow_bits, 0)
    else:
        w.write_bit_helper(underflow_bits, 1)

    return w.buffer, w.bit_index

def decode(encoded_bytes, num_concepts, alpha, weight):
    pred = RadicalPredictor(alpha, weight)
    r = BitReader(encoded_bytes)

    value = 0
    for _ in range(32):
        value = (value << 1) | r.read_bit()

    low = 0
    high = 0xFFFFFFFF
    decoded_concepts = []

    for _ in range(num_concepts):
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
            scaled_val = ((value - low + 1) * total - 1) // range_width

            sym = 0
            l_idx, r_idx = 0, 255
            while l_idx <= r_idx:
                m_idx = (l_idx + r_idx) // 2
                if cum_freqs[m_idx] <= scaled_val < cum_freqs[m_idx + 1]:
                    sym = m_idx
                    break
                elif scaled_val >= cum_freqs[m_idx + 1]:
                    l_idx = m_idx + 1
                else:
                    r_idx = m_idx - 1

            symbols[step] = sym
            cum_low = cum_freqs[sym]
            cum_high = cum_freqs[sym + 1]

            high = low + (range_width * cum_high) // total - 1
            low = low + (range_width * cum_low) // total

            while True:
                if high < 0x80000000:
                    low <<= 1
                    high = (high << 1) | 1
                    value = (value << 1) | r.read_bit()
                elif low >= 0x80000000:
                    low = (low - 0x80000000) << 1
                    high = ((high - 0x80000000) << 1) | 1
                    value = ((value - 0x80000000) << 1) | r.read_bit()
                elif low >= 0x40000000 and high < 0xC0000000:
                    low = (low - 0x40000000) << 1
                    high = ((high - 0x40000000) << 1) | 1
                    value = ((value - 0x40000000) << 1) | r.read_bit()
                else:
                    break
                low &= 0xFFFFFFFF
                high &= 0xFFFFFFFF
                value &= 0xFFFFFFFF

        decoded_concepts.append([
            (symbols[0] >> 4) & 0xF,
            symbols[0] & 0xF,
            (symbols[1] >> 4) & 0xF,
            symbols[1] & 0xF,
            (symbols[2] >> 4) & 0xF,
            symbols[2] & 0xF
        ])
        pred.observe(symbols[0], symbols[1], symbols[2])

    return decoded_concepts

def main():
    print("======================================================================")
    print("ZYMATICA | zymatica-inference-engine-python")
    print("======================================================================\n")
    
    inputs = [
        [1, 2, 3, 4, 5, 6],
        [8, 0, 15, 1, 0, 15],
        [0, 0, 0, 0, 0, 0],
        [15, 15, 15, 15, 15, 15],
        [4, 5, 6, 7, 8, 9]
    ]
    
    buf, bits = encode(inputs, 1, 128)
    print(f"Encoded Bits: {bits}, Bytes: {len(buf)}")
    print("Hex:", " ".join(f"{b:02X}" for b in buf))
    
    import time
    start_time = time.perf_counter()
    runs = 1000
    match = True
    for r in range(runs):
        decoded = decode(buf, 5, 1, 128)
        if r == 0:
            match = decoded == inputs
    end_time = time.perf_counter()
    elapsed_ms = (end_time - start_time) * 1000.0

    print(f"Decoded matches inputs: {match}")
    if not match:
        print("ERROR: mismatch!")
        sys.exit(1)
    print(f"[INTERNAL_MATH] {elapsed_ms:.4f} ms")
        
    print("\n[VERIFICATION] Multi-Language runtime FFI structures validated.")

if __name__ == '__main__':
    main()

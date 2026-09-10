# Gemma-4-Language-U Verification and Mathematical Execution Proof Harness
# Watermark: ip zymatica.space | astronautshe.com | Gemma-4-Language-U

import sys
import hashlib
import struct
import numpy as np
from scipy.fft import dct, idct

# --- Subroutine 1: SVD / DCT Vector Spectral Projections ---
def test_svd_dct():
    print("[1] Running SVD & DCT Spectral Decomposition Proof...")
    M, N = 64, 64
    RANK = 4
    K_COEF = 8  # spectral coefficient count

    # Generate smooth low-rank target weight matrix W
    t = np.linspace(0, 2 * np.pi, M)
    u1, v1 = np.sin(t), np.cos(t)
    u2, v2 = np.sin(2 * t), np.cos(2 * t)
    W_true = np.outer(u1, v1) + np.outer(u2, v2)
    
    # Run SVD
    U, S, Vh = np.linalg.svd(W_true, full_matrices=False)
    U_r = U[:, :RANK] * np.sqrt(S[:RANK])
    V_r = Vh[:RANK, :].T * np.sqrt(S[:RANK])
    
    # Compress singular vectors via DCT-II spectral truncation
    U_rec = np.zeros_like(U_r)
    V_rec = np.zeros_like(V_r)
    
    for col in range(RANK):
        # Truncate U vector
        u_dct = dct(U_r[:, col], norm='ortho')
        u_dct_trunc = np.zeros_like(u_dct)
        u_dct_trunc[:K_COEF] = u_dct[:K_COEF]
        U_rec[:, col] = idct(u_dct_trunc, norm='ortho')
        
        # Truncate V vector
        v_dct = dct(V_r[:, col], norm='ortho')
        v_dct_trunc = np.zeros_like(v_dct)
        v_dct_trunc[:K_COEF] = v_dct[:K_COEF]
        V_rec[:, col] = idct(v_dct_trunc, norm='ortho')
        
    W_rec = U_rec @ V_rec.T
    
    # Metrics
    mse = np.mean((W_true - W_rec) ** 2)
    cosine_sim = np.dot(W_true.flatten(), W_rec.flatten()) / (np.linalg.norm(W_true) * np.linalg.norm(W_rec) + 1e-9)
    compression_ratio = W_true.nbytes / (2 * (RANK * K_COEF) * 4)
    
    print(f"    - Target Matrix Shape:  {M}x{N}")
    print(f"    - SVD Rank:             {RANK}")
    print(f"    - Reconstructed MSE:    {mse:.6f}")
    print(f"    - Cosine Similarity:    {cosine_sim * 100:.2f}%")
    print(f"    - Compression Ratio:    {compression_ratio:.2f}x")
    assert cosine_sim > 0.95, "Fidelity is too low for SVD-DCT spectral projections."
    print("    [+] SVD/DCT spectral projection pipeline verified.\n")


# --- Subroutine 2: Chirp Packetization & XOR-FEC Recovery ---
SYNC_MARKER = 0xBB
PKT_SIZE = 255
TRANSPORT_HDR = 3
DATA_PER_PKT = PKT_SIZE - TRANSPORT_HDR  # 252 Bytes

def xor_fec_parity(data_packets):
    parity = bytearray(DATA_PER_PKT)
    for pkt in data_packets:
        data_part = pkt[TRANSPORT_HDR:]
        for idx in range(min(len(data_part), DATA_PER_PKT)):
            parity[idx] ^= data_part[idx]
    return bytes(parity)

def pack_payload(payload_bytes, num_data_packets):
    total_capacity = num_data_packets * DATA_PER_PKT
    if len(payload_bytes) < total_capacity:
        payload_bytes = payload_bytes.ljust(total_capacity, b'\x00')
    else:
        payload_bytes = payload_bytes[:total_capacity]
        
    data_packets = []
    total_packets = num_data_packets + 1
    for idx in range(num_data_packets):
        chunk = payload_bytes[idx * DATA_PER_PKT : (idx + 1) * DATA_PER_PKT]
        header = bytes([SYNC_MARKER, idx, total_packets])
        data_packets.append(header + chunk)
        
    parity_data = xor_fec_parity(data_packets)
    parity_header = bytes([SYNC_MARKER, num_data_packets, total_packets])
    parity_packet = parity_header + parity_data
    return data_packets + [parity_packet]

def test_xor_fec():
    print("[2] Running XOR-FEC Chirp Packetization Proof...")
    raw_payload = b"ip zymatica.space | " * 50  # 1000 Bytes
    payload_hash = hashlib.sha256(raw_payload).hexdigest()
    
    # Pack payload
    num_data_pkts = 4
    packets = pack_payload(raw_payload, num_data_pkts)
    
    # Drop Packet 2 during simulated transit
    dropped_index = 2
    print(f"    - Simulating transmission channel drop: Packet index {dropped_index} lost.")
    received_packets = [pkt for idx, pkt in enumerate(packets) if idx != dropped_index]
    
    # Receiver XOR-FEC Reconstruction
    received_indices = {pkt[1] for pkt in received_packets}
    total_packets = received_packets[0][2]
    missing_index = next(idx for idx in range(total_packets) if idx not in received_indices)
    
    recovered_data = bytearray(DATA_PER_PKT)
    for pkt in received_packets:
        data_part = pkt[TRANSPORT_HDR:]
        for idx in range(DATA_PER_PKT):
            recovered_data[idx] ^= data_part[idx]
            
    recovered_packet = bytes([SYNC_MARKER, missing_index, total_packets]) + bytes(recovered_data)
    
    # Reassemble and sort
    reassembled_packets = list(received_packets) + [recovered_packet]
    reassembled_packets.sort(key=lambda x: x[1])
    
    reassembled_payload = bytearray()
    for idx in range(num_data_pkts):
        reassembled_payload.extend(reassembled_packets[idx][TRANSPORT_HDR:])
        
    reassembled_payload = bytes(reassembled_payload[:len(raw_payload)])
    reassembled_hash = hashlib.sha256(reassembled_payload).hexdigest()
    
    print(f"    - Original Payload SHA-256:    {payload_hash}")
    print(f"    - Reassembled Payload SHA-256: {reassembled_hash}")
    assert payload_hash == reassembled_hash, "Checksum validation failed! Data corrupted."
    print("    [+] Lossless XOR-FEC reconstruction validated. No data loss.\n")


# --- Subroutine 3: 32-Bit Range Coder on Cuneiform-U 6D Coordinates ---
class RadicalPredictor:
    def __init__(self, alpha=1):
        self.alpha = alpha
        self.trans_rc = [alpha] * 256
        self.prev_rc = 0

    def observe(self, rc):
        self.trans_rc[rc] += 128
        self.prev_rc = rc

    def get_cum_freqs(self):
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + self.trans_rc[i]
        return cum_freqs

def test_range_coder():
    print("[3] Running 32-Bit Range Coder Verification Loop...")
    inputs = [12, 45, 99, 150, 230]  # sample coordinate byte values
    
    # --- Range Encode ---
    pred = RadicalPredictor()
    low = 0
    high = 0xFFFFFFFF
    underflow = 0
    bit_buffer = []
    
    def write_bit(bit):
        bit_buffer.append(bit)
        
    def write_bit_helper(bit, count):
        write_bit(bit)
        for _ in range(count):
            write_bit(1 - bit)

    for val in inputs:
        cum_freqs = pred.get_cum_freqs()
        total = cum_freqs[256]
        cum_low = cum_freqs[val]
        cum_high = cum_freqs[val + 1]
        
        range_width = high - low + 1
        high = low + (range_width * cum_high) // total - 1
        low = low + (range_width * cum_low) // total
        
        while True:
            if high < 0x80000000:
                write_bit_helper(0, underflow)
                underflow = 0
                low <<= 1
                high = (high << 1) | 1
            elif low >= 0x80000000:
                write_bit_helper(1, underflow)
                underflow = 0
                low = (low - 0x80000000) << 1
                high = ((high - 0x80000000) << 1) | 1
            elif low >= 0x40000000 and high < 0xC0000000:
                underflow += 1
                low = (low - 0x40000000) << 1
                high = ((high - 0x40000000) << 1) | 1
            else:
                break
            low &= 0xFFFFFFFF
            high &= 0xFFFFFFFF
        pred.observe(val)
        
    underflow += 1
    if low < 0x40000000:
        write_bit_helper(0, underflow)
    else:
        write_bit_helper(1, underflow)
        
    print(f"    - Input Coordinate Symbols: {inputs}")
    print(f"    - Encoded Bit Buffer Length: {len(bit_buffer)} bits")
    
    # --- Range Decode ---
    pred_dec = RadicalPredictor()
    bit_pos = 0
    
    def read_bit():
        nonlocal bit_pos
        if bit_pos >= len(bit_buffer):
            return 0
        bit = bit_buffer[bit_pos]
        bit_pos += 1
        return bit
        
    value = 0
    for _ in range(32):
        value = (value << 1) | read_bit()
        
    low = 0
    high = 0xFFFFFFFF
    decoded_vals = []
    
    for _ in range(len(inputs)):
        cum_freqs = pred_dec.get_cum_freqs()
        total = cum_freqs[256]
        range_width = high - low + 1
        scaled_val = ((value - low + 1) * total - 1) // range_width
        
        # Binary search search for symbol
        l_idx, r_idx = 0, 255
        sym = 0
        while l_idx <= r_idx:
            m_idx = (l_idx + r_idx) // 2
            if cum_freqs[m_idx] <= scaled_val < cum_freqs[m_idx + 1]:
                sym = m_idx
                break
            elif scaled_val >= cum_freqs[m_idx + 1]:
                l_idx = m_idx + 1
            else:
                r_idx = m_idx - 1
                
        decoded_vals.append(sym)
        cum_low = cum_freqs[sym]
        cum_high = cum_freqs[sym + 1]
        
        high = low + (range_width * cum_high) // total - 1
        low = low + (range_width * cum_low) // total
        
        while True:
            if high < 0x80000000:
                low <<= 1
                high = (high << 1) | 1
                value = (value << 1) | read_bit()
            elif low >= 0x80000000:
                low = (low - 0x80000000) << 1
                high = ((high - 0x80000000) << 1) | 1
                value = ((value - 0x80000000) << 1) | read_bit()
            elif low >= 0x40000000 and high < 0xC0000000:
                low = (low - 0x40000000) << 1
                high = ((high - 0x40000000) << 1) | 1
                value = ((value - 0x40000000) << 1) | read_bit()
            else:
                break
            low &= 0xFFFFFFFF
            high &= 0xFFFFFFFF
            value &= 0xFFFFFFFF
        pred_dec.observe(sym)
        
    print(f"    - Decoded Coordinate Symbols: {decoded_vals}")
    assert inputs == decoded_vals, "Decoded values do not match inputs! Lossless range coding failed."
    print("    [+] Lossless range coding execution validated.\n")


# --- Main Runner ---
def main():
    print("======================================================================")
    print("  GEMMA-4-LANGUAGE-U | SYSTEM EXECUTION & MATHEMATICAL PROOF HARNESS")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("======================================================================\n")
    
    test_svd_dct()
    test_xor_fec()
    test_range_coder()
    
    print("[SUCCESS] All system execution and mathematical proofs validated successfully!")

if __name__ == "__main__":
    main()

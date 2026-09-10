#!/usr/bin/env python3
# Watermark: ip zymatica.space | astronautshe.com
# RAK Miner A - Transmitter Script

import os
import sys
import time
import socket
import argparse
import hashlib

# ==============================================================================
# LLD-AC Range Coding Logic (Invention 07) with Native C Acceleration (Yang)
# ==============================================================================
import ctypes
import os

class Concept6D(ctypes.Structure):
    _fields_ = [
        ("domain", ctypes.c_uint8),
        ("subdomain", ctypes.c_uint8),
        ("operation", ctypes.c_uint8),
        ("modality", ctypes.c_uint8),
        ("depth", ctypes.c_uint8),
        ("polarity", ctypes.c_uint8),
    ]

_cuneiform_lib = None
for lib_name in ["cuneiform_u_v3.dll", "libcuneiform_u_v3.so", "libcuneiform_u_v3.dylib"]:
    try:
        script_dir = os.path.dirname(os.path.abspath(__file__))
        path = os.path.join(script_dir, lib_name)
        if os.path.exists(path):
            _cuneiform_lib = ctypes.CDLL(path)
            break
    except Exception:
        pass

if _cuneiform_lib:
    try:
        _cuneiform_lib.cuneiform_u_v3_encode_dll.argtypes = [
            ctypes.POINTER(Concept6D), ctypes.c_uint32,
            ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint32,
            ctypes.c_uint32, ctypes.c_uint32
        ]
        _cuneiform_lib.cuneiform_u_v3_encode_dll.restype = ctypes.c_int
        print("  [+] Cuneiform-U v3 native acceleration loaded successfully! (Yang Mode)")
    except Exception as e:
        print(f"  [-] Failed to bind Cuneiform-U DLL functions: {e}. Falling back to Python (Yin Mode).")
        _cuneiform_lib = None
else:
    print("  [*] Native Cuneiform-U shared library not found. Running in Python interpreter fallback (Yin Mode).")

class PythonRadicalPredictor:
    def __init__(self, alpha=1, weight=128):
        self.alpha = alpha
        self.weight = weight
        self.trans_rc = {}
        self.trans_rf = {}
        self.trans_ra = {}
        self.prev_rc = 0
        self.prev_rf = 0
        self.prev_ra = 0

    def observe(self, rc, rf, ra):
        key_rc = self.prev_rc
        if key_rc not in self.trans_rc:
            self.trans_rc[key_rc] = {}
        self.trans_rc[key_rc][rc] = self.trans_rc[key_rc].get(rc, 0) + self.weight

        key_rf = (rc << 8) | self.prev_rf
        if key_rf not in self.trans_rf:
            self.trans_rf[key_rf] = {}
        self.trans_rf[key_rf][rf] = self.trans_rf[key_rf].get(rf, 0) + self.weight

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

def range_encode_radicals_py(radicals, alpha=1, weight=128):
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

def range_encode_radicals(radicals, alpha=1, weight=128):
    if _cuneiform_lib:
        try:
            c_concepts = (Concept6D * len(radicals))()
            for i, (rc, rf, ra) in enumerate(radicals):
                c_concepts[i].domain = (rc >> 4) & 0xF
                c_concepts[i].subdomain = rc & 0xF
                c_concepts[i].operation = (rf >> 4) & 0xF
                c_concepts[i].modality = rf & 0xF
                c_concepts[i].depth = (ra >> 4) & 0xF
                c_concepts[i].polarity = ra & 0xF

            max_buf_len = max(256, len(radicals) * 4)
            out_buf = (ctypes.c_uint8 * max_buf_len)()
            bits = _cuneiform_lib.cuneiform_u_v3_encode_dll(
                c_concepts, len(radicals), out_buf, max_buf_len, alpha, weight
            )
            if bits >= 0:
                bytes_len = (bits + 7) // 8
                return bytes(out_buf[:bytes_len])
        except Exception as e:
            print(f"  [-] Native encoding exception: {e}. Falling back to Python.")

    return range_encode_radicals_py(radicals, alpha, weight)

# ==============================================================================
# XOR-FEC Chirp Packetization Logic (Invention 05)
# ==============================================================================
SYNC_MARKER    = 0xBB
PKT_SIZE       = 255
TRANSPORT_HDR  = 3
DATA_PER_PKT   = PKT_SIZE - TRANSPORT_HDR  # 252 Bytes

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
    elif len(payload_bytes) > total_capacity:
        payload_bytes = payload_bytes[:total_capacity]
        
    data_packets = []
    total_packets = num_data_packets + 1
    
    for idx in range(num_data_packets):
        chunk = payload_bytes[idx * DATA_PER_PKT : (idx + 1) * DATA_PER_PKT]
        header = bytes([SYNC_MARKER, idx, total_packets])
        data_packets.append(header + chunk)
        
    # Generate XOR-parity packet
    parity_data = xor_fec_parity(data_packets)
    parity_header = bytes([SYNC_MARKER, num_data_packets, total_packets])
    parity_packet = parity_header + parity_data
    
    return data_packets + [parity_packet]

# ==============================================================================
# Main Transmitter Execution
# ==============================================================================
def main():
    parser = argparse.ArgumentParser(description="Zymatica LoRa Protocol - RAK Miner A (Transmitter)")
    parser.add_argument("--mode", choices=["udp", "serial"], default="udp", help="Connection interface: udp (local network) or serial (USB/UART)")
    parser.add_argument("--ip", default="192.168.1.51", help="Receiver's IP address (needed for UDP mode)")
    parser.add_argument("--port", type=int, default=50001, help="Receiver's port (needed for UDP mode)")
    parser.add_argument("--device", default="/dev/ttyUSB0", help="Serial port interface (needed for serial mode)")
    parser.add_argument("--baud", type=int, default=115200, help="Serial baud rate")
    args = parser.parse_args()

    print("=" * 80)
    print("  ZYMATICA | Language-U RAK-Miner A (Transmitter Node)")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    # 1. Raw Intent / Coordinates Preparation
    # Equivalent to a sequence of 6D coordinate concepts mapped via Cuneiform-U
    input_radicals = [
        (0x12, 0x01, 0x80),
        (0x12, 0x01, 0x80),
        (0x11, 0x00, 0xA0),
        (0x11, 0x00, 0xA0),
        (0x11, 0x00, 0xA0),
        (0x21, 0x01, 0xA0),
        (0x22, 0x02, 0xF0),
        (0x22, 0x02, 0xF0),
        (0x05, 0x0B, 0xC0), # Coordinate expansion elements
        (0x05, 0x0B, 0xC0)
    ]
    
    print(f"\n[1] Mapped Intent Coordinates (Cuneiform-U):")
    for idx, rad in enumerate(input_radicals):
        print(f"  - Concept {idx+1:02d}: RC=0x{rad[0]:02X}, RF=0x{rad[1]:02X}, RA=0x{rad[2]:02X}")

    # 2. Compress using Range Encoding
    print(f"\n[2] Applying LLD-AC Range Compression...")
    compressed_bytes = range_encode_radicals(input_radicals, alpha=1, weight=128)
    print(f"  - Original Size: {len(input_radicals)*3} bytes")
    print(f"  - Compressed Size: {len(compressed_bytes)} bytes")
    print(f"  - Compression Ratio: {len(input_radicals)*3 / len(compressed_bytes):.2f}x")

    # Save payload checksum to broadcast for validation verification
    payload_hash = hashlib.sha256(compressed_bytes).hexdigest()
    print(f"  - Payload Checksum (SHA-256): {payload_hash}")

    # 3. Apply XOR-FEC Chirp Packetization
    print(f"\n[3] Packetizing into XOR-FEC Frames (255-Byte Blocks)...")
    num_data_packets = 1 # Small payload easily fits in 1 packet + 1 parity packet
    packets = pack_payload(compressed_bytes, num_data_packets)
    print(f"  - Created {len(packets)} total frames (including 1 FEC Parity frame).")

    # 4. Transmit Payload
    if args.mode == "udp":
        print(f"\n[4] Initializing UDP Transmission to {args.ip}:{args.port}...")
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        
        # We also broadcast the verification metadata header first (hash, count, compressed_len)
        meta_payload = f"META:{len(input_radicals)}:{payload_hash}:{len(compressed_bytes)}".encode('utf-8')
        sock.sendto(meta_payload, (args.ip, args.port))
        time.sleep(0.5)

        for i, pkt in enumerate(packets):
            print(f"  -> Sending Frame {i}/{len(packets)-1} ({len(pkt)} bytes)...")
            sock.sendto(pkt, (args.ip, args.port))
            time.sleep(0.5)
            
        print("\n[SUCCESS] Transmission sequence completed.")
        
    elif args.mode == "serial":
        print(f"\n[4] Initializing Serial LoRa P2P Transmission on {args.device}...")
        try:
            import serial
        except ImportError:
            print("  [-] Error: 'pyserial' library not found. Install it using 'pip install pyserial'")
            sys.exit(1)
            
        try:
            ser = serial.Serial(args.device, args.baud, timeout=2)
            print(f"  - Port opened successfully.")
            
            # Setup RAK LoRa P2P parameters
            # AT Commands configured for P2P mode: frequency, bandwidth, spreading factor, coding rate
            # RAK AT-command set: at+set_config=lorap2p:915000000:7:0:1:8:20
            ser.write(b"at+set_config=lora:work_mode:1\r\n") # Set to P2P
            time.sleep(0.5)
            print(ser.read_all().decode('utf-8', errors='ignore').strip())
            
            # Send metadata (including compressed_len)
            meta_payload = f"META:{len(input_radicals)}:{payload_hash}:{len(compressed_bytes)}"
            meta_hex = meta_payload.encode('utf-8').hex()
            cmd = f"at+send=lorap2p:{meta_hex}\r\n".encode('utf-8')
            print(f"  - Broadcasting Metadata: {meta_payload}")
            ser.write(cmd)
            time.sleep(1.0)
            
            # Send frames
            for i, pkt in enumerate(packets):
                pkt_hex = pkt.hex()
                cmd = f"at+send=lorap2p:{pkt_hex}\r\n".encode('utf-8')
                print(f"  - Sending Frame {i:02d} (LoRa payload: {pkt_hex[:20]}...)")
                ser.write(cmd)
                time.sleep(1.0)
                
            ser.close()
            print("\n[SUCCESS] Serial transmission sequence completed.")
            
        except Exception as e:
            print(f"  [-] Serial port communication error: {e}")
            sys.exit(1)

if __name__ == "__main__":
    main()

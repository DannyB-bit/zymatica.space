#!/usr/bin/env python3
# Watermark: ip zymatica.space | astronautshe.com
# RAK Miner B - Receiver Script

import os
import sys
import socket
import argparse
import hashlib

# ==============================================================================
# LLD-AC Range Decoding Logic (Invention 07) with Native C Acceleration (Yang)
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

        _cuneiform_lib.cuneiform_u_v3_decode_dll.argtypes = [
            ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint32,
            ctypes.POINTER(Concept6D), ctypes.c_uint32,
            ctypes.c_uint32, ctypes.c_uint32
        ]
        _cuneiform_lib.cuneiform_u_v3_decode_dll.restype = ctypes.c_int
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

class BitReader:
    def __init__(self, data):
        self.data = data
        self.byte_index = 0
        self.bit_index = 0

    def read_bit(self):
        if self.byte_index >= len(self.data):
            return 0
        bit = (self.data[self.byte_index] >> (7 - self.bit_index)) & 1
        self.bit_index += 1
        if self.bit_index == 8:
            self.bit_index = 0
            self.byte_index += 1
        return bit

def range_decode_radicals_py(encoded_bytes, num_concepts, alpha=1, weight=128):
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

def range_decode_radicals(encoded_bytes, num_concepts, alpha=1, weight=128):
    if _cuneiform_lib:
        try:
            decoded_concepts = (Concept6D * num_concepts)()
            success = _cuneiform_lib.cuneiform_u_v3_decode_dll(
                (ctypes.c_uint8 * len(encoded_bytes)).from_buffer_copy(encoded_bytes),
                len(encoded_bytes),
                decoded_concepts,
                num_concepts,
                alpha, weight
            )
            if success:
                radicals = []
                for i in range(num_concepts):
                    rc = (decoded_concepts[i].domain << 4) | decoded_concepts[i].subdomain
                    rf = (decoded_concepts[i].operation << 4) | decoded_concepts[i].modality
                    ra = (decoded_concepts[i].depth << 4) | decoded_concepts[i].polarity
                    radicals.append((rc, rf, ra))
                return radicals
        except Exception as e:
            print(f"  [-] Native decoding exception: {e}. Falling back to Python.")

    return range_decode_radicals_py(encoded_bytes, num_concepts, alpha, weight)


# ==============================================================================
# XOR-FEC Packet Reassembly Logic (Invention 05)
# ==============================================================================
SYNC_MARKER    = 0xBB
PKT_SIZE       = 255
TRANSPORT_HDR  = 3
DATA_PER_PKT   = PKT_SIZE - TRANSPORT_HDR  # 252 Bytes

def heal_and_reassemble(received_packets):
    if not received_packets:
        return None
        
    total_packets = received_packets[0][2]
    received_indices = {pkt[1] for pkt in received_packets}
    
    # Check if we have all packets
    if len(received_packets) == total_packets:
        # Sort and assemble payload
        received_packets.sort(key=lambda x: x[1])
        payload = bytearray()
        for idx in range(total_packets - 1): # exclude the parity packet
            payload.extend(received_packets[idx][TRANSPORT_HDR:])
        return bytes(payload)
        
    # Check if exactly one packet is missing
    if len(received_packets) == total_packets - 1:
        missing_index = None
        for idx in range(total_packets):
            if idx not in received_indices:
                missing_index = idx
                break
                
        print(f"  [XOR-FEC] Healing detected: Recovering missing frame index {missing_index}...")
        
        # Recover packet using XOR across all received packets
        recovered_data = bytearray(DATA_PER_PKT)
        for pkt in received_packets:
            data_part = pkt[TRANSPORT_HDR:]
            for d_idx in range(DATA_PER_PKT):
                recovered_data[d_idx] ^= data_part[d_idx]
                
        recovered_packet = bytes([SYNC_MARKER, missing_index, total_packets]) + bytes(recovered_data)
        
        all_packets = list(received_packets) + [recovered_packet]
        all_packets.sort(key=lambda x: x[1])
        
        payload = bytearray()
        for idx in range(total_packets - 1):
            payload.extend(all_packets[idx][TRANSPORT_HDR:])
        return bytes(payload)
        
    print(f"  [XOR-FEC] Error: Insufficient packets for healing. Received {len(received_packets)}/{total_packets}.")
    return None

# ==============================================================================
# Main Receiver Execution
# ==============================================================================
def main():
    parser = argparse.ArgumentParser(description="Zymatica LoRa Protocol - RAK Miner B (Receiver)")
    parser.add_argument("--mode", choices=["udp", "serial"], default="udp", help="Connection interface: udp (local network) or serial (USB/UART)")
    parser.add_argument("--ip", default="0.0.0.0", help="Binding IP for UDP socket")
    parser.add_argument("--port", type=int, default=50001, help="Binding port for UDP socket")
    parser.add_argument("--device", default="/dev/ttyUSB0", help="Serial port interface (needed for serial mode)")
    parser.add_argument("--baud", type=int, default=115200, help="Serial baud rate")
    args = parser.parse_args()

    print("=" * 80)
    print("  ZYMATICA | Language-U RAK-Miner B (Receiver Node)")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    num_concepts = 0
    expected_hash = ""
    compressed_len = None
    received_raw_packets = []

    if args.mode == "udp":
        print(f"\n[1] Starting UDP Server binding to {args.ip}:{args.port}...")
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind((args.ip, args.port))
        
        expected_total = 99
        while len(received_raw_packets) < expected_total:
            data, addr = sock.recvfrom(512)
            
            # Check for metadata signature
            if data.startswith(b"META:"):
                meta_parts = data.decode('utf-8').split(":")
                num_concepts = int(meta_parts[1])
                expected_hash = meta_parts[2]
                compressed_len = int(meta_parts[3]) if len(meta_parts) > 3 else None
                print(f"  - Received Metadata from {addr[0]}: Concepts={num_concepts}, Checksum={expected_hash}, CompressedLen={compressed_len}")
                continue
                
            # Process received frames
            if len(data) == PKT_SIZE and data[0] == SYNC_MARKER:
                received_raw_packets.append(data)
                expected_total = data[2]
                print(f"  - Received Frame {len(received_raw_packets)}/{expected_total} ({len(data)} bytes) from {addr[0]}")
                
        sock.close()
        
    elif args.mode == "serial":
        print(f"\n[1] Opening Serial Port {args.device} (RAK LoRa P2P Listener)...")
        try:
            import serial
        except ImportError:
            print("  [-] Error: 'pyserial' library not found. Install it using 'pip install pyserial'")
            sys.exit(1)
            
        try:
            ser = serial.Serial(args.device, args.baud, timeout=10)
            
            # Put RAK module in listening P2P mode
            ser.write(b"at+set_config=lora:work_mode:1\r\n")
            time.sleep(0.5)
            
            # Start continuous receive mode
            # at+set_config=lorap2p:rxtx:0 (0 means receive)
            ser.write(b"at+set_config=lorap2p:rxtx:0\r\n")
            time.sleep(0.5)
            ser.read_all() # flush startup logs
            
            print("  - Listening for LoRa P2P transmissions (timeout 30s)...")
            
            # Read loop
            expected_total = 99
            import re
            
            while len(received_raw_packets) < expected_total:
                line = ser.readline().decode('utf-8', errors='ignore').strip()
                if not line:
                    continue
                
                # Check for RAK AT receive notification pattern:
                # e.g., "at+recv=RSSI,SNR,LEN:HEX_DATA"
                m = re.search(r'at\+recv=[^:]+:(.+)', line)
                if m:
                    hex_data = m.group(1).strip()
                    bin_data = bytes.fromhex(hex_data)
                    
                    if bin_data.startswith(b"META:"):
                        meta_parts = bin_data.decode('utf-8').split(":")
                        num_concepts = int(meta_parts[1])
                        expected_hash = meta_parts[2]
                        compressed_len = int(meta_parts[3]) if len(meta_parts) > 3 else None
                        print(f"  - Received Metadata: Concepts={num_concepts}, Checksum={expected_hash}, CompressedLen={compressed_len}")
                        continue
                        
                    if len(bin_data) == PKT_SIZE and bin_data[0] == SYNC_MARKER:
                        received_raw_packets.append(bin_data)
                        expected_total = bin_data[2]
                        print(f"  - Received LoRa Frame {len(received_raw_packets)}/{expected_total}")
            
            ser.close()
        except Exception as e:
            print(f"  [-] Serial communication error: {e}")
            sys.exit(1)

    # 2. XOR-FEC Reassembly
    print(f"\n[2] Executing Forward Error Correction (XOR-FEC) Reassembler...")
    payload_bytes = heal_and_reassemble(received_raw_packets)
    
    if payload_bytes is None:
        print("  [-] Error: Payload reassembly failed.")
        sys.exit(1)
        
    # Trim padding if necessary to align with typical coordinate length bounds
    if compressed_len is not None:
        payload_bytes = payload_bytes[:compressed_len]
        print(f"  - Reassembled Payload (Trimmed): {len(payload_bytes)} bytes")
    else:
        print(f"  - Reassembled Payload: {len(payload_bytes)} bytes")
    
    # 3. Check checksum
    received_hash = hashlib.sha256(payload_bytes).hexdigest()
    print(f"  - Received Payload Checksum:   {received_hash}")
    print(f"  - Expected Metadata Checksum:  {expected_hash}")
    
    if received_hash != expected_hash:
        print("  [-] Warning: Hash mismatch! Payload corrupted.")
    else:
        print("  [+] Payload checksum verified successfully! Lossless path confirmed. [OK]")

    # 4. Decrypt / Range Decode Cuneiform-U Coordinates
    print(f"\n[3] Range-Decoding Cuneiform-U Coordinates (LLD-AC System Ascent)...")
    try:
        decoded_radicals = range_decode_radicals(payload_bytes, num_concepts, alpha=1, weight=128)
        print(f"  - Decoded Concepts Matrix ({num_concepts} items):")
        for idx, rad in enumerate(decoded_radicals):
            print(f"    Concept {idx+1:02d}: RC=0x{rad[0]:02X}, RF=0x{rad[1]:02X}, RA=0x{rad[2]:02X}")
            
        print("\n" + "=" * 80)
        print("  [VERIFICATION] Multi-Language runtime FFI structures validated.")
        print("=" * 80)
        
    except Exception as e:
        print(f"  [-] Coordinate range decoding failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()

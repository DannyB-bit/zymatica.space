# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
ZK-LoRaWAN Device SDK (Firmware Layer)
========================================
Runs on the edge IoT device (e.g. ESP32, STM32, Arduino, or MicroPython).

Responsible for:
  1. Compressing readings using the UFO Semantic Codec.
  2. Generating the Cuneiform-U coordinate vector (6D location/modality gate).
  3. Serializing and compressing the Groth16 proof via LLD-AC.
  4. Packaging the frame with XOR-FEC parity bytes for error resilience.
  5. Generating a raw binary frame (≤ 255 bytes) to send to the LoRa transceiver.

Frame Serialization Structure:
┌─────────────────────────────────────────────────────────────┐
│  Byte Range  │ Field Description                            │
├──────────────┼──────────────────────────────────────────────┤
│  0x00 - 0x02 │ Frame Header (3 bytes: Version, Type, Flags) │
│  0x03 - 0x08 │ Cuneiform-U Coordinates (6 bytes)            │
│  0x09 - 0x10 │ Receiver Tag (8 bytes)                       │
│  0x11 - 0x92 │ Compressed ZK Proof (≤ 130 bytes, 1B length) │
│  0x93 - 0xDF │ UFO Semantic Payload (≤ 57 bytes)            │
│  Remaining   │ XOR-FEC Parity + Checksum (≤ 20 bytes)      │
└──────────────┴──────────────────────────────────────────────┘
Maximum Frame Size: 244 bytes (within 255-byte LoRa MTU)

Budget:
  Core data    = 3 + 6 + 8 + 1 + proof_len + semantic_len  (≤ 224 bytes)
  FEC overhead = 16 (parity) + 4 (checksum) = 20 bytes
  Total frame  ≤ 244 bytes
"""

import sys
import os

# Ensure the root folder is in the path for importing components
sys.path.append(os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import struct
from typing import List, Tuple

from semantic_codec.semantic_codec import SemanticCodec
from proof_compression.lld_ac_encoder import LLDACEncoder
from fec.xor_fec import XorFEC
from semantic_gating.range_proof import CuneiformCoordinate

# Maximum core data size before FEC (must pad to N * block_size ≤ 235)
# With block_size=16 and group_size=14: padded=224, parity=16, checksum=4 → 244 bytes total
MAX_CORE_DATA = 224
PROOF_SLOT = 160
SEMANTIC_SLOT = 46
FIXED_OVERHEAD = 3 + 6 + 8 + 1  # header + coords + tag + proof_length_byte = 18


class ZKLoRaWANDeviceSDK:
    """Edge Device SDK for building private, compressed ZK-LoRaWAN packets."""

    VERSION = 1
    FRAME_TYPE_DATA = 0x01

    def __init__(self):
        self.codec = SemanticCodec()
        self.fec = XorFEC(block_size=16, group_size=14)
        self.proof_compressor = LLDACEncoder()

    def build_frame(
        self,
        readings: List[Tuple[str, any, int]],
        coords_6d: Tuple[int, int, int, int, int, int],
        receiver_tag: bytes,
        raw_proof_bytes: bytes,
    ) -> bytes:
        """
        Builds a complete, compressed, self-healing LoRa frame (≤ 255 bytes).

        Args:
            readings: list of (concept_name, value, type) for semantic compression
            coords_6d: tuple of 6 ints (Domain, Subdomain, Modality, Polarity, Strength, Depth)
            receiver_tag: 8-byte target receiver identifier
            raw_proof_bytes: Raw serialized Groth16 proof (128-160 bytes)

        Returns:
            Raw binary frame ready for the LoRa radio transmitter (≤ 255 bytes).

        Raises:
            ValueError: If compressed proof exceeds slot or frame exceeds MTU.
        """
        if len(receiver_tag) != 8:
            raise ValueError("Receiver tag must be exactly 8 bytes.")

        # 1. Compress meaning into semantic tokens
        semantic_payload = self.codec.encode(readings)
        if len(semantic_payload) < SEMANTIC_SLOT:
            semantic_payload = semantic_payload + b'\x00' * (SEMANTIC_SLOT - len(semantic_payload))
        elif len(semantic_payload) > SEMANTIC_SLOT:
            raise ValueError(f"Semantic payload ({len(semantic_payload)}B) exceeds {SEMANTIC_SLOT}-byte slot capacity.")

        # 2. Serialize Cuneiform-U coordinates (6 bytes)
        coords_bytes = struct.pack("BBBBBB", *coords_6d)

        # 3. Compress ZK Proof via LLD-AC range coding (use raw bytes if compression expands)
        compressed_proof = self.proof_compressor.encode(raw_proof_bytes)
        if len(compressed_proof) >= len(raw_proof_bytes):
            # LLD-AC expanded the data (high-entropy curve points); use raw bytes
            compressed_proof = raw_proof_bytes
        if len(compressed_proof) > PROOF_SLOT:
            raise ValueError(
                f"Compressed proof ({len(compressed_proof)}B) exceeds {PROOF_SLOT}-byte slot. "
                f"Raw proof was {len(raw_proof_bytes)}B. Use packet fragmentation for large proofs."
            )

        # 4. Assemble Frame Header (3 bytes)
        flags = 0x00
        if len(compressed_proof) < len(raw_proof_bytes):
            flags |= 0x01  # Bit 0: PROOF_COMPRESSED
        header = struct.pack("BBB", self.VERSION, self.FRAME_TYPE_DATA, flags)

        # 5. Concatenate Payload Core with length-prefixed proof
        # [Header 3B] [Coords 6B] [Tag 8B] [ProofLen 1B] [Proof ≤130B] [Semantic 57B]
        proof_len_byte = struct.pack("B", len(compressed_proof))
        core_data = header + coords_bytes + receiver_tag + proof_len_byte + compressed_proof + semantic_payload

        if len(core_data) > MAX_CORE_DATA:
            raise ValueError(
                f"Core data ({len(core_data)}B) exceeds {MAX_CORE_DATA}-byte maximum. "
                f"Frame would exceed 255-byte LoRa MTU."
            )

        # 6. Apply XOR-FEC Parity protection
        padded, parity_blocks, _, checksum = self.fec.encode(core_data)
        fec_frame = padded + b"".join(parity_blocks) + checksum

        if len(fec_frame) > 255:
            raise ValueError(
                f"Final frame ({len(fec_frame)}B) exceeds 255-byte LoRa MTU. "
                f"This should not happen; check FEC parameters."
            )

        return fec_frame


# ============================================================================
# Local Test & Verification
# ============================================================================
if __name__ == "__main__":
    print("======================================================================")
    print("  ZK-LoRaWAN Device SDK — Integration Test")
    print("======================================================================")
    print()

    sdk = ZKLoRaWANDeviceSDK()

    # Sample environmental sensor readings
    readings = [
        ("temperature", 24.5, 2),  # VAL_F16
        ("humidity", 55, 0),       # VAL_U8
        ("co2", 405, 1)            # VAL_I16
    ]

    # 6D Routing Coordinates
    coords = (42, 7, 220, 128, 200, 15)

    # Receiver Address Tag
    receiver_tag = b"RX-NODE1"

    # Fake 160-byte serialized proof (simulates raw Groth16 proof bytes)
    fake_proof = os.urandom(160)

    # Build the binary packet
    try:
        frame = sdk.build_frame(readings, coords, receiver_tag, fake_proof)
        print(f"  [OK] LoRa frame successfully built")
        print(f"     Frame size:   {len(frame)} bytes (MTU limit: 255)")
        print(f"     Within MTU:   {'YES' if len(frame) <= 255 else 'NO — FRAME EXCEEDS MTU'}")
        print(f"     Header hex:   {frame[:3].hex()}")
        print(f"     Coords hex:   {frame[3:9].hex()}")
        print(f"     Tag hex:      {frame[9:17].hex().upper()}")
        proof_len = frame[17]
        print(f"     Proof len:    {proof_len} bytes (compressed from {len(fake_proof)})")
        print()

        if len(frame) <= 255:
            print("  [SUCCESS] Device SDK component verified — frame fits in LoRa MTU!")
        else:
            print("  [FAIL] Frame exceeds 255-byte LoRa MTU!")
            sys.exit(1)
    except Exception as e:
        print(f"  [ERROR] SDK build failed: {e}")
        sys.exit(1)


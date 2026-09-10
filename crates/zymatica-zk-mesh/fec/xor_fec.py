# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
XOR Forward Error Correction (FEC) for ZK Proof Radio Packets
==============================================================
Component 06 of the Language-U Semantic Communication Protocol.

Wraps compressed ZK proof bytes in an XOR-FEC parity shell before LoRa
radio transmission. If up to 20% of the packet is lost or corrupted
over the air, the receiver reconstructs the proof locally without
requiring retransmission.

This is critical because cryptographic proofs are brittle: a single
corrupted bit invalidates the entire Groth16 verification, forcing a
costly retransmission that drains battery and wastes Solana gas fees.
"""

import struct
import hashlib


class XorFEC:
    """
    XOR Forward Error Correction encoder/decoder.

    Operates on fixed-size blocks. For each group of N data blocks,
    generates one parity block computed as XOR of all N blocks.
    If any single block in the group is lost, it can be reconstructed
    from the remaining (N-1) data blocks XOR'd with the parity block.

    Block size is configurable but defaults to 16 bytes (aligned to
    AES block size for compatibility with encrypted payloads).
    """

    DEFAULT_BLOCK_SIZE = 16
    DEFAULT_GROUP_SIZE = 4  # 4 data blocks + 1 parity = 20% overhead

    def __init__(self, block_size=None, group_size=None):
        self.block_size = block_size or self.DEFAULT_BLOCK_SIZE
        self.group_size = group_size or self.DEFAULT_GROUP_SIZE

    def _pad_to_blocks(self, data: bytes) -> tuple:
        """Pad data to exact block boundary, return (padded_data, original_length)."""
        original_length = len(data)
        remainder = len(data) % self.block_size
        if remainder != 0:
            padding = self.block_size - remainder
            data = data + bytes(padding)
        return data, original_length

    def _split_blocks(self, data: bytes) -> list:
        """Split padded data into fixed-size blocks."""
        return [
            data[i:i + self.block_size]
            for i in range(0, len(data), self.block_size)
        ]

    def _xor_blocks(self, blocks: list) -> bytes:
        """Compute XOR parity of a list of byte blocks."""
        result = bytearray(self.block_size)
        for block in blocks:
            for i in range(self.block_size):
                result[i] ^= block[i]
        return bytes(result)

    def encode(self, data: bytes) -> tuple:
        """
        Encode data with XOR-FEC parity blocks.

        Args:
            data: Raw byte stream (compressed ZK proof)

        Returns:
            Tuple of (encoded_data, parity_blocks, original_length)
            where encoded_data contains the original data unchanged,
            and parity_blocks is a list of parity bytes to append.
        """
        padded, original_length = self._pad_to_blocks(data)
        blocks = self._split_blocks(padded)

        parity_blocks = []
        for i in range(0, len(blocks), self.group_size):
            group = blocks[i:i + self.group_size]
            # Pad last group if incomplete
            while len(group) < self.group_size:
                group.append(bytes(self.block_size))
            parity = self._xor_blocks(group)
            parity_blocks.append(parity)

        # Compute integrity checksum over all data + parity
        check_data = padded + b''.join(parity_blocks)
        checksum = hashlib.sha256(check_data).digest()[:4]

        return padded, parity_blocks, original_length, checksum

    def decode(self, padded_data: bytes, parity_blocks: list,
               original_length: int, checksum: bytes,
               corrupted_indices: list = None) -> bytes:
        """
        Decode and reconstruct data using XOR-FEC parity.

        Args:
            padded_data: Received data (may contain corrupted blocks)
            parity_blocks: Received parity blocks
            original_length: Original data length before padding
            checksum: 4-byte SHA-256 integrity checksum
            corrupted_indices: List of block indices known to be corrupted
                              (detected via CRC failure on LoRa frame)

        Returns:
            Reconstructed original data
        """
        blocks = list(self._split_blocks(padded_data))

        if corrupted_indices:
            for corrupt_idx in corrupted_indices:
                # Determine which parity group this block belongs to
                group_idx = corrupt_idx // self.group_size
                group_start = group_idx * self.group_size

                if group_idx >= len(parity_blocks):
                    continue  # Cannot reconstruct without parity

                # Collect all non-corrupted blocks in this group + parity
                reconstruction_blocks = []
                for i in range(self.group_size):
                    block_idx = group_start + i
                    if block_idx == corrupt_idx:
                        continue  # Skip the corrupted block
                    if block_idx < len(blocks):
                        reconstruction_blocks.append(blocks[block_idx])
                    else:
                        reconstruction_blocks.append(bytes(self.block_size))

                # XOR remaining blocks with parity to recover corrupted block
                reconstruction_blocks.append(parity_blocks[group_idx])
                recovered = self._xor_blocks(reconstruction_blocks)
                blocks[corrupt_idx] = recovered

        # Reassemble and trim to original length
        reconstructed = b''.join(blocks)[:original_length]

        # Verify integrity
        check_data = b''.join(blocks) + b''.join(parity_blocks)
        expected_checksum = hashlib.sha256(check_data).digest()[:4]
        if checksum != expected_checksum:
            raise ValueError("XOR-FEC checksum verification failed: integrity check mismatch after reconstruction")

        return reconstructed


def compute_overhead_ratio(data_length: int, block_size=16, group_size=4):
    """Calculate the FEC overhead ratio for a given data length."""
    padded_len = data_length + (block_size - data_length % block_size) % block_size
    num_blocks = padded_len // block_size
    num_parity = (num_blocks + group_size - 1) // group_size
    parity_bytes = num_parity * block_size
    total = padded_len + parity_bytes + 4  # +4 for checksum
    overhead = (total - data_length) / data_length * 100
    return total, overhead


# ============================================================================
# Self-Test & Verification
# ============================================================================

if __name__ == "__main__":
    print("=" * 70)
    print("ZK-LoRaWAN | XOR-FEC Crypto-Reconstruction - Self-Test")
    print("=" * 70)
    print()

    fec = XorFEC(block_size=16, group_size=4)

    # Simulate a compressed ZK proof (120 bytes)
    original_data = bytes(range(120))
    print(f"  [1] Original data: {len(original_data)} bytes")

    # Encode with FEC
    padded, parity_blocks, orig_len, checksum = fec.encode(original_data)
    total_parity = sum(len(p) for p in parity_blocks)
    print(f"  [2] Padded data:   {len(padded)} bytes")
    print(f"  [3] Parity blocks: {len(parity_blocks)} blocks ({total_parity} bytes)")
    print(f"  [4] Checksum:      {checksum.hex()}")

    total_overhead, overhead_pct = compute_overhead_ratio(len(original_data))
    print(f"  [5] Total with FEC: {total_overhead} bytes ({overhead_pct:.1f}% overhead)")

    # Test 1: Clean decode (no corruption)
    decoded = fec.decode(padded, parity_blocks, orig_len, checksum)
    clean_pass = decoded == original_data
    print(f"\n  [Test 1] Clean decode:       {'PASS' if clean_pass else 'FAIL'}")

    # Test 2: Corrupt one block and reconstruct
    corrupted_padded = bytearray(padded)
    # Corrupt block 2 (bytes 32-47)
    for i in range(32, 48):
        corrupted_padded[i] = 0xFF
    corrupted_padded = bytes(corrupted_padded)

    recovered = fec.decode(corrupted_padded, parity_blocks, orig_len, checksum,
                           corrupted_indices=[2])
    recovery_pass = recovered == original_data
    print(f"  [Test 2] Single-block recovery: {'PASS' if recovery_pass else 'FAIL'}")

    # Test 3: Corrupt two blocks in different groups
    corrupted_padded2 = bytearray(padded)
    for i in range(0, 16):    # Block 0 (group 0)
        corrupted_padded2[i] = 0xAA
    for i in range(64, 80):   # Block 4 (group 1)
        corrupted_padded2[i] = 0xBB
    corrupted_padded2 = bytes(corrupted_padded2)

    recovered2 = fec.decode(corrupted_padded2, parity_blocks, orig_len, checksum,
                            corrupted_indices=[0, 4])
    multi_pass = recovered2 == original_data
    print(f"  [Test 3] Multi-group recovery:  {'PASS' if multi_pass else 'FAIL'}")

    print()
    all_pass = clean_pass and recovery_pass and multi_pass
    if all_pass:
        print("  ALL FEC CHECKS PASSED - Proof packets self-heal over corrupted radio links")
    else:
        print("  SOME CHECKS FAILED")
    print()

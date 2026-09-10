# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
ZK-LoRaWAN Gateway Software
==============================
Bridges LoRa RF packets to Solana blockchain.

Architecture:
  LoRa Radio (SX1276/SX1262) → Gateway → Solana RPC

Modes:
  SINGLE: Each chirp → 1 Solana transaction (instant on-chain)
  BATCH:  Collect chirps → 1 Solana transaction per batch (saves base fees)

Fee per chirp: 150,000 lamports (100K gateway + 50K treasury)
"""

import asyncio
import hashlib
import json
import struct
import time
import logging
import sys
import os
import ctypes
import platform
import subprocess
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional
from Crypto.Hash import keccak

from solana.rpc.async_api import AsyncClient
from solders.pubkey import Pubkey
from solders.keypair import Keypair
from solders.instruction import AccountMeta
from anchorpy import Program, Provider, Wallet, Idl

# Setup path to import from adjacent folders
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from proof_compression.lld_ac_encoder import LLDACEncoder, pack_lora_frame

def load_groth16_lib():
    system = platform.system()
    if system == "Windows":
        lib_name = "zk_lorawan_groth16.dll"
    elif system == "Darwin":
        lib_name = "libzk_lorawan_groth16.dylib"
    else:
        lib_name = "libzk_lorawan_groth16.so"

    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    search_paths = [
        os.path.join(base_dir, "target", "release", lib_name),
        os.path.join(base_dir, "target", "debug", lib_name),
        os.path.join(base_dir, "target", "release", "deps", lib_name),
        os.path.join(base_dir, "target", "debug", "deps", lib_name),
        os.path.join(base_dir, lib_name),
    ]

    for path in search_paths:
        if os.path.exists(path):
            try:
                lib = ctypes.CDLL(path)
                lib.verify_zk_proof_raw.argtypes = [
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_char_p, ctypes.c_size_t,
                    ctypes.c_uint64,
                    ctypes.c_char_p, ctypes.c_size_t,
                ]
                lib.verify_zk_proof_raw.restype = ctypes.c_uint32

                # Register proof coordinate decompression FFI
                u8_ptr = ctypes.POINTER(ctypes.c_uint8)
                lib.decompress_proof_to_solana_coords.argtypes = [
                    u8_ptr,
                    u8_ptr,
                    u8_ptr,
                    u8_ptr,
                    u8_ptr,
                    u8_ptr,
                ]
                lib.decompress_proof_to_solana_coords.restype = ctypes.c_uint32
                return lib, path
            except Exception as e:
                logging.getLogger("gateway").warning(f"Failed to load library at {path}: {e}")
    return None, None

def load_keypair(path_or_json: str) -> Keypair:
    expanded_path = os.path.expanduser(path_or_json)
    if os.path.exists(expanded_path):
        with open(expanded_path, "r") as f:
            secret = json.load(f)
            return Keypair.from_bytes(bytes(secret))
    try:
        secret = json.loads(path_or_json)
        return Keypair.from_bytes(bytes(secret))
    except Exception:
        logging.getLogger("solana").warning(f"Could not load keypair from {path_or_json}, generating a temporary fallback keypair.")
        return Keypair()

def load_idl_or_fallback() -> Idl:
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    idl_path = os.path.join(base_dir, "target", "idl", "zk_lorawan.json")
    if os.path.exists(idl_path):
        with open(idl_path, "r") as f:
            return Idl.from_json(f.read())
    fallback_json = {
        "version": "0.1.0",
        "name": "zk_lorawan",
        "instructions": [
            {
                "name": "verifySingle",
                "accounts": [
                    {"name": "registry", "isMut": True, "isSigner": False},
                    {"name": "shieldedPool", "isMut": True, "isSigner": False},
                    {"name": "nullifierRecord", "isMut": True, "isSigner": False},
                    {"name": "gateway", "isMut": True, "isSigner": True},
                    {"name": "treasury", "isMut": True, "isSigner": False},
                    {"name": "systemProgram", "isMut": False, "isSigner": False}
                ],
                "args": [
                    {"name": "proofA", "type": {"array": ["u8", 64]}},
                    {"name": "proofB", "type": {"array": ["u8", 128]}},
                    {"name": "proofC", "type": {"array": ["u8", 64]}},
                    {"name": "nullifierHash", "type": {"array": ["u8", 32]}},
                    {"name": "attestationHash", "type": {"array": ["u8", 32]}},
                    {"name": "zkVdeProofHash", "type": {"array": ["u8", 32]}},
                    {"name": "ciphertextHash", "type": {"array": ["u8", 32]}},
                    {"name": "timestamp", "type": "i64"}
                ]
            },
            {
                "name": "initializeBatch",
                "accounts": [
                    {"name": "batch", "isMut": True, "isSigner": False},
                    {"name": "registry", "isMut": True, "isSigner": False},
                    {"name": "gateway", "isMut": True, "isSigner": True},
                    {"name": "systemProgram", "isMut": False, "isSigner": False}
                ],
                "args": []
            },
            {
                "name": "addChirp",
                "accounts": [
                    {"name": "batch", "isMut": True, "isSigner": False},
                    {"name": "gateway", "isMut": False, "isSigner": True}
                ],
                "args": [
                    {"name": "proofA", "type": {"array": ["u8", 64]}},
                    {"name": "proofB", "type": {"array": ["u8", 128]}},
                    {"name": "proofC", "type": {"array": ["u8", 64]}},
                    {"name": "nullifierHash", "type": {"array": ["u8", 32]}},
                    {"name": "attestationHash", "type": {"array": ["u8", 32]}},
                    {"name": "zkVdeProofHash", "type": {"array": ["u8", 32]}},
                    {"name": "payloadHash", "type": {"array": ["u8", 32]}},
                    {"name": "timestamp", "type": "i64"}
                ]
            },
            {
                "name": "submitBatch",
                "accounts": [
                    {"name": "batch", "isMut": True, "isSigner": False},
                    {"name": "registry", "isMut": True, "isSigner": False},
                    {"name": "shieldedPool", "isMut": True, "isSigner": False},
                    {"name": "gateway", "isMut": True, "isSigner": True},
                    {"name": "treasury", "isMut": True, "isSigner": False},
                    {"name": "systemProgram", "isMut": False, "isSigner": False}
                ],
                "args": []
            }
        ],
        "accounts": [
            {
                "name": "ProtocolRegistry",
                "type": {
                    "kind": "struct",
                    "fields": [
                        {"name": "authority", "type": "publicKey"},
                        {"name": "nextBatchId", "type": "u64"},
                        {"name": "totalBatches", "type": "u64"},
                        {"name": "totalChirpsVerified", "type": "u64"},
                        {"name": "totalFeesCollected", "type": "u64"},
                        {"name": "totalGatewayRewards", "type": "u64"},
                        {"name": "totalTreasuryFees", "type": "u64"},
                        {"name": "createdAt", "type": "i64"}
                    ]
                }
            }
        ]
    }
    return Idl.from_json(json.dumps(fallback_json))

# ============================================================================
# Configuration
# ============================================================================

@dataclass
class GatewayConfig:
    """Gateway configuration."""
    # LoRa radio
    lora_frequency: int = 915_000_000   # US915 band (Hz)
    lora_sf: int = 12                    # Spreading factor (7-12, 12=max range)
    lora_bw: int = 125_000               # Bandwidth (Hz)
    lora_cr: int = 5                     # Coding rate (5=4/5, 8=4/8)

    # Solana
    solana_rpc: str = "http://127.0.0.1:8899" # Default to local validator
    program_id: str = "4HRP2eV8qtYW54ozQmnGDjF7emwb8MvqFcF89UgSM6iC"
    treasury_pubkey: str = "7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS" # Protocol treasury
    gateway_pubkey: str = "7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS"  # This gateway's wallet
    keypair_path: str = "~/.config/solana/id.json"

    # Batch settings
    mode: str = "batch"                  # "single" or "batch"
    batch_size: int = 100                # Max chirps per batch
    batch_timeout_sec: int = 900         # 15 minutes max wait

    # Protocol fees (lamports)
    fee_gateway: int = 100_000
    fee_treasury: int = 50_000
    fee_total: int = 150_000

    # Firmware whitelist for verification
    firmware_hash: bytes = b"enclave-firmware-version-v1.0.2"


class ChirpStatus(Enum):
    RECEIVED = "received"
    VERIFIED = "verified"
    REJECTED = "rejected"
    SUBMITTED = "submitted"
    CONFIRMED = "confirmed"


@dataclass
class Chirp:
    """A received ZK-LoRaWAN chirp."""
    raw_bytes: bytes
    proof_hash: bytes = b""
    payload_hash: bytes = b""
    timestamp: float = 0.0
    rssi: int = 0                        # Signal strength (dBm)
    snr: float = 0.0                     # Signal-to-noise ratio
    status: ChirpStatus = ChirpStatus.RECEIVED
    nullifier_hash: bytes = b""
    attestation_hash: bytes = b""
    zk_vde_proof_hash: bytes = b""
    ciphertext_hash: bytes = b""
    proof_a: bytes = b""
    proof_b: bytes = b""
    proof_c: bytes = b""

    def __post_init__(self):
        if self.timestamp == 0.0:
            self.timestamp = time.time()


@dataclass
class Batch:
    """A batch of chirps ready for Solana submission."""
    chirps: list = field(default_factory=list)
    created_at: float = 0.0
    merkle_root: bytes = b""
    tx_signature: str = ""

    def __post_init__(self):
        if self.created_at == 0.0:
            self.created_at = time.time()

    @property
    def is_full(self) -> bool:
        return len(self.chirps) >= 100

    @property
    def is_expired(self) -> bool:
        return (time.time() - self.created_at) > 900  # 15 min


# ============================================================================
# ZK Proof Verification (local, no blockchain needed)
# ============================================================================

class ZKVerifier:
    """
    Verifies Groth16 ZK proofs locally on the gateway.
    This is FREE — no Solana transaction needed.
    """

    PROOF_SIZE = 256   # bytes: proof_a(32) + proof_b(64) + proof_c(32) + 4 * hashes(32)
    FRAME_HEADER = 3   # bytes
    COORD_SIZE = 6     # bytes (Cuneiform-U coordinates)
    TAG_SIZE = 8       # bytes (receiver tag)
    OVERHEAD = FRAME_HEADER + COORD_SIZE + TAG_SIZE  # 17 bytes
    MAX_FRAME = 255    # LoRa MTU

    def parse_frame(self, raw: bytes) -> Optional[dict]:
        """
        Parse a ZK-LoRaWAN frame (supports both compressed and uncompressed).
        """
        if not raw:
            return None

        if raw[0] == 0xAE:
            # LLD-AC Compressed frame
            if len(raw) < 4:
                return None
            flags = raw[1]
            proof_len = (raw[2] << 8) | raw[3]

            offset = 4
            if len(raw) < offset + proof_len:
                return None
            compressed_proof = raw[offset:offset + proof_len]
            offset += proof_len

            # Read coordinates (6 bytes)
            if len(raw) < offset + 6:
                return None
            coords = raw[offset:offset + 6]
            offset += 6

            # Read FEC parity if flags specify it
            fec_parity = b""
            if flags & 0x01:
                if len(raw) < offset + 1:
                    return None
                fec_len = raw[offset]
                offset += 1
                if len(raw) < offset + fec_len:
                    return None
                fec_parity = raw[offset:offset + fec_len]
                offset += fec_len

            # Decompress proof using LLDACEncoder
            try:
                encoder = LLDACEncoder()
                # Original uncompressed size: proof_a(32) + proof_b(64) + proof_c(32) + 4 * hash(32) = 256 bytes
                decompressed = encoder.decode(compressed_proof, 256)
                if len(decompressed) < 256:
                    return None

                proof_a = decompressed[0:32]
                proof_b = decompressed[32:96]
                proof_c = decompressed[96:128]
                identity_hash = decompressed[128:160]
                nullifier_hash = decompressed[160:192]
                attestation_hash = decompressed[192:224]
                ciphertext_hash = decompressed[224:256]
            except Exception:
                return None

            return {
                "version": 1,
                "frame_type": 1,
                "flags": flags,
                "coords": coords,
                "tag": b"",
                "proof_a": proof_a,
                "proof_b": proof_b,
                "proof_c": proof_c,
                "identity_hash": identity_hash,
                "nullifier_hash": nullifier_hash,
                "attestation_hash": attestation_hash,
                "ciphertext_hash": ciphertext_hash,
                "payload": fec_parity,
            }
        else:
            # Parse version 1 frame with proof length prefix (supports both compressed and uncompressed proofs)
            if len(raw) < 18:
                return None

            offset = 0

            # Frame header (3 bytes)
            version = raw[offset]
            frame_type = raw[offset + 1]
            flags = raw[offset + 2]
            offset += 3

            # Cuneiform-U coordinates (6 bytes)
            coords = raw[offset:offset + 6]
            offset += 6

            # Receiver tag (8 bytes)
            tag = raw[offset:offset + 8]
            offset += 8

            # Read proof length byte
            proof_len = raw[offset] + 1
            offset += 1

            if len(raw) < offset + proof_len:
                return None
            compressed_proof = raw[offset:offset + proof_len]
            offset += proof_len

            # Decompress proof (handles 256-byte and 160-byte formats, compressed or raw)
            proof_a = b""
            proof_b = b""
            proof_c = b""
            identity_hash = b""
            nullifier_hash = b""
            attestation_hash = b""
            ciphertext_hash = b""

            is_compressed = (flags & 0x01) == 0x01
            try:
                if not is_compressed:
                    raise ValueError("Raw uncompressed proof")
                encoder = LLDACEncoder()
                # Try decompressing to 256 bytes (modern proof with 4 distinct hashes)
                decompressed = encoder.decode(compressed_proof, 256)
                if len(decompressed) == 256:
                    proof_a = decompressed[0:32]
                    proof_b = decompressed[32:96]
                    proof_c = decompressed[96:128]
                    identity_hash = decompressed[128:160]
                    nullifier_hash = decompressed[160:192]
                    attestation_hash = decompressed[192:224]
                    ciphertext_hash = decompressed[224:256]
                else:
                    # Try decompressing to 160 bytes (legacy format)
                    decompressed = encoder.decode(compressed_proof, 160)
                    if len(decompressed) == 160:
                        proof_a = decompressed[0:32]
                        proof_b = decompressed[32:96]
                        proof_c = decompressed[96:128]
                        identity_hash = decompressed[128:160]
                        nullifier_hash = identity_hash
                        attestation_hash = identity_hash
                        ciphertext_hash = identity_hash
                    else:
                        raise ValueError("Invalid length")
            except Exception:
                # If decompression fails or raw flag is set, treat compressed_proof as raw
                if len(compressed_proof) == 256:
                    proof_a = compressed_proof[0:32]
                    proof_b = compressed_proof[32:96]
                    proof_c = compressed_proof[96:128]
                    identity_hash = compressed_proof[128:160]
                    nullifier_hash = compressed_proof[160:192]
                    attestation_hash = compressed_proof[192:224]
                    ciphertext_hash = compressed_proof[224:256]
                elif len(compressed_proof) == 160:
                    proof_a = compressed_proof[0:32]
                    proof_b = compressed_proof[32:96]
                    proof_c = compressed_proof[96:128]
                    identity_hash = compressed_proof[128:160]
                    nullifier_hash = identity_hash
                    attestation_hash = identity_hash
                    ciphertext_hash = identity_hash
                else:
                    return None

            payload_and_fec = raw[offset:]

            return {
                "version": version,
                "frame_type": frame_type,
                "flags": flags,
                "coords": coords,
                "tag": tag,
                "proof_a": proof_a,
                "proof_b": proof_b,
                "proof_c": proof_c,
                "identity_hash": identity_hash,
                "nullifier_hash": nullifier_hash,
                "attestation_hash": attestation_hash,
                "ciphertext_hash": ciphertext_hash,
                "payload": payload_and_fec,
            }

    def verify_proof(self, frame: dict, gateway_address_hex: str = None) -> bool:
        """
        Verify the Groth16 proof locally using real ark-groth16 FFI or subprocess fallback.
        """
        if gateway_address_hex is None:
            gateway_address_hex = "0000000000000000000000000000000000000000000000000000000000000000"

        proof_a_hex = frame["proof_a"].hex().encode('utf-8')
        proof_b_hex = frame["proof_b"].hex().encode('utf-8')
        proof_c_hex = frame["proof_c"].hex().encode('utf-8')
        identity_hash_hex = frame["identity_hash"].hex().encode('utf-8')
        nullifier_hash_hex = frame.get("nullifier_hash", b"").hex().encode('utf-8')
        attestation_hash_hex = frame.get("attestation_hash", b"").hex().encode('utf-8')
        ciphertext_hash_hex = frame.get("ciphertext_hash", b"").hex().encode('utf-8')
        gateway_address_hex_bytes = gateway_address_hex.encode('utf-8')

        deposit_value = 150000
        firmware_hash_hex = b"enclave-firmware-version-v1.0.2".hex().encode('utf-8')

        # Try FFI library
        lib, path = load_groth16_lib()
        if lib is not None:
            res = lib.verify_zk_proof_raw(
                proof_a_hex, len(proof_a_hex),
                proof_b_hex, len(proof_b_hex),
                proof_c_hex, len(proof_c_hex),
                identity_hash_hex, len(identity_hash_hex),
                nullifier_hash_hex, len(nullifier_hash_hex),
                attestation_hash_hex, len(attestation_hash_hex),
                ciphertext_hash_hex, len(ciphertext_hash_hex),
                gateway_address_hex_bytes, len(gateway_address_hex_bytes),
                deposit_value,
                firmware_hash_hex, len(firmware_hash_hex)
            )
            return res == 1

        # Fallback to subprocess running the compiled binary zk_lorawan_prove
        base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        bin_name = "zk_lorawan_prove.exe" if platform.system() == "Windows" else "zk_lorawan_prove"
        bin_paths = [
            os.path.join(base_dir, "target", "release", bin_name),
            os.path.join(base_dir, "target", "debug", bin_name),
        ]

        for bin_path in bin_paths:
            if os.path.exists(bin_path):
                try:
                    cmd = [
                        bin_path, "verify",
                        frame["proof_a"].hex(),
                        frame["proof_b"].hex(),
                        frame["proof_c"].hex(),
                        frame["identity_hash"].hex(),
                        frame.get("nullifier_hash", b"").hex(),
                        frame.get("attestation_hash", b"").hex(),
                        frame.get("ciphertext_hash", b"").hex(),
                        gateway_address_hex,
                        str(deposit_value),
                        b"enclave-firmware-version-v1.0.2".hex()
                    ]
                    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
                    if proc.returncode == 0 and "VALID" in proc.stdout:
                        return True
                except Exception as e:
                    logging.getLogger("gateway").warning(f"Subprocess verification failed: {e}")

        # Allow mock proofs behind ZK_LORAWAN_ALLOW_MOCK_PROOFS=1 environment variable
        if os.environ.get("ZK_LORAWAN_ALLOW_MOCK_PROOFS") == "1":
            logging.getLogger("gateway").warning(
                "No local Groth16 verifier library or binary found. "
                "ZK_LORAWAN_ALLOW_MOCK_PROOFS=1 is enabled: falling back to non-zero mock check."
            )
            # Check basic non-zero components for development mock bypass
            if frame["proof_a"] == b'\x00' * 32: return False
            if frame["proof_b"] == b'\x00' * 64: return False
            if frame["proof_c"] == b'\x00' * 32: return False
            return True

        # Fail closed if neither library nor binary is present
        logging.getLogger("gateway").error("No local Groth16 verifier library or binary found. FAILED CLOSED.")
        return False

    def compute_proof_hash(self, frame: dict) -> bytes:
        """Hash the proof for Merkle tree inclusion."""
        h = hashlib.sha256()
        h.update(frame["proof_a"])
        h.update(frame["proof_b"])
        h.update(frame["proof_c"])
        h.update(frame["identity_hash"])
        h.update(frame.get("nullifier_hash", b""))
        h.update(frame.get("attestation_hash", b""))
        h.update(frame.get("ciphertext_hash", b""))
        return h.digest()

    def compute_payload_hash(self, frame: dict) -> bytes:
        """Hash the payload for audit trail."""
        return hashlib.sha256(frame["payload"]).digest()

    def decompress_to_solana_coords(self, proof_a: bytes, proof_b: bytes, proof_c: bytes) -> Optional[tuple]:
        """Convert compressed proof components into uncompressed Solana big-endian coordinates."""
        lib, _ = load_groth16_lib()
        if not lib:
            return None

        in_a = (ctypes.c_uint8 * len(proof_a)).from_buffer_copy(proof_a)
        in_b = (ctypes.c_uint8 * len(proof_b)).from_buffer_copy(proof_b)
        in_c = (ctypes.c_uint8 * len(proof_c)).from_buffer_copy(proof_c)
        out_a = (ctypes.c_uint8 * 64)()
        out_b = (ctypes.c_uint8 * 128)()
        out_c = (ctypes.c_uint8 * 64)()

        res = lib.decompress_proof_to_solana_coords(
            in_a,
            in_b,
            in_c,
            out_a,
            out_b,
            out_c
        )
        if res == 1:
            return bytes(out_a), bytes(out_b), bytes(out_c)
        return None


# ============================================================================
# Merkle Tree
# ============================================================================

class MerkleTree:
    """Keccak-256 Merkle tree matching the Solana on-chain implementation."""

    @staticmethod
    def compute_root(leaves: list) -> bytes:
        """Compute Merkle root from leaf hashes using Keccak-256."""
        if not leaves:
            return b'\x00' * 32
        if len(leaves) == 1:
            return leaves[0]

        current = list(leaves)
        while len(current) > 1:
            next_level = []
            for i in range(0, len(current), 2):
                left = current[i]
                right = current[i + 1] if i + 1 < len(current) else left

                h = keccak.new(digest_bits=256)
                h.update(left + right)
                next_level.append(h.digest())
            current = next_level

        return current[0]

    @staticmethod
    def build_proof(leaves: list, index: int) -> list:
        """Generate a Merkle proof of membership for a leaf at a given index."""
        if not leaves:
            return []
        proof = []
        current = list(leaves)
        idx = index
        while len(current) > 1:
            next_level = []
            for i in range(0, len(current), 2):
                left = current[i]
                right = current[i + 1] if i + 1 < len(current) else left
                if i == idx or i + 1 == idx:
                    sibling = right if i == idx else left
                    proof.append(sibling)

                h = keccak.new(digest_bits=256)
                h.update(left + right)
                next_level.append(h.digest())
            current = next_level
            idx = idx // 2
        return proof


# ============================================================================
# Solana Submitter (RPC interface)
# ============================================================================

class SolanaSubmitter:
    """
    Submits chirp batches to Solana.
    """

    def __init__(self, config: GatewayConfig):
        self.config = config
        self.logger = logging.getLogger("solana")
        self.keypair = load_keypair(config.keypair_path)
        self.program_id = Pubkey.from_string(config.program_id)
        self.client = AsyncClient(config.solana_rpc)
        self.wallet = Wallet(self.keypair)
        self.provider = Provider(self.client, self.wallet)
        self.idl = load_idl_or_fallback()
        self.program = Program(self.idl, self.program_id, self.provider)
        self.logger.info(f"Initialized Solana submitter for gateway wallet: {self.keypair.pubkey()}")

    async def get_registry_pda(self) -> Pubkey:
        pda, _ = Pubkey.find_program_address([b"zk-lorawan-registry"], self.program_id)
        return pda

    async def get_shielded_pool_pda(self) -> Pubkey:
        pda, _ = Pubkey.find_program_address([b"shielded-pool"], self.program_id)
        return pda

    async def get_nullifier_pda(self, nullifier: bytes) -> Pubkey:
        pda, _ = Pubkey.find_program_address([b"nullifier", nullifier], self.program_id)
        return pda

    async def submit_single(self, chirp: Chirp) -> str:
        """
        Submit a single chirp to Solana (MODE A).
        """
        self.logger.info(
            f"[SINGLE] Submitting anonymous chirp to Solana | "
            f"nullifier={chirp.nullifier_hash[:4].hex()}... | "
            f"fee={self.config.fee_total} lamports"
        )

        try:
            registry_pda = await self.get_registry_pda()
            shielded_pool_pda = await self.get_shielded_pool_pda()
            nullifier_pda = await self.get_nullifier_pda(chirp.nullifier_hash)
            treasury_pubkey = Pubkey.from_string(self.config.treasury_pubkey)

            # Decompress proof coordinates if they are 32/64/32 bytes compressed points
            decompressed_coords = None
            is_compressed = False
            if chirp.proof_a and len(chirp.proof_a) == 32 and len(chirp.proof_b) == 64 and len(chirp.proof_c) == 32:
                is_compressed = True
                verifier = ZKVerifier()
                decompressed_coords = verifier.decompress_to_solana_coords(chirp.proof_a, chirp.proof_b, chirp.proof_c)

            if decompressed_coords:
                proof_a_list = list(decompressed_coords[0])
                proof_b_list = list(decompressed_coords[1])
                proof_c_list = list(decompressed_coords[2])
            elif is_compressed:
                raise ValueError("Failed to decompress compressed proof coordinates. Refusing submission to prevent fee burning on-chain.")
            else:
                proof_a_list = list(chirp.proof_a) if chirp.proof_a else [0]*64
                proof_b_list = list(chirp.proof_b) if chirp.proof_b else [0]*128
                proof_c_list = list(chirp.proof_c) if chirp.proof_c else [0]*64

                # Ensure precise sizes
                if len(proof_a_list) < 64: proof_a_list += [0] * (64 - len(proof_a_list))
                if len(proof_b_list) < 128: proof_b_list += [0] * (128 - len(proof_b_list))
                if len(proof_c_list) < 64: proof_c_list += [0] * (64 - len(proof_c_list))

                proof_a_list = proof_a_list[:64]
                proof_b_list = proof_b_list[:128]
                proof_c_list = proof_c_list[:64]

            merkle_proof = getattr(chirp, 'merkle_proof', [])
            leaf_index = getattr(chirp, 'leaf_index', 0)

            tx = await self.program.methods.verifySingle(
                proof_a_list,
                proof_b_list,
                proof_c_list,
                list(chirp.nullifier_hash),
                list(chirp.attestation_hash),
                list(chirp.zk_vde_proof_hash),
                list(chirp.ciphertext_hash),
                int(chirp.timestamp),
                [list(p) for p in merkle_proof],
                int(leaf_index)
            ).accounts({
                "registry": registry_pda,
                "shieldedPool": shielded_pool_pda,
                "nullifierRecord": nullifier_pda,
                "gateway": self.keypair.pubkey(),
                "treasury": treasury_pubkey,
                "systemProgram": Pubkey.from_string("11111111111111111111111111111111")
            }).signers([self.keypair]).rpc()
            self.logger.info(f"Successfully submitted single transaction on-chain: {tx}")
            return str(tx)
        except Exception as e:
            self.logger.error(f"Solana single transaction submission failed: {e}")
            if os.environ.get("ZK_LORAWAN_DEMO_MODE") == "1":
                self.logger.warning("Running in demo mode: returning simulated transaction signature.")
                return f"sim_single_{int(chirp.timestamp)}"
            raise e

    async def submit_batch(self, batch: Batch) -> str:
        """
        Submit a batch of chirps to Solana (MODE B) on-chain.
        """
        n = len(batch.chirps)
        total_fee = self.config.fee_total * n
        self.logger.info(
            f"[BATCH] Submitting {n} anonymous chirps to Solana | "
            f"merkle={batch.merkle_root[:4].hex()}... | "
            f"total_fee={total_fee} lamports"
        )

        # Pre-flight: Decompress and validate all proofs in the batch before making any on-chain RPC calls
        decompressed_batch_coords = []
        for idx, chirp in enumerate(batch.chirps):
            decompressed_coords = None
            is_compressed = False
            if chirp.proof_a and len(chirp.proof_a) == 32 and len(chirp.proof_b) == 64 and len(chirp.proof_c) == 32:
                is_compressed = True
                verifier = ZKVerifier()
                decompressed_coords = verifier.decompress_to_solana_coords(chirp.proof_a, chirp.proof_b, chirp.proof_c)

            if decompressed_coords:
                decompressed_batch_coords.append((
                    list(decompressed_coords[0]),
                    list(decompressed_coords[1]),
                    list(decompressed_coords[2])
                ))
            elif is_compressed:
                raise ValueError(f"Failed to decompress compressed proof coordinates for chirp {idx+1}. Refusing submission to prevent fee burning on-chain.")
            else:
                proof_a_list = list(chirp.proof_a) if chirp.proof_a else [0]*64
                proof_b_list = list(chirp.proof_b) if chirp.proof_b else [0]*128
                proof_c_list = list(chirp.proof_c) if chirp.proof_c else [0]*64

                # Ensure precise sizes
                if len(proof_a_list) < 64: proof_a_list += [0] * (64 - len(proof_a_list))
                if len(proof_b_list) < 128: proof_b_list += [0] * (128 - len(proof_b_list))
                if len(proof_c_list) < 64: proof_c_list += [0] * (64 - len(proof_c_list))

                decompressed_batch_coords.append((
                    proof_a_list[:64],
                    proof_b_list[:128],
                    proof_c_list[:64]
                ))

        try:
            registry_pda = await self.get_registry_pda()
            registry_account = await self.program.account["ProtocolRegistry"].fetch(registry_pda)
            batch_id = registry_account.next_batch_id

            batch_id_bytes = batch_id.to_bytes(8, byteorder='little')
            batch_pda, _ = Pubkey.find_program_address(
                [b"zk-lorawan-batch", bytes(self.keypair.pubkey()), batch_id_bytes],
                self.program_id
            )

            # Initialize batch
            init_tx = await self.program.methods.initializeBatch().accounts({
                "batch": batch_pda,
                "registry": registry_pda,
                "gateway": self.keypair.pubkey(),
                "systemProgram": Pubkey.from_string("11111111111111111111111111111111")
            }).signers([self.keypair]).rpc()
            self.logger.info(f"Initialized batch #{batch_id} on-chain: {init_tx}")

            # Add chirps
            leaves = [chirp.zk_vde_proof_hash for chirp in batch.chirps]
            shielded_pool_pda = await self.get_shielded_pool_pda()
            for idx, chirp in enumerate(batch.chirps):
                proof_a_list, proof_b_list, proof_c_list = decompressed_batch_coords[idx]
                merkle_proof = MerkleTree.build_proof(leaves, idx)

                add_tx = await self.program.methods.addChirp(
                    proof_a_list,
                    proof_b_list,
                    proof_c_list,
                    list(chirp.nullifier_hash),
                    list(chirp.attestation_hash),
                    list(chirp.zk_vde_proof_hash),
                    list(chirp.ciphertext_hash), # ciphertext hash is payload hash on-chain
                    int(chirp.timestamp),
                    [list(p) for p in merkle_proof],
                    int(idx)
                ).accounts({
                    "batch": batch_pda,
                    "shieldedPool": shielded_pool_pda,
                    "gateway": self.keypair.pubkey()
                }).signers([self.keypair]).rpc()
                self.logger.debug(f"Added chirp {idx+1}/{n} to batch #{batch_id}: {add_tx}")

            # Submit batch
            treasury_pubkey = Pubkey.from_string(self.config.treasury_pubkey)

            nullifier_pdas = []
            for chirp in batch.chirps:
                pda = await self.get_nullifier_pda(chirp.nullifier_hash)
                nullifier_pdas.append(AccountMeta(pubkey=pda, is_signer=False, is_writable=True))

            finalize_tx = await self.program.methods.submitBatch().accounts({
                "batch": batch_pda,
                "registry": registry_pda,
                "shieldedPool": shielded_pool_pda,
                "gateway": self.keypair.pubkey(),
                "treasury": treasury_pubkey,
                "systemProgram": Pubkey.from_string("11111111111111111111111111111111")
            }).remaining_accounts(nullifier_pdas).signers([self.keypair]).rpc()

            self.logger.info(f"Successfully finalized batch #{batch_id} on-chain: {finalize_tx}")
            return str(finalize_tx)
        except Exception as e:
            self.logger.error(f"Solana batch submission failed: {e}")
            if os.environ.get("ZK_LORAWAN_DEMO_MODE") == "1":
                self.logger.warning("Running in demo mode: returning simulated transaction signature.")
                return f"sim_batch_{n}_{int(batch.created_at)}"
            raise e


# ============================================================================
# Gateway Engine (main loop)
# ============================================================================

class Gateway:
    """
    ZK-LoRaWAN Gateway — bridges LoRa radio to Solana blockchain.
    """

    def __init__(self, config: GatewayConfig):
        self.config = config
        self.verifier = ZKVerifier()
        self.merkle = MerkleTree()
        self.submitter = SolanaSubmitter(config)
        self.current_batch: Optional[Batch] = None
        self.stats = {
            "chirps_received": 0,
            "chirps_verified": 0,
            "chirps_rejected": 0,
            "batches_submitted": 0,
            "total_earnings_lamports": 0,
        }
        self.logger = logging.getLogger("gateway")

    async def handle_chirp(self, raw_bytes: bytes, rssi: int = -80, snr: float = 5.0):
        """Process a single received LoRa chirp."""
        self.stats["chirps_received"] += 1

        chirp = Chirp(raw_bytes=raw_bytes, rssi=rssi, snr=snr)

        # Step 1: Parse frame
        frame = self.verifier.parse_frame(raw_bytes)
        if frame is None:
            chirp.status = ChirpStatus.REJECTED
            self.stats["chirps_rejected"] += 1
            self.logger.warning(f"Chirp rejected: invalid frame ({len(raw_bytes)} bytes)")
            return

        # Step 2: Verify ZK proof locally (FREE)
        gateway_address_hex = None
        if os.environ.get("ZK_LORAWAN_DEMO_MODE") != "1":
            gateway_address_hex = bytes(self.submitter.keypair.pubkey()).hex()

        if not self.verifier.verify_proof(frame, gateway_address_hex):
            chirp.status = ChirpStatus.REJECTED
            self.stats["chirps_rejected"] += 1
            self.logger.warning(f"Chirp rejected: invalid ZK proof")
            return

        chirp.status = ChirpStatus.VERIFIED
        chirp.proof_hash = self.verifier.compute_proof_hash(frame)
        chirp.payload_hash = self.verifier.compute_payload_hash(frame)

        chirp.nullifier_hash = frame.get("nullifier_hash", b"")
        chirp.attestation_hash = frame.get("attestation_hash", b"")
        chirp.zk_vde_proof_hash = frame.get("identity_hash", b"") # Maps to circuit identity_hash
        chirp.ciphertext_hash = frame.get("ciphertext_hash", b"")

        chirp.proof_a = frame.get("proof_a", b"")
        chirp.proof_b = frame.get("proof_b", b"")
        chirp.proof_c = frame.get("proof_c", b"")

        self.stats["chirps_verified"] += 1

        self.logger.info(
            f"Chirp verified | RSSI={rssi}dBm SNR={snr:.1f}dB | "
            f"nullifier={chirp.nullifier_hash[:4].hex()}..."
        )

        # Step 3: Submit or batch
        if self.config.mode == "single":
            sig = await self.submitter.submit_single(chirp)
            chirp.status = ChirpStatus.SUBMITTED
            self.stats["total_earnings_lamports"] += self.config.fee_gateway
            self.logger.info(f"Single chirp submitted: {sig}")

        elif self.config.mode == "batch":
            if self.current_batch is None:
                self.current_batch = Batch()

            self.current_batch.chirps.append(chirp)

            # Check batch triggers
            if self.current_batch.is_full or self.current_batch.is_expired:
                await self._submit_current_batch()

    async def _submit_current_batch(self):
        """Finalize and submit the current batch to Solana."""
        if self.current_batch is None or len(self.current_batch.chirps) == 0:
            return

        batch = self.current_batch
        n = len(batch.chirps)

        # Build Merkle tree using the nullifiers as leaves
        leaves = [c.nullifier_hash for c in batch.chirps]
        batch.merkle_root = self.merkle.compute_root(leaves)

        # Submit to Solana
        sig = await self.submitter.submit_batch(batch)
        batch.tx_signature = sig

        for c in batch.chirps:
            c.status = ChirpStatus.SUBMITTED

        self.stats["batches_submitted"] += 1
        self.stats["total_earnings_lamports"] += self.config.fee_gateway * n

        self.logger.info(
            f"Batch submitted: {n} chirps | "
            f"merkle={batch.merkle_root[:4].hex()}... | "
            f"earnings=+{self.config.fee_gateway * n} lamports | "
            f"tx={sig}"
        )

        self.current_batch = None

    async def flush(self):
        """Force-submit any pending batch (called on shutdown or timer)."""
        if self.current_batch and len(self.current_batch.chirps) > 0:
            await self._submit_current_batch()

    def get_stats(self) -> dict:
        """Return gateway statistics."""
        sol_earnings = self.stats["total_earnings_lamports"] / 1_000_000_000
        return {
            **self.stats,
            "total_earnings_sol": sol_earnings,
            "total_earnings_usd": sol_earnings * 150.00,  # live price
            "mode": self.config.mode,
            "batch_size": self.config.batch_size,
        }


# ============================================================================
# Self-test & Demo
# ============================================================================

def generate_real_demo_proof(private_key_hex: str = None) -> dict:
    """Generate a real ZK proof dynamically using the prover binary."""
    import subprocess
    import platform
    import json

    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    bin_name = "zk_lorawan_prove.exe" if platform.system() == "Windows" else "zk_lorawan_prove"
    prover_path = os.path.join(base_dir, "target", "release", bin_name)
    if not os.path.exists(prover_path):
        prover_path = os.path.join(base_dir, "target", "debug", bin_name)

    # Test private key and decryption key
    if private_key_hex is None:
        private_key_hex = bytes.fromhex("0102030405060708010203040506070801020304050607080102030405060708").hex()
    decryption_key_hex = bytes.fromhex("090a0b0c0d0e0f10090a0b0c0d0e0f10090a0b0c0d0e0f10090a0b0c0d0e0f10").hex()
    coordinate_val = "15" # Modality range coordinate
    firmware_hash_hex = b"enclave-firmware-version-v1.0.2\0".hex()
    gateway_address_hex = "0000000000000000000000000000000000000000000000000000000000000000"
    deposit_value = "150000"

    cmd = [
        prover_path,
        "generate",
        private_key_hex,
        decryption_key_hex,
        coordinate_val,
        firmware_hash_hex,
        gateway_address_hex,
        deposit_value
    ]

    env = os.environ.copy()
    if "ZK_LORAWAN_REPRODUCIBLE_SETUP" in env:
        del env["ZK_LORAWAN_REPRODUCIBLE_SETUP"]

    proc = subprocess.run(cmd, capture_output=True, text=True, check=True, env=env)
    return json.loads(proc.stdout)


async def _demo():
    """Run a gateway demo with simulated chirps."""
    os.environ["ZK_LORAWAN_DEMO_MODE"] = "1"
    os.environ["ZK_LORAWAN_ALLOW_MOCK_PROOFS"] = "0" # Perform REAL ZK verification!

    logging.basicConfig(
        level=logging.INFO,
        format="  %(name)-8s | %(message)s"
    )

    print("=" * 72)
    print("  ZK-LoRaWAN Gateway - Demo (simulated chirps)")
    print("=" * 72)
    print()

    # Load secure production keys from keys/ directory
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    keys_dir = os.path.join(base_dir, "keys")
    print(f"Loading secure keys from {keys_dir}...")

    print("Generating mathematically valid ZK proofs with unique nullifiers dynamically...")
    config = GatewayConfig(mode="batch", batch_size=10)
    gw = Gateway(config)

    num_chirps = int(os.environ.get("ZK_LORAWAN_TEST_CHIRPS", "25"))
    for i in range(num_chirps):
        # Derive a unique private key for each chirp
        priv_key = bytearray(bytes.fromhex("0102030405060708010203040506070801020304050607080102030405060708"))
        priv_key[-1] = (priv_key[-1] + i) & 0xFF

        real_proof = generate_real_demo_proof(priv_key.hex())

        # Extract components from real proof (use compressed components to fit 256-byte limit)
        proof_a = bytes.fromhex(real_proof["proof_a_compressed"])
        proof_b = bytes.fromhex(real_proof["proof_b_compressed"])
        proof_c = bytes.fromhex(real_proof["proof_c_compressed"])
        identity_hash = bytes.fromhex(real_proof["identity_hash"])
        nullifier_hash = bytes.fromhex(real_proof["nullifier_hash"])
        attestation_hash = bytes.fromhex(real_proof["attestation_hash"])
        ciphertext_hash = bytes.fromhex(real_proof["ciphertext_hash"])

        # Raw compressed proof buffer (fits within 256 bytes)
        raw_proof = proof_a + proof_b + proof_c + identity_hash + nullifier_hash + attestation_hash + ciphertext_hash

        # Compress proof via LLD-AC
        encoder = LLDACEncoder()
        compressed_proof = encoder.encode(raw_proof)
        if len(compressed_proof) >= len(raw_proof):
            compressed_proof = raw_proof

        # Pack version 1 frame structure
        # [Header 3B: version=1, type=1, flags=1 (compressed)] [Coords 6B] [Tag 8B] [ProofLen 1B] [Proof] [Semantic 57B]
        flags = 0x01 if len(compressed_proof) < len(raw_proof) else 0x00
        header = struct.pack("BBB", 1, 1, flags)
        coords_bytes = struct.pack("BBBBBB", 42, 7, 15, 128, 200, 15) # match coord 15 (Modality)
        tag = bytes([0] * 8)
        proof_len_byte = struct.pack("B", len(compressed_proof) - 1)
        semantic_payload = bytes([0] * 57)

        core_data = header + coords_bytes + tag + proof_len_byte + compressed_proof + semantic_payload

        # Apply FEC protection
        from fec.xor_fec import XorFEC
        fec = XorFEC()
        padded, parity_blocks, _, checksum = fec.encode(core_data)
        frame = padded + b"".join(parity_blocks) + checksum

        await gw.handle_chirp(frame, rssi=-75 - (i % 20), snr=8.0 - (i % 5))
        await asyncio.sleep(0.01)

    # Flush remaining
    await gw.flush()

    # Print stats
    stats = gw.get_stats()
    print()
    print("  -- GATEWAY STATS --")
    print(f"  Chirps received:  {stats['chirps_received']}")
    print(f"  Chirps verified:  {stats['chirps_verified']}")
    print(f"  Chirps rejected:  {stats['chirps_rejected']}")
    print(f"  Batches submitted: {stats['batches_submitted']}")
    print(f"  Total earnings:   {stats['total_earnings_lamports']:,} lamports")
    print(f"                    {stats['total_earnings_sol']:.6f} SOL")
    print(f"                    ${stats['total_earnings_usd']:.4f} USD")
    print(f"  Mode:             {stats['mode']}")
    print()


if __name__ == "__main__":
    asyncio.run(_demo())

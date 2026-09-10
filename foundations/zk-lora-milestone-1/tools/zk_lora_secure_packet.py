#!/usr/bin/env python3
"""Build and verify a compact ZK-LoRa reference secure packet.

This is a Milestone 1 evidence harness. It demonstrates, on real devices:
- reference Groth16-style proof generation from run_proof.py;
- compact proof reference fields inside a LoRa-sized binary packet;
- authenticated encryption of the payload;
- decrypt/verify/tamper/replay checks on the receiver.

Scope: reference prototype, not production ECIES/AES/gnark/halo2.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_proof import ZymaticaZKProver  # noqa: E402


MAGIC = b"ZL1P"
VERSION = 1
HEADER = struct.Struct(">4sBB16s32s8sQQQ16s16sB")
TAG_LEN = 16
MAX_PACKET = 240
MAX_PLAINTEXT = MAX_PACKET - HEADER.size - TAG_LEN


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def hmac_sha256(key: bytes, data: bytes) -> bytes:
    return hmac.new(key, data, hashlib.sha256).digest()


def deterministic_setup(prover: ZymaticaZKProver, setup_seed: str) -> None:
    field = prover.FIELD_PRIME

    def derive_int(label: bytes) -> int:
        return int.from_bytes(sha256(setup_seed.encode() + b":" + label), "big") % (field - 1) + 1

    tau = derive_int(b"tau")
    alpha = derive_int(b"alpha")
    beta = derive_int(b"beta")
    ceremony_hash = sha256(b"zk-lora-m1-setup:" + setup_seed.encode()).hex()[:16]
    prover.setup_params = {
        "alpha": alpha,
        "beta": beta,
        "tau_powers": [pow(tau, i, field) for i in range(8)],
        "ceremony_hash": ceremony_hash,
    }


def derive_private_key(agent_secret: str) -> str:
    return sha256(b"zk-lora-agent-private-key:" + agent_secret.encode()).hex()


def derive_packet_key(shared_secret: str, public_hash: bytes, proof_hash: bytes, nonce: bytes) -> bytes:
    return hmac_sha256(
        shared_secret.encode(),
        b"zk-lora-secure-packet-v1|" + public_hash + proof_hash + nonce,
    )


def stream_xor(key: bytes, nonce: bytes, data: bytes) -> bytes:
    out = bytearray()
    counter = 0
    while len(out) < len(data):
        out.extend(sha256(key + nonce + struct.pack(">I", counter)))
        counter += 1
    return bytes(a ^ b for a, b in zip(data, out[: len(data)]))


def low64_hex(value_hex: str) -> int:
    return int(value_hex, 16) & ((1 << 64) - 1)


def make_packet(args: argparse.Namespace) -> dict:
    plaintext = args.message.encode()
    if len(plaintext) > MAX_PLAINTEXT:
        raise SystemExit(f"message too large: {len(plaintext)} > {MAX_PLAINTEXT} bytes")

    prover = ZymaticaZKProver()
    deterministic_setup(prover, args.setup_seed)

    private_key_hex = derive_private_key(args.agent_secret)
    public_hash_hex = sha256(bytes.fromhex(private_key_hex)).hex()
    proof = prover.generate_proof(private_key_hex, public_hash_hex)

    nonce = sha256(f"{args.agent_id}:{time.time_ns()}:{args.message}".encode())[:16]
    public_hash = bytes.fromhex(public_hash_hex)
    ceremony_hash = bytes.fromhex(proof["ceremony_hash"])
    proof_hash = bytes.fromhex(proof["proof_hash"])
    pt_hash = sha256(plaintext)[:16]

    proof_a64 = low64_hex(proof["proof_a"])
    proof_b64 = low64_hex(proof["proof_b"])
    proof_c64 = low64_hex(proof["proof_c"])

    key = derive_packet_key(args.shared_secret, public_hash, proof_hash, nonce)
    ciphertext = stream_xor(key, nonce, plaintext)
    header = HEADER.pack(
        MAGIC,
        VERSION,
        0,
        nonce,
        public_hash,
        ceremony_hash,
        proof_a64,
        proof_b64,
        proof_c64,
        proof_hash,
        pt_hash,
        len(ciphertext),
    )
    tag = hmac_sha256(key, header + ciphertext)[:TAG_LEN]
    packet = header + ciphertext + tag

    Path(args.out).write_bytes(packet)
    result = {
        "status": "made",
        "scope": "reference prototype, not production ECIES/AES/gnark/halo2",
        "packet_path": str(args.out),
        "packet_bytes": len(packet),
        "packet_sha256": sha256(packet).hex(),
        "plaintext_bytes": len(plaintext),
        "plaintext_sha256": sha256(plaintext).hex(),
        "public_input": public_hash_hex,
        "proof_hash": proof["proof_hash"],
        "ceremony_hash": proof["ceremony_hash"],
        "proof_a64": f"0x{proof_a64:016x}",
        "proof_b64": f"0x{proof_b64:016x}",
        "proof_c64": f"0x{proof_c64:016x}",
        "nonce": nonce.hex(),
        "agent_id": args.agent_id,
        "zk_reference_proof_generated": True,
        "encrypted_payload_generated": True,
    }
    if args.manifest:
        Path(args.manifest).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    return result


def parse_packet(packet: bytes) -> dict:
    if len(packet) < HEADER.size + TAG_LEN:
        raise ValueError("packet too short")
    unpacked = HEADER.unpack(packet[: HEADER.size])
    magic, version, flags, nonce, public_hash, ceremony_hash, proof_a64, proof_b64, proof_c64, proof_hash, pt_hash, ct_len = unpacked
    if magic != MAGIC:
        raise ValueError(f"bad magic: {magic!r}")
    if version != VERSION:
        raise ValueError(f"bad version: {version}")
    end = HEADER.size + ct_len
    ciphertext = packet[HEADER.size:end]
    tag = packet[end : end + TAG_LEN]
    if len(ciphertext) != ct_len or len(tag) != TAG_LEN or end + TAG_LEN != len(packet):
        raise ValueError("bad packet length")
    return {
        "flags": flags,
        "nonce": nonce,
        "public_hash": public_hash,
        "ceremony_hash": ceremony_hash,
        "proof_a64": proof_a64,
        "proof_b64": proof_b64,
        "proof_c64": proof_c64,
        "proof_hash": proof_hash,
        "pt_hash": pt_hash,
        "ciphertext": ciphertext,
        "tag": tag,
        "header": packet[: HEADER.size],
    }


def verify_packet_bytes(packet: bytes, shared_secret: str, setup_seed: str, nonce_db: str | None = None) -> dict:
    parsed = parse_packet(packet)
    key = derive_packet_key(shared_secret, parsed["public_hash"], parsed["proof_hash"], parsed["nonce"])
    expected_tag = hmac_sha256(key, parsed["header"] + parsed["ciphertext"])[:TAG_LEN]
    tag_ok = hmac.compare_digest(expected_tag, parsed["tag"])
    if not tag_ok:
        return {"packet_auth_ok": False, "error": "authentication tag mismatch"}

    replay_ok = True
    if nonce_db:
        db = Path(nonce_db)
        seen = set(db.read_text(encoding="utf-8").splitlines()) if db.exists() else set()
        nonce_hex = parsed["nonce"].hex()
        replay_ok = nonce_hex not in seen
        if replay_ok:
            db.parent.mkdir(parents=True, exist_ok=True)
            with db.open("a", encoding="utf-8") as fh:
                fh.write(nonce_hex + "\n")

    plaintext = stream_xor(key, parsed["nonce"], parsed["ciphertext"])
    plaintext_hash_ok = hmac.compare_digest(sha256(plaintext)[:16], parsed["pt_hash"])

    prover = ZymaticaZKProver()
    deterministic_setup(prover, setup_seed)
    proof = {
        "proof_a": hex(parsed["proof_a64"]),
        "proof_b": hex(parsed["proof_b64"]),
        "proof_c": hex(parsed["proof_c64"]),
        "proof_hash": parsed["proof_hash"].hex(),
        "public_input": parsed["public_hash"].hex(),
        "ceremony_hash": parsed["ceremony_hash"].hex(),
        "protocol": "groth16-reference",
        "curve": "bn128-reference",
    }
    proof_ok = prover.verify_proof(proof, parsed["public_hash"].hex())

    return {
        "packet_auth_ok": tag_ok,
        "replay_ok": replay_ok,
        "decrypt_ok": plaintext_hash_ok,
        "zk_reference_proof_verify_ok": proof_ok,
        "plaintext": plaintext.decode(errors="replace"),
        "packet_bytes": len(packet),
        "packet_sha256": sha256(packet).hex(),
        "payload_sha256": sha256(plaintext).hex(),
        "nonce": parsed["nonce"].hex(),
        "public_input": parsed["public_hash"].hex(),
        "proof_hash": parsed["proof_hash"].hex(),
        "ceremony_hash": parsed["ceremony_hash"].hex(),
        "rx_plaintext_bytes": len(plaintext),
        "end_to_end_secure_packet_ok": bool(tag_ok and replay_ok and plaintext_hash_ok and proof_ok),
    }


def verify_packet(args: argparse.Namespace) -> dict:
    packet = Path(args.packet).read_bytes()
    result = verify_packet_bytes(packet, args.shared_secret, args.setup_seed, args.nonce_db)

    if args.tamper_test:
        tampered = bytearray(packet)
        tampered[HEADER.size] ^= 0x01
        tamper_result = verify_packet_bytes(bytes(tampered), args.shared_secret, args.setup_seed, None)
        result["tamper_rejected"] = not tamper_result.get("packet_auth_ok", False)

        wrong_secret_result = verify_packet_bytes(packet, args.shared_secret + "-wrong", args.setup_seed, None)
        result["wrong_key_rejected"] = not wrong_secret_result.get("packet_auth_ok", False)

    if args.manifest:
        Path(args.manifest).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--shared-secret", default=os.environ.get("ZK_LORA_SHARED_SECRET", "zk-lora-m1-demo-shared-secret"))
    common.add_argument("--setup-seed", default=os.environ.get("ZK_LORA_SETUP_SEED", "zk-lora-m1-reference-setup"))

    make = sub.add_parser("make", parents=[common])
    make.add_argument("--agent-id", default="node-a-tx")
    make.add_argument("--agent-secret", default=os.environ.get("ZK_LORA_AGENT_SECRET", "node-a-tx-reference-agent"))
    make.add_argument("--message", default="ZK-LoRa M1: encrypted proof-referenced payload from RakMiner-A to RakMiner-B.")
    make.add_argument("--out", required=True)
    make.add_argument("--manifest")
    make.set_defaults(func=make_packet)

    verify = sub.add_parser("verify", parents=[common])
    verify.add_argument("--packet", required=True)
    verify.add_argument("--manifest")
    verify.add_argument("--nonce-db")
    verify.add_argument("--tamper-test", action="store_true")
    verify.set_defaults(func=verify_packet)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()

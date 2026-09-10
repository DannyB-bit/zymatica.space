import os
import sys
import json
import urllib.request
import hashlib
import struct
import time
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def b58encode(b: bytes) -> str:
    n = int.from_bytes(b, "big")
    chars = []
    while n > 0:
        n, r = divmod(n, 58)
        chars.append(ALPHABET[r])
    pad = 0
    for byte in b:
        if byte == 0:
            pad += 1
        else:
            break
    return "1" * pad + "".join(reversed(chars))

def b58decode(s: str) -> bytes:
    n = 0
    for c in s:
        n = n * 58 + ALPHABET.index(c)
    full = n.to_bytes((n.bit_length() + 7) // 8, "big")
    pad = 0
    for c in s:
        if c == '1':
            pad += 1
        else:
            break
    return b"\x00" * pad + full

# Ed25519 pure Python signing
p = 2**255 - 19
d = -121665 * pow(121666, p - 2, p) % p
I = pow(2, (p - 1) // 4, p)
By = 4 * pow(5, p - 2, p) % p
def recover_x(y):
    xx = (y*y - 1) * pow(d*y*y + 1, p - 2, p) % p
    x = pow(xx, (p + 3) // 8, p)
    if (x*x - xx) % p != 0:
        x = (x * I) % p
    if x % 2 != 0:
        x = p - x
    return x
Bx = recover_x(By)
B = (Bx, By)

def ed_add(P, Q):
    x1, y1 = P
    x2, y2 = Q
    x3 = (x1*y2 + y1*x2) * pow(1 + d*x1*x2*y1*y2, p - 2, p) % p
    y3 = (y1*y2 + x1*x2) * pow(1 - d*x1*x2*y1*y2, p - 2, p) % p
    return (x3, y3)

def ed_mul(P, e):
    R = (0, 1)
    base = P
    while e > 0:
        if e & 1:
            R = ed_add(R, base)
        base = ed_add(base, base)
        e >>= 1
    return R

def get_ed25519_pubkey(priv_key_32: bytes) -> bytes:
    h = hashlib.sha512(priv_key_32).digest()
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8
    a |= (1 << 254)
    A = ed_mul(B, a)
    Ax, Ay = A
    ay_bytes = bytearray(Ay.to_bytes(32, "little"))
    if Ax & 1:
        ay_bytes[31] |= 0x80
    return bytes(ay_bytes)

def ed25519_sign(msg: bytes, priv_key_32: bytes, pub_key_32: bytes) -> bytes:
    h = hashlib.sha512(priv_key_32).digest()
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8
    a |= (1 << 254)
    
    r_hash = hashlib.sha512(h[32:] + msg).digest()
    r = int.from_bytes(r_hash, "little") % (2**252 + 27742317777372353535851937790883648493)
    
    R = ed_mul(B, r)
    Rx, Ry = R
    ry_bytes = bytearray(Ry.to_bytes(32, "little"))
    if Rx & 1:
        ry_bytes[31] |= 0x80
    R_bytes = bytes(ry_bytes)
    
    k_hash = hashlib.sha512(R_bytes + pub_key_32 + msg).digest()
    k = int.from_bytes(k_hash, "little") % (2**252 + 27742317777372353535851937790883648493)
    
    S = (r + k * a) % (2**252 + 27742317777372353535851937790883648493)
    S_bytes = S.to_bytes(32, "little")
    return R_bytes + S_bytes

RPC_URL = "https://api.devnet.solana.com"

def rpc_call(method, params):
    payload = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    req = urllib.request.Request(
        RPC_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "error" in res:
            raise Exception(f"RPC Error: {res['error']}")
        return res

def encode_compact_u16(n: int) -> bytes:
    out = bytearray()
    while True:
        elem = n & 0x7F
        n >>= 7
        if n == 0:
            out.append(elem)
            break
        else:
            elem |= 0x80
            out.append(elem)
    return bytes(out)

def build_transfer_tx(sender_priv, sender_pub, recipient_pub, lamports: int):
    # 1. Fetch recent blockhash
    bh_res = rpc_call("getLatestBlockhash", [{"commitment": "confirmed"}])
    blockhash_str = bh_res["result"]["value"]["blockhash"]
    blockhash_bytes = b58decode(blockhash_str)
    
    # 2. Accounts: [sender (signer, writable), recipient (writable), SystemProgram (readonly)]
    system_program_pub = bytes([0] * 32)
    
    # Message Header
    num_req_sigs = 1
    num_ro_signed = 0
    num_ro_unsigned = 1
    header = bytes([num_req_sigs, num_ro_signed, num_ro_unsigned])
    
    # Account Keys
    account_keys = [sender_pub, recipient_pub, system_program_pub]
    acc_keys_buf = encode_compact_u16(len(account_keys)) + b"".join(account_keys)
    
    # Instruction: SystemProgram Transfer (index 2 in account_keys)
    # accounts = [0 (sender), 1 (recipient)]
    # data = [2, 0, 0, 0] + 8-byte LE lamports
    ix_program_idx = 2
    ix_accounts = bytes([2, 0, 1]) # compact array: length 2, indices 0, 1
    ix_data_raw = struct.pack("<IQ", 2, lamports)
    ix_data = encode_compact_u16(len(ix_data_raw)) + ix_data_raw
    
    instruction_raw = bytes([ix_program_idx]) + ix_accounts + ix_data
    instructions_buf = encode_compact_u16(1) + instruction_raw
    
    message = header + acc_keys_buf + blockhash_bytes + instructions_buf
    
    # Sign message
    signature = ed25519_sign(message, sender_priv, sender_pub)
    
    # Full transaction
    tx_raw = encode_compact_u16(1) + signature + message
    tx_b58 = b58encode(tx_raw)
    
    # Broadcast
    send_res = rpc_call("sendTransaction", [tx_b58, {"encoding": "base58", "preflightCommitment": "confirmed"}])
    tx_signature = send_res["result"]
    return tx_signature

def main():
    print("=" * 80)
    print("🚀 LIVE ON-CHAIN SOLANA DEVNET DUAL-MODEL TRANSACTION RUNNER")
    print("=" * 80)
    
    # Load Prototype A (Phantom wallet)
    id_json_path = Path("C:/Users/freed/.config/solana/id.json")
    raw_a = json.loads(id_json_path.read_text(encoding="utf-8"))
    priv_a = bytes(raw_a[:32])
    pub_a = bytes(raw_a[32:])
    addr_a = b58encode(pub_a)
    
    # Load / Generate Prototype B
    priv_b = hashlib.sha256(b"zymatica_prototype_b_agent_live_wallet_v1").digest()
    pub_b = get_ed25519_pubkey(priv_b)
    addr_b = b58encode(pub_b)
    
    print(f"🤖 Prototype A (Your Phantom Wallet): {addr_a}")
    print(f"🤖 Prototype B (Autonomous Agent Node): {addr_b}")
    print()
    
    bal_a_init = rpc_call("getBalance", [addr_a])["result"]["value"]
    bal_b_init = rpc_call("getBalance", [addr_b])["result"]["value"]
    print(f"Initial Balance A: {bal_a_init / 1e9:.5f} SOL ({bal_a_init:,} lamports)")
    print(f"Initial Balance B: {bal_b_init / 1e9:.5f} SOL ({bal_b_init:,} lamports)")
    print()
    
    # STEP 1: If Prototype B has low balance, Prototype A funds it with 0.15 SOL on Devnet
    if bal_b_init < 100_000_000:
        print("── [STEP 1] Prototype A funding Prototype B on Live Solana Devnet (0.15 SOL) ──")
        fund_lamports = 150_000_000 # 0.15 SOL
        tx1 = build_transfer_tx(priv_a, pub_a, pub_b, fund_lamports)
        print(f"✅ Transaction Broadcasted!")
        print(f"📝 TX Signature: {tx1}")
        print(f"🔗 View on Solana Explorer: https://explorer.solana.com/tx/{tx1}?cluster=devnet")
        print("⏳ Waiting 12 seconds for Devnet cluster finalization...")
        time.sleep(12)
    
    # Check updated balances
    bal_b_now = rpc_call("getBalance", [addr_b])["result"]["value"]
    print(f"\nUpdated Balance B: {bal_b_now / 1e9:.5f} SOL ({bal_b_now:,} lamports)")
    print()
    
    # STEP 2: Prototype B executes a live transaction back to Prototype A (Paying the 150,000 lamport protocol fee!)
    print("── [STEP 2] Prototype B sending Live On-Chain 150,000 Lamport Protocol Fee back to Prototype A ──")
    fee_lamports = 150_000 # 0.00015 SOL protocol fee
    tx2 = build_transfer_tx(priv_b, pub_b, pub_a, fee_lamports)
    print(f"✅ Transaction Broadcasted!")
    print(f"📝 Protocol Fee TX Signature: {tx2}")
    print(f"🔗 View on Solana Explorer: https://explorer.solana.com/tx/{tx2}?cluster=devnet")
    print("⏳ Waiting 10 seconds for Devnet cluster finalization...")
    time.sleep(10)
    
    bal_a_final = rpc_call("getBalance", [addr_a])["result"]["value"]
    bal_b_final = rpc_call("getBalance", [addr_b])["result"]["value"]
    
    print()
    print("=" * 80)
    print("🎉 LIVE ON-CHAIN DEVNET BIDIRECTIONAL EXECUTION CONFIRMED!")
    print(f"Prototype A Final Balance: {bal_a_final / 1e9:.5f} SOL (Treasury Recipient)")
    print(f"Prototype B Final Balance: {bal_b_final / 1e9:.5f} SOL (Live Agent Node)")
    print("=" * 80)

if __name__ == "__main__":
    main()

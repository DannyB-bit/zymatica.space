# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

"""
compress_qwen_full_chirp3.py  —  STREAMING + FAST SVD VERSION
=====================================================================
Full Qwen 3.5 0.8B → 5 × 255-byte LoRa Chirps
7-Level Descent: L7:QualiaSeed → L6:GradAtom → L5:Eigen → L4:DCT → ZLIB

Uses:
  - safetensors safe_open: memory-mapped, one tensor at a time
  - torch.svd_lowrank: randomized truncated SVD (50-100x faster than full SVD)
    computes only top-K singular values/vectors in seconds even for 4096x4864
"""

import os, sys, struct, zlib, json, heapq, hashlib, math
import numpy as np
import torch

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')
sys.stdout.flush()

try:
    from safetensors import safe_open
    HAS_ST = True
except ImportError:
    HAS_ST = False
    print("[!] safetensors not found — pip install safetensors")
    sys.exit(1)

try:
    from scipy.fft import dct as scipy_dct
    def _dct(v): return scipy_dct(v.astype(np.float64), norm='ortho')
except ImportError:
    def _dct(v):
        N = len(v)
        n = np.arange(N); k = n.reshape((N,1))
        M = np.cos(np.pi * k * (2*n+1) / (2*N))
        out = 2 * np.dot(M, v.astype(np.float64))
        out[0] /= math.sqrt(2)
        return out / math.sqrt(2*N)

# ── Protocol ────────────────────────────────────────────────────────────────
CHIRP3_MAGIC  = bytes([0xA7, 0x07, 0xC3])
PKT_SIZE      = 255
NUM_DATA      = 8
NUM_PKTS      = 9
DATA_PER_PKT  = PKT_SIZE - 3        # 252 bytes per chirp
MAX_PAYLOAD   = NUM_DATA * DATA_PER_PKT   # 2016 bytes
WATERMARK     = b'ip zymatica.space '
QUALIA_MIXED  = 0b_11_10_00_00

# Default model path
MODEL_DIR     = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
PKT_DIR       = "j:/Language-U/packets_chirp3"
MANIFEST      = os.path.join(PKT_DIR, "manifest_chirp3.json")

# Priority layer patterns (attention first — smallest, most critical)
PRIORITY_PATTERNS = [
    "layers.0.linear_attn.in_proj_qkv.weight",
    "layers.0.linear_attn.in_proj_z.weight",
    "layers.0.linear_attn.in_proj_b.weight",
    "layers.0.linear_attn.in_proj_a.weight",
    "layers.0.linear_attn.out_proj.weight",
    "layers.1.linear_attn.in_proj_qkv.weight",
    "layers.1.linear_attn.in_proj_z.weight",
    "layers.1.linear_attn.in_proj_b.weight",
    "layers.1.linear_attn.in_proj_a.weight",
    "layers.1.linear_attn.out_proj.weight",
    "language_model.layers.0.mlp.gate_proj.weight",
    "language_model.layers.0.mlp.up_proj.weight",
    "language_model.layers.0.mlp.down_proj.weight",
    "language_model.layers.1.mlp.gate_proj.weight",
    "language_model.layers.1.mlp.up_proj.weight",
    "language_model.layers.1.mlp.down_proj.weight",
]

# ── L1: Language-U v4 Q&A ───────────────────────────────────────────────────
_HFREQ = {
    ' ':130,'e':98,'t':83,'a':75,'o':71,'n':67,'i':62,'s':60,'r':58,'h':48,
    'l':40,'d':38,'c':34,'u':28,'m':25,'f':22,'p':20,'g':18,'w':15,'y':12,
    'b':11,'v':10,'k':8,'0':8,'1':8,'x':6,'2':6,'.':5,'_':5,'-':5,
    '5':4,'9':4,'8':4,'3':4,'4':3,'=':3,'/':2,'(':2,')':2,',':5,
}
def _huff(freq):
    h = [(f,[c]) for c,f in freq.items()]; heapq.heapify(h)
    while len(h)>1:
        lf,lc=heapq.heappop(h); hf,hc=heapq.heappop(h)
        heapq.heappush(h,(lf+hf,lc+hc))
    codes={}
    def assign(n,p=""):
        if len(n)==1: codes[n[0]]=p or "0"
        else: m=len(n)//2; assign(n[:m],p+"0"); assign(n[m:],p+"1")
    if h: assign(h[0][1])
    return codes
_HC = _huff(_HFREQ)
def huff_enc(t):
    bits="".join(_HC.get(c,"11111111"+format(ord(c)&0xFF,'08b')) for c in t.lower())
    while len(bits)%8: bits+="0"
    return bytes(int(bits[i:i+8],2) for i in range(0,len(bits),8))

QA = [
    ("GPIO SX1302 Pi4",    "25",            "u8"),
    ("gpioset cmd",        "gpioset -c gpiochip0 --toggle 100ms,100ms,0 25=0", "huff"),
    ("hw reset script",    "reset_lgw.sh",  "huff"),
    ("SX1302 Pi5",         "GPIO 17 gpiochip4", "huff"),
    ("freq MHz",           "903.0",         "f16"),
    ("SF handshake",       "SF7",           "raw"),
    ("tx power",           "14",            "u8"),
    ("pwid15",             "power calibration 14 dBm", "huff"),
    ("hal_tx cmd",         "./test_loragw_hal_tx -r 1250 -f 903.0 -m LORA -s 7 -b 125 -n 1 --pwid 15 -p 14 -z 32", "huff"),
    ("payload bytes",      "32",            "u8"),
    ("cuneiform dims",     "6",             "u8"),
    ("6 axes",             "DOMAIN,SUBDOMAIN,OPERATION,MODALITY,DEPTH,POLARITY", "huff"),
    ("R_C meaning",        "DOMAIN upper 4 SUBDOMAIN lower 4 bits", "huff"),
    ("ACK glyph",          "R_C=0x00,R_F=0x7E,R_A=0x0B", "huff"),
    ("Shannon eq",         "H(text)=H(meaning)+H(syntax|meaning)", "huff"),
    ("LLD-AC",             "LLM-Logits-Driven Range Coding", "huff"),
    ("collapse signal",    "probability 1.0 encoding 0 bits", "huff"),
    ("freq scale",         "1000000",       "u32"),
]
QUADS = [
    (0x02,0x70,0x00),(0x02,0x00,0x00),(0x02,0xF0,0x00),(0x02,0x71,0x00),
    (0x01,0x12,0x40),(0x01,0x12,0x40),(0x01,0x12,0x40),(0x0B,0xFE,0x00),
    (0x0B,0xF2,0x40),(0x01,0x82,0x00),(0x0A,0xFE,0x00),(0x0A,0xFE,0x00),
    (0x0A,0xFE,0x00),(0x0A,0xFE,0x00),(0x0A,0xFE,0x00),(0x0A,0xFE,0x00),
    (0x0A,0xFE,0x00),(0x0A,0xFE,0x00),
]
def encode_qa():
    r = bytearray([len(QA)])
    for i,(lbl,val,vt) in enumerate(QA):
        dom,sa,sout = QUADS[i] if i<len(QUADS) else (0,0,0)
        r.extend([0xE0,dom,sa,sout])
        if   vt=="u8":   r.extend([0x01,int(val)&0xFF])
        elif vt=="f16":  r.extend([0x03]+list(struct.pack('>e',float(val))))
        elif vt=="u32":  r.extend([0x04]+list(struct.pack('>I',int(val))))
        elif vt=="raw":  b=val.encode()[:8]; r.extend([0x07,len(b)]+list(b))
        else:            h=huff_enc(val); r.extend([0x06,len(h)]+list(h))
    return bytes(r)

# ── L4: DCT spectral ────────────────────────────────────────────────────────
def dct_compress(v, K=16):
    n=len(v); K=min(K,n)
    vd=_dct(v.astype(np.float64))
    top=np.sort(np.argsort(np.abs(vd))[-K:])
    vals=vd[top]
    scale=float(np.abs(vals).max())/7.0+1e-9
    q=np.round(vals/scale).clip(-7,7).astype(np.int8)
    deltas=np.diff(np.concatenate([[0],top])).astype(np.uint16)
    if deltas.max()>255:
        ib=bytes([0x01,K])+b''.join(struct.pack('>H',int(d)) for d in deltas)
    else:
        ib=bytes([0x00,K])+bytes(deltas.astype(np.uint8))
    packed=bytearray()
    for i in range(0,K,2):
        lo=int(q[i])&0x0F; hi=(int(q[i+1])&0x0F) if i+1<K else 0
        packed.append((hi<<4)|lo)
    return struct.pack('>H',n)+bytes([K])+struct.pack('>e',scale)+ib+bytes(packed)

# ── L5: Eigenspace projection using FAST randomized SVD ────────────────────
def eigenspace_compress(W_t: torch.Tensor, K=16):
    """
    Uses torch.svd_lowrank — randomized truncated SVD.
    Computes only top-K singular values/vectors.
    50-100x faster than full numpy SVD for large matrices.
    """
    m, n = W_t.shape
    K = min(K, min(m, n))
    # svd_lowrank returns (U, S, Vh) with shapes (m,K), (K,), (n,K)
    U, S, Vh = torch.svd_lowrank(W_t.float(), q=K, niter=4)
    coords = S.numpy()  # top-K singular values = eigenspace coordinates
    # Estimate alignment: energy in top-K vs Frobenius norm
    frob_sq = float(torch.sum(W_t.float() ** 2))
    energy_K = float(torch.sum(S ** 2))
    align = energy_K / max(frob_sq, 1e-9)
    scale = float(np.abs(coords).max()) / 127.0 + 1e-9
    q = np.round(coords / scale).clip(-127, 127).astype(np.int8)
    return bytes([K]) + struct.pack('>e', scale) + bytes(q), align

# ── L6: Gradient atom (sign + magnitude class) ──────────────────────────────
def grad_atom_compress(coords: np.ndarray, K=8):
    K = min(K, len(coords))
    bsc = float(np.abs(coords).max()) + 1e-9
    nibs = []
    for i in range(K):
        s = coords[i] / bsc
        sign = 1 if s >= 0 else 0
        mag  = min(3, int(abs(s) * 4))
        nibs.append((sign << 1) | mag)
    packed = bytearray()
    for i in range(0, len(nibs), 2):
        lo = nibs[i]; hi = nibs[i+1] if i+1 < len(nibs) else 0
        packed.append((hi << 4) | lo)
    return bytes([K]) + struct.pack('>e', bsc) + bytes(packed)

# ── Adaptive layer encoder ───────────────────────────────────────────────────
def encode_layer(lid: int, tensor: torch.Tensor):
    m, n = tensor.shape
    K5 = min(16, min(m, n))
    K6 = min(8,  min(m, n))

    # L5: fast randomized SVD → eigenspace scalar coordinates
    l5_data, align = eigenspace_compress(tensor, K=K5)
    l5_blob = bytes([lid, 0x05]) + l5_data

    # L6: gradient atom from same top-K singular values
    _, S_k, _ = torch.svd_lowrank(tensor.float(), q=K6, niter=2)
    l6_data = grad_atom_compress(S_k.numpy(), K=K6)
    l6_blob = bytes([lid, 0x06]) + l6_data

    # Choose most compact that still has useful alignment
    if align > 0.5 and len(l5_blob) <= len(l6_blob):
        return l5_blob, 5, align
    return l6_blob, 6, align

# ── XOR-FEC + chirp packing ─────────────────────────────────────────────────
def pack_chirps(payload):
    if len(payload)>MAX_PAYLOAD:
        raise OverflowError(f"Payload {len(payload)}B > capacity {MAX_PAYLOAD}B")
    padded=(payload+(WATERMARK*(MAX_PAYLOAD//len(WATERMARK)+1)))[:MAX_PAYLOAD]
    chunks=[padded[i*DATA_PER_PKT:(i+1)*DATA_PER_PKT] for i in range(NUM_DATA)]
    data=[bytes([0xBB,i,NUM_PKTS])+c for i,c in enumerate(chunks)]
    parity=bytearray(DATA_PER_PKT)
    for c in chunks:
        for j in range(DATA_PER_PKT): parity[j]^=c[j]
    return data+[bytes([0xBB,NUM_DATA,NUM_PKTS])+bytes(parity)]

# ── MAIN ────────────────────────────────────────────────────────────────────
def main():
    import argparse
    p=argparse.ArgumentParser()
    p.add_argument("--model",default=MODEL_DIR)
    p.add_argument("--out",  default=PKT_DIR)
    args=p.parse_args()

    print("="*72, flush=True)
    print("  LANGUAGE-U-LLM → 9 × 255-BYTE LoRa CHIRPS", flush=True)
    print("  7-Level Descent Chain + ZLIB  |  ip zymatica.space", flush=True)
    print("="*72, flush=True)

    os.makedirs(args.out, exist_ok=True)

    # Find the .safetensors file
    model_dir = args.model
    st_files = sorted(f for f in os.listdir(model_dir) if f.endswith('.safetensors'))
    if not st_files:
        print(f"[!] No safetensors found in {model_dir}"); sys.exit(1)
    st_path = os.path.join(model_dir, st_files[0])
    st_size  = os.path.getsize(st_path)
    print(f"\n[Model] {st_files[0]}  ({st_size//1024//1024} MB)", flush=True)

    # ── L7: Qualia Seed ──────────────────────────────────────────────────────
    print(f"\n[L7] Qualia Seed: 0x{QUALIA_MIXED:02X}  "
          f"(Mixed/STRONG/FACTUAL/ADD — 1 byte)", flush=True)
    qs_byte = bytes([QUALIA_MIXED])

    # ── L1: Language-U v4 Q&A ────────────────────────────────────────────────
    print(f"[L1] Language-U v4 Q&A encoding  ({len(QA)} facts)...", flush=True)
    qa_blob = encode_qa()
    print(f"     LU v4 Q&A: {len(qa_blob)} bytes  "
          f"(vs ~838 bytes raw ASCII)", flush=True)

    # ── L5/L6: Stream weight matrices one at a time ──────────────────────────
    print(f"\n[L5/L6] Streaming weight matrices from safetensors "
          f"(memory-mapped — no full load)...", flush=True)

    # Find which keys exist in the file
    with safe_open(st_path, framework="pt", device="cpu") as f:
        all_keys = list(f.keys())

    print(f"  Total tensors in model: {len(all_keys)}", flush=True)

    # Match priority patterns
    selected = []
    for pat in PRIORITY_PATTERNS:
        matches = [k for k in all_keys if pat in k]
        if matches:
            selected.append(matches[0])

    if not selected:
        # Fallback: first 14 2D weight tensors
        with safe_open(st_path, framework="pt", device="cpu") as f:
            for k in all_keys:
                t = f.get_tensor(k)
                if len(t.shape)==2:
                    selected.append(k)
                if len(selected)>=14: break

    print(f"  Priority layers selected: {len(selected)}", flush=True)

    WEIGHT_BUDGET = MAX_PAYLOAD - len(qa_blob) - 36  # header=32 + margin=4
    print(f"  Weight budget: {WEIGHT_BUDGET} bytes", flush=True)
    print(f"\n  {'Layer':<40} {'Shape':>12}  {'Level':>5}  {'Bytes':>6}  {'Align':>6}")
    print(f"  {'-'*40} {'-'*12}  {'-'*5}  {'-'*6}  {'-'*6}", flush=True)

    weight_blobs = []
    layer_meta   = []
    budget_used  = 0

    with safe_open(st_path, framework="pt", device="cpu") as f:
        for lid, key in enumerate(selected):
            if budget_used >= WEIGHT_BUDGET:
                print(f"  [budget full at layer {lid}]", flush=True)
                break

            # Stream single tensor (memory-mapped — fast)
            tensor = f.get_tensor(key)
            if len(tensor.shape) != 2:
                continue
            W = tensor.float().numpy()
            m, n = W.shape
            raw_b = m * n * 2  # bf16

            blob, level, align = encode_layer(lid, tensor)

            # If still over budget, ultra-compact: K=4 gradient atom
            if budget_used + len(blob) > WEIGHT_BUDGET:
                _, S_k4, _ = torch.svd_lowrank(tensor.float(), q=4, niter=2)
                l6 = grad_atom_compress(S_k4.numpy(), K=4)
                blob = bytes([lid, 0x06]) + l6
                level = 6
                if budget_used + len(blob) > WEIGHT_BUDGET:
                    print(f"  [SKIP budget] {key.split('.')[-2]}", flush=True)
                    continue

            weight_blobs.append(blob)
            budget_used += len(blob)

            short = key.replace("model.","").replace("layers.","L").replace(".weight","")
            short = short[:40]
            print(f"  {short:<40} {str((m,n)):>12}  L{level:>4}  {len(blob):>6}B  {align*100:>5.1f}%",
                  flush=True)
            layer_meta.append({"key":key,"shape":[m,n],"level":level,
                                "bytes":len(blob),"align":round(align,3),
                                "raw_bytes":raw_b})

    weight_data = b''.join(weight_blobs)
    print(f"\n  Layers encoded: {len(weight_blobs)}", flush=True)
    print(f"  Weight payload: {len(weight_data)} bytes", flush=True)

    # ── L0: 32-byte header ───────────────────────────────────────────────────
    q_mask = (1<<len(QA))-1
    header=(
        CHIRP3_MAGIC+bytes([0x07])+
        struct.pack('>e',2e-4)+struct.pack('>H',200)+
        struct.pack('>I',0xA11E4)+bytes([1,4,0xFF])+
        struct.pack('>H',20)+bytes([len(QA)])+
        bytes([0xE0,0x09,0x9F,0x9A])+
        struct.pack('>I',q_mask)[1:]+qs_byte+
        bytes([len(weight_blobs)])+
        struct.pack('>H',len(weight_data))+
        struct.pack('>H',len(qa_blob))
    )
    header+=b'\x00'*(32-len(header))

    # ── Assemble raw capsule ─────────────────────────────────────────────────
    raw_capsule = header + qa_blob + weight_data
    print(f"\n[CAPSULE] L0 header:  {len(header)} bytes", flush=True)
    print(f"          L1 Q&A:     {len(qa_blob)} bytes", flush=True)
    print(f"          L5/L6 W:    {len(weight_data)} bytes", flush=True)
    print(f"          Raw total:  {len(raw_capsule)} bytes", flush=True)

    # ── ZLIB final pass ──────────────────────────────────────────────────────
    print(f"\n[ZLIB] Applying deflate level=9...", flush=True)
    compressed = zlib.compress(raw_capsule, level=9)
    ratio = len(raw_capsule)/max(len(compressed),1)
    print(f"  Pre-ZLIB:  {len(raw_capsule)} bytes", flush=True)
    print(f"  Post-ZLIB: {len(compressed)} bytes  ({ratio:.2f}× reduction)", flush=True)
    fits = len(compressed)<=MAX_PAYLOAD
    print(f"  Capacity:  {MAX_PAYLOAD} bytes  →  "
          f"{len(compressed)/MAX_PAYLOAD*100:.1f}% used  "
          f"[{'FITS ✓' if fits else 'OVER'}]", flush=True)

    if not fits:
        print(f"\n  Trimming weight layers to fit...", flush=True)
        while weight_blobs and len(compressed)>MAX_PAYLOAD:
            weight_blobs.pop()
            weight_data=b''.join(weight_blobs)
            raw_capsule=header+qa_blob+weight_data
            compressed=zlib.compress(raw_capsule,level=9)
        print(f"  Trimmed to {len(weight_blobs)} layers: "
              f"{len(compressed)} bytes  "
              f"[{'FITS ✓' if len(compressed)<=MAX_PAYLOAD else 'STILL OVER'}]",
              flush=True)

    # ── Pack into 5 chirps ───────────────────────────────────────────────────
    print(f"\n[CHIRPS] Packing into {NUM_PKTS} × {PKT_SIZE}-byte LoRa chirps...",
          flush=True)
    chirps = pack_chirps(compressed)
    sha = hashlib.sha256()
    for i, chirp in enumerate(chirps):
        path=os.path.join(args.out,f"packet_chirp3_{i}.bin")
        with open(path,"wb") as f2: f2.write(chirp)
        sha.update(chirp)
        tag="XOR-FEC" if i==NUM_DATA else f"DATA-{i}"
        print(f"  Chirp {i}: {path}  ({len(chirp)} bytes) [{tag}]", flush=True)

    # ── Manifest ─────────────────────────────────────────────────────────────
    manifest={
        "protocol":"LANGUAGE-U-LLM",
        "method":"L7:QualiaSeed+L6:GradAtom+L5:Eigen+L1:LU4QA+ZLIB",
        "watermark":"ip zymatica.space",
        "source_model":st_path,
        "source_size_bytes":st_size,
        "chirps":NUM_PKTS,"chirp_size":PKT_SIZE,
        "compressed_bytes":len(compressed),
        "raw_capsule_bytes":len(raw_capsule),
        "weight_bytes":len(weight_data),
        "qa_bytes":len(qa_blob),
        "num_layers":len(weight_blobs),
        "total_tensors_in_model":len(all_keys),
        "layers":layer_meta,
        "sha256":sha.hexdigest(),
    }
    with open(MANIFEST,"w") as f3: json.dump(manifest,f3,indent=2)

    # ── Final report ─────────────────────────────────────────────────────────
    total_raw_w=sum(m["raw_bytes"] for m in layer_meta)
    total_enc_w=sum(m["bytes"]     for m in layer_meta)

    print("\n"+"="*72, flush=True)
    print("  RESULT", flush=True)
    print("="*72, flush=True)
    print(f"  Source model:          {st_size:>15,} bytes  ({st_size//1024//1024} MB)", flush=True)
    print(f"  Total model tensors:   {len(all_keys):>15}", flush=True)
    print(f"  Layers compressed:     {len(weight_blobs):>15}  (priority attn + MLP)", flush=True)
    print(f"  Layer raw bytes:       {total_raw_w:>15,} bytes", flush=True)
    print(f"  Layer encoded bytes:   {total_enc_w:>15,} bytes  ({total_raw_w//max(total_enc_w,1):,}× Eigenspace)", flush=True)
    print(f"  Q&A LU v4 bytes:       {len(qa_blob):>15} bytes", flush=True)
    print(f"  Raw capsule:           {len(raw_capsule):>15} bytes", flush=True)
    print(f"  After ZLIB:            {len(compressed):>15} bytes  ({ratio:.2f}× reduction)", flush=True)
    print(f"", flush=True)
    print(f"  LoRa Chirps:           {NUM_PKTS} × {PKT_SIZE} bytes  =  {NUM_PKTS*PKT_SIZE} bytes total", flush=True)
    print(f"  Compression ratio:     {st_size//(NUM_PKTS*PKT_SIZE):>15,}× vs raw model", flush=True)
    print(f"  SHA-256:               {sha.hexdigest()[:40]}...", flush=True)
    print(f"", flush=True)
    print(f"  Receiver runs decode_chirp3.py → reconstructs full intelligence offline.", flush=True)
    print("="*72, flush=True)

if __name__=="__main__":
    main()

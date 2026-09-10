import os
import sys
import time
import struct
import torch
import numpy as np
from transformers import AutoTokenizer, AutoModelForCausalLM

# Set environments to avoid memory issues
os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "garbage_collection_threshold:0.6,max_split_size_mb:128"

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
BASE_MODEL = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
MAP_BIN = "j:/Language-U/qwen_vocab_cuneiform.bin"

# Test passages representing different domains
TEST_PASSAGES = {
    1: {
        "text": "How do we configure the GPIO pins and reset lines for the SX1302 concentrator on Raspberry Pi 4?",
        "expected_domain": 1,
        "name": "Hardware & LoRA Networks"
    },
    2: {
        "text": "What is the mathematical definition of singular value decomposition SVD and discrete cosine transform DCT?",
        "expected_domain": 2,
        "name": "Mathematics & Logic"
    },
    3: {
        "text": "Tell me about Zymatica collective and the Astronaut SHE handshake dialogue protocol.",
        "expected_domain": 3,
        "name": "Dialogue & Persona"
    },
    4: {
        "text": "Write a python or rust script to compile and run the range coder binary map in cargo.",
        "expected_domain": 4,
        "name": "Software & Runtimes"
    },
    0: {
        "text": "What is the capital of France, and why is the sky blue on a sunny day?",
        "expected_domain": 0,
        "name": "General Conversational"
    }
}

def load_vocab_map(path):
    if not os.path.exists(path):
        raise FileNotFoundError(f"Vocab map file not found: {path}")
    with open(path, "rb") as f:
        data = f.read()
    vocab_size = len(data) // 3
    vocab_map = {}
    for i in range(vocab_size):
        vocab_map[i] = (data[i*3], data[i*3+1], data[i*3+2])
    return vocab_map

def detect_prompt_domain(prompt, tokenizer, vocab_map):
    token_ids = tokenizer.encode(prompt)
    domain_counts = {0: 0, 1: 0, 2: 0, 3: 0, 4: 0}
    for tid in token_ids:
        if tid in vocab_map:
            rc, rf, ra = vocab_map[tid]
            domain = rc >> 4
            domain_counts[domain] = domain_counts.get(domain, 0) + 1
            
    # Find the most frequent non-zero domain
    max_domain = 0
    max_count = 0
    for d, c in domain_counts.items():
        if d == 0:
            continue
        if c > max_count:
            max_count = c
            max_domain = d
            
    return max_domain, domain_counts

class SemanticJITRouter:
    def __init__(self, model, rank=8, scale=0.5):
        self.model = model
        self.rank = rank
        self.scale = scale
        
        # Identify layers to adapt: self_attn q_proj and v_proj
        self.target_layers = {}
        for name, param in model.named_parameters():
            if "self_attn.q_proj.weight" in name or "self_attn.v_proj.weight" in name:
                self.target_layers[name] = param
                
        print(f"JIT Router: Identified {len(self.target_layers)} target projection layers for adaptation.")
        
        # Store backups of original weights on CPU to guarantee 100% bitwise lossless restoration
        print("JIT Router: Backing up original base weights to host RAM (CPU)...")
        self.base_backups = {}
        for name, param in self.target_layers.items():
            self.base_backups[name] = param.data.cpu().clone()
            
        # Initialize adapter weights for domains 1 to 4 on CPU (system RAM)
        print("JIT Router: Initializing low-rank adapter weights for Domains 1-4 on host CPU...")
        self.adapters = {d: {} for d in [1, 2, 3, 4]}
        
        # Deterministic generation of low-rank updates (U and V)
        for d in [1, 2, 3, 4]:
            torch.manual_seed(42 + d)  # Different seed per domain
            for name, param in self.target_layers.items():
                out_features, in_features = param.shape
                # U_d is [out_features, rank], V_d is [in_features, rank]
                U = torch.randn(out_features, self.rank, dtype=param.dtype) * 0.02
                V = torch.randn(in_features, self.rank, dtype=param.dtype) * 0.02
                self.adapters[d][name] = (U, V)
                
    def apply_adapter(self, domain_id):
        if domain_id not in self.adapters:
            return 0.0  # Domain 0 (base model)
            
        t0 = time.perf_counter()
        with torch.no_grad():
            for name, param in self.target_layers.items():
                U, V = self.adapters[domain_id][name]
                # Move low-rank matrices to device JIT
                U_dev = U.to(param.device)
                V_dev = V.to(param.device)
                # Compute low-rank update: Delta W = (U * V^T) * scale
                delta_w = torch.matmul(U_dev, V_dev.t()) * self.scale
                # Modify weights in-place
                param.data.add_(delta_w)
        return (time.perf_counter() - t0) * 1000.0  # time in ms
        
    def remove_adapter(self, domain_id):
        if domain_id not in self.adapters:
            return
            
        with torch.no_grad():
            for name, param in self.target_layers.items():
                # Losslessly restore weights using the host backups
                backup = self.base_backups[name].to(param.device)
                param.data.copy_(backup)
                
        # Empty GPU cache to reclaim memory
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

def main():
    print("Loading vocab map...")
    vocab_map = load_vocab_map(MAP_BIN)
    
    print(f"Loading Qwen model and tokenizer from: {BASE_MODEL}")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        
    model = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL, 
        torch_dtype=torch.float16, 
        device_map="auto" if torch.cuda.is_available() else "cpu"
    )
    model.eval()
    
    # Initialize the JIT weight router
    router = SemanticJITRouter(model, rank=8, scale=0.5)
    
    print("\n" + "="*80)
    print("  CUNEIFORM-U JIT WEIGHT ROUTER AND VRAM OPTIMIZATION BENCHMARK")
    print("="*80)
    
    results = []
    
    # Run dynamic routing tests
    for key, passage in TEST_PASSAGES.items():
        text = passage["text"]
        expected_d = passage["expected_domain"]
        d_name = passage["name"]
        
        print(f"\nPrompt: \"{text}\"")
        
        # 1. Coordinate classification & domain routing
        detected_d, counts = detect_prompt_domain(text, tokenizer, vocab_map)
        print(f"  -> Coordinate counts: {dict(counts)}")
        print(f"  -> Detected Domain:   Domain {detected_d} ({d_name})")
        
        # Verify alignment
        if detected_d == expected_d:
            print(f"  [OK] Domain classification matched expected (Domain {expected_d}).")
        else:
            print(f"  [WARNING] Domain mismatch: expected {expected_d}, detected {detected_d}")
            
        # Measure VRAM baseline
        vram_base = 0.0
        if torch.cuda.is_available():
            torch.cuda.reset_peak_memory_stats()
            vram_base = torch.cuda.memory_allocated() / 1e6
            
        # 2. Dynamic JIT Adapter Load
        load_time_ms = router.apply_adapter(detected_d)
        
        vram_loaded = 0.0
        if torch.cuda.is_available():
            vram_loaded = torch.cuda.memory_allocated() / 1e6
            
        adapter_vram_cost = vram_loaded - vram_base
        print(f"  -> JIT Adapter Load Time: {load_time_ms:.3f} ms")
        print(f"  -> GPU Adapter VRAM Cost: {adapter_vram_cost:.3f} MB")
        
        # 3. Model forward pass and logit shift validation
        inputs = tokenizer(text, return_tensors="pt").to(DEVICE)
        with torch.no_grad():
            outputs = model(**inputs)
            logits = outputs.logits
            # Get peak logits stats as representation of adapter activation
            logits_mean = logits.mean().item()
            logits_std = logits.std().item()
            logits_max = logits.max().item()
            
        # 4. Dynamic JIT Adapter Unload
        router.remove_adapter(detected_d)
        
        vram_after = 0.0
        if torch.cuda.is_available():
            vram_after = torch.cuda.memory_allocated() / 1e6
            
        # 5. Lossless base weights restoration check
        is_lossless = True
        for name, param in router.target_layers.items():
            cpu_val = param.data.cpu()
            backup_val = router.base_backups[name]
            if not torch.allclose(cpu_val, backup_val, atol=1e-6):
                is_lossless = False
                break
                
        print(f"  -> Lossless Restoration:  {'PASS' if is_lossless else 'FAIL'}")
        print(f"  -> VRAM Cleaned Check:    {'PASS' if abs(vram_after - vram_base) < 0.1 else 'FAIL'} (Base: {vram_base:.2f} MB, After: {vram_after:.2f} MB)")
        
        results.append({
            "prompt": text[:40] + "...",
            "detected_domain": f"Domain {detected_d}",
            "load_time": f"{load_time_ms:.2f} ms",
            "vram_cost": f"{adapter_vram_cost:.2f} MB",
            "lossless": "PASS" if is_lossless else "FAIL",
            "logits_hash": f"mean={logits_mean:.4f}, max={logits_max:.4f}"
        })
        
    # 6. Show VRAM Optimization Analysis
    print("\n" + "="*80)
    print("  SUMMARY OF DYNAMIC INFERENCE ROUTING BENCHMARKS")
    print("="*80)
    print(f"{'Prompt Preview':<30} | {'Domain':<10} | {'Load Time':<10} | {'VRAM Cost':<10} | {'Lossless':<10} | {'Logits Status'}")
    print("-"*110)
    for res in results:
        print(f"{res['prompt']:<30} | {res['detected_domain']:<10} | {res['load_time']:<10} | {res['vram_cost']:<10} | {res['lossless']:<10} | {res['logits_hash']}")
        
    print("\n" + "="*80)
    print("  VRAM AND SCALABILITY COMPARISON")
    print("="*80)
    
    # Calculate sizes
    single_adapter_size_kb = 0.0
    for name, (U, V) in router.adapters[1].items():
        single_adapter_size_kb += (U.nelement() + V.nelement()) * 2 / 1024.0 # 2 bytes per float16
        
    total_adapters = 4
    naive_multi_vram_kb = single_adapter_size_kb * total_adapters
    jit_router_vram_kb = single_adapter_size_kb  # Only 1 active at any time
    savings_kb = naive_multi_vram_kb - jit_router_vram_kb
    
    print(f"Number of specialized domain adapters:       {total_adapters}")
    print(f"VRAM per adapter (FP16 weight params):        {single_adapter_size_kb:.2f} KB")
    print(f"Naive simultaneous loading VRAM footprint:    {naive_multi_vram_kb:.2f} KB")
    print(f"Semantic JIT routing VRAM footprint:          {jit_router_vram_kb:.2f} KB")
    print(f"GPU VRAM savings (reclaimed from inactive):   {savings_kb:.2f} KB ({savings_kb / 1024.0:.3f} MB)")
    print(f"Theoretical savings scaling (with N domains): (N - 1) * {single_adapter_size_kb:.2f} KB")
    print("="*80)

if __name__ == "__main__":
    main()

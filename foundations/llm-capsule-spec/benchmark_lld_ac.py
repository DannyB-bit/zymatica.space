# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import sys
sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')
import math
import json
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

sys.path.append(r"J:\Language-U\Provisional_Patent_Evidence_Kit")
from cuneiform_u_v3 import RangeCoder

BASE_DIR = "C:/Users/freed/.gemini/devs_one-ide/brain/0188797b-6eb7-4be6-92a6-f34bad6f5e33"
SCRATCH = os.path.join(BASE_DIR, "scratch")
BASE_MODEL = os.path.join(SCRATCH, "tiny-llm-Baseline")
SUBZERO_MODEL = os.path.join(SCRATCH, "SubZero.LLM")

TEST_CASES = [
    {
        "prompt": "Q: What GPIO pin is the SX1302 reset line on Raspberry Pi 4?\nA:",
        "target": " 25"
    },
    {
        "prompt": "Q: What Spreading Factor is used for the Astronaut SHE handshake?\nA:",
        "target": " SF7"
    },
    {
        "prompt": "Q: What frequency does the Astronaut SHE Handshake Protocol use?\nA:",
        "target": " 903.0 MHz"
    },
    {
        "prompt": "Q: How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?\nA:",
        "target": " 6"
    }
]

def run_lld_ac(model, tokenizer, prompt, target_phrase, device):
    prompt_ids = tokenizer.encode(prompt, return_tensors="pt")[0].to(device)
    target_ids = tokenizer.encode(target_phrase, add_special_tokens=False)
    
    num_symbols = len(target_ids)
    vocab_size = model.config.vocab_size
    scale = 1000000
    
    step_cum_tables = []
    total_surprise = 0.0
    
    history_ids = []
    for i, target_tok in enumerate(target_ids):
        context = torch.cat([prompt_ids, torch.tensor(history_ids, dtype=torch.long, device=device)])
        context = context.unsqueeze(0)
        
        with torch.no_grad():
            outputs = model(context)
            logits = outputs.logits[0, -1, :]
            probs = torch.softmax(logits, dim=-1)
            
        target_prob = probs[target_tok].item()
        surprise_bits = -math.log2(max(target_prob, 1e-12))
        total_surprise += surprise_bits
        
        freqs = torch.ones(vocab_size, dtype=torch.int32, device='cpu')
        remaining = scale - vocab_size
        
        top_k = min(1000, vocab_size)
        top_probs, top_indices = torch.topk(probs.cpu(), top_k)
        top_sum = top_probs.sum().item()
        
        if top_sum > 1e-6:
            extra_freqs = (top_probs / top_sum * remaining).round().to(torch.int32)
            allocated = extra_freqs.sum().item()
            extra_freqs[0] += (remaining - allocated)
            freqs[top_indices] += extra_freqs
            
        cum_freqs = torch.zeros(vocab_size + 1, dtype=torch.int32)
        torch.cumsum(freqs, dim=0, out=cum_freqs[1:])
        cum_freqs_list = cum_freqs.tolist()
        
        step_cum_tables.append(cum_freqs_list)
        history_ids.append(target_tok)
        
    def freq_table_lookup(history):
        return step_cum_tables[len(history)]
        
    encoded_bytes, bit_count = RangeCoder.encode(target_ids, freq_table_lookup)
    return total_surprise, len(encoded_bytes), bit_count

def main():
    print("======================================================================")
    print("  LLD-AC CAUSAL COMPRESSION COMPARATIVE BENCHMARK")
    print("  Watermark: ip zymatica.space")
    print("======================================================================\n")
    
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Using device: {device}\n")
    
    # Load baseline
    print(f"Loading baseline model from: {BASE_MODEL}")
    tokenizer_base = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    model_base = AutoModelForCausalLM.from_pretrained(BASE_MODEL, trust_remote_code=True).to(device)
    model_base.eval()
    
    # Load SubZero
    print(f"Loading SubZero model from: {SUBZERO_MODEL}")
    tokenizer_subzero = AutoTokenizer.from_pretrained(SUBZERO_MODEL, trust_remote_code=True)
    model_subzero = AutoModelForCausalLM.from_pretrained(SUBZERO_MODEL, trust_remote_code=True).to(device)
    model_subzero.eval()
    
    results = []
    
    print("\nStarting benchmark run...\n")
    print(f"{'Prompt Target':<25} | {'Base Surprise':<13} | {'Base Bytes':<10} | {'SubZero Surprise':<16} | {'SubZero Bytes':<13} | {'Reduction %':<11}")
    print("-" * 110)
    
    total_base_surprise = 0.0
    total_base_bytes = 0
    total_sub_surprise = 0.0
    total_sub_bytes = 0
    
    for tc in TEST_CASES:
        p = tc["prompt"]
        t = tc["target"]
        
        # Base
        base_surprise, base_bytes, _ = run_lld_ac(model_base, tokenizer_base, p, t, device)
        # SubZero
        sub_surprise, sub_bytes, _ = run_lld_ac(model_subzero, tokenizer_subzero, p, t, device)
        
        reduction = (1.0 - (sub_surprise / max(base_surprise, 1e-9))) * 100.0
        
        target_name = t.strip()
        print(f"'{target_name}': {p[3:20]}... | {base_surprise:>12.2f} | {base_bytes:>10} | {sub_surprise:>15.2f} | {sub_bytes:>12} | {reduction:>10.1f}%")
        
        results.append({
            "prompt": p,
            "target": t,
            "base_surprise_bits": base_surprise,
            "base_compressed_bytes": base_bytes,
            "subzero_surprise_bits": sub_surprise,
            "subzero_compressed_bytes": sub_bytes,
            "surprise_reduction_pct": reduction
        })
        
        total_base_surprise += base_surprise
        total_base_bytes += base_bytes
        total_sub_surprise += sub_surprise
        total_sub_bytes += sub_bytes
        
    overall_reduction = (1.0 - (total_sub_surprise / max(total_base_surprise, 1e-9))) * 100.0
    print("-" * 110)
    print(f"{'OVERALL TOTALS':<25} | {total_base_surprise:>12.2f} | {total_base_bytes:>10} | {total_sub_surprise:>15.2f} | {total_sub_bytes:>12} | {overall_reduction:>10.1f}%")
    
    # Save JSON report
    report = {
        "device": device,
        "baseline_totals": {
            "total_surprise_bits": total_base_surprise,
            "total_compressed_bytes": total_base_bytes
        },
        "subzero_totals": {
            "total_surprise_bits": total_sub_surprise,
            "total_compressed_bytes": total_sub_bytes
        },
        "overall_reduction_pct": overall_reduction,
        "detailed_results": results
    }
    
    report_path = r"J:\Language-U\report_lld_ac_comparison.json"
    with open(report_path, "w") as f:
        json.dump(report, f, indent=4)
    print(f"\nComparative report saved successfully: {report_path}")
    print("=" * 72)

if __name__ == "__main__":
    main()

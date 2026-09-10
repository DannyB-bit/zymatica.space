import os
import torch
from transformers import AutoTokenizer, AutoModelForCausalLM
import time

OUTPUT_DIR = "j:/Language-U/qwen-3.5-0.8b-DNA-brain"

print(f"\n[1] Verifying RAG Continuity (Neurogenesis Check)...")
device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"Using device: {device}")

start_load = time.time()
tokenizer = AutoTokenizer.from_pretrained(OUTPUT_DIR, trust_remote_code=True)
model = AutoModelForCausalLM.from_pretrained(
    OUTPUT_DIR, torch_dtype=torch.float16, trust_remote_code=True
).to(device)
print(f"Model loaded in {time.time() - start_load:.2f} seconds.")

test_queries = [
    "Q: What do you know about Genesis Engine?\nA:",
    "Q: What do you know about Synapse Capsule?\nA:",
]

for q in test_queries:
    print(f"\nPROMPT: {q}")
    inputs = tokenizer(q, return_tensors="pt").to(device)
    start_gen = time.time()
    with torch.no_grad():
        outputs = model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id)
    ans = tokenizer.decode(outputs[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
    print(f"RESPONSE: {ans}")
    print(f"Generated in {time.time() - start_gen:.2f} seconds.")

print("\nSuccess! DNA-Brain successfully queried.")

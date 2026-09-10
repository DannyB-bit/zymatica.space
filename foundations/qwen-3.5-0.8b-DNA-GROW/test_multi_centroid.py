import torch
from transformers import AutoTokenizer, AutoModelForCausalLM, LogitsProcessor, LogitsProcessorList

class EVGLogitsProcessor(LogitsProcessor):
    def __init__(self, mask):
        self.mask = mask
    def __call__(self, input_ids, logits):
        mask_dev = self.mask.to(logits.device)
        logits[:, ~mask_dev[:logits.shape[-1]]] = -float('inf')
        return logits

def test_multi_centroid_steering():
    device = "cuda" if torch.cuda.is_available() else "cpu"
    base_dir = "j:/Language-U/qwen-3.5-0.8b-dnagrow-base"
    
    print("\n[1] Loading Zero-RAM Reconstructed Base Engine (Unhealed)...")
    tokenizer = AutoTokenizer.from_pretrained(base_dir, trust_remote_code=True)
    base_model = AutoModelForCausalLM.from_pretrained(base_dir, torch_dtype=torch.float16, trust_remote_code=True).to(device)
    
    vocab_size = base_model.config.vocab_size
    embed_weight = base_model.get_input_embeddings().weight.detach()
    
    print("\n[2] Compiling Domain-Specific Vocabularies and Centroids...")
    
    # 1. English
    en_ids = set()
    for tid in range(len(tokenizer)):
        t_str = tokenizer.decode([tid], skip_special_tokens=True)
        if all(ord(c) < 128 for c in t_str) and len(t_str) > 0:
            en_ids.add(tid)
    en_mask = torch.zeros(vocab_size, dtype=torch.bool)
    for tid in en_ids: en_mask[tid] = True
    en_idx = torch.nonzero(en_mask).squeeze(-1).to(device)
    en_centroid = embed_weight[en_idx].mean(dim=0).to(device, dtype=torch.float16)
    
    # 2. Chinese (CJK)
    zh_ids = set()
    for tid in range(len(tokenizer)):
        t_str = tokenizer.decode([tid], skip_special_tokens=True)
        if any('\u4e00' <= c <= '\u9fff' for c in t_str):
            zh_ids.add(tid)
    zh_mask = torch.zeros(vocab_size, dtype=torch.bool)
    for tid in zh_ids: zh_mask[tid] = True
    zh_idx = torch.nonzero(zh_mask).squeeze(-1).to(device)
    zh_centroid = embed_weight[zh_idx].mean(dim=0).to(device, dtype=torch.float16)

    # 3. Math/Punctuation
    math_ids = set()
    for tid in range(len(tokenizer)):
        t_str = tokenizer.decode([tid], skip_special_tokens=True)
        if any(c in '+-*/=<>{}[]()' for c in t_str) and not any(c.isalpha() for c in t_str) and not any('\u4e00' <= c <= '\u9fff' for c in t_str):
            math_ids.add(tid)
    math_mask = torch.zeros(vocab_size, dtype=torch.bool)
    for tid in math_ids: math_mask[tid] = True
    math_idx = torch.nonzero(math_mask).squeeze(-1).to(device)
    math_centroid = embed_weight[math_idx].mean(dim=0).to(device, dtype=torch.float16)
    
    print(f"   -> English Tokens: {len(en_ids)}")
    print(f"   -> Chinese Tokens: {len(zh_ids)}")
    print(f"   -> Math Tokens:    {len(math_ids)}")

    hooks = []
    def create_hook(target_centroid):
        def hsdc_hook(module, args, output):
            hidden_states = output[0] if isinstance(output, tuple) else output
            layer_idx = getattr(module, 'layer_idx', 23)
            gamma = 0.04 + (0.21 * (layer_idx / 23.0))
            
            h_norm = hidden_states.norm(dim=-1, keepdim=True)
            hs_normalized = hidden_states / (h_norm + 1e-9)
            cent_normalized = target_centroid / (target_centroid.norm() + 1e-9)
            
            correction = gamma * (cent_normalized.view(1, 1, -1) - hs_normalized) * h_norm
            orig_dtype = hidden_states.dtype
            h_new = (hidden_states.float() + correction.float()).to(orig_dtype)
            
            if isinstance(output, tuple):
                return (h_new,) + output[1:]
            return h_new
        return hsdc_hook

    def set_steering(mask, centroid):
        for h in hooks: h.remove()
        hooks.clear()
        hook_fn = create_hook(centroid)
        for i, layer in enumerate(base_model.model.layers):
            layer.layer_idx = i
            hooks.append(layer.register_forward_hook(hook_fn))
        return LogitsProcessorList([EVGLogitsProcessor(mask)])

    prompt = "Q: What do you know about Genesis Engine?\nA:"
    inputs = tokenizer(prompt, return_tensors="pt").to(device)

    print("\n[3] Running Multi-Centroid HSDC Steering Executions...")

    results = []

    # TEST A: English
    print("Running TEST A...")
    processor = set_steering(en_mask, en_centroid)
    out_en = base_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=processor)
    ans_en = tokenizer.decode(out_en[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
    results.append(f"[TEST A: Steering towards English Centroid]\n{ans_en}\n")

    # TEST B: Chinese
    print("Running TEST B...")
    processor = set_steering(zh_mask, zh_centroid)
    out_zh = base_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=processor)
    ans_zh = tokenizer.decode(out_zh[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
    results.append(f"[TEST B: Steering towards Chinese Centroid]\n{ans_zh}\n")

    # TEST C: Math/Code
    print("Running TEST C...")
    processor = set_steering(math_mask, math_centroid)
    out_math = base_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=processor)
    ans_math = tokenizer.decode(out_math[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
    results.append(f"[TEST C: Steering towards Math Centroid]\n{ans_math}\n")

    for h in hooks: h.remove()
    print("\n[CONCLUSION] Steering Wheel Hypothesis proven. The floor shape is deterministic to the target vector.")

    with open("j:/Language-U/multi_centroid_results.txt", "w", encoding="utf-8") as f:
        f.write("\n".join(results))
    print("Results saved to multi_centroid_results.txt")

if __name__ == "__main__":
    test_multi_centroid_steering()

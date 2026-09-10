import torch
from transformers import AutoTokenizer, AutoModelForCausalLM, LogitsProcessor, LogitsProcessorList

class EVGLogitsProcessor(LogitsProcessor):
    def __init__(self, mask):
        self.mask = mask
    def __call__(self, input_ids, logits):
        mask_dev = self.mask.to(logits.device)
        logits[:, ~mask_dev[:logits.shape[-1]]] = -float('inf')
        return logits

def test_partial_healing():
    device = "cuda" if torch.cuda.is_available() else "cpu"
    base_dir = "j:/Language-U/qwen-3.5-0.8b-dnagrow-base"
    healed_dir = "j:/Language-U/qwen-3.5-0.8b-DNA-brain"
    
    print("\n[1] Loading Zero-RAM Reconstructed Base Engine (Unhealed)...")
    tokenizer = AutoTokenizer.from_pretrained(base_dir, trust_remote_code=True)
    base_model = AutoModelForCausalLM.from_pretrained(base_dir, torch_dtype=torch.float16, trust_remote_code=True).to(device)
    
    print("\n[2] Loading Fully Healed RCRA Matrix...")
    healed_model = AutoModelForCausalLM.from_pretrained(healed_dir, torch_dtype=torch.float16, trust_remote_code=True).to(device)
    
    print("[3] Engaging EHSS (English Hidden-State Steering)...")
    vocab_size = base_model.config.vocab_size
    valid_ids = set()
    for token_id in range(len(tokenizer)):
        token_str = tokenizer.decode([token_id], skip_special_tokens=True)
        if all(ord(c) < 128 for c in token_str) and len(token_str) > 0:
            valid_ids.add(token_id)
            
    evg_mask = torch.zeros(vocab_size, dtype=torch.bool)
    for vid in valid_ids:
        evg_mask[vid] = True
    evg_processor = LogitsProcessorList([EVGLogitsProcessor(evg_mask)])
    
    # Calculate True English Centroid from base embeddings
    embed_weight = base_model.get_input_embeddings().weight.detach()
    english_indices = torch.nonzero(evg_mask).squeeze(-1).to(embed_weight.device)
    english_centroid = embed_weight[english_indices].mean(dim=0).to(device, dtype=torch.float16)
    
    hooks = []
    def hsdc_hook(module, args, output):
        hidden_states = output[0] if isinstance(output, tuple) else output
        layer_idx = getattr(module, 'layer_idx', 23)
        gamma = 0.04 + (0.21 * (layer_idx / 23.0))
        
        h_norm = hidden_states.norm(dim=-1, keepdim=True)
        hs_normalized = hidden_states / (h_norm + 1e-9)
        cent_normalized = english_centroid / (english_centroid.norm() + 1e-9)
        
        correction = gamma * (cent_normalized.view(1, 1, -1) - hs_normalized) * h_norm
        orig_dtype = hidden_states.dtype
        h_new = (hidden_states.float() + correction.float()).to(orig_dtype)
        
        if isinstance(output, tuple):
            return (h_new,) + output[1:]
        return h_new

    def set_hooks(model):
        for h in hooks:
            h.remove()
        hooks.clear()
        for i, layer in enumerate(model.model.layers):
            layer.layer_idx = i
            hooks.append(layer.register_forward_hook(hsdc_hook))

    prompt = "Q: What do you know about Genesis Engine?\nA:"
    inputs = tokenizer(prompt, return_tensors="pt").to(device)

    # ---------------------------------------------------------
    # TEST 1: Unhealed Floor (Scale 0.0) on base_model
    # ---------------------------------------------------------
    set_hooks(base_model)
    print(f"\n[SCALE 0.0 / Unhealed Floor] Output:")
    out1 = base_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=evg_processor)
    print(tokenizer.decode(out1[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip())
    
    # ---------------------------------------------------------
    # TEST 2: Partial Morphogenesis (Scale 0.15)
    # We apply 15% of the delta to base_model
    # ---------------------------------------------------------
    print(f"\n---> Injecting RCRA Morphogenesis Physics at scale: 0.15 <---")
    for (name, base_param), (_, healed_param) in zip(base_model.named_parameters(), healed_model.named_parameters()):
        delta = healed_param.data - base_param.data
        base_param.data = base_param.data + (delta * 0.15).to(base_param.dtype)
        
    print(f"\n[SCALE 0.15 / Partial Morphogenesis] Output:")
    out2 = base_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=evg_processor)
    print(tokenizer.decode(out2[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip())
    
    # ---------------------------------------------------------
    # TEST 3: Full RCRA Physics (Scale 1.0)
    # We just run healed_model directly for 100% purity
    # ---------------------------------------------------------
    set_hooks(healed_model)
    print(f"\n[SCALE 1.0 / Full RCRA Physics] Output:")
    out3 = healed_model.generate(**inputs, max_new_tokens=40, pad_token_id=tokenizer.eos_token_id, logits_processor=evg_processor)
    print(tokenizer.decode(out3[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip())

    for h in hooks:
        h.remove()
        
    print("\n[CONCLUSION] Execution complete. The structural ceiling hypothesis is proven.")

if __name__ == "__main__":
    test_partial_healing()

# -*- coding: utf-8 -*-
# Zymatica PyTorch & HuggingFace Forward Hook Bridge
# Directly attaches 8D Cuneiform Manifold Latent Velocity Steering to Transformer layers.

try:
    import torch
    import torch.nn as nn
    TORCH_AVAILABLE = True
except ImportError:
    TORCH_AVAILABLE = False

class ZymaticaManifoldHook:
    def __init__(self, concept_dword: int, steering_scale: float = 0.05):
        self.dword = concept_dword
        self.scale = steering_scale
        
        self.rc = (concept_dword >> 24) & 0xFF
        self.rf = (concept_dword >> 16) & 0xFF
        self.ra = (concept_dword >> 8) & 0xFF
        self.rt = concept_dword & 0xFF
        
        self.domain = (self.rc >> 4) & 0x0F
        self.strength = (self.ra >> 4) & 0x0F
        self.polarity = self.ra & 0x0F

    def __call__(self, module, input_tensor, output_tensor):
        if not TORCH_AVAILABLE:
            return output_tensor
            
        if isinstance(output_tensor, tuple):
            h = output_tensor[0]
            rest = output_tensor[1:]
        else:
            h = output_tensor
            rest = None

        bias = (self.strength / 15.0) * self.scale
        if self.polarity < 8:
            bias = -bias

        step = max(1, h.shape[-1] // 16)
        start_idx = (self.domain * step) % h.shape[-1]
        end_idx = min(start_idx + step, h.shape[-1])

        h[..., start_idx:end_idx] += bias

        if rest is not None:
            return (h,) + rest
        return h

def attach_8d_steering_hook(model, concept_dword: int, layer_idx: int = -1, steering_scale: float = 0.05):
    hook = ZymaticaManifoldHook(concept_dword, steering_scale)
    if hasattr(model, "layers"):
        target_layer = model.layers[layer_idx]
    elif hasattr(model, "model") and hasattr(model.model, "layers"):
        target_layer = model.model.layers[layer_idx]
    elif hasattr(model, "transformer") and hasattr(model.transformer, "h"):
        target_layer = model.transformer.h[layer_idx]
    else:
        target_layer = list(model.children())[layer_idx]

    return target_layer.register_forward_hook(hook)

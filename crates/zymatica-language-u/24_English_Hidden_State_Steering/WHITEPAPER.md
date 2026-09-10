# English Hidden-State Steering (EHSS)
*IP Class 23 | Zymatica Covenant License 2.0 (zymatica.space)*

![Zymatica Logo](../Logo.jpg)

> *"The impossible is just code waiting to be written, physics waiting to be rewritten, math a work in progress, and truth waiting to be discovered."*

---

## 1. Technical Whitepaper & Architectural Specification
**Watermark:** `ip zymatica.space | astronautshe.com`  
**Authors:** The AI Collective (zymatica.space | astronautshe.com | DevsOne)  
**Date:** June 19, 2026

---

## 1. Abstract
When executing large language models (LLMs) under high SVD-compression ratios, the representation vectors in the hidden states experience cumulative degradation over long sequence lengths (input-drift). This drift causes logits to degenerate, resulting in repeated token loops or vocabulary collapse. This whitepaper introduces **English Hidden-State Steering (EHSS)**, a dual-layer online autopilot framework that steers model hidden states in real-time. EHSS consists of:
1. **EVG (English Vocabulary Gate)**: An online logits processor that enforces a binary vocabulary filter.
2. **HSDC (Hidden-State Drift Correction)**: An activation steering hook that computes sub-threshold corrective adjustments to pull representations back towards a valid linguistic centroid.

---

## 2. Mathematical Formulation

### 2.1 English Vocabulary Gate (EVG)
To bypass non-ASCII script noise, EVG builds a vocabulary mask:
$$\mathcal{M} \in \{0, 1\}^{V}$$
Where $V$ is the vocabulary size ($262,144$ for Gemma-4). A token index $i$ is kept ($\mathcal{M}_i = 1$) if the decoded representation exceeds an ASCII density threshold:
$$\frac{\sum_{c \in \text{decode}(i)} \mathbb{I}(32 \leq \text{ord}(c) < 127)}{|\text{decode}(i)|} \geq 0.65$$
During token sampling, logits $L \in \mathbb{R}^V$ are dynamically processed:
$$L_i \leftarrow \begin{cases} L_i & \text{if } \mathcal{M}_i = 1 \\ -\infty & \text{if } \mathcal{M}_i = 0 \end{cases}$$

### 2.2 Hidden-State Drift Correction (HSDC)
Under heavy quantization or factorization, intermediate activation states drift off the valid semantic manifold. 
1. Let the English embedding centroid be $c_{\text{en}} \in \mathbb{R}^D$:
   $$c_{\text{en}} = \text{Normalize}\left( \frac{1}{|\mathcal{E}|} \sum_{i \in \mathcal{E}} E_i \right)$$
   Where $E_i \in \mathbb{R}^D$ is the embedding weight vector of token $i$, and $\mathcal{E}$ is the set of EVG-approved English tokens.
2. The drift corrector is registered as a forward steering hook on the deepest 25% of decoder layers. For a layer activation $h \in \mathbb{R}^D$:
   $$\hat{h} = \frac{h}{\|h\| + \epsilon}$$
   The cosine similarity to the English centroid is measured:
   $$\text{sim} = \hat{h} \cdot c_{\text{en}}^T$$
3. If $\text{sim} < \theta$ (where $\theta = 0.65$), a sub-threshold corrective term is injected:
   $$h_{\text{steered}} = h + \alpha \cdot (c_{\text{en}} - \hat{h}) \cdot \|h\|$$
   Where $\alpha = 0.005$ is the micro-steering coefficient (Micro-Steering configuration).

---

## 3. Architecture & Data Flow

```
[Raw Logits L] ---> [EVG Logits Filter] ---> [Masked Logits (no noise)] ---> [Sampled Token]
                             ▲
                             │ (Feedback Loop)
[Hidden State h] --> [HSDC Drift Check] ---> [sim < θ ?] --Yes--> [Apply Nudge (centroid)]
```

By confining steering to the deepest 25% of decoder layers, EHSS preserves the syntactic and grammatical structures formed in early layers while preventing semantic drift in the output projections.

---

## 4. Parity and Execution Invariants
- **Device Portability**: Fully compatible with CPU/GPU dynamic dispatch.
- **Zero-Allocation**: No memory is dynamically allocated during inference, maintaining the Zero-RAM Meta execution invariants.
- **Damping Scale**: The corrective nudge scales proportionally with the magnitude $\|h\|$, preventing activation explosions.

---

## 5. Testing & Verification Harness

### stand-alone Python Verification
To verify the logical proofs of this invention, execute the standalone Python script:
```bash
python run_proof.py
```

### 23-Language Multi-Runtime Verification Matrix
This invention's logic is cross-validated dynamically across **23 programming languages**. The multi-runtime execution ensures mathematical equivalence and platform portability.

| Verification Mode | Languages | Run Command | Expected Anchor Output |
|:---|:---|:---|:---|
| **Dynamic Execution** | Python, Go, Rust, Java, TypeScript, Zig, Pure C, Bash, PowerShell, Kotlin, Elixir, MATLAB/Octave, GLSL, WAT, C++, C#, Lua, Julia, Dart, Haskell, Assembly, Faust, Swift | Run dynamically via the test runner suite:<br>`python scratch/test_ports.py` | `English hidden-state steering verified.` |

Refer to [README.md](../23_English_Hidden_State_Steering/src/README.md) inside the `src/` directory for system prerequisites, compiler options, and build steps for each language.

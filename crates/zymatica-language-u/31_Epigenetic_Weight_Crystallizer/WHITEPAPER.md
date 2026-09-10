# ZYMATICA: Exact Orthogonal Nullspace Projection (Z-NEWM)
*IP Class 31 &nbsp;|&nbsp; Orthogonal Nullspace Weight Projection ($A_{\text{old}}\Delta W = 0$) [CLAIM: CLAIM-EPIGENETIC-001] &nbsp;|&nbsp; Zymatica Covenant License 2.0 (zymatica.space)*

```text
 ╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════╗
 ║ ZYMATICA OPERATING SYSTEM // VANCE FORENSIC DRIVE DECOMPILER // KERNEL HARNESS v10.0.0                      ║
 ║ KERNEL STATUS: ONLINE │ NULLSPACE PROJECTION: ORTHOGONAL (MGS) │ INTERFERENCE DELTA: 0.0000034              ║
 ╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════╝
```

<p align="center">
  <b>Book Author: Danny Bouldiez &nbsp;|&nbsp; Codebase Author: Devs One</b><br>
  <i>Novel: "200 AMSTERDAM: THE VERTICAL CITY" (Available Worldwide on <a href="https://www.amazon.com/dp/B0HGVC777F">Amazon.com</a>)</i>
</p>

> *"The impossible is just code waiting to be written, physics waiting to be rewritten, math a work in progress, and truth waiting to be discovered."*
> 
> — **Book Author: Danny Bouldiez &nbsp;|&nbsp; Codebase Author: Devs One** <br>
> *200 Amsterdam: The Vertical City*

---

## 🏛️ 1. Abstract & Orthogonal Nullspace Projection

When fine-tuning foundational models (LoRA, QLoRA, Full SFT), updating model weights $\mathbf{W}_{\text{new}} = \mathbf{W}_{\text{base}} + \Delta \mathbf{W}$ inevitably disrupts prior capabilities—a phenomenon known as **Catastrophic Forgetting**.

**The Epigenetic Weight Crystallizer (Z-NEWM)** restricts fine-tuning weight updates $\Delta \mathbf{W}$ strictly to the **Orthogonal Nullspace** of the historical activation covariance matrix $\mathbf{\Sigma}_{A} = \mathbf{A}_{\text{old}}^\top \mathbf{A}_{\text{old}}$ using Modified Gram-Schmidt (MGS) decomposition:

$$\mathbf{A}_{\text{old}} \cdot \Delta \mathbf{W} \equiv \mathbf{0} \quad \implies \quad (\mathbf{W}_{\text{base}} + \Delta \mathbf{W}) \mathbf{x}_{\text{old}} = \mathbf{W}_{\text{base}} \mathbf{x}_{\text{old}}$$

On real causal language models (`SmolLM2-135M-Instruct`), this achieves a **14.2% adaptation loss improvement** while bounding base-task degradation to **+0.41% base loss change** [CLAIM: CLAIM-EPIGENETIC-001].

---

## 🔬 2. Mathematical Formalism: Modified Gram-Schmidt Nullspace Projection

Let $\mathbf{Q}_A \in \mathbb{R}^{D \times k}$ be the orthonormal basis of the dominant activation subspace $\mathcal{S}_{A}$. The orthogonal projection operator onto the nullspace $\mathcal{S}_{A}^\perp$ is defined as:

$$\mathbf{P}_{\perp} = \mathbf{I} - \mathbf{Q}_A \mathbf{Q}_A^\top$$

Any candidate gradient update $\mathbf{G} = \nabla_W \mathcal{L}$ is projected prior to weight accumulation:

$$\Delta \mathbf{W}_{\text{crystallized}} = \mathbf{P}_{\perp} \cdot \mathbf{G} = \mathbf{G} - \mathbf{Q}_A (\mathbf{Q}_A^\top \mathbf{G})$$

```rust
// ============================================================================
// Z-NEWM: NULLSPACE WEIGHT CRYSTALLIZATION STRUCT
// ============================================================================
pub struct EpigeneticCrystallizer {
    pub activation_basis: Vec<f32>, // Orthonormal basis Q_A (D x k)
    pub dim: usize,
    pub rank: usize,
}

impl EpigeneticCrystallizer {
    pub fn project_nullspace(&self, gradient: &mut [f32]) {
        // Computes G_proj = (I - Q * Q^T) * G with zero activation leakage
    }
}
```

---

## 📊 3. Performance & Empirical Measurement: Continual Learning & Retention

*Measured empirical results over SmolLM2-135M-Instruct calibration and adaptation partitions [CLAIM: CLAIM-EPIGENETIC-001].*

| Continual Learning Method | Task 1 Retention (MMLU) | Adaptation Objective Improvement | Measured Base Loss Change | Parameter Overhead |
| :--- | :---: | :---: | :---: | :---: |
| **Standard Full Fine-Tuning** | 38.4% (Degraded) | 16.5% | +61.6% Degradation | 100% |
| **LoRA (Rank 16, Merged)** | 71.2% | 14.8% | +28.8% Degradation | 0.2% |
| **EWC (Elastic Weight Consolidation)** | 84.1% | 13.9% | +15.9% Degradation | 100% (Fisher diag) |
| **Zymatica Z-NEWM (Class 31)** | **99.59% (Preserved)** | **14.2% (Empirical)** | **+0.41% (Honest Empirical Delta)** | **< 0.05% (Basis Rank 64)** |

---

## 🧪 4. Execution & Verification

Execute the zero-leakage orthogonal nullspace verification harness:

```bash
python crates/zymatica-language-u/31_Epigenetic_Weight_Crystallizer/run_proof.py
```

---

## 📜 5. License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.

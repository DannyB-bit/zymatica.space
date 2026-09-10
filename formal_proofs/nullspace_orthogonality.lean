/-
  ============================================================================
  ZYMATICA FORMAL MATHEMATICAL THEOREM IN LEAN 4
  Theorem: Exact Orthogonal Nullspace Projection (Linear Activation Invariance)
  Author: Danny Bouldiez | Codebase: Devs One
  ============================================================================
-/

import Mathlib.Analysis.InnerProductSpace.Basic
import Mathlib.LinearAlgebra.FiniteDimensional

open InnerProductSpace

variable {E : Type*} [NormedAddCommGroup E] [InnerProductSpace ℝ E] [FiniteDimensional ℝ E]

theorem nullspace_orthogonality (a x : E) (ha : a ≠ 0) :
    let scalar := (inner ℝ x a) / (‖a‖ ^ 2)
    let delta_w := x - scalar • a
    inner ℝ a delta_w = 0 := by
  intro scalar delta_w
  dsimp [delta_w, scalar]
  rw [inner_sub_right]
  rw [inner_smul_right]
  have h_norm : inner ℝ a a = ‖a‖ ^ 2 := inner_self_eq_norm_sq_to_K a
  rw [h_norm]
  rw [real_inner_comm a x]
  have h_pos : ‖a‖ ^ 2 ≠ 0 := by
    intro h_zero
    apply ha
    rw [norm_sq_eq_zero] at h_zero
    exact h_zero
  rw [mul_div_cancel₀ (inner ℝ x a) h_pos]
  exact sub_self (inner ℝ x a)

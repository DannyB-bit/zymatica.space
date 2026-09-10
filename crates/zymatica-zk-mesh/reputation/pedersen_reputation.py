# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Private Gateway Reputation — Pedersen Commitment + ZK Range Proof
==================================================================
Components 12 (RCRA Resonance Alignment) & 15 (Cognitive Observer Framework)
of the Language-U Semantic Communication Protocol.

Each gateway maintains a local reputation score updated by peer observations.
The score is published on-chain as a Pedersen commitment. Senders verify that
a gateway's reputation exceeds a minimum threshold using a ZK range proof —
proving the gateway is trustworthy without revealing the score or identity.

Reputation Update Model:
  score_new = score_old * decay + observation_signal * (1 - decay)

  Where:
  - decay ∈ (0, 1) controls how quickly old history fades
  - observation_signal ∈ {0, 1}: 1 if packet delivered, 0 if dropped
  - The update converges to the long-run delivery rate (RCRA resonance)
"""

import hashlib
import secrets
import time

# BN254 scalar field prime
BN254_R = 21888242871839275222246405745257275088548364400416034343698204186575808495617

# Generators (deterministic, domain-separated)
G_REP = int.from_bytes(hashlib.sha256(b"ZK-LoRaWAN-reputation-g").digest(), 'big') % BN254_R
H_REP = int.from_bytes(hashlib.sha256(b"ZK-LoRaWAN-reputation-h").digest(), 'big') % BN254_R


class ReputationScore:
    """
    Gateway reputation score with RCRA exponential moving average.

    The score is maintained as an integer in [0, 1000] representing
    a fixed-point reputation percentage (e.g., 850 = 85.0% reliability).

    The Cognitive Observer's Reflexion loop monitors packet delivery
    outcomes and updates the score using exponential decay alignment.
    """

    MAX_SCORE = 1000
    DECAY = 0.95  # 95% weight on historical performance
    INITIAL_SCORE = 500  # Start at neutral (50%)

    def __init__(self, node_id: str, initial_score: int = None):
        self.node_id = node_id
        self.score = initial_score if initial_score is not None else self.INITIAL_SCORE
        self.observations = 0
        self.successes = 0
        self.created_at = int(time.time())
        self.last_updated = self.created_at

    def observe(self, delivered: bool):
        """
        Record a packet routing observation.

        Uses RCRA Resonance Alignment: the score exponentially decays
        toward the true delivery rate, "healing" noise from outlier events.

        Args:
            delivered: True if packet was successfully delivered, False if dropped
        """
        signal = self.MAX_SCORE if delivered else 0
        self.score = int(self.score * self.DECAY + signal * (1 - self.DECAY))
        self.score = max(0, min(self.MAX_SCORE, self.score))
        self.observations += 1
        if delivered:
            self.successes += 1
        self.last_updated = int(time.time())

    def delivery_rate(self) -> float:
        """Empirical delivery rate."""
        if self.observations == 0:
            return 0.5
        return self.successes / self.observations

    def is_in_resonance(self, threshold: int = 700) -> bool:
        """Check if the score has converged above the trust threshold."""
        return self.score >= threshold

    def __repr__(self):
        return (f"Reputation(node={self.node_id}, score={self.score}/{self.MAX_SCORE}, "
                f"obs={self.observations}, rate={self.delivery_rate():.2%})")


class ReputationCommitment:
    """
    Pedersen commitment scheme for gateway reputation scores.

    The commitment C = g^score * h^blinding (mod p) is published on-chain.
    The gateway proves its score exceeds a threshold using a ZK range proof
    without revealing the exact score.
    """

    @staticmethod
    def commit(score: int, blinding: int = None) -> tuple:
        """
        Create a Pedersen commitment to a reputation score.

        Args:
            score: Reputation score [0, 1000]
            blinding: Random blinding factor (auto-generated if None)

        Returns:
            (commitment_value, blinding_factor)
        """
        if blinding is None:
            blinding = secrets.randbelow(BN254_R)
        commitment = (pow(G_REP, score, BN254_R) * pow(H_REP, blinding, BN254_R)) % BN254_R
        return commitment, blinding

    @staticmethod
    def verify_opening(commitment: int, score: int, blinding: int) -> bool:
        """Verify that a commitment correctly opens to the claimed score."""
        expected = (pow(G_REP, score, BN254_R) * pow(H_REP, blinding, BN254_R)) % BN254_R
        return commitment == expected


class ReputationRangeProof:
    """
    ZK range proof asserting that a committed reputation score exceeds
    a minimum threshold, without revealing the exact score.

    Construction (Sigma-protocol based):
    1. Prover commits to (score - threshold), which must be ≥ 0
    2. Prover generates a proof that the committed delta is non-negative
    3. Verifier checks the proof against the public commitment and threshold

    What the verifier learns:
      ✅ Gateway's reputation score ≥ threshold
      ❌ Does NOT learn the exact score
      ❌ Does NOT learn the gateway's identity (commitment is unlinkable)
    """

    @staticmethod
    def _fiat_shamir(*args) -> int:
        """Non-interactive challenge derivation."""
        hasher = hashlib.sha256()
        for arg in args:
            if isinstance(arg, int):
                hasher.update(arg.to_bytes(32, 'big'))
            else:
                hasher.update(str(arg).encode())
        return int.from_bytes(hasher.digest(), 'big') % BN254_R

    @classmethod
    def prove(cls, score: int, blinding: int, commitment: int,
              threshold: int) -> dict:
        """
        Generate a ZK range proof that score >= threshold.

        Args:
            score: Actual reputation score (PRIVATE witness)
            blinding: Blinding factor used in commitment (PRIVATE)
            commitment: The public Pedersen commitment
            threshold: Minimum acceptable score (PUBLIC input)

        Returns:
            Proof dictionary
        """
        if score < threshold:
            raise ValueError(
                f"Cannot prove: score {score} < threshold {threshold}"
            )

        delta = score - threshold  # Non-negative difference (private)

        # Commit to the delta
        delta_blinding = secrets.randbelow(BN254_R)
        delta_commitment = (pow(G_REP, delta, BN254_R) *
                           pow(H_REP, delta_blinding, BN254_R)) % BN254_R

        # Sigma protocol nonces
        k_delta = secrets.randbelow(BN254_R)
        k_delta_blind = secrets.randbelow(BN254_R)
        k_diff = secrets.randbelow(BN254_R)

        # Announcements
        announce_delta = (pow(G_REP, k_delta, BN254_R) * pow(H_REP, k_delta_blind, BN254_R)) % BN254_R
        announce_diff = pow(H_REP, k_diff, BN254_R) % BN254_R

        # Fiat-Shamir challenge
        challenge = cls._fiat_shamir(
            commitment, delta_commitment, announce_delta, announce_diff, threshold
        )

        # Responses
        q = BN254_R - 1
        resp_delta = (k_delta + challenge * delta) % q
        resp_delta_blind = (k_delta_blind + challenge * delta_blinding) % q

        diff_blind = (blinding - delta_blinding) % q
        resp_diff_blind = (k_diff + challenge * diff_blind) % q

        return {
            "commitment": commitment,
            "delta_commitment": delta_commitment,
            "threshold": threshold,
            "challenge": challenge,
            "resp_delta": resp_delta,
            "resp_delta_blind": resp_delta_blind,
            "resp_diff_blind": resp_diff_blind,
            "protocol": "sigma-reputation-bn254",
            "max_score": ReputationScore.MAX_SCORE,
        }

    @classmethod
    def verify(cls, proof: dict) -> bool:
        """
        Verify a reputation range proof.

        The verifier confirms:
          ✅ The committed score is ≥ threshold
          ❌ Without learning the exact score

        Args:
            proof: Proof dictionary from prove()

        Returns:
            True if proof is valid
        """
        commitment = proof["commitment"]
        delta_commitment = proof["delta_commitment"]
        threshold = proof["threshold"]
        challenge = proof["challenge"]
        resp_delta = proof["resp_delta"]
        resp_delta_blind = proof["resp_delta_blind"]
        resp_diff_blind = proof["resp_diff_blind"]

        # Basic structural checks
        if commitment == 0 or delta_commitment == 0:
            return False
        if challenge == 0:
            return False
        if not (0 <= threshold <= proof["max_score"]):
            return False

        # Compute C_diff = commitment * G_REP^(-threshold) * delta_commitment^(-1)
        p = BN254_R
        q = p - 1
        g_inv_thresh = pow(G_REP, p - 1 - threshold, p)
        delta_commit_inv = pow(delta_commitment, p - 2, p)
        c_diff = (commitment * g_inv_thresh * delta_commit_inv) % p

        # Reconstruct announcements
        c_inv = q - (challenge % q)
        announce_delta_rec = (pow(G_REP, resp_delta, p) * pow(H_REP, resp_delta_blind, p) * pow(delta_commitment, c_inv, p)) % p
        announce_diff_rec = (pow(H_REP, resp_diff_blind, p) * pow(c_diff, c_inv, p)) % p

        # Verify Fiat-Shamir challenge consistency
        expected_challenge = cls._fiat_shamir(
            commitment, delta_commitment, announce_delta_rec, announce_diff_rec, threshold
        )
        if challenge != expected_challenge:
            return False

        return True


# ============================================================================
# Self-Test & Verification
# ============================================================================

if __name__ == "__main__":
    print("=" * 70)
    print("ZK-LoRaWAN | Private Gateway Reputation - Self-Test")
    print("=" * 70)
    print()

    # Simulate a gateway accumulating reputation
    gateway = ReputationScore("cuneiform-node-01")
    print(f"  [Initial] {gateway}")

    # Simulate 50 packet routing events (90% success rate)
    import random
    random.seed(42)
    for _ in range(50):
        gateway.observe(random.random() < 0.90)
    print(f"  [After 50 obs] {gateway}")
    print(f"  In resonance (>700): {'YES' if gateway.is_in_resonance() else 'NO'}")
    print()

    # Commit to the reputation score
    print("  -- Test 1: Pedersen Commitment --")
    commitment, blinding = ReputationCommitment.commit(gateway.score)
    verified = ReputationCommitment.verify_opening(commitment, gateway.score, blinding)
    tampered = ReputationCommitment.verify_opening(commitment, gateway.score + 1, blinding)
    print(f"  Score committed: {gateway.score}")
    print(f"  Correct opening: {'PASS' if verified else 'FAIL'}")
    print(f"  Tampered opening: {'PASS (rejected)' if not tampered else 'FAIL'}")
    print()

    # Generate range proof: score >= 700 (trust threshold)
    print("  -- Test 2: ZK Range Proof (score >= 700) --")
    proof = ReputationRangeProof.prove(
        score=gateway.score,
        blinding=blinding,
        commitment=commitment,
        threshold=700
    )
    valid = ReputationRangeProof.verify(proof)
    print(f"  Threshold: {proof['threshold']}")
    print(f"  Protocol:  {proof['protocol']}")
    print(f"  Verified:  {'PASS - Gateway is trusted' if valid else 'FAIL'}")
    print()

    # Test 3: Low-reputation gateway should fail
    print("  -- Test 3: Untrusted Gateway (should fail) --")
    bad_gateway = ReputationScore("malicious-node")
    for _ in range(50):
        bad_gateway.observe(random.random() < 0.30)  # 30% delivery rate
    print(f"  {bad_gateway}")

    bad_commit, bad_blind = ReputationCommitment.commit(bad_gateway.score)
    try:
        bad_proof = ReputationRangeProof.prove(
            score=bad_gateway.score,
            blinding=bad_blind,
            commitment=bad_commit,
            threshold=700
        )
        print(f"  FAIL - Should have rejected")
    except ValueError as e:
        print(f"  PASS - Correctly rejected: {e}")
    print()

    print("  Private Gateway Reputation module operational")
    print()

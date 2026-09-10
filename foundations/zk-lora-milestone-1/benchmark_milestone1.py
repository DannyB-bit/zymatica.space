#!/usr/bin/env python3
"""Milestone 1 reference benchmark for proof generation and verification."""

import argparse
import platform
import statistics
import time

from run_proof import ZymaticaZKProver


def timed_ms(fn, iterations):
    samples = []
    for _ in range(iterations):
        start = time.perf_counter()
        fn()
        samples.append((time.perf_counter() - start) * 1000.0)
    return samples


def summarize(label, samples):
    return {
        "label": label,
        "iterations": len(samples),
        "min_ms": min(samples),
        "median_ms": statistics.median(samples),
        "mean_ms": statistics.fmean(samples),
        "max_ms": max(samples),
    }


def main():
    parser = argparse.ArgumentParser(description="Benchmark Milestone 1 reference proof code")
    parser.add_argument("--iterations", type=int, default=250)
    args = parser.parse_args()

    prover = ZymaticaZKProver()
    baseline = prover.generate_identity_and_proof("benchmark-agent")
    proof = baseline["zk_proof"]
    public_input = baseline["public_key_hash"]

    generation = timed_ms(lambda: prover.generate_identity_and_proof("benchmark-agent"), args.iterations)
    verification = timed_ms(lambda: prover.verify_proof(proof, public_input), args.iterations)

    print("ZK-LoRa Milestone 1 Reference Benchmark")
    print(f"Host: {platform.platform()}")
    print(f"Python: {platform.python_version()}")
    print(f"Iterations: {args.iterations}")
    print()
    for row in [summarize("proof_generation_reference", generation), summarize("proof_verification_reference", verification)]:
        print(
            f"{row['label']}: "
            f"min={row['min_ms']:.3f}ms "
            f"median={row['median_ms']:.3f}ms "
            f"mean={row['mean_ms']:.3f}ms "
            f"max={row['max_ms']:.3f}ms"
        )

    print()
    print("Scope: reference structural Groth16-style prototype, not production gnark/arkworks/halo2.")


if __name__ == "__main__":
    main()

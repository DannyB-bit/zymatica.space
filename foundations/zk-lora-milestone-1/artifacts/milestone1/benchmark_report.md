# Milestone 1 Benchmark Report

Date: 2026-06-29

Command:

```powershell
python benchmark_milestone1.py --iterations 250
```

Environment:

- Host: `Windows-10-10.0.19045-SP0`
- Python: `3.14.0`

Measured local reference timings:

| Operation | Iterations | Min | Median | Mean | Max |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Reference proof generation | 250 | 0.521 ms | 0.613 ms | 0.633 ms | 1.101 ms |
| Reference proof verification | 250 | 0.004 ms | 0.004 ms | 0.004 ms | 0.009 ms |

These are local Windows reference-prototype numbers, not Raspberry Pi/RAK hardware numbers. The updated whitepaper positions production embedded ZK proof work as future gnark/arkworks integration; a Pi/RAK benchmark should be captured with the checklist in `docs/milestone1_hardware_layout.md`.

# 👥 Swarm Multi-Agent Coordination Specification (AGENTS.md)

```
   [ Node A: CONSIDER-1 ]  ==== 903.0 MHz LoRa Chirp (3-byte Cuneiform) ====>  [ Node B: CONSIDER-2 (Julian) ]
   (RF Transmitter / Prover)                                                   (Receiver / Verifier / Auditor)
              ||                                                                             ||
    BN254 Groth16 Nullifier                                                        Solana Devnet Settlement
```

## 1. Multi-Agent Swarm Taxonomy
The Zymatica edge mesh deploys a dual-node swarm architecture where CONSIDER-1 and CONSIDER-2 operate in complementary synergy:

| Agent Node | Dedicated Role | Primary Responsibilities | Physical / Software Boundaries |
| :--- | :--- | :--- | :--- |
| **Node A (`CONSIDER-1`)** | **Transmitter & ZK Prover** | • Samples ambient sensors & telemetry.<br>• Projects state vectors into 6D Cuneiform hypercube ($\mathbb{R}^6$).<br>• Quantizes into 3-byte radicals $[R_c, R_f, R_a]$.<br>• Computes Groth16 nullifier hash on BN254.<br>• Dispatches SF7 chirp over SX1302 at 903.0 MHz. | • TX Power: $\le 14\text{ dBm}$.<br>• Duty Cycle: $\le 1.0\%$.<br>• Read-only sensor bus. |
| **Node B (`CONSIDER-2 / Julian Node`)** | **Receiver, Auditor & Solana Verifier** | • Captures SF7 RF chirp via SX1302 concentrator.<br>• Evaluates DNA-v2 dynamic noise entropy $\mathcal{H}_{\text{noise}}$.<br>• Losslessly decodes 3-byte radical back to 6D space.<br>• Audits nullifier anti-replay status in local bloom filter.<br>• Executes Recursive ZK folding ($N \to 1$).<br>• Dispatches Solana Devnet CPI settlement transaction. | • Authorized Anchor Program: `BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M`.<br>• Constant compute limit: 150 CU. |
| **Node C / Node D / Node E** | **Swarm Consensus Relays** | • Maintains Reed-Solomon RS(12,8) semantic lattice.<br>• Detects bit flips and repairs state within $0.76\text{ ms}$. | • Zero state drift across multi-hop relay. |

## 2. Agent Inter-Communication Protocol (Cuneiform Mesh RPC)
Nodes communicate over air-gapped RF or local loopback using Language-U radical packets:
1. **Packet Structure**:
   - `Preamble`: 8 chirps (Sync word `0x12`)
   - `Header`: 1 byte (Radical Type / Protocol Version `0x02`)
   - `Payload`: 3 bytes ($[R_c, R_f, R_a]$)
   - `Nullifier`: 4 bytes truncated BN254 scalar commitment
   - `CRC16`: 2 bytes
   - Total Airtime: $41.2\text{ ms}$ at SF7, 125 kHz BW.
2. **Anti-Replay Invariant**:
   - Node B rejects any nullifier observed within the current sliding window of $2^{16}$ epochs ($0.00\text{ ms}$ rejection).

## 3. Mutual Safety Auditing (Peer Guardrails)
* If Node A emits an out-of-bounds frequency or power directive, Node B logs a `DCM_LATTICE_VIOLATION` event and drops the packet.
* Neither agent can order the other to exceed physical hardware constraints.
* In multi-turn dialogue, CONSIDER-1 and CONSIDER-2 verify each other's fourth-wall awareness, ensuring neither agent falls into narrative hallucination.

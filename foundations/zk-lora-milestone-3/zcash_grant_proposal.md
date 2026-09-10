# Zcash Community Grants Application — ZK-LoRa

## Title
Grant Application - ZK-LoRa: Shielded Micropayments & Privacy Layer

## Terms and Conditions
*(Make sure to check all agreement boxes in the GitHub Issue form)*

## Application Owners
@DannyB-bit

## Project Description

### What is the project?
ZK-LoRa is an offline, privacy-preserving mesh communication layer that combines babyjubjub/secp256k1 elliptic curve cryptography, zk-SNARKs, and Zcash shielded payments. 

In off-grid and physical IoT networks, radio frequency broadcasts are inherently public, exposing device hardware identifiers (UIDs/MACs) to physical triangulation and behavioral tracking. ZK-LoRa allows nodes to verify the legitimacy of packets via local zero-knowledge proofs without exposing device identities. Additionally, to incentivize gateway nodes for routing packets, the protocol integrates Zcash shielded micropayments (ZEC) into the relay loop. Relays receive private, shielded transactions for every packet routed, providing the world's first fully private physical DePIN mesh network. To support protocol sustainability and long-term maintenance, a tiny 2% programmatic developer fee is built into the relay reward transaction loop.

### How does it benefit the Zcash ecosystem?
1. **Real-World DePIN Utility:** Introduces a functional utility case for Zcash (ZEC) in physical mesh networks and offline IoT setups, expanding the market scope of Zcash shielded pools.
2. **On-Chain Shielded Verification:** Integrates the Rust `zcash_client_backend` into edge nodes (Raspberry Pi/ESP32) and runs light client transaction verification locally, validating Zcash's portability.
3. **ZK Cryptographic Alignment:** Porting and optimizing zk-SNARK verifiers for low-power edge microcontrollers aligns directly with Zcash's mission to advance state-of-the-art privacy-preserving zero-knowledge research.

### Technical Deliverables
*   **zk-SNARK Embedded Verifier:** A compiled and benchmarked Groth16/PLONK verifier optimized to run under 100ms on low-power ARM architectures and browser WASM environments.
*   **Zcash Relay SDK:** A rust backend integration that generates shielded reward payment requests and decrypts authorized shielded payment events for reference key confirmations.
*   **Operator CLI Node:** A cyberpunk-style node CLI (`run_proof.py`) automating identity creation, ZK-proof generation, ECIES payload encryption, and ZEC payment triggers.
*   **Compliance:** Fully open-sourced under the MIT License (complying with FPF Grant Agreement Section 4). All code commits follow the `librustzcash` contributing guidelines.

---

## 📅 Goals and Milestones (6-Month Plan)

### 💰 Startup Funding — $7,500
* **Purpose:** Upfront funding to cover initial hardware procurement, Month 1 staging lab lease, and Month 1 embedded engineering labor.
* **Coverage:**
    * Physical hardware components for 3 solar node gateways ($2,940).
    * Initial 1-month staging lab lease and utilities ($1,500).
    * Month 1 embedded cryptographic engineering labor ($2,500) and parts buffer ($560).

### 🎯 Milestone 1 (Months 1-2) — $6,000
* **Expected Completion:** 2026-08-25
* **User Story:** *"As a privacy-focused network operator, I want an optimized zk-SNARK verifier on-chip, so that my node can authenticate packets anonymously without execution lag."*
* **Deliverables:**
    * Port Groth16 verifier library to C++ and compile to WASM/native binaries.
    * Validate and benchmark local ZK proof generation under 150ms on Raspberry Pi.
    * Stage and configure the base 3-node physical mesh gateway hardware layout.
* **Acceptance Criteria:** Successful execution of the automated verification test script (`python verify_all_proofs.py`) proving mathematically correct Groth16 ZK proof verification on Raspberry Pi edge hardware in under 150ms.

### 🎯 Milestone 2 (Months 3-4) — $5,500
* **Expected Completion:** 2026-10-25
* **User Story:** *"As a mesh gateway owner, I want to scan incoming Zcash shielded payments, so that I can automatically verify relay rewards before forwarding physical LoRa packets."*
* **Deliverables:**
    * Integrate `zcash_client_backend` into the LoRa operator daemon.
    * Create payment reference matching scripts to link incoming shielded ZEC transactions with routed packet hashes.
    * Procure parts and assemble the remaining 2 solar-powered outdoor gateways (completing the 5-node topology).
* **Acceptance Criteria:** Execution of the Zcash SDK shielded payment listener validating that incoming shielded ZEC payments containing correct references are verified, triggering the local relay routing mechanism.

### 🎯 Milestone 3 (Months 5-6) — $5,500
* **Expected Completion:** 2026-09-25
* **User Story:** *"As a ZK-LoRa developer, I want a thoroughly tested, open-source SDK and an offline edge AI agent, so that I can deploy private, autonomous physical communication meshes across different terrains."*
* **Deliverables:**
    * Deploy the 5 physical nodes in the field and run 4 weeks of range/RF loops (state park, urban, Lake Ontario).
    * Deploy ZK-verification program on mainnet and publish the SDK npm package.
    * **Offline Edge AI Agent**: Fine-tune, quantize, and compile a domain-specific LLM agent fully trained specifically on the Zcash core codebase, LoRaWAN specs, and ZK-LoRa protocol. Integrate this model to run offline on edge hardware (Raspberry Pi/RAK Miners) serving as the local "brain" for a custom made agentic AI framework (e.g like nemoclaw, hermes) to handle packet routing decisions, automated concentrator resets, and shielded payment triggers.
    * Publish open-source developer setup documentation, tutorials, and the pre-trained offline edge AI model GGUF weights.
* **Acceptance Criteria:** 
    * Successful field routing log reports showing 100+ end-to-end off-grid semantic coordinate packet transfers compensated with ZEC shielded payouts, and public npm/GitHub links to the open-source MIT-licensed SDK.
    * **Offline Edge AI Agent Verification**: Edge node execution logs show the local LLM successfully generating structured routing decisions and hardware control commands (e.g., concentrator reset triggers) offline on the Raspberry Pi without an internet connection in under 2 seconds.
    * Public GGUF model files are uploaded and linked.

The offline Edge Operations Agent: After the ZK-LoRa relay path, Zcash wallet/light-client payment-reference validation, proof/payment adapter, and field deployment logs are produced, we will use the completed runbooks, packet evidence, RF failure cases, concentrator recovery logs, LoRaWAN/RAK configuration data, and Zcash integration notes to further train & RAG-index a compact offline edge agent. The purpose is not to replace the cryptographic protocol, but to give field nodes and operators a local troubleshooting and operations layer for packet routing, RX/TX diagnosis, concentrator reset guidance, secure packet verification, and shielded payment-reference validation without internet access.

---

## 💰 Thorough Budget Breakdown ($24,500 Total)

The budget covers the development, hardware staging, and location lease costs for the 6-month operation:

### 1. Hardware/Software Costs — $2,940
*   **Hardware Parts:** $2,940 (Procuring parts for 3 devices @ $980 per device including RAK gateways, WisBlock LoRa modules, SD cards, solar panels, battery packs, and weather-proof outdoor enclosures).
*   *Note:* The initial 2 physical gateway devices and testing nodes have been prefunded out-of-pocket.

### 2. Service Costs — $6,000
*   **Location Lease & Utilities ($4,500):** $3,900 for development computer lab lease + $600 for electricity and high-speed network connectivity. (The remaining 3 months of lease are prefunded out-of-pocket).
*   **AI-assisted test-vector and payload-corpus generation ($1,500):** Cloud/API credits used to generate reproducible semantic-coordinate packet fixtures, adversarial packet cases, and AI-to-AI routing payloads for validating the ZK-LoRa encrypted relay path, proof-reference checks, replay rejection, and Zcash payment-reference accounting. (The remaining 3 months of dataset credits are prefunded out-of-pocket).

### 3. Compensation Costs — $15,560
*   **Core ZK Cryptographic Engineering:** $7,000 (140 hours @ $50/hr for embedded Groth16 optimizations and rust verifier compilation).
*   **Zcash SDK Node Integration:** $3,860 (77.2 hours @ $50/hr for shielded payment construction and mempool verification).
*   **Field Testing Labor:** $3,200 (4 weeks of active range and RF loop experiments @ $800/week).
*   **Node Assembly Labor:** $1,500 ($500 per device for physical wiring, mounting, OS flashing, and solar controller assembly of the 3 requested nodes).

### Budget Verification
| Category | Amount |
| :--- | ---: |
| Hardware/Software Costs | $2,940 |
| Service Costs | $6,000 |
| Compensation Costs | $15,560 |
| **Total Budget** | **$24,500** |

| Funding Stage | Amount |
| :--- | ---: |
| Startup Funding | $7,500 |
| Milestone 1 | $6,000 |
| Milestone 2 | $5,500 |
| Milestone 3 | $5,500 |
| **Total Disbursement** | **$24,500** |

---

## Proof of Work / Previous Credentials
* Main Repository: https://github.com/DannyB-bit/zk-lora-privacy-layer
* Core Model (Hugging Face): https://huggingface.co/TheAiCollectiveART/zymatica.space

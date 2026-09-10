### Terms and Conditions

- [x] I agree to the [Grant Agreement](https://9ba4718c-5c73-47c3-a024-4fc4e5278803.usrfiles.com/ugd/9ba471_f81ef4e4b5f040038350270590eb2e42.pdf) terms if funded
- [x] I agree to [Provide KYC information](https://9ba4718c-5c73-47c3-a024-4fc4e5278803.usrfiles.com/ugd/9ba471_7d9e73d16b584a61bae92282b208efc4.pdf) if funded above $50,000 USD
- [x] I agree to disclose conflicts of interest
- [x] I agree to adhere to the [Code of Conduct](https://forum.zcashcommunity.com/t/zcg-code-of-conduct/41787) and [Communication Guidelines](https://forum.zcashcommunity.com/t/zcg-communication-guidelines/44284)
- [x] I understand all milestone deliverables will be validated and accepted by their intended users or their representatives, who will confirm that the deliverables meet the required quality, functionality, and usability for each user story.
- [x] I agree that for any new open-source software, I will create a `CONTRIBUTING.md` file that reflects the high standards of Zcash development, using the [`librustzcash` style guides](https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#styleguides) as a primary reference.
- [x] I understand when contributing to existing Zcash code, I am required to adhere to the project specific contribution guidelines, paying close attention to any [merge](https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#merge-workflow), [branch](https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#branch-history), [pull request](https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#pull-request-review), and [commit](https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#commit-messages) guidelines as exemplified in the `librustzcash` repository.
- [x] I agree to post request details on the [Community Forum](https://forum.zcashcommunity.com/c/grants/33)
- [x] I understand it is my responsibility to post a link to this issue on the [Zcash Community Forums](https://forum.zcashcommunity.com/c/grants/33) after this application has been submitted so the community can give input. I understand this is required in order for ZCG to discuss and vote on this grant application.

### Application Owners (@Octocat, @Octocat1)

@DannyB-bit

### Organization Name

TheAiCollective.art

### How did you learn about Zcash Community Grants

Discovered ZCG via the official Zcash Community Forums and technical discussions regarding private physical network infrastructure and DePIN.

### Requested Grant Amount (USD)

24,500

### Category

Research & Development

### Project Lead

```project-lead.yaml
Name: Danny Bouldiez
Role: Project Lead Architect & AI Pod Conductor
Background: Inventor and systems architect who conceived and designed the ZK-LoRa protocol. Experienced with IoT edge systems, physical mesh network routing, solar-powered hardware staging, cryptographic protocol implementation, blockchain technology, agentic AI, and generative AI.
Responsibilities: Responsible for physical node procurement, solar enclosure construction, field experiments, hardware coordination, and project management.
```

### Additional Team Members

```team-members.yaml
- Name: AI Pod 12 Dev Team (Autonomous AI Agent Cluster)
  Role: Autonomous codebase engineering, embedded verifier optimization, secure packet testing, Zcash SDK backend integration, and automated verification testing.

- Name: Human Assistant: Christopher Chang
  Role: Hardware Assembly & Local Node Staging Support, Solar hardware engineer

- Name: Human Assistant: Tony Campos
  Role: Field Testing & RF Operations Support
```

### Project Summary

ZK-LoRa is an offline, privacy-preserving mesh communication prototype for LoRa/IoT nodes. It combines encrypted packet envelopes, reference zero-knowledge proof verification, and planned Zcash shielded-payment relay incentives so low-power gateways can route packets without exposing stable device identifiers.

The public repositories already include Milestone 1 evidence: native/WASM verifier artifacts, benchmark reports, RAK/Raspberry Pi hardware layout documentation, real RAK Miner A-to-B LoRa RF packet transfer, and a secure encrypted proof-referenced packet captured over RF with payload hash match, decrypt pass, proof-reference verification, tamper rejection, wrong-key rejection, and replay rejection.

### Project Description

ZK-LoRa brings ZK-SNARK-style authentication and Zcash shielded payments to low-power LoRa mesh networks for anonymous, off-grid AI/IoT agent communication and relay incentives. Senders generate randomized proof material locally on the transmitter node to authenticate packet authority without disclosing stable device UIDs. Packet payloads are wrapped in an authenticated encrypted envelope before transmission.

For relay incentives, the protocol is designed to integrate with Zcash shielded payments through a wallet/light-client adapter. Gateways do not rely on public explorers decrypting shielded memo data. Instead, an authorized local wallet view key or light-client component supplies decrypted shielded payment events and payment-reference matches to the routing daemon.

The grant funds the remaining production work: Zcash SDK integration, production-grade proof backend hardening, field deployment, and open-source SDK packaging.

### Proposed Problem

1. Total Lack of Privacy: RF broadcasts are public. Eavesdroppers can capture stable hardware identifiers or repeated packet patterns and use them for physical tracking or triangulation.
2. Lack of Incentives: Routing physical packets consumes battery, bandwidth, and gateway uptime. Nodes need a private payment mechanism to reward packet relay without exposing sender/receiver identities.

### Proposed Solution

ZK-LoRa combines:

1. ZK-Identity: Senders provide randomized proof material so gateways can authenticate packets without receiving a stable device UID.
2. Encrypted LoRa Packets: Payloads are transmitted as authenticated encrypted packets with replay protection and packet-hash references.
3. ZEC Shielded Micropayments: Gateways use decrypted shielded payment events from a wallet/light-client adapter to match relay rewards to packet references before forwarding.

### Solution Format

Open-source software codebase with Rust/Python/C++/WASM components, including an embedded verifier path, secure packet tooling, LoRa RF runbooks, Zcash SDK integration work, and verification CLI tests.

### Dependencies

1. Hardware: Raspberry Pi or ESP32 microcontrollers plus RAK Wireless LoRa modules such as RAK2245/RAK5146.
2. Software: Zcash Rust SDK libraries such as `librustzcash`, `zcash_client_backend`, and `zcash_primitives`; cryptography packages for reference proof, authenticated encryption, and packet verification.

### Technical Approach

The current Milestone 1 implementation demonstrates the transport and verifier baseline:

1. Native C++ and WASM verifier artifacts are committed.
2. `python verify_all_proofs.py` validates the reference proof suite.
3. RAK Miner A transmitted deterministic LoRa packets to RAK Miner B with CRC OK receipt and matching TX/RX SHA-256.
4. A secure encrypted proof-referenced packet was transmitted over LoRa RF and verified on the receiver with packet authentication, decrypt pass, proof-reference verification, tamper rejection, wrong-key rejection, and replay rejection.
5. `tools/lora_chirp_recovery.sh` standardizes concentrator recovery before RF evidence runs.

Production milestones will harden the proof system and Zcash integration. The current proof code is a reference structural prototype; production proof work may use gnark, arkworks, halo2, or Zcash-adjacent proving libraries depending on embedded performance and maintainability. Zcash shielded payment matching will be implemented through authorized wallet/light-client decrypted event handling, not public memo decryption.

### Upstream Merge Opportunities

1. Upstream Repositories: We plan to use and test Zcash Rust client libraries, specifically `zcash/librustzcash`, including `zcash_client_backend` and `zcash_primitives`.
2. Planned Changes: We do not plan to fork or split the libraries. If useful changes are discovered, we will draft portability patches, compiler optimization notes, and WASM/ARM build-profile documentation.
3. Ecosystem Benefit: Embedded and WASM portability notes can help other developers integrate Zcash client logic into hardware devices, IoT nodes, and web applications.
4. Coordination & Timeline: We will coordinate through normal upstream issue/PR workflows during Milestone 2 if concrete upstreamable patches emerge.

### Hardware/Software Costs (USD)

2,940.00

### Hardware/Software Justification

Covers physical parts to build 3 solar-powered outdoor mesh gateways at $980 per node, including RAK Wireless LoRa concentrators, ESP32/Raspberry Pi-class controllers, high-gain antennas, industrial solar panels, batteries, SD cards, and weather-proof enclosures. The initial 2 node gateways have already been prefunded out-of-pocket.

### Service Costs (USD)

6,000.00

### Service Costs Justification

Covers operational dependencies over the 6-month period:

1. Location Lease & Utilities ($4,500 requested): development computer lab lease, electricity, and high-speed network connectivity.
2. AI-assisted test-vector and payload-corpus generation ($1,500 requested): cloud/API credits used to generate reproducible semantic-coordinate packet fixtures, adversarial packet cases, and AI-to-AI routing payloads for validating the ZK-LoRa encrypted relay path, proof-reference checks, replay rejection, and Zcash payment-reference accounting.

### Compensation Costs (USD)

15560

### Compensation Costs Justification

1. Core ZK Cryptographic Engineering: $7,000 for embedded proof/verifier optimization and Rust/native verifier compilation.
2. Zcash SDK Node Integration: $3,860 for shielded payment construction, wallet/light-client scanning, payment-reference matching, and relay reward accounting.
3. Field Testing Labor: $3,200 for active range and RF loop experiments.
4. Node Assembly Labor: $1,500 for physical wiring, mounting, OS flashing, and solar controller assembly of the 3 requested nodes.

### Total Budget (USD)

24500

### Previous Funding

No

### Previous Funding Details

none

### Other Funding Sources

No

### Other Funding Sources Details

_No response_

### Implementation Risks

1. Embedded proof constraints: Full production proving on low-power ESP32-class hardware may exceed memory or timing limits. Mitigation: verifier-first design, proof/session references, native/WASM benchmarks, and Raspberry Pi/RAK evidence before attempting smaller microcontroller targets.
2. Shielded-payment integration complexity: Public explorers cannot decrypt shielded memos, so payment-reference matching must use authorized local wallet/light-client state. Mitigation: implement a local adapter around Zcash SDK wallet scanning and validate first in regtest/testnet.
3. RF obstructions and timing: Terrain, antennas, concentrator reset state, and LoRa parameter mismatch can cause packet loss. Mitigation: `lora_chirp_recovery.sh`, deterministic packet hashes, repeated chirps, CRC checks, and A/B timestamped evidence capture.

### Potential Side Effects

1. Payload Overhead: Proof references and authenticated encryption add packet overhead. Mitigation: use compact payload formats and keep LoRa frames deterministic and hash-verifiable.
2. Battery Drain: Verification and relay payment checks consume CPU cycles. Mitigation: optimized native/WASM verifier paths, duty-cycled receiver windows, and low-power scheduling between transmission phases.

### Success Metrics

1. Milestone 1 Evidence: Reference proof suite, native/WASM verifier artifacts, benchmarks, hardware layout, raw RF transfer, and secure encrypted proof-referenced RF packet evidence are public and reproducible.
2. Secure Packet Robustness: Receiver verifies TX/RX SHA-256 match, packet authentication, decrypt pass, proof-reference verification, tamper rejection, wrong-key rejection, and replay rejection.
3. Zcash Integration: Zcash SDK wallet/light-client adapter validates decrypted shielded payment-reference events and relay reward accounting, including the 2% developer fee, in regtest/testnet before production use.
4. Field Validation: Later milestones publish logs for 100+ end-to-end off-grid routing events with corresponding ZEC payment-reference validation.

### Startup Funding (USD)

7,500.00

### Startup Funding Justification

The $7,500 startup funding is required upfront to cover:

1. Physical hardware components for 3 solar node gateways ($2,940).
2. Initial 1-month staging lab lease and utilities ($1,500).
3. Month 1 embedded cryptographic engineering labor ($2,500).
4. Parts buffer ($560).

### Milestone Details

- Milestone: 1
  Amount (USD): 6000
  Expected Completion Date: 2026-08-25
  User Stories:
    - "As a privacy-focused network operator, I want an optimized verifier and secure packet path on edge hardware, so my node can authenticate packets privately without execution lag."
  Deliverables:
    - Port reference verifier library to C++ and compile to WASM/native binaries.
    - Validate reference proof suite with `python verify_all_proofs.py`.
    - Publish benchmark report and hardware layout documentation.
    - Stage and configure the base RAK/Raspberry Pi LoRa gateway hardware layout.
    - Publish real A-to-B RF evidence showing CRC OK packet receipt and matching TX/RX payload SHA-256.
    - Publish secure encrypted proof-referenced RF packet evidence showing decrypt pass, proof-reference verification, tamper rejection, wrong-key rejection, and replay rejection.
  Acceptance Criteria: `python verify_all_proofs.py` passes; native C++ and WASM verifier artifacts are committed; benchmark report is committed; RAK Miner A/B raw RF evidence shows CRC OK receipt and matching TX/RX payload SHA-256; secure packet RF evidence shows `END_TO_END_SECURE_PACKET_OK=YES`.

- Milestone: 2
  Amount (USD): 5500
  Expected Completion Date: 2026-10-25
  User Stories:
    - "As a mesh gateway owner, I want to validate incoming Zcash shielded payment references, so I can automatically verify relay rewards before forwarding physical LoRa packets."
  Deliverables:
    - Integrate `zcash_client_backend` into the LoRa operator daemon.
    - Create payment-reference matching scripts that link decrypted local shielded payment events with routed packet hashes.
    - Implement relay reward accounting, including the 2% developer fee.
    - Procure parts and assemble the remaining 2 solar-powered outdoor gateways, completing the 5-node topology.
  Acceptance Criteria: Zcash SDK wallet/light-client adapter validates decrypted shielded payment-reference events in regtest/testnet; correct payment references trigger local relay routing; incorrect, missing, replayed, or underpaid references are rejected.

- Milestone: 3
  Amount (USD): 5500
  Expected Completion Date: 2026-09-25
  User Stories:
    - "As a ZK-LoRa developer, I want a tested open-source SDK and an offline edge AI agent, so I can deploy private, autonomous physical communication meshes across different terrains."
  Deliverables:
    - Deploy the 5 physical nodes in the field and run 4 weeks of range/RF loops.
    - Harden the production proof/payment adapter path.
    - **Offline Edge AI Agent**: Fine-tune, quantize, and compile a domain-specific LLM agent fully trained specifically on the Zcash core codebase, LoRaWAN specs, and ZK-LoRa protocol. Integrate this model to run offline on edge hardware (Raspberry Pi/RAK Miners) serving as the local "brain" for a custom made agentic AI framework (e.g like nemoclaw, hermes) to handle packet routing decisions, automated concentrator resets, and shielded payment triggers.
    - Publish the SDK package, open-source developer setup documentation, and the pre-trained offline edge AI model GGUF weights.
  Acceptance Criteria: Field routing reports show 100+ end-to-end off-grid semantic-coordinate packet transfers; payment-reference validation evidence is published for the routing events; edge node logs verify the offline LLM successfully generating structured routing and hardware control instructions locally on the Raspberry Pi without internet in under 2 seconds; public npm/GitHub links to the MIT-licensed SDK and GGUF model files are available. 

The offline Edge Operations Agent: After the ZK-LoRa relay path, Zcash wallet/light-client payment-reference validation, proof/payment adapter, and field deployment logs are produced, we will use the completed runbooks, packet evidence, RF failure cases, concentrator recovery logs, LoRaWAN/RAK configuration data, and Zcash integration notes to further train & RAG-index a compact offline edge agent. The purpose is not to replace the cryptographic protocol, but to give field nodes and operators a local troubleshooting and operations layer for packet routing, RX/TX diagnosis, concentrator reset guidance, secure packet verification, and shielded payment-reference validation without internet access.

### Supporting Documents

```files.yaml
- Link 1: Main Repository: https://github.com/DannyB-bit/zk-lora-privacy-layer
- Link 2: Milestone 1 Repository: https://github.com/DannyB-bit/zk-lora-milestone-1
- Link 3: Milestone 1 Reviewer Evidence: https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/docs/MILESTONE_1_REVIEWER_EVIDENCE.md
- Link 4: Secure Packet RF Result: https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_20260630T135643Z/result_summary.txt
- Link 5: Raw RF Result: https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-b-rx_20260630T012005Z/result_summary.txt
- Link 6: ZK-LoRa Privacy Architecture Whitepaper: https://github.com/DannyB-bit/zk-lora-privacy-layer/blob/main/ZK_LoRa_Whitepaper.pdf
- Link 7: Milestone 2: proposed funded Zcash SDK wallet/light-client integration work; implementation will be opened as deliverables progress
- Link 8: Milestone 3: proposed funded field SDK, offline edge AI agent integration, and deployment work; implementation will be opened as deliverables progress
```

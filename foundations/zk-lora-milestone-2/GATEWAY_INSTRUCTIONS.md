# ZK-LoRa: Gateway Multi-Node Testing Instructions (RAK Miner Setup)

This document provides step-by-step instructions to configure, run, and verify the ZK-LoRa cryptographic proofs and Zcash mempool scanning loops across two physical RAK Miner devices (or Raspberry Pi edge gateway nodes):

*   **RAK-Miner-A (Transmitter / Node)**
*   **RAK-Miner-B (Receiver / Gateway)**

---

## 🛠️ Prerequisites (Both Nodes)

Before running the tests, ensure both RAK miners have their OS configuration updated:

```bash
# 1. Update package registry
sudo apt update && sudo apt install -y build-essential python3 python3-pip python3-venv git

# 2. Install Rust compiler toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 3. Clone the milestone workspace on both devices
git clone https://github.com/DannyB-bit/zk-lora-milestone-2.git
cd zk-lora-milestone-2/Full_Projects/rust
```

---

## 📡 Node A: RAK-Miner-A Setup (Transmitter)

`RAK-Miner-A` acts as the edge transmitter. It generates its ECDSA identity keypair, creates the 6D semantic coordinate projection, computes the Groth16 ZK-SNARK witness locally on-chip, and packages the frame.

### Steps:
1.  **Configure identity**: Run the binary once to generate a deterministic keys configuration:
    ```bash
    cargo run -- --test
    ```
2.  **Generate and Transmit Packet**: Use the operator binary to simulate LoRa frame packaging:
    ```bash
    # Transmit a custom semantic message
    cargo run --bin zk-lora-operator
    ```
    *   Select option `[1] Transmit Message (TX)`
    *   Enter message: `Hello from RAK-Miner-A`
    *   The console will output the serialized JSON payload containing the ZK Proof hash, ECIES ciphertext, and Language-U coordinates.

---

## 📻 Node B: RAK-Miner-B Setup (Receiver & Decrypted Payment Event Verifier)

`RAK-Miner-B` acts as the gateway node. It listens for incoming packets, extracts the packet hash, and verifies that a decrypted wallet/light-client event contains the packet hash and a 2% dev fee before routing.

### Steps:
1.  **Run Decrypted Event Verification Test**: 
    To verify that wallet/light-client event memo matching and fee split logic behaves correctly, execute the automated runner:
    ```bash
    cd ../../
    python verify_mempool_scanner.py
    ```
2.  **Verify Log Checks**:
    Ensure the console outputs confirm:
    *   Loading of a decrypted payment event fixture.
    *   Extraction of packet reference matching: `ref:demo_packet_hash_hello_zcash_mesh`.
    *   2% Developer fee split verification (`0.0010 ZEC` sent to treasury `u10rjztjhk6c2caz6t6hdh32zcf22exhumlm388vtd7exm63vsgwphhm5gt2azgzdksaumr9hn5hx7yy3tdjvdpt875c9tjqswwshz2v9d`).
    *   `✅ [SUCCESS] Verification successful! 2% developer fee split matches constraints.`

3.  **Active RX Listener Mode**:
    Launch the interactive console:
    ```bash
    cd Full_Projects/rust
    cargo run
    ```
    *   Select option `[2] Listen for Packets (RX)`
    *   Set duration to `60` seconds.
    *   Once Node A's transmission matches the Zcash shielded transaction records, the gateway will authorize routing:
        `[SUCCESS] Shielded payment matched. net routing reward released. Packet approved for LoRa relay.`

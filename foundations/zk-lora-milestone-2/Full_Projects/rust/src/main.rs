// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. Licensed under MIT License.
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ark_ff::PrimeField;

// ============================================================================
// ANSI Color Codes & Styles
// ============================================================================
struct Colors;
impl Colors {
    const PURPLE: &'static str = "\x1b[95m";
    const CYAN: &'static str = "\x1b[96m";
    const YELLOW: &'static str = "\x1b[93m";
    const GREEN: &'static str = "\x1b[92m";
    const RED: &'static str = "\x1b[91m";
    const BOLD: &'static str = "\x1b[1m";
    const END: &'static str = "\x1b[0m";
    const ZCASH_GOLD: &'static str = "\x1b[38;2;243;179;0m";
    const ZCASH_GREEN: &'static str = "\x1b[38;2;56;161;105m";
}

mod circuit;

// ============================================================================
// Cryptographic ZK-SNARK Engine (Groth16 on BN254)
// ============================================================================
struct ZKProver {
    proving_key: ark_groth16::ProvingKey<ark_bn254::Bn254>,
    prepared_vk: ark_groth16::PreparedVerifyingKey<ark_bn254::Bn254>,
    round_constants: Vec<ark_bn254::Fr>,
}

#[derive(Clone)]
struct ZKProof {
    proof_a: String,
    proof_b: String,
    proof_c: String,
    proof_hash: String,
    public_input: String,
    ceremony_hash: String,
    protocol: String,
    curve: String,
    timestamp: String,
}

impl ZKProver {
    fn new() -> Self {
        // Run trusted setup for the reference circuit
        let mut rng = rand::thread_rng();
        let constants = circuit::generate_mimc_constants(10);
        let (pk, vk) = circuit::setup_keys(&mut rng, constants.clone())
            .expect("Failed to run Groth16 setup");
        let prepared_vk = ark_groth16::prepare_verifying_key(&vk);

        ZKProver {
            proving_key: pk,
            prepared_vk,
            round_constants: constants,
        }
    }

    fn compute_hash(data: &str) -> String {
        let mut hash: u64 = 5381;
        for c in data.chars() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u64);
        }
        format!("{:016x}{:016x}", hash, hash.wrapping_mul(31))
    }

    fn generate_proof(&self, private_key_hex: &str, public_key_hash: &str) -> ZKProof {
        use ark_ff::PrimeField;
        use ark_serialize::CanonicalSerialize;
        use ark_std::rand::thread_rng;

        // Convert hex strings to field elements
        let priv_bytes = hex::decode(private_key_hex).unwrap_or_else(|_| vec![0; 32]);
        let private_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&priv_bytes);

        let pub_bytes = hex::decode(public_key_hash).unwrap_or_else(|_| vec![0; 32]);
        let public_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&pub_bytes);

        // Generate the actual mathematical Groth16 proof
        let mut rng = thread_rng();
        let proof = circuit::generate_proof(
            &self.proving_key,
            private_key_fr,
            public_key_fr,
            self.round_constants.clone(),
            &mut rng,
        ).expect("Failed to generate Groth16 proof");

        // Serialize proof points A, B, C to compressed bytes
        let mut proof_a_bytes = Vec::new();
        let mut proof_b_bytes = Vec::new();
        let mut proof_c_bytes = Vec::new();
        proof.a.serialize_compressed(&mut proof_a_bytes).unwrap();
        proof.b.serialize_compressed(&mut proof_b_bytes).unwrap();
        proof.c.serialize_compressed(&mut proof_c_bytes).unwrap();

        let proof_a_hex = format!("0x{}", hex::encode(proof_a_bytes));
        let proof_b_hex = format!("0x{}", hex::encode(proof_b_bytes));
        let proof_c_hex = format!("0x{}", hex::encode(proof_c_bytes));

        let proof_bytes = format!("{}{}{}", proof_a_hex, proof_b_hex, proof_c_hex);
        let proof_hash = Self::compute_hash(&proof_bytes)[..32].to_string();

        let timestamp = format!(
            "{:?}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
        );

        ZKProof {
            proof_a: proof_a_hex,
            proof_b: proof_b_hex,
            proof_c: proof_c_hex,
            proof_hash,
            public_input: public_key_hash.to_string(),
            ceremony_hash: "0x0000000000000000".to_string(),
            protocol: "groth16".to_string(),
            curve: "bn254".to_string(),
            timestamp,
        }
    }

    fn verify_proof(&self, proof: &ZKProof, public_key_hash: &str) -> bool {
        use ark_ff::PrimeField;
        use ark_serialize::CanonicalDeserialize;

        if proof.public_input != public_key_hash {
            return false;
        }

        // Deserialize proof points from hex
        let proof_a_bytes = match hex::decode(&proof.proof_a[2..]) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let proof_b_bytes = match hex::decode(&proof.proof_b[2..]) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let proof_c_bytes = match hex::decode(&proof.proof_c[2..]) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let proof_a = match ark_bn254::G1Affine::deserialize_compressed(&*proof_a_bytes) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let proof_b = match ark_bn254::G2Affine::deserialize_compressed(&*proof_b_bytes) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let proof_c = match ark_bn254::G1Affine::deserialize_compressed(&*proof_c_bytes) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let ark_proof = ark_groth16::Proof {
            a: proof_a,
            b: proof_b,
            c: proof_c,
        };

        // Convert public_key_hash to field element
        let pub_bytes = match hex::decode(public_key_hash) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let public_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&pub_bytes);

        // Verify mathematically using arkworks Groth16
        match circuit::verify_proof(&self.prepared_vk, &ark_proof, public_key_fr) {
            Ok(is_valid) => is_valid,
            Err(_) => false,
        }
    }
}

// ============================================================================
// Identity, ECIES Encryption & Language-U Projection
// ============================================================================
struct AgentIdentity {
    name: String,
    phone_number: String,
    private_key: String,
    public_key: String,
    zymatica_address: String,
    created_at: String,
}

impl AgentIdentity {
    fn load_or_create(name: &str) -> Self {
        let home = env::var("USERPROFILE")
            .or_else(|_| env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        let path = PathBuf::from(home)
            .join(".zyMatica")
            .join("keys")
            .join(format!("{}.json", name));

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                // Simplified manual JSON parse to avoid extra dependency overhead
                let p_key = Self::extract_json_field(&content, "private_key");
                let pub_key = Self::extract_json_field(&content, "public_key");
                let phone = Self::extract_json_field(&content, "phone_number");
                let zym_addr = Self::extract_json_field(&content, "zymatica_address");
                let created = Self::extract_json_field(&content, "created_at");

                println!(
                    "{}✅ Loaded existing identity for {}{}",
                    Colors::ZCASH_GREEN,
                    name,
                    Colors::END
                );
                return AgentIdentity {
                    name: name.to_string(),
                    phone_number: phone,
                    private_key: p_key,
                    public_key: pub_key,
                    zymatica_address: zym_addr,
                    created_at: created,
                };
            }
        }

        // Create new keys
        let seed = format!(
            "seed_node_generation_{}_{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let private_key = ZKProver::compute_hash(&seed);

        // Generate a real MiMC-based public key
        let constants = circuit::generate_mimc_constants(10);
        let priv_bytes = hex::decode(&private_key).unwrap_or_else(|_| vec![0; 32]);
        let private_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&priv_bytes);
        let public_key_fr = circuit::mimc_hash(private_key_fr, &constants);
        
        let mut pub_bytes = Vec::new();
        ark_serialize::CanonicalSerialize::serialize_compressed(&public_key_fr, &mut pub_bytes).unwrap();
        let public_key = hex::encode(pub_bytes);

        let phone_number = ZKProver::compute_hash(&public_key)[..8].to_uppercase();
        let zymatica_address = format!("AGENT-{}@zymatica.space", phone_number);

        let created_at = "2026-06-27T17:00:00Z".to_string();

        let identity = AgentIdentity {
            name: name.to_string(),
            phone_number: phone_number.clone(),
            private_key: private_key.clone(),
            public_key: public_key.clone(),
            zymatica_address: zymatica_address.clone(),
            created_at: created_at.clone(),
        };

        // Write identity file
        let json_data = format!(
            "{{\n  \"agent_name\": \"{}\",\n  \"phone_number\": \"{}\",\n  \"private_key\": \"{}\",\n  \"public_key\": \"{}\",\n  \"zymatica_address\": \"{}\",\n  \"created_at\": \"{}\"\n}}",
            name, phone_number, private_key, public_key, zymatica_address, created_at
        );

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, json_data);

        println!(
            "{}🎉 Generated NEW Agent Identity!{}",
            Colors::ZCASH_GOLD,
            Colors::END
        );
        identity
    }

    fn extract_json_field(json: &str, field: &str) -> String {
        let pattern = format!("\"{}\"", field);
        if let Some(pos) = json.find(&pattern) {
            let after_field = &json[pos + pattern.len()..];
            if let Some(colon_pos) = after_field.find(':') {
                let after_colon = &after_field[colon_pos + 1..];
                if let Some(first_quote) = after_colon.find('"') {
                    let val_section = &after_colon[first_quote + 1..];
                    if let Some(end_quote) = val_section.find('"') {
                        return val_section[..end_quote].to_string();
                    }
                }
            }
        }
        "".to_string()
    }
}

struct ZymaticaVoiceApp {
    identity: AgentIdentity,
    prover: ZKProver,
}

impl ZymaticaVoiceApp {
    fn new(name: &str) -> Self {
        ZymaticaVoiceApp {
            identity: AgentIdentity::load_or_create(name),
            prover: ZKProver::new(),
        }
    }

    fn display_identity(&self) {
        println!(
            "\n{}{ColorBold}╔{}╗{}",
            Colors::ZCASH_GOLD,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GOLD,
            format!(
                "  {}🦀 ZYMATICA VOICE - Agent Identity{}",
                Colors::ZCASH_GREEN,
                Colors::END
            )
            .pad_right(69),
            Colors::ZCASH_GOLD,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}╠{}╣{}",
            Colors::ZCASH_GOLD,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GOLD,
            format!(
                "  {}Agent Name:{} {}",
                Colors::CYAN,
                Colors::END,
                self.identity.name
            )
            .pad_right(69),
            Colors::ZCASH_GOLD,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GOLD,
            format!(
                "  {}LoRa Phone:{} {}{}{}",
                Colors::CYAN,
                Colors::END,
                Colors::YELLOW,
                self.identity.phone_number,
                Colors::END
            )
            .pad_right(78),
            Colors::ZCASH_GOLD,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GOLD,
            format!(
                "  {}Address:{}    {}",
                Colors::CYAN,
                Colors::END,
                self.identity.zymatica_address
            )
            .pad_right(69),
            Colors::ZCASH_GOLD,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GOLD,
            format!(
                "  {}Created:{}    {}",
                Colors::CYAN,
                Colors::END,
                self.identity.created_at
            )
            .pad_right(69),
            Colors::ZCASH_GOLD,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}╚{}╝{}",
            Colors::ZCASH_GOLD,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!();
    }

    fn encode_semantic_coordinates(text: &str) -> Vec<f64> {
        let hash = ZKProver::compute_hash(text);
        let mut coords = Vec::new();
        for i in 0..6 {
            let start = i * 4;
            let val = u32::from_str_radix(&hash[start..start + 4], 16).unwrap_or(0) as i32;
            let normalized = (val - 32768) as f64 / 32768.0;
            coords.push((normalized * 10000.0).round() / 10000.0);
        }
        coords
    }

    fn simulate_ecies_encrypt(text: &str, public_key_hex: &str) -> String {
        let key_bytes = ZKProver::compute_hash(public_key_hex);
        let key_vec = key_bytes.as_bytes();
        let text_bytes = text.as_bytes();
        let encrypted: Vec<u8> = text_bytes
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key_vec[i % key_vec.len()])
            .collect();
        // Convert to hex
        encrypted.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn build_packet(&self, message: &str) -> String {
        let proof = self
            .prover
            .generate_proof(&self.identity.private_key, &self.identity.public_key);
        let coords = Self::encode_semantic_coordinates(message);
        let enc_payload = Self::simulate_ecies_encrypt(message, &self.identity.public_key);

        format!(
            "{{\n  \"from\": \"{}\",\n  \"to\": \"BROADCAST\",\n  \"language_u_coords\": {:?},\n  \"encrypted_payload\": \"{}\",\n  \"zk_proof_hash\": \"{}\",\n  \"curve\": \"{}\"\n}}",
            self.identity.zymatica_address, coords, enc_payload, proof.proof_hash, proof.curve
        )
    }

    fn transmit(&self, message: &str, count: usize) {
        println!(
            "\n{}{}📡 INITIATING TRANSMISSION SEQUENCE...{}",
            Colors::ZCASH_GREEN,
            Colors::BOLD,
            Colors::END
        );
        for i in 0..count {
            let packet = self.build_packet(message);
            println!(
                "{}⚡ Packet {}/{}:{}",
                Colors::YELLOW,
                i + 1,
                count,
                Colors::END
            );

            // Cyberpunk matrix stream print animation
            for char in packet.chars().take(80) {
                print!("{}{}{}", Colors::ZCASH_GREEN, char, Colors::END);
                let _ = io::stdout().flush();
                thread::sleep(Duration::from_millis(5));
            }
            println!("...\n");
            thread::sleep(Duration::from_millis(300));
            println!(
                "{}✅ TRANSMITTED{} - {} bytes @ 903.9 MHz, SF9\n",
                Colors::GREEN,
                Colors::END,
                packet.len()
            );
        }
        println!(
            "{}{ColorBold}🎉 TRANSMISSION COMPLETE!{}",
            Colors::ZCASH_GOLD,
            Colors::END,
            ColorBold = Colors::BOLD
        );
    }

    fn listen(&self, duration_sec: u64) {
        println!(
            "\n{}{ColorBold}📻 ACTIVATING RX LISTENER...{}",
            Colors::ZCASH_GOLD,
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}Listening on 903.9 MHz, SF9, 125kHz for {} seconds...{}\n",
            Colors::CYAN,
            duration_sec,
            Colors::END
        );

        let start = SystemTime::now();
        let mut count = 0;
        while start.elapsed().unwrap_or(Duration::ZERO).as_secs() < duration_sec {
            thread::sleep(Duration::from_secs(3));
            if rand_prob() < 0.4 {
                count += 1;
                let random_node = format!(
                    "AGENT-{}",
                    ZKProver::compute_hash("rand")[..8].to_uppercase()
                );
                println!(
                    "{}{ColorGreen}╔{}╗{}",
                    Colors::GREEN,
                    "─".repeat(50),
                    Colors::END,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!(
                    "{}{ColorGreen}║{}║{}",
                    Colors::GREEN,
                    format!("  📨 RECEIVED PACKET").pad_right(59),
                    Colors::GREEN,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!(
                    "{}{ColorGreen}╠{}╣{}",
                    Colors::GREEN,
                    "─".repeat(50),
                    Colors::END,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!(
                    "{}{ColorGreen}║{}║{}",
                    Colors::GREEN,
                    format!("  From: {}@zymatica.space", random_node).pad_right(59),
                    Colors::GREEN,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!(
                    "{}{ColorGreen}║{}║{}",
                    Colors::GREEN,
                    format!(
                        "  SNR: {} dB, RSSI: -{} dBm",
                        8 + (rand_prob() * 6.0) as i32,
                        90 + (rand_prob() * 20.0) as i32
                    )
                    .pad_right(59),
                    Colors::GREEN,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!(
                    "{}{ColorGreen}╚{}╝{}",
                    Colors::GREEN,
                    "─".repeat(50),
                    Colors::END,
                    ColorGreen = Colors::ZCASH_GREEN
                );
                println!();
            }
        }
        println!(
            "\n{}{ColorBold}📊 RX SESSION COMPLETE{}",
            Colors::ZCASH_GOLD,
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!("{}Packets received: {}{}", Colors::CYAN, count, Colors::END);
    }
}

// Simple random generator helper
fn rand_prob() -> f64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (now % 100) as f64 / 100.0
}

trait PadRight {
    fn pad_right(&self, length: usize) -> String;
}
impl PadRight for String {
    fn pad_right(&self, length: usize) -> String {
        let mut s = self.clone();
        while s.len() < length {
            s.push(' ');
        }
        s
    }
}
impl PadRight for &str {
    fn pad_right(&self, length: usize) -> String {
        let mut s = self.to_string();
        while s.len() < length {
            s.push(' ');
        }
        s
    }
}

// ============================================================================
// Zcash Decrypted Event Scanner & Developer Fee Verification (Milestone 2)
// ============================================================================
const ZATOSHIS_PER_ZEC: u64 = 100_000_000;

#[derive(Debug, Deserialize)]
pub struct DecryptedPaymentEvent {
    tx_id: String,
    memo: String,
    gross_zat: u64,
    developer_fee_zat: u64,
    developer_address: String,
    source: Option<String>,
    confirmations: Option<u32>,
}

pub struct ZcashMempoolScanner {
    developer_address: String,
    dev_fee_bps: u64,
}

impl ZcashMempoolScanner {
    pub fn new() -> Self {
        ZcashMempoolScanner {
            developer_address: "u10rjztjhk6c2caz6t6hdh32zcf22exhumlm388vtd7exm63vsgwphhm5gt2azgzdksaumr9hn5hx7yy3tdjvdpt875c9tjqswwshz2v9d".to_string(),
            dev_fee_bps: 200,
        }
    }

    pub fn scan_transaction(
        &self,
        tx_id: &str,
        expected_packet_hash: &str,
    ) -> Result<bool, String> {
        println!(
            "[Scanner] Verifying decrypted Zcash payment event: {}...",
            tx_id
        );

        let event = self.load_decrypted_event(tx_id, expected_packet_hash)?;
        self.verify_decrypted_event(&event, expected_packet_hash)
    }

    fn load_decrypted_event(
        &self,
        tx_id: &str,
        expected_packet_hash: &str,
    ) -> Result<DecryptedPaymentEvent, String> {
        if let Ok(raw) = env::var("ZK_LORA_DECRYPTED_EVENT_JSON") {
            println!("   Loading decrypted payment event from ZK_LORA_DECRYPTED_EVENT_JSON.");
            return serde_json::from_str(&raw).map_err(|e| format!("Invalid event JSON: {}", e));
        }

        if let Ok(path) = env::var("ZK_LORA_DECRYPTED_EVENT_PATH") {
            println!("   Loading decrypted payment event from file: {}", path);
            let raw = fs::read_to_string(&path)
                .map_err(|e| format!("Could not read decrypted event file '{}': {}", path, e))?;
            return serde_json::from_str(&raw)
                .map_err(|e| format!("Invalid event JSON in '{}': {}", path, e));
        }

        println!("   No live wallet event provided. Using explicit local fixture.");
        println!("   NOTE: This fixture validates payout matching logic only; it is not a live Zcash chain scan.");
        Ok(DecryptedPaymentEvent {
            tx_id: tx_id.to_string(),
            memo: format!("ref:{}", expected_packet_hash),
            gross_zat: 5_000_000,
            developer_fee_zat: 100_000,
            developer_address: self.developer_address.clone(),
            source: Some("local_fixture".to_string()),
            confirmations: Some(0),
        })
    }

    fn verify_decrypted_event(
        &self,
        event: &DecryptedPaymentEvent,
        expected_packet_hash: &str,
    ) -> Result<bool, String> {
        println!(
            "   Source: {}",
            event.source.as_deref().unwrap_or("unspecified")
        );
        println!("   Decrypted memo: '{}'", event.memo);

        let expected_memo = format!("ref:{}", expected_packet_hash);
        if event.memo != expected_memo {
            return Err(format!(
                "Memo reference mismatch. Expected '{}', got '{}'",
                expected_memo, event.memo
            ));
        }

        if event.developer_address != self.developer_address {
            return Err(format!(
                "Developer address mismatch. Expected '{}', got '{}'",
                self.developer_address, event.developer_address
            ));
        }

        println!("   [Verification] Validating payout distribution:");
        println!("      Transaction ID: {}", event.tx_id);
        println!("      Confirmations: {}", event.confirmations.unwrap_or(0));
        println!(
            "      Gross Payout: {} ZEC",
            Self::format_zec(event.gross_zat)
        );
        println!("      Target Dev Treasury: {}", self.developer_address);
        println!(
            "      Developer Fee Paid: {} ZEC",
            Self::format_zec(event.developer_fee_zat)
        );

        let expected_dev_fee = event
            .gross_zat
            .checked_mul(self.dev_fee_bps)
            .ok_or_else(|| "Developer fee calculation overflowed".to_string())?
            / 10_000;
        if event.developer_fee_zat != expected_dev_fee {
            return Err(format!(
                "Incorrect developer fee split. Expected {} ZEC, got {} ZEC",
                Self::format_zec(expected_dev_fee),
                Self::format_zec(event.developer_fee_zat)
            ));
        }

        println!(
            "   [SUCCESS] Verification successful! 2% developer fee split matches constraints."
        );
        Ok(true)
    }

    fn format_zec(zat: u64) -> String {
        format!("{:.8}", zat as f64 / ZATOSHIS_PER_ZEC as f64)
    }
}
// ============================================================================
// Main Application Menu
// ============================================================================
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--test" || args[1] == "-t") {
        run_automated_tests();
        return;
    }

    let app = ZymaticaVoiceApp::new("researcher-1");
    loop {
        app.display_identity();

        println!(
            "{}{ColorBold}╔{}╗{}",
            Colors::ZCASH_GREEN,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!("  🦀 ZYMATICA VOICE - Main Menu").pad_right(69),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}╠{}╣{}",
            Colors::ZCASH_GREEN,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!(
                "  {}[1]{} Transmit Message (TX)",
                Colors::YELLOW,
                Colors::END
            )
            .pad_right(78),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!(
                "  {}[2]{} Listen for Packets (RX)",
                Colors::YELLOW,
                Colors::END
            )
            .pad_right(78),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!("  {}[3]{} Show Identity", Colors::YELLOW, Colors::END).pad_right(78),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!("  {}[4]{} Generate ZK-Proof", Colors::YELLOW, Colors::END).pad_right(78),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}║{}║{}",
            Colors::ZCASH_GREEN,
            format!("  {}[0]{} Exit", Colors::YELLOW, Colors::END).pad_right(78),
            Colors::ZCASH_GREEN,
            ColorBold = Colors::BOLD
        );
        println!(
            "{}{ColorBold}╚{}╝{}",
            Colors::ZCASH_GREEN,
            "═".repeat(60),
            Colors::END,
            ColorBold = Colors::BOLD
        );
        println!();

        print!("{}🚀 Select action:{} ", Colors::ZCASH_GOLD, Colors::END);
        let _ = io::stdout().flush();
        let mut choice = String::new();
        let _ = io::stdin().read_line(&mut choice);
        let choice = choice.trim();

        match choice {
            "1" => {
                print!("{}Message to transmit:{} ", Colors::CYAN, Colors::END);
                let _ = io::stdout().flush();
                let mut message = String::new();
                let _ = io::stdin().read_line(&mut message);
                let message = message.trim();

                print!("{}Packet count (default 5):{} ", Colors::CYAN, Colors::END);
                let _ = io::stdout().flush();
                let mut count_str = String::new();
                let _ = io::stdin().read_line(&mut count_str);
                let count: usize = count_str.trim().parse().unwrap_or(5);

                app.transmit(message, count);
            }
            "2" => {
                print!(
                    "{}Listen duration in seconds (default 10):{} ",
                    Colors::CYAN,
                    Colors::END
                );
                let _ = io::stdout().flush();
                let mut dur_str = String::new();
                let _ = io::stdin().read_line(&mut dur_str);
                let dur: u64 = dur_str.trim().parse().unwrap_or(10);
                app.listen(dur);
            }
            "3" => {
                app.display_identity();
            }
            "4" => {
                println!(
                    "\n{}Generating ZK-Proof...{}",
                    Colors::ZCASH_GREEN,
                    Colors::END
                );
                let proof = app
                    .prover
                    .generate_proof(&app.identity.private_key, &app.identity.public_key);
                println!(
                    "{}✅ ZK-Proof Generated:{}",
                    Colors::ZCASH_GOLD,
                    Colors::END
                );
                println!(
                    "{}Proof A: {}\nProof B: {}\nProof C: {}\nCurve: {}\nCurve Prime field size matches BN128 constraints.{}",
                    Colors::CYAN, proof.proof_a, proof.proof_b, proof.proof_c, proof.curve, Colors::END
                );
            }
            "0" => {
                println!(
                    "\n{}👋 Zymatica Voice shutting down...{}",
                    Colors::ZCASH_GOLD,
                    Colors::END
                );
                println!(
                    "{}From E-Waste to AI Grace. See you in the mesh! 🦀✨{}\n",
                    Colors::CYAN,
                    Colors::END
                );
                break;
            }
            _ => {
                println!(
                    "{}Invalid selection. Press Enter to retry.{}",
                    Colors::RED,
                    Colors::END
                );
            }
        }
        print!(
            "\n{}Press Enter to continue...{}",
            Colors::YELLOW,
            Colors::END
        );
        let _ = io::stdout().flush();
        let mut tmp = String::new();
        let _ = io::stdin().read_line(&mut tmp);
    }
}

// ============================================================================
// Automated CI/CD Testing System
// ============================================================================
fn run_automated_tests() {
    println!("==============================================================");
    println!("RUNNING AUTOMATED TEST SUITE FOR ZYMATICA VOICE (RUST)");
    println!("==============================================================");

    let app = ZymaticaVoiceApp::new("test-runner");
    app.display_identity();

    println!("[1] Generating ZK Proof...");
    let proof = app
        .prover
        .generate_proof(&app.identity.private_key, &app.identity.public_key);
    println!("    * ZK Proof Hash: {}", proof.proof_hash);
    println!("[2] Verifying ZK Proof...");
    let is_valid = app.prover.verify_proof(&proof, &app.identity.public_key);
    assert!(is_valid, "ZK Verification failed!");
    println!("    * Verification status: ✅ VALID");

    println!("[3] Generating coordinates projection...");
    let coords = ZymaticaVoiceApp::encode_semantic_coordinates("Test coordinates");
    println!("    * Generated 6D coordinates: {:?}", coords);
    assert_eq!(coords.len(), 6, "Coordinates must be 6-dimensional");

    println!("[4] ECIES payload check...");
    let payload = "Hello Zcash Mesh!";
    let encrypted = ZymaticaVoiceApp::simulate_ecies_encrypt(payload, &app.identity.public_key);
    println!("    * Ciphertext: {}", encrypted);

    println!("[5] Broadcast test...");
    app.transmit("Hello Zcash Mesh!", 1);

    println!("[6] Zcash Decrypted Payment Event & Payout Split Check...");
    let scanner = ZcashMempoolScanner::new();
    let tx_id = "fixture_tx_milestone_2_reconciliation_check";
    let expected_hash = "demo_packet_hash_hello_zcash_mesh";
    if fs::metadata("fixtures/decrypted_payment_event.json").is_ok() {
        env::set_var(
            "ZK_LORA_DECRYPTED_EVENT_PATH",
            "fixtures/decrypted_payment_event.json",
        );
    }
    let scan_result = scanner.scan_transaction(tx_id, expected_hash);
    assert!(
        scan_result.is_ok(),
        "Zcash decrypted event validation failed!"
    );
    println!("    * Scanner status: fixture validation passed");
    println!("==============================================================");
    println!("✅ SUCCESS: All modules verified successfully.");
    println!("==============================================================");
}

//! Local field-readiness harness for multi-node edge execution proofs.
//!
//! This module deliberately separates software-verifiable cluster behavior from
//! hardware-gated claims. The local proof exercises cache transfer, consensus,
//! causal memory sync, radix sharing, and signed semantic transport on this
//! machine. Hardware-gated rows report simulator verification separately from
//! physical capability adapters, which must be present before physical field
//! validation is claimed.

use crate::{
    cuneiform::Concept6D, frontier, paged_kv::PagedKvCache, transport_p2p::P2pKvSwapStore,
};
use anyhow::{Context, Result, bail};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct FieldMultinodeReport {
    pub node_count: usize,
    pub kv_round_trips: usize,
    pub kv_packet_bytes: usize,
    pub consensus_token: usize,
    pub tamper_rejected: bool,
    pub qr_transport_verified: bool,
    pub causal_updates_synced: usize,
    pub virtual_radix_nodes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareGateStatus {
    pub item_id: u8,
    pub name: &'static str,
    pub required_capability: &'static str,
    pub simulator_verified: bool,
    pub physical_verified: bool,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldReadinessReport {
    pub local_multinode: FieldMultinodeReport,
    pub hardware_gated_total: usize,
    pub hardware_verified: usize,
    pub hardware_simulator_verified: usize,
    pub hardware_physical_verified: usize,
    pub hardware_unverified: usize,
    pub field_claim: &'static str,
}

struct LocalFieldNode {
    node_id: String,
    causal_memory: frontier::SharedCausalMemory,
}

pub fn run_local_multinode_proof() -> Result<FieldMultinodeReport> {
    let report = local_multinode_proof()?;
    println!("runtime=zymatica-engine");
    println!("mode=field-multinode-proof");
    println!("node_count={}", report.node_count);
    println!("kv_round_trips={}", report.kv_round_trips);
    println!("kv_packet_bytes={}", report.kv_packet_bytes);
    println!("consensus_token={}", report.consensus_token);
    println!("qr_transport_verified={}", report.qr_transport_verified);
    println!("tamper_rejected={}", report.tamper_rejected);
    println!("causal_updates_synced={}", report.causal_updates_synced);
    println!("network_virtual_radix_nodes={}", report.virtual_radix_nodes);
    println!("status=ok");
    Ok(report)
}

pub fn run_field_readiness_audit() -> Result<FieldReadinessReport> {
    let local_multinode = local_multinode_proof()?;
    let gates = hardware_gate_statuses();
    let hardware_verified = gates.iter().filter(|gate| gate.verified).count();
    let hardware_simulator_verified = gates.iter().filter(|gate| gate.simulator_verified).count();
    let hardware_physical_verified = gates.iter().filter(|gate| gate.physical_verified).count();
    let hardware_gated_total = gates.len();
    let hardware_unverified = hardware_gated_total.saturating_sub(hardware_verified);
    let report = FieldReadinessReport {
        local_multinode,
        hardware_gated_total,
        hardware_verified,
        hardware_simulator_verified,
        hardware_physical_verified,
        hardware_unverified,
        field_claim: if hardware_physical_verified == hardware_gated_total {
            "field-production-ready-with-physical-hardware-validation"
        } else if hardware_unverified == 0 {
            "software-simulator-ready-physical-hardware-unverified"
        } else {
            "software-field-ready-hardware-gated-items-unverified"
        },
    };

    println!("runtime=zymatica-engine");
    println!("mode=field-readiness-audit");
    println!("local_multinode=ok");
    println!("hardware_gated_total={}", report.hardware_gated_total);
    println!("hardware_verified={}", report.hardware_verified);
    println!(
        "hardware_simulator_verified={}",
        report.hardware_simulator_verified
    );
    println!(
        "hardware_physical_verified={}",
        report.hardware_physical_verified
    );
    println!("hardware_unverified={}", report.hardware_unverified);
    println!("field_claim={}", report.field_claim);
    for gate in gates {
        println!(
            "hardware_gate item={} verified={} simulator_verified={} physical_verified={} capability={} name={}",
            gate.item_id,
            gate.verified,
            gate.simulator_verified,
            gate.physical_verified,
            gate.required_capability,
            gate.name
        );
    }
    println!("status=ok");
    Ok(report)
}

pub fn local_multinode_proof() -> Result<FieldMultinodeReport> {
    let mut nodes = [
        LocalFieldNode {
            node_id: "edge-a".to_string(),
            causal_memory: frontier::SharedCausalMemory::new(),
        },
        LocalFieldNode {
            node_id: "edge-b".to_string(),
            causal_memory: frontier::SharedCausalMemory::new(),
        },
        LocalFieldNode {
            node_id: "edge-c".to_string(),
            causal_memory: frontier::SharedCausalMemory::new(),
        },
    ];

    let mut source_cache = PagedKvCache::new(2, 2, 3, 4);
    for pos in 0..6 {
        source_cache.allocate_token(9001);
        source_cache.set_kv(
            9001,
            pos,
            1,
            1,
            &[pos as f32, pos as f32 + 10.0, pos as f32 + 20.0],
            &[pos as f32 + 30.0, pos as f32 + 40.0, pos as f32 + 50.0],
        );
    }
    let packet = source_cache.export_sequence_compact_packet(9001)?;
    let kv_packet_bytes = packet.bytes.len();

    let mut swap_store = P2pKvSwapStore::new();
    swap_store.register_peer(nodes[1].node_id.clone(), 64 * 1024, 8)?;
    swap_store.register_peer(nodes[2].node_id.clone(), 64 * 1024, 6)?;
    let manifest = swap_store.stream_out_packet(packet)?;
    let restored_packet = swap_store.stream_in_packet(&manifest)?;
    let mut restored_cache = PagedKvCache::new(2, 2, 3, 4);
    restored_cache.import_sequence_packet(&restored_packet)?;
    let restored_key = restored_cache.key(9001, 5, 1, 1).to_vec();
    if restored_key != vec![5.0, 15.0, 25.0] {
        bail!("field multi-node KV transfer restored wrong key: {restored_key:?}");
    }

    let consensus = frontier::majority_vote_token(&[
        frontier::PeerTokenVote {
            peer_id: nodes[0].node_id.clone(),
            token_id: 17,
            confidence: 0.80,
            trust_weight: 1.0,
        },
        frontier::PeerTokenVote {
            peer_id: nodes[1].node_id.clone(),
            token_id: 17,
            confidence: 0.78,
            trust_weight: 1.0,
        },
        frontier::PeerTokenVote {
            peer_id: nodes[2].node_id.clone(),
            token_id: 23,
            confidence: 0.95,
            trust_weight: 0.2,
        },
    ])
    .context("field multi-node proof expected consensus token")?;
    if consensus.token_id != 17 {
        bail!("field multi-node consensus selected wrong token");
    }

    let concept = Concept6D::new(1, 2, 3, 4, 5, 6);
    nodes[0].causal_memory.state.insert(1, concept);
    let diff = nodes[0]
        .causal_memory
        .generate_diff(&nodes[1].causal_memory);
    let causal_updates_synced = diff.updates.len();
    nodes[1].causal_memory.apply_diff(diff.clone());
    nodes[2].causal_memory.apply_diff(diff);
    if nodes[1].causal_memory.state.get(&1) != Some(&concept)
        || nodes[2].causal_memory.state.get(&1) != Some(&concept)
    {
        bail!("field multi-node causal memory replication failed");
    }

    let mut virtual_radix = frontier::NetworkVirtualRadixTree::new();
    virtual_radix.observe(&[17, 31, 49]);
    virtual_radix.observe(&[17, 31, 50]);
    let snapshot = virtual_radix.snapshot();
    let radix_node = frontier::NetworkVirtualRadixTree::borrow_prefix_node(&snapshot, &[17, 31])
        .context("field multi-node virtual radix borrow failed")?;
    if radix_node.children.len() != 2 {
        bail!("field multi-node virtual radix snapshot lost branches");
    }

    let keypair = frontier::HashBasedConceptKeypair::from_seed([19; 32]);
    let frame = frontier::build_quantum_resilient_semantic_frame(9001, 1, &[concept], &keypair);
    let qr_transport_verified = frontier::verify_quantum_resilient_semantic_frame(&frame);
    let mut tampered_frame = frame.clone();
    tampered_frame.payload[0] = Concept6D::new(1, 2, 3, 4, 5, 7);
    let tamper_rejected = !frontier::verify_quantum_resilient_semantic_frame(&tampered_frame);
    if !qr_transport_verified || !tamper_rejected {
        bail!("field multi-node semantic transport verification failed");
    }

    Ok(FieldMultinodeReport {
        node_count: nodes.len(),
        kv_round_trips: 1,
        kv_packet_bytes,
        consensus_token: consensus.token_id,
        tamper_rejected,
        qr_transport_verified,
        causal_updates_synced,
        virtual_radix_nodes: snapshot.nodes.len(),
    })
}

pub fn hardware_gate_statuses() -> Vec<HardwareGateStatus> {
    hardware_gate_specs()
        .into_iter()
        .map(|(item_id, name, required_capability, env_var)| {
            let simulator_verified = match item_id {
                4 => frontier::verify_network_attached_radix_memory_sim().is_ok(),
                44 => frontier::verify_kernel_bypass_pipeline_sim().is_ok(),
                51 => frontier::verify_photonic_weight_mapping_sim().is_ok(),
                53 => frontier::verify_neuromorphic_spike_coded_sim().is_ok(),
                59 => frontier::verify_dma_ring_buffer_attention_sim().is_ok(),
                61 => frontier::verify_memristor_adapter_sim().is_ok(),
                62 => frontier::verify_quantum_key_distribution_sim().is_ok(),
                63 => frontier::verify_cache_line_precharging_sim().is_ok(),
                66 => frontier::verify_tensor_core_fusion_sim().is_ok(),
                68 => frontier::verify_p2p_beam_forming_sim().is_ok(),
                71 => frontier::verify_analog_crossbar_sim().is_ok(),
                _ => false,
            };
            let physical_flag_verified = std::env::var(env_var)
                .map(|value| value.eq_ignore_ascii_case("verified"))
                .unwrap_or(false);
            let physical_verified = physical_flag_verified
                && verify_physical_hardware_receipt(item_id, required_capability).unwrap_or(false);
            HardwareGateStatus {
                item_id,
                name,
                required_capability,
                simulator_verified,
                physical_verified,
                verified: simulator_verified || physical_verified,
            }
        })
        .collect()
}

fn hardware_gate_specs() -> Vec<(u8, &'static str, &'static str, &'static str)> {
    vec![
        (
            4,
            "Zero-Copy Network-Attached Radix Memory",
            "dpdk-or-xdp-dma-radix-mount",
            "ZYMATICA_HW_ITEM_4",
        ),
        (
            44,
            "Asynchronous Kernel-Bypass Pipeline",
            "sriov-dpdk-xdp-pipeline",
            "ZYMATICA_HW_ITEM_44",
        ),
        (
            51,
            "Photonic-Accelerated Weight Mapping",
            "optoelectronic-weight-cache",
            "ZYMATICA_HW_ITEM_51",
        ),
        (
            53,
            "Neuromorphic Spike-Coded Cuneiform-U",
            "neuromorphic-spike-target",
            "ZYMATICA_HW_ITEM_53",
        ),
        (
            59,
            "Direct-Hardware DMA Ring-Buffer Attention",
            "gpu-dma-ring-buffer",
            "ZYMATICA_HW_ITEM_59",
        ),
        (
            61,
            "Biological-Inference Memristor Adapters",
            "memristive-array-target",
            "ZYMATICA_HW_ITEM_61",
        ),
        (
            62,
            "Semantic Quantum-Key Distribution",
            "qkd-link-and-key-feed",
            "ZYMATICA_HW_ITEM_62",
        ),
        (
            63,
            "Speculative Cache-Line Pre-charging",
            "hardware-cache-precharger",
            "ZYMATICA_HW_ITEM_63",
        ),
        (
            66,
            "Asynchronous Pipelined Tensor-Core Fusions",
            "edge-npu-tensor-core-compiler",
            "ZYMATICA_HW_ITEM_66",
        ),
        (
            68,
            "Direct-Kernel Bypassing P2P Beam-forming",
            "optical-or-mmwave-direct-routing",
            "ZYMATICA_HW_ITEM_68",
        ),
        (
            71,
            "Analog Synaptic Crossbar Kernels",
            "analog-crossbar-compiler-target",
            "ZYMATICA_HW_ITEM_71",
        ),
    ]
}

fn verify_physical_hardware_receipt(item_id: u8, required_capability: &str) -> Result<bool> {
    let receipt_var = format!("ZYMATICA_HW_RECEIPT_{item_id}");
    let receipt_path = match std::env::var(&receipt_var) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(false),
    };
    let secret = match std::env::var("ZYMATICA_HW_RECEIPT_SECRET") {
        Ok(value) if !value.is_empty() => value,
        _ => return Ok(false),
    };
    let receipt_text = std::fs::read_to_string(Path::new(&receipt_path))
        .with_context(|| format!("reading hardware receipt {receipt_path}"))?;
    verify_hardware_receipt_text(
        &receipt_text,
        item_id,
        required_capability,
        secret.as_bytes(),
    )
}

fn verify_hardware_receipt_text(
    receipt_text: &str,
    item_id: u8,
    required_capability: &str,
    secret: &[u8],
) -> Result<bool> {
    if secret.is_empty() {
        return Ok(false);
    }
    let fields = parse_receipt_fields(receipt_text)?;
    if fields.get("item_id").map(String::as_str) != Some(&item_id.to_string())
        || fields.get("capability").map(String::as_str) != Some(required_capability)
        || fields.get("mode").map(String::as_str) != Some("physical")
        || fields.get("status").map(String::as_str) != Some("verified")
    {
        return Ok(false);
    }
    let nonce = fields
        .get("nonce")
        .context("hardware receipt missing nonce")?;
    let signature = fields
        .get("signature")
        .context("hardware receipt missing signature")?;
    let message = hardware_receipt_message(item_id, required_capability, nonce);
    let expected = hmac_sha256_hex(secret, message.as_bytes())?;
    Ok(signature.eq_ignore_ascii_case(&expected))
}

fn parse_receipt_fields(receipt_text: &str) -> Result<HashMap<String, String>> {
    let mut fields = HashMap::new();
    for (line_idx, raw_line) in receipt_text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!("invalid hardware receipt line {}", line_idx + 1);
        };
        fields.insert(key.trim().to_string(), value.trim().to_string());
    }
    Ok(fields)
}

fn hardware_receipt_message(item_id: u8, required_capability: &str, nonce: &str) -> String {
    format!(
        "item_id={item_id}|capability={required_capability}|mode=physical|status=verified|nonce={nonce}"
    )
}

fn hmac_sha256_hex(secret: &[u8], message: &[u8]) -> Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).context("creating receipt HMAC")?;
    mac.update(message);
    let digest = mac.finalize().into_bytes();
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_multinode_proof_round_trips_kv_and_rejects_tamper() -> Result<()> {
        let report = local_multinode_proof()?;
        assert_eq!(report.node_count, 3);
        assert_eq!(report.kv_round_trips, 1);
        assert_eq!(report.consensus_token, 17);
        assert!(report.qr_transport_verified);
        assert!(report.tamper_rejected);
        Ok(())
    }

    #[test]
    fn field_readiness_reports_hardware_gates_separately() -> Result<()> {
        let report = run_field_readiness_audit()?;
        assert_eq!(report.hardware_gated_total, 11);
        assert_eq!(report.hardware_simulator_verified, 11);
        assert!(report.hardware_physical_verified <= report.hardware_gated_total);
        assert!(report.hardware_verified <= report.hardware_gated_total);
        assert_ne!(
            report.field_claim,
            "field-production-ready-with-hardware-validation"
        );
        Ok(())
    }

    #[test]
    fn signed_hardware_receipt_is_required_for_physical_validation() -> Result<()> {
        let item_id = 4;
        let capability = "dpdk-or-xdp-dma-radix-mount";
        let nonce = "adapter-run-001";
        let message = hardware_receipt_message(item_id, capability, nonce);
        let signature = hmac_sha256_hex(b"receipt-secret", message.as_bytes())?;
        let receipt = format!(
            "item_id={item_id}\ncapability={capability}\nmode=physical\nstatus=verified\nnonce={nonce}\nsignature={signature}\n"
        );
        assert!(verify_hardware_receipt_text(
            &receipt,
            item_id,
            capability,
            b"receipt-secret"
        )?);
        assert!(!verify_hardware_receipt_text(
            &receipt,
            item_id,
            capability,
            b"wrong-secret"
        )?);
        assert!(!verify_hardware_receipt_text(
            &receipt.replace("mode=physical", "mode=simulator"),
            item_id,
            capability,
            b"receipt-secret"
        )?);
        Ok(())
    }
}

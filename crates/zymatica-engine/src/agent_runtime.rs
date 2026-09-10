use crate::cuneiform::Concept6D;
use anyhow::{Context, Result, bail};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentIdentity {
    pub id: String,
    pub public_key_hex: String,
}

pub struct AgentKeypair {
    signing_key: SigningKey,
    identity: AgentIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedEnvelope {
    pub identity: AgentIdentity,
    pub payload: Value,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    pub tool: String,
    pub ok: bool,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyKernel {
    pub allowed_tools: BTreeSet<String>,
    pub max_input_bytes: usize,
    pub require_signature: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
    pub concept: [u8; 6],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticMemory {
    entries: BTreeMap<String, MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryHit {
    pub id: String,
    pub text: String,
    pub distance: u32,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEvent {
    pub seq: u64,
    pub event_type: String,
    pub payload: Value,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct DurableAgentLog {
    path: PathBuf,
    next_seq: u64,
    previous_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Blackboard {
    pub messages: Vec<BlackboardMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlackboardMessage {
    pub role: String,
    pub content: String,
    pub concept: Option<[u8; 6]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpManifest {
    pub protocol: String,
    pub server_name: String,
    pub tools: Vec<ToolSpec>,
    pub resources: Vec<String>,
    pub prompts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCard {
    pub protocol: String,
    pub name: String,
    pub identity: AgentIdentity,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct A2aMessage {
    pub from: AgentIdentity,
    pub to_agent: String,
    pub message_type: String,
    pub body: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProofReport {
    pub event_count: usize,
    pub final_hash: String,
    pub hash_tool_output: String,
    pub wasm_add_output: i32,
    pub memory_hit: String,
    pub signature_verified: bool,
}

impl AgentKeypair {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        let public_key_hex = hex_encode(&verifying_key.to_bytes());
        let id = format!("agent-{}", &public_key_hex[..16]);
        Self {
            signing_key,
            identity: AgentIdentity { id, public_key_hex },
        }
    }

    pub fn identity(&self) -> AgentIdentity {
        self.identity.clone()
    }

    pub fn sign_payload(&self, payload: Value) -> Result<SignedEnvelope> {
        let bytes = canonical_payload_bytes(&payload)?;
        let signature = self.signing_key.sign(&bytes);
        Ok(SignedEnvelope {
            identity: self.identity(),
            payload,
            signature_hex: hex_encode(&signature.to_bytes()),
        })
    }
}

impl SignedEnvelope {
    pub fn verify(&self) -> Result<()> {
        let key_bytes = hex_decode_fixed::<32>(&self.identity.public_key_hex)?;
        let sig_bytes = hex_decode_fixed::<64>(&self.signature_hex)?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)?;
        let signature = Signature::from_bytes(&sig_bytes);
        let payload = canonical_payload_bytes(&self.payload)?;
        verifying_key
            .verify(&payload, &signature)
            .context("verifying signed agent envelope")
    }
}

impl Default for PolicyKernel {
    fn default() -> Self {
        Self {
            allowed_tools: ["hash.sha256", "memory.put", "memory.search", "wasm.add_i32"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            max_input_bytes: 4096,
            require_signature: true,
        }
    }
}

impl PolicyKernel {
    pub fn validate(&self, call: &ToolCall, envelope: Option<&SignedEnvelope>) -> Result<()> {
        if !self.allowed_tools.contains(&call.tool) {
            bail!("tool '{}' is not allowed by policy", call.tool);
        }
        let input_bytes = serde_json::to_vec(call).context("serializing tool call for policy")?;
        if input_bytes.len() > self.max_input_bytes {
            bail!(
                "tool call input is {} bytes, exceeding policy max {}",
                input_bytes.len(),
                self.max_input_bytes
            );
        }
        if self.require_signature {
            envelope
                .context("policy requires a signed envelope")?
                .verify()
                .context("policy signature check failed")?;
        }
        Ok(())
    }
}

impl SemanticMemory {
    pub fn put(&mut self, id: impl Into<String>, text: impl Into<String>, concept: Concept6D) {
        let id = id.into();
        self.entries.insert(
            id.clone(),
            MemoryEntry {
                id,
                text: text.into(),
                concept: concept.axes(),
            },
        );
    }

    pub fn get(&self, id: &str) -> Option<&MemoryEntry> {
        self.entries.get(id)
    }

    pub fn search_by_concept(&self, target: Concept6D, limit: usize) -> Vec<MemoryHit> {
        let mut hits: Vec<_> = self
            .entries
            .values()
            .map(|entry| {
                let concept = concept_from_axes(entry.concept);
                let distance = concept.manhattan_distance(target);
                MemoryHit {
                    id: entry.id.clone(),
                    text: entry.text.clone(),
                    distance,
                    similarity: concept.normalized_similarity(target),
                }
            })
            .collect();
        hits.sort_by(|a, b| a.distance.cmp(&b.distance).then_with(|| a.id.cmp(&b.id)));
        hits.truncate(limit);
        hits
    }
}

impl DurableAgentLog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating agent log dir {}", parent.display()))?;
        }
        let existing = if path.exists() {
            Self::read_events(&path)?
        } else {
            Vec::new()
        };
        let next_seq = existing.last().map(|event| event.seq + 1).unwrap_or(0);
        let previous_hash = existing
            .last()
            .map(|event| event.hash.clone())
            .unwrap_or_else(|| "0".repeat(64));
        Ok(Self {
            path,
            next_seq,
            previous_hash,
        })
    }

    pub fn append(&mut self, event_type: impl Into<String>, payload: Value) -> Result<AgentEvent> {
        let seq = self.next_seq;
        let event_type = event_type.into();
        let previous_hash = self.previous_hash.clone();
        let hash = hash_event(seq, &event_type, &payload, &previous_hash)?;
        let event = AgentEvent {
            seq,
            event_type,
            payload,
            previous_hash,
            hash,
        };
        let line = serde_json::to_string(&event).context("serializing agent event")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening agent log {}", self.path.display()))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        self.next_seq += 1;
        self.previous_hash = event.hash.clone();
        Ok(event)
    }

    pub fn read_events(path: impl AsRef<Path>) -> Result<Vec<AgentEvent>> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading agent log {}", path.display()))?;
        let mut previous_hash = "0".repeat(64);
        for (idx, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let event: AgentEvent = serde_json::from_str(line)
                .with_context(|| format!("parsing agent log line {}", idx + 1))?;
            if event.seq != events.len() as u64 {
                bail!("agent log sequence mismatch at line {}", idx + 1);
            }
            if event.previous_hash != previous_hash {
                bail!("agent log hash chain mismatch at line {}", idx + 1);
            }
            let expected = hash_event(
                event.seq,
                &event.event_type,
                &event.payload,
                &event.previous_hash,
            )?;
            if expected != event.hash {
                bail!("agent log event hash mismatch at line {}", idx + 1);
            }
            previous_hash = event.hash.clone();
            events.push(event);
        }
        Ok(events)
    }
}

impl Blackboard {
    pub fn post(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        concept: Option<Concept6D>,
    ) {
        self.messages.push(BlackboardMessage {
            role: role.into(),
            content: content.into(),
            concept: concept.map(Concept6D::axes),
        });
    }

    pub fn latest_by_role(&self, role: &str) -> Option<&BlackboardMessage> {
        self.messages.iter().rev().find(|msg| msg.role == role)
    }
}

pub fn default_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "hash.sha256".to_string(),
            description: "Hash UTF-8 text with SHA-256".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["text"],
                "properties": {"text": {"type": "string"}}
            }),
        },
        ToolSpec {
            name: "memory.put".to_string(),
            description: "Store text at a Cuneiform-U coordinate".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["id", "text", "concept"],
                "properties": {
                    "id": {"type": "string"},
                    "text": {"type": "string"},
                    "concept": {"type": "array", "items": {"type": "integer"}, "minItems": 6, "maxItems": 6}
                }
            }),
        },
        ToolSpec {
            name: "memory.search".to_string(),
            description: "Search semantic memory by Cuneiform-U distance".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["concept"],
                "properties": {
                    "concept": {"type": "array", "items": {"type": "integer"}, "minItems": 6, "maxItems": 6},
                    "limit": {"type": "integer"}
                }
            }),
        },
        ToolSpec {
            name: "wasm.add_i32".to_string(),
            description: "Run a sandboxed WASM i32 addition function".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["a", "b"],
                "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}}
            }),
        },
    ]
}

pub fn execute_tool(
    call: &ToolCall,
    memory: &mut SemanticMemory,
    policy: &PolicyKernel,
    envelope: Option<&SignedEnvelope>,
) -> Result<ToolResult> {
    policy.validate(call, envelope)?;
    let output = match call.tool.as_str() {
        "hash.sha256" => {
            let text = call
                .args
                .get("text")
                .and_then(Value::as_str)
                .context("hash.sha256 requires args.text string")?;
            json!({"sha256": hex_encode(Sha256::digest(text.as_bytes()).as_slice())})
        }
        "memory.put" => {
            let id = call
                .args
                .get("id")
                .and_then(Value::as_str)
                .context("memory.put requires args.id string")?;
            let text = call
                .args
                .get("text")
                .and_then(Value::as_str)
                .context("memory.put requires args.text string")?;
            let concept = concept_arg(&call.args)?;
            memory.put(id, text, concept);
            json!({"stored": id})
        }
        "memory.search" => {
            let concept = concept_arg(&call.args)?;
            let limit = call.args.get("limit").and_then(Value::as_u64).unwrap_or(4) as usize;
            json!({"hits": memory.search_by_concept(concept, limit)})
        }
        "wasm.add_i32" => {
            let a = call
                .args
                .get("a")
                .and_then(Value::as_i64)
                .context("wasm.add_i32 requires args.a integer")? as i32;
            let b = call
                .args
                .get("b")
                .and_then(Value::as_i64)
                .context("wasm.add_i32 requires args.b integer")? as i32;
            json!({"value": run_wasm_add_i32(a, b)?})
        }
        other => bail!("unknown tool {other}"),
    };
    Ok(ToolResult {
        tool: call.tool.clone(),
        ok: true,
        output,
    })
}

pub fn mcp_manifest() -> McpManifest {
    McpManifest {
        protocol: "mcp-compatible-manifest".to_string(),
        server_name: "zymatica-agent-runtime".to_string(),
        tools: default_tool_specs(),
        resources: vec![
            "zymatica://memory/semantic".to_string(),
            "zymatica://kv/cache-packets".to_string(),
            "zymatica://trace/wal".to_string(),
        ],
        prompts: vec![
            "zymatica.agent.plan".to_string(),
            "zymatica.agent.verify".to_string(),
            "zymatica.agent.repair-json".to_string(),
        ],
    }
}

pub fn agent_card(identity: AgentIdentity) -> AgentCard {
    AgentCard {
        protocol: "a2a-compatible-card".to_string(),
        name: "zymatica-edge-agent".to_string(),
        identity,
        capabilities: vec![
            "openai-compatible-text".to_string(),
            "mcp-tools".to_string(),
            "signed-tool-calls".to_string(),
            "durable-wal".to_string(),
            "semantic-memory".to_string(),
            "cache-to-cache-packets".to_string(),
        ],
    }
}

pub fn run_agent_runtime_proof(log_path: impl AsRef<Path>) -> Result<AgentProofReport> {
    let log_path = log_path.as_ref();
    if log_path.exists() {
        fs::remove_file(log_path)
            .with_context(|| format!("removing old agent proof log {}", log_path.display()))?;
    }
    let keypair = AgentKeypair::from_seed([7_u8; 32]);
    let mut log = DurableAgentLog::open(log_path)?;
    let mut memory = SemanticMemory::default();
    let policy = PolicyKernel::default();
    let mut blackboard = Blackboard::default();
    let concept = Concept6D::new(1, 2, 3, 4, 5, 6);

    blackboard.post("planner", "hash prompt and store memory", Some(concept));
    log.append("blackboard.post", json!(blackboard.messages.last()))?;

    let hash_call = ToolCall {
        tool: "hash.sha256".to_string(),
        args: json!({"text": "Zymatica real agent runtime proof"}),
    };
    let hash_envelope = keypair.sign_payload(serde_json::to_value(&hash_call)?)?;
    let signature_verified = hash_envelope.verify().is_ok();
    let hash_result = execute_tool(&hash_call, &mut memory, &policy, Some(&hash_envelope))?;
    log.append("tool.result", serde_json::to_value(&hash_result)?)?;

    let put_call = ToolCall {
        tool: "memory.put".to_string(),
        args: json!({
            "id": "proof-memory",
            "text": "Cuneiform memory anchor",
            "concept": concept.axes()
        }),
    };
    let put_envelope = keypair.sign_payload(serde_json::to_value(&put_call)?)?;
    let put_result = execute_tool(&put_call, &mut memory, &policy, Some(&put_envelope))?;
    log.append("tool.result", serde_json::to_value(&put_result)?)?;

    let wasm_call = ToolCall {
        tool: "wasm.add_i32".to_string(),
        args: json!({"a": 28, "b": 14}),
    };
    let wasm_envelope = keypair.sign_payload(serde_json::to_value(&wasm_call)?)?;
    let wasm_result = execute_tool(&wasm_call, &mut memory, &policy, Some(&wasm_envelope))?;
    log.append("tool.result", serde_json::to_value(&wasm_result)?)?;

    let search_call = ToolCall {
        tool: "memory.search".to_string(),
        args: json!({"concept": concept.axes(), "limit": 1}),
    };
    let search_envelope = keypair.sign_payload(serde_json::to_value(&search_call)?)?;
    let search_result = execute_tool(&search_call, &mut memory, &policy, Some(&search_envelope))?;
    log.append("tool.result", serde_json::to_value(&search_result)?)?;

    let events = DurableAgentLog::read_events(log_path)?;
    let final_hash = events
        .last()
        .map(|event| event.hash.clone())
        .context("agent proof produced no events")?;
    let hash_tool_output = hash_result
        .output
        .get("sha256")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let wasm_add_output = wasm_result
        .output
        .get("value")
        .and_then(Value::as_i64)
        .unwrap_or_default() as i32;
    let memory_hit = search_result
        .output
        .get("hits")
        .and_then(Value::as_array)
        .and_then(|hits| hits.first())
        .and_then(|hit| hit.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Ok(AgentProofReport {
        event_count: events.len(),
        final_hash,
        hash_tool_output,
        wasm_add_output,
        memory_hit,
        signature_verified,
    })
}

pub fn run_wasm_add_i32(a: i32, b: i32) -> Result<i32> {
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(
        &engine,
        r#"
        (module
          (func (export "add_i32") (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.add))
        "#,
    )
    .context("compiling sandboxed WASM tool")?;
    let mut store = wasmi::Store::new(&engine, ());
    let linker = wasmi::Linker::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .context("instantiating sandboxed WASM tool")?;
    let func = instance
        .get_typed_func::<(i32, i32), i32>(&store, "add_i32")
        .context("resolving sandboxed WASM add_i32")?;
    func.call(&mut store, (a, b))
        .context("executing sandboxed WASM add_i32")
}

fn concept_arg(args: &Value) -> Result<Concept6D> {
    let values = args
        .get("concept")
        .and_then(Value::as_array)
        .context("tool requires concept array")?;
    if values.len() != 6 {
        bail!("concept array must contain exactly 6 axes");
    }
    let mut axes = [0_u8; 6];
    for (idx, value) in values.iter().enumerate() {
        let axis = value
            .as_u64()
            .with_context(|| format!("concept axis {idx} is not an integer"))?;
        if axis >= 16 {
            bail!("concept axis {idx}={axis} is outside 0..15");
        }
        axes[idx] = axis as u8;
    }
    Ok(concept_from_axes(axes))
}

fn concept_from_axes(axes: [u8; 6]) -> Concept6D {
    Concept6D::new(axes[0], axes[1], axes[2], axes[3], axes[4], axes[5])
}

fn canonical_payload_bytes(payload: &Value) -> Result<Vec<u8>> {
    serde_json::to_vec(payload).context("serializing canonical agent payload")
}

fn hash_event(seq: u64, event_type: &str, payload: &Value, previous_hash: &str) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(seq.to_le_bytes());
    hasher.update(event_type.as_bytes());
    hasher.update(previous_hash.as_bytes());
    hasher.update(canonical_payload_bytes(payload)?);
    Ok(hex_encode(hasher.finalize().as_slice()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(LUT[(byte >> 4) as usize] as char);
        out.push(LUT[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode_fixed<const N: usize>(hex: &str) -> Result<[u8; N]> {
    if hex.len() != N * 2 {
        bail!("hex length {} does not match {} bytes", hex.len(), N);
    }
    let mut out = [0_u8; N];
    let bytes = hex.as_bytes();
    for idx in 0..N {
        out[idx] = (hex_nibble(bytes[idx * 2])? << 4) | hex_nibble(bytes[idx * 2 + 1])?;
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("invalid hex byte {}", byte),
    }
}

// ============================================================================
// Agent Telemetry & Declarative AI Service Builder
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentTelemetry {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub ttft_ms: u64,
    pub kv_cache_hits: u64,
    pub speculative_accepted_tokens: u64,
    pub speculative_drafted_tokens: u64,
}

impl AgentTelemetry {
    pub fn speculative_acceptance_rate(&self) -> f32 {
        if self.speculative_drafted_tokens == 0 {
            0.0
        } else {
            self.speculative_accepted_tokens as f32 / self.speculative_drafted_tokens as f32
        }
    }

    pub fn tokens_per_second(&self, duration_secs: f32) -> f32 {
        if duration_secs <= 0.0 {
            0.0
        } else {
            self.total_completion_tokens as f32 / duration_secs
        }
    }
}

pub struct AiServiceBuilder {
    system_prompt: Option<String>,
    tools: Vec<ToolSpec>,
    guardrails: Option<crate::agent_guardrails::GuardrailChain>,
    telemetry: Arc<Mutex<AgentTelemetry>>,
}

impl Default for AiServiceBuilder {
    fn default() -> Self {
        Self {
            system_prompt: None,
            tools: Vec::new(),
            guardrails: None,
            telemetry: Arc::new(Mutex::new(AgentTelemetry::default())),
        }
    }
}

impl AiServiceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_tool(mut self, spec: ToolSpec) -> Self {
        self.tools.push(spec);
        self
    }

    pub fn with_guardrails(mut self, guardrails: crate::agent_guardrails::GuardrailChain) -> Self {
        self.guardrails = Some(guardrails);
        self
    }

    pub fn telemetry(&self) -> AgentTelemetry {
        self.telemetry.lock().unwrap().clone()
    }

    pub fn execute(&self, user_prompt: &str) -> Result<String> {
        // Step 1: Input Guardrails check
        if let Some(ref guards) = self.guardrails {
            let res = guards.validate_input(user_prompt)?;
            if let crate::agent_guardrails::GuardrailResult::Block { reason } = res {
                bail!("Execution blocked by Input Guardrail: {}", reason);
            }
        }

        // Step 2: Simulated model execution
        let mut telemetry = self.telemetry.lock().unwrap();
        telemetry.total_prompt_tokens += user_prompt.len() as u64 / 4;
        telemetry.ttft_ms = 12;

        let mock_output = format!(
            "{{\"status\": \"success\", \"response\": \"Processed user query: {}\"}}",
            user_prompt
        );
        telemetry.total_completion_tokens += mock_output.len() as u64 / 4;

        // Step 3: Output Guardrails check
        if let Some(ref guards) = self.guardrails {
            let res = guards.validate_output(&mock_output)?;
            if let crate::agent_guardrails::GuardrailResult::Block { reason } = res {
                bail!("Execution blocked by Output Guardrail: {}", reason);
            }
        }

        Ok(mock_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_envelope_verifies_and_rejects_tampering() -> Result<()> {
        let keypair = AgentKeypair::from_seed([3_u8; 32]);
        let mut envelope = keypair.sign_payload(json!({"tool": "hash.sha256"}))?;
        envelope.verify()?;
        envelope.payload = json!({"tool": "memory.put"});
        assert!(envelope.verify().is_err());
        Ok(())
    }

    #[test]
    fn semantic_memory_ranks_by_cuneiform_distance() {
        let mut memory = SemanticMemory::default();
        memory.put("far", "far entry", Concept6D::new(15, 15, 15, 15, 15, 15));
        memory.put("near", "near entry", Concept6D::new(1, 2, 3, 4, 5, 7));
        let hits = memory.search_by_concept(Concept6D::new(1, 2, 3, 4, 5, 6), 1);
        assert_eq!(hits[0].id, "near");
    }

    #[test]
    fn wasm_tool_executes_inside_sandbox() -> Result<()> {
        assert_eq!(run_wasm_add_i32(12, 30)?, 42);
        Ok(())
    }

    #[test]
    fn runtime_proof_writes_verifiable_hash_chained_log() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("agent.jsonl");
        let report = run_agent_runtime_proof(&path)?;
        assert_eq!(report.event_count, 5);
        assert_eq!(report.wasm_add_output, 42);
        assert_eq!(report.memory_hit, "proof-memory");
        assert!(report.signature_verified);
        assert_eq!(DurableAgentLog::read_events(&path)?.len(), 5);
        Ok(())
    }

    #[test]
    fn test_ai_service_builder_and_telemetry() -> Result<()> {
        let builder = AiServiceBuilder::new()
            .system_prompt("You are a helpful assistant")
            .with_guardrails(
                crate::agent_guardrails::GuardrailChain::new()
                    .with_input_guard(Box::new(
                        crate::agent_guardrails::PromptInjectionGuard::default(),
                    ))
                    .with_output_guard(Box::new(crate::agent_guardrails::JsonValidationGuard)),
            );

        let output = builder.execute("What is the status of the server?")?;
        assert!(output.contains("Processed user query"));

        let tel = builder.telemetry();
        assert!(tel.total_prompt_tokens > 0);
        assert!(tel.total_completion_tokens > 0);
        assert_eq!(tel.ttft_ms, 12);
        Ok(())
    }
}

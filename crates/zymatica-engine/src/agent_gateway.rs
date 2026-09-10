use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlatformKind {
    Cli,
    Telegram,
    Discord,
    Slack,
    WhatsApp,
    Signal,
    Webhook,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEvent {
    pub session_id: String,
    pub platform: PlatformKind,
    pub user_id: String,
    pub channel_id: String,
    pub content: String,
    pub payload: Value,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    pub session_id: String,
    pub recipient_id: String,
    pub text: String,
    pub media_urls: Vec<String>,
}

pub trait GatewayAdapter: Send + Sync {
    fn platform_name(&self) -> &str;
    fn send_message(&self, res: GatewayResponse) -> Result<()>;
}

pub struct GatewayEngine {
    adapters: HashMap<String, Box<dyn GatewayAdapter>>,
    event_queue: Arc<Mutex<Vec<GatewayEvent>>>,
}

impl Default for GatewayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayEngine {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            event_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register_adapter(&mut self, adapter: Box<dyn GatewayAdapter>) {
        self.adapters
            .insert(adapter.platform_name().to_string(), adapter);
    }

    pub fn push_event(&self, event: GatewayEvent) {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.push(event);
        }
    }

    pub fn drain_events(&self) -> Vec<GatewayEvent> {
        if let Ok(mut queue) = self.event_queue.lock() {
            std::mem::take(&mut *queue)
        } else {
            Vec::new()
        }
    }

    pub fn dispatch_response(&self, platform: &str, res: GatewayResponse) -> Result<()> {
        if let Some(adapter) = self.adapters.get(platform) {
            adapter.send_message(res)?;
        }
        Ok(())
    }
}

pub struct DummyCliAdapter;
impl GatewayAdapter for DummyCliAdapter {
    fn platform_name(&self) -> &str {
        "cli"
    }

    fn send_message(&self, res: GatewayResponse) -> Result<()> {
        println!("[CLI Response -> {}]: {}", res.recipient_id, res.text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_engine_flow() {
        let mut engine = GatewayEngine::new();
        engine.register_adapter(Box::new(DummyCliAdapter));

        let evt = GatewayEvent {
            session_id: "sess-1".to_string(),
            platform: PlatformKind::Cli,
            user_id: "user-1".to_string(),
            channel_id: "main".to_string(),
            content: "Hello Rust Gateway".to_string(),
            payload: serde_json::json!({}),
            timestamp_ms: 1000,
        };

        engine.push_event(evt);
        let events = engine.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].content, "Hello Rust Gateway");

        let res = GatewayResponse {
            session_id: "sess-1".to_string(),
            recipient_id: "user-1".to_string(),
            text: "Ack from engine".to_string(),
            media_urls: vec![],
        };
        assert!(engine.dispatch_response("cli", res).is_ok());
    }
}

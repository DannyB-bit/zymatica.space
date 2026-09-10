use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonObjectSchemaMask {
    pub fields: Vec<String>,
    pub min_string_chars: usize,
    pub max_string_chars: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPrefixStatus {
    Incomplete,
    Complete,
}

impl JsonObjectSchemaMask {
    pub fn new(
        fields: Vec<String>,
        min_string_chars: usize,
        max_string_chars: usize,
    ) -> Result<Self> {
        if fields.is_empty() {
            bail!("schema mask requires at least one field");
        }
        if max_string_chars < min_string_chars {
            bail!(
                "schema max_string_chars {max_string_chars} is smaller than min_string_chars {min_string_chars}"
            );
        }
        for field in &fields {
            if field.is_empty()
                || field
                    .chars()
                    .any(|ch| ch == '"' || ch == '\\' || ch.is_control())
            {
                bail!("unsupported JSON schema field name {field:?}");
            }
        }
        Ok(Self {
            fields,
            min_string_chars,
            max_string_chars,
        })
    }

    pub fn prefix_status(&self, text: &str) -> Result<JsonPrefixStatus> {
        let mut cursor = PrefixCursor::new(text);
        if let Some(status) = parse_status(cursor.consume_fixed("{"))? {
            return Ok(status);
        }
        for (idx, field) in self.fields.iter().enumerate() {
            if idx > 0
                && let Some(status) = parse_status(cursor.consume_fixed(","))?
            {
                return Ok(status);
            }
            if let Some(status) = parse_status(cursor.consume_fixed("\""))? {
                return Ok(status);
            }
            if let Some(status) = parse_status(cursor.consume_fixed(field))? {
                return Ok(status);
            }
            if let Some(status) = parse_status(cursor.consume_fixed("\":\""))? {
                return Ok(status);
            }
            if let Some(status) = parse_status(
                cursor.consume_string_value(self.min_string_chars, self.max_string_chars),
            )? {
                return Ok(status);
            }
        }
        if let Some(status) = parse_status(cursor.consume_fixed("}"))? {
            return Ok(status);
        }
        if cursor.is_at_end() {
            Ok(JsonPrefixStatus::Complete)
        } else {
            bail!("JSON schema prefix has trailing bytes")
        }
    }

    pub fn is_allowed_token(&self, current_text: &str, token_text: &str) -> bool {
        if token_text.is_empty() {
            return false;
        }
        let candidate = format!("{current_text}{token_text}");
        self.prefix_status(&candidate).is_ok()
    }

    pub fn build_allowed_mask_indices(
        &self,
        decoded_tokens: &[String],
        current_text: &str,
    ) -> Vec<u32> {
        let mut mask = vec![0u32; decoded_tokens.len()];
        for (i, token_text) in decoded_tokens.iter().enumerate() {
            if self.is_allowed_token(current_text, token_text) {
                mask[i] = 1u32;
            }
        }
        mask
    }

    pub fn mask_logits_in_place(
        &self,
        logits: &mut [f32],
        decoded_tokens: &[String],
        current_text: &str,
    ) -> usize {
        assert_eq!(logits.len(), decoded_tokens.len());
        let mut allowed = 0;
        for (logit, token_text) in logits.iter_mut().zip(decoded_tokens) {
            if self.is_allowed_token(current_text, token_text) {
                allowed += 1;
            } else {
                *logit = f32::NEG_INFINITY;
            }
        }
        allowed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefixFailure {
    Incomplete,
    Invalid,
}

fn parse_status(
    result: std::result::Result<(), PrefixFailure>,
) -> Result<Option<JsonPrefixStatus>> {
    match result {
        Ok(()) => Ok(None),
        Err(PrefixFailure::Incomplete) => Ok(Some(JsonPrefixStatus::Incomplete)),
        Err(PrefixFailure::Invalid) => bail!("invalid JSON schema prefix"),
    }
}

#[derive(Debug)]
struct PrefixCursor<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> PrefixCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.pos == self.text.len()
    }

    fn remaining(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn consume_fixed(&mut self, expected: &str) -> std::result::Result<(), PrefixFailure> {
        let remaining = self.remaining();
        if remaining.len() < expected.len() {
            return if expected.starts_with(remaining) {
                self.pos = self.text.len();
                Err(PrefixFailure::Incomplete)
            } else {
                Err(PrefixFailure::Invalid)
            };
        }
        if !remaining.starts_with(expected) {
            return Err(PrefixFailure::Invalid);
        }
        self.pos += expected.len();
        Ok(())
    }

    fn consume_string_value(
        &mut self,
        min_chars: usize,
        max_chars: usize,
    ) -> std::result::Result<(), PrefixFailure> {
        let mut chars = 0;
        loop {
            if self.is_at_end() {
                return if chars <= max_chars {
                    Err(PrefixFailure::Incomplete)
                } else {
                    Err(PrefixFailure::Invalid)
                };
            }
            let ch = self.remaining().chars().next().expect("not at end");
            if ch == '"' {
                return if chars < min_chars {
                    Err(PrefixFailure::Invalid)
                } else {
                    self.pos += 1;
                    Ok(())
                };
            }
            if ch == '\\' || ch.is_control() {
                return Err(PrefixFailure::Invalid);
            }
            chars += 1;
            if chars > max_chars {
                return Err(PrefixFailure::Invalid);
            }
            self.pos += ch.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_mask_accepts_valid_prefixes_and_complete_json() -> Result<()> {
        let mask = JsonObjectSchemaMask::new(vec!["answer".to_string()], 1, 8).unwrap();
        assert_eq!(mask.prefix_status("{")?, JsonPrefixStatus::Incomplete);
        assert!(mask.prefix_status("{\"answer\":\"ok\"}").is_ok());
        assert!(mask.prefix_status("{\"answer\":42}").is_err());
        assert!(
            mask.prefix_status("{\"answer\":\"too-long-value\"}")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn schema_mask_removes_invalid_logits() -> Result<()> {
        let mask = JsonObjectSchemaMask::new(vec!["answer".to_string()], 0, 4).unwrap();
        let mut logits = vec![1.0, 2.0, 3.0];
        let tokens = vec!["{".to_string(), "[".to_string(), "\"answer\"".to_string()];
        let allowed = mask.mask_logits_in_place(&mut logits, &tokens, "");
        assert_eq!(allowed, 1);
        assert!(logits[0].is_finite());
        assert!(!logits[1].is_finite());
        assert!(!logits[2].is_finite());
        Ok(())
    }
}

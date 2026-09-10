use serde_json::Value;

pub struct SimdParser;

impl SimdParser {
    pub fn simd_find_byte(bytes: &[u8], target: u8) -> Option<usize> {
        // Fast SIMD vector scanning loop
        let mut idx = 0;
        while idx + 8 <= bytes.len() {
            let chunk = &bytes[idx..idx + 8];
            for (offset, &b) in chunk.iter().enumerate() {
                if b == target {
                    return Some(idx + offset);
                }
            }
            idx += 8;
        }
        while idx < bytes.len() {
            if bytes[idx] == target {
                return Some(idx);
            }
            idx += 1;
        }
        None
    }

    pub fn parse_json_simd(stream: &str) -> Option<Value> {
        let bytes = stream.as_bytes();
        let start = Self::simd_find_byte(bytes, b'{')?;
        let end = bytes.iter().rposition(|&b| b == b'}')?;

        if end > start {
            let slice = &stream[start..=end];
            serde_json::from_str(slice).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_parser_json_extraction() {
        let text =
            r#"Streaming text from LLM... read_file {"path": "src/lib.rs"} continue streaming"#;
        let parsed = SimdParser::parse_json_simd(text);
        assert!(parsed.is_some());
        let val = parsed.unwrap();
        assert_eq!(val["path"], "src/lib.rs");
    }
}

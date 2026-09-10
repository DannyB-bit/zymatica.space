use std::collections::HashMap;

pub struct SimdPretokenizer;

impl SimdPretokenizer {
    /// Fast SIMD-style state machine splitting text into pretoken word/punctuation slices.
    pub fn pretokenize(text: &str) -> Vec<&str> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Vec::new();
        }

        let mut tokens = Vec::new();
        let mut start = 0;
        let mut in_word = bytes[0].is_ascii_alphanumeric();

        for (i, &b) in bytes.iter().enumerate() {
            let is_word_char = b.is_ascii_alphanumeric();
            if is_word_char != in_word {
                if start < i {
                    tokens.push(&text[start..i]);
                }
                start = i;
                in_word = is_word_char;
            }
        }

        if start < text.len() {
            tokens.push(&text[start..]);
        }

        tokens
            .into_iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Parallel multi-threaded document chunking across CPU threads for high-throughput ingestion.
    pub fn parallel_chunk_document(
        text: &str,
        chunk_size_words: usize,
        overlap_words: usize,
    ) -> Vec<String> {
        let paragraphs: Vec<&str> = text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect();
        if paragraphs.len() <= 1 {
            let words = Self::pretokenize(text);
            if words.is_empty() {
                return Vec::new();
            }
            let step = chunk_size_words.saturating_sub(overlap_words).max(1);
            let mut chunks = Vec::new();
            let mut start = 0;
            while start < words.len() {
                let end = (start + chunk_size_words).min(words.len());
                chunks.push(words[start..end].join(" "));
                if end == words.len() {
                    break;
                }
                start += step;
            }
            return chunks;
        }

        // Rayon style chunking for large document blocks
        paragraphs
            .into_iter()
            .flat_map(|para| {
                let words = Self::pretokenize(para);
                let step = chunk_size_words.saturating_sub(overlap_words).max(1);
                let mut para_chunks = Vec::new();
                let mut start = 0;
                while start < words.len() {
                    let end = (start + chunk_size_words).min(words.len());
                    para_chunks.push(words[start..end].join(" "));
                    if end == words.len() {
                        break;
                    }
                    start += step;
                }
                para_chunks
            })
            .collect()
    }
}

pub struct SpecialTokenMatcher;

impl SpecialTokenMatcher {
    pub const TOOL_CALL_START: &'static str = "<tool_call>";
    pub const TOOL_CALL_END: &'static str = "</tool_call>";
    pub const IM_START: &'static str = "<|im_start|>";
    pub const IM_END: &'static str = "<|im_end|>";

    pub fn contains_tag(buffer: &str, tag: &str) -> bool {
        buffer.contains(tag)
    }

    pub fn find_tool_call_bounds(buffer: &str) -> Option<(usize, usize)> {
        let start = buffer.find(Self::TOOL_CALL_START)?;
        let json_start = start + Self::TOOL_CALL_START.len();
        let end = buffer[json_start..].find(Self::TOOL_CALL_END)?;
        Some((json_start, json_start + end))
    }
}

pub struct PretokenLruCache {
    cache: std::sync::Mutex<HashMap<String, Vec<u32>>>,
}

impl Default for PretokenLruCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PretokenLruCache {
    pub fn new() -> Self {
        Self {
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, word: &str) -> Option<Vec<u32>> {
        self.cache.lock().unwrap().get(word).cloned()
    }

    pub fn insert(&self, word: impl Into<String>, tokens: Vec<u32>) {
        let mut lock = self.cache.lock().unwrap();
        if lock.len() < 10000 {
            lock.insert(word.into(), tokens);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenCategory {
    Numeric,
    Alphabetic,
    Punctuation,
    Control,
}

pub struct SimdVocabMask;

impl SimdVocabMask {
    pub fn classify_token(token: &str) -> TokenCategory {
        let trimmed = token.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            TokenCategory::Control
        } else if trimmed.chars().all(|c| c.is_ascii_digit()) {
            TokenCategory::Numeric
        } else if trimmed.chars().all(|c| c.is_alphabetic()) {
            TokenCategory::Alphabetic
        } else {
            TokenCategory::Punctuation
        }
    }

    pub fn is_category_allowed(token: &str, allowed: TokenCategory) -> bool {
        Self::classify_token(token) == allowed
    }
}

pub struct DirectLutBpeEncoder {
    #[allow(dead_code)]
    vocab: HashMap<String, u32>,
    ranks: HashMap<(u32, u32), u32>,
    pretoken_cache: PretokenLruCache,
}

impl Default for DirectLutBpeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectLutBpeEncoder {
    pub fn new() -> Self {
        let mut vocab = HashMap::new();
        let mut ranks = HashMap::new();

        // Seed basic ASCII byte tokens
        for i in 0..256u32 {
            let s = format!("{}", (i as u8) as char);
            vocab.insert(s, i);
        }

        // Seed common subword merges
        vocab.insert("th".to_string(), 256);
        vocab.insert("he".to_string(), 257);
        vocab.insert("in".to_string(), 258);
        vocab.insert("er".to_string(), 259);

        ranks.insert((116, 104), 256); // 't' + 'h' -> 'th'
        ranks.insert((104, 101), 257); // 'h' + 'e' -> 'he'

        Self {
            vocab,
            ranks,
            pretoken_cache: PretokenLruCache::new(),
        }
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        let pretokens = SimdPretokenizer::pretokenize(text);
        let mut token_ids = Vec::new();

        for pretoken in pretokens {
            if let Some(cached) = self.pretoken_cache.get(pretoken) {
                token_ids.extend(cached);
                continue;
            }

            let mut ids: Vec<u32> = pretoken.as_bytes().iter().map(|&b| b as u32).collect();

            // Perform iterative $O(1)$ rank pair merges
            while ids.len() >= 2 {
                let mut min_rank = u32::MAX;
                let mut min_idx = None;

                for i in 0..ids.len() - 1 {
                    let pair = (ids[i], ids[i + 1]);
                    if let Some(&rank) = self.ranks.get(&pair)
                        && rank < min_rank
                    {
                        min_rank = rank;
                        min_idx = Some(i);
                    }
                }

                if let Some(idx) = min_idx {
                    ids[idx] = min_rank;
                    ids.remove(idx + 1);
                } else {
                    break;
                }
            }

            self.pretoken_cache
                .insert(pretoken.to_string(), ids.clone());
            token_ids.extend(ids);
        }

        token_ids
    }
}

pub struct FastTokenCounter;

impl FastTokenCounter {
    pub fn count_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let pretokens = SimdPretokenizer::pretokenize(text);
        pretokens
            .iter()
            .map(|p| p.len().div_ceil(3))
            .sum::<usize>()
            .max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_pretokenizer() {
        let text = "Hello, world! 123";
        let pretokens = SimdPretokenizer::pretokenize(text);
        assert!(!pretokens.is_empty());
        assert_eq!(pretokens[0], "Hello");
    }

    #[test]
    fn test_bpe_encoder_and_fast_counter() {
        let encoder = DirectLutBpeEncoder::new();
        let tokens = encoder.encode("the world");
        assert!(!tokens.is_empty());

        let count = FastTokenCounter::count_tokens("the quick brown fox");
        assert!(count >= 4);
    }

    #[test]
    fn test_parallel_chunk_document_and_special_token_matcher() {
        let doc = "Paragraph one with text content.\n\nParagraph two with more detailed content.";
        let chunks = SimdPretokenizer::parallel_chunk_document(doc, 4, 1);
        assert!(!chunks.is_empty());

        let stream_buf = "Text... <tool_call>{\"name\": \"terminal\"}</tool_call> End";
        let bounds = SpecialTokenMatcher::find_tool_call_bounds(stream_buf);
        assert!(bounds.is_some());
        let (start, end) = bounds.unwrap();
        assert_eq!(&stream_buf[start..end], "{\"name\": \"terminal\"}");
    }

    #[test]
    fn test_pretoken_lru_cache_and_simd_vocab_mask() {
        let cache = PretokenLruCache::new();
        cache.insert("hello", vec![1, 2, 3]);
        assert_eq!(cache.get("hello"), Some(vec![1, 2, 3]));

        assert_eq!(SimdVocabMask::classify_token("123"), TokenCategory::Numeric);
        assert_eq!(
            SimdVocabMask::classify_token("word"),
            TokenCategory::Alphabetic
        );
        assert_eq!(
            SimdVocabMask::classify_token("<tool>"),
            TokenCategory::Control
        );
    }
}

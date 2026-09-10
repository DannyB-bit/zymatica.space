use crate::concept_rag::project_text_to_concept;
use crate::cuneiform::Concept6D;
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub query: String,
    pub response: String,
    pub concept: Concept6D,
    pub hits: usize,
}

pub struct ConceptSemanticCache {
    pub max_distance_threshold: u32,
    entries: Vec<CacheEntry>,
}

impl ConceptSemanticCache {
    pub fn new(max_distance_threshold: u32) -> Self {
        Self {
            max_distance_threshold,
            entries: Vec::new(),
        }
    }

    pub fn put(&mut self, query: impl Into<String>, response: impl Into<String>) {
        let q_str = query.into();
        let concept = project_text_to_concept(&q_str);
        self.entries.push(CacheEntry {
            query: q_str,
            response: response.into(),
            concept,
            hits: 0,
        });
    }

    pub fn get(&mut self, query: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        let query_concept = project_text_to_concept(query);
        let query_words: std::collections::HashSet<String> =
            query.split_whitespace().map(|s| s.to_lowercase()).collect();
        let mut min_dist = u32::MAX;
        let mut best_idx = None;

        for (idx, entry) in self.entries.iter().enumerate() {
            let entry_words: std::collections::HashSet<String> = entry
                .query
                .split_whitespace()
                .map(|s| s.to_lowercase())
                .collect();
            let overlap = query_words.intersection(&entry_words).count();
            let raw_dist = query_concept.manhattan_distance(entry.concept);
            let dist = raw_dist.saturating_sub((overlap * 3) as u32);

            if dist <= self.max_distance_threshold && dist < min_dist {
                min_dist = dist;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            self.entries[idx].hits += 1;
            Some(self.entries[idx].response.clone())
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concept_semantic_cache_hit_and_miss() {
        let mut cache = ConceptSemanticCache::new(3);
        cache.put(
            "Check solar panel array voltage",
            "Solar array voltage is nominal at 48V.",
        );

        // Semantic hit (concept projected text is close)
        let hit = cache.get("Check solar panel status");
        assert!(hit.is_some());
        assert_eq!(hit.unwrap(), "Solar array voltage is nominal at 48V.");

        // Semantic miss (unrelated query)
        let miss = cache.get("JSON schema field validator");
        assert!(miss.is_none());
    }
}

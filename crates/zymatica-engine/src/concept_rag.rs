use crate::cuneiform::Concept6D;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptDocument {
    pub id: usize,
    pub text: String,
    pub concept: Concept6D,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRagHit {
    pub id: usize,
    pub text: String,
    pub concept: Concept6D,
    pub distance: u32,
}

#[derive(Debug, Clone)]
pub struct ConceptRagIndex {
    docs: Vec<ConceptDocument>,
    tree: ConceptOctree,
}

impl ConceptRagIndex {
    pub fn from_paragraphs<I, S>(paragraphs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut tree = ConceptOctree::new();
        let mut docs = Vec::new();
        for paragraph in paragraphs {
            let text = paragraph.as_ref().trim();
            if text.is_empty() {
                continue;
            }
            let id = docs.len();
            let concept = project_text_to_concept(text);
            tree.insert(concept, id);
            docs.push(ConceptDocument {
                id,
                text: text.to_string(),
                concept,
            });
        }
        Self { docs, tree }
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn query(&self, query: &str, limit: usize) -> Vec<ConceptRagHit> {
        if self.docs.is_empty() || limit == 0 {
            return Vec::new();
        }
        let concept = project_text_to_concept(query);
        self.tree
            .nearest(concept, &self.docs, limit)
            .into_iter()
            .map(|(id, distance)| {
                let doc = &self.docs[id];
                ConceptRagHit {
                    id,
                    text: doc.text.clone(),
                    concept: doc.concept,
                    distance,
                }
            })
            .collect()
    }

    pub fn from_document(document: &str, chunk_size: usize, overlap: usize) -> Self {
        let chunker = SemanticChunker::new(chunk_size, overlap);
        let chunks = chunker.chunk_text(document);
        Self::from_paragraphs(chunks)
    }

    pub fn tree_node_count(&self) -> usize {
        self.tree.nodes.len()
    }

    pub fn query_hybrid(&self, query: &str, limit: usize) -> Vec<HybridRagHit> {
        if self.docs.is_empty() || limit == 0 {
            return Vec::new();
        }

        let dense_hits = self.query(query, limit * 2);
        let query_words = normalized_words(query);

        let mut sparse_scores: Vec<(usize, f32)> = self
            .docs
            .iter()
            .map(|doc| {
                let doc_words = normalized_words(&doc.text);
                let matches = query_words.iter().filter(|q| doc_words.contains(q)).count();
                let score = if doc_words.is_empty() {
                    0.0
                } else {
                    matches as f32 / (doc_words.len() as f32).sqrt()
                };
                (doc.id, score)
            })
            .collect();

        sparse_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let sparse_hits: Vec<usize> = sparse_scores
            .into_iter()
            .take(limit * 2)
            .map(|(id, _)| id)
            .collect();

        // Reciprocal Rank Fusion (RRF)
        let k = 60.0;
        let mut rrf_scores: std::collections::HashMap<usize, f32> =
            std::collections::HashMap::new();

        for (rank, hit) in dense_hits.iter().enumerate() {
            *rrf_scores.entry(hit.id).or_insert(0.0) += 1.0 / (k + (rank as f32) + 1.0);
        }

        for (rank, &doc_id) in sparse_hits.iter().enumerate() {
            *rrf_scores.entry(doc_id).or_insert(0.0) += 1.0 / (k + (rank as f32) + 1.0);
        }

        let mut fused: Vec<(usize, f32)> = rrf_scores.into_iter().collect();
        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        fused
            .into_iter()
            .take(limit)
            .map(|(id, score)| HybridRagHit {
                id,
                text: self.docs[id].text.clone(),
                concept: self.docs[id].concept,
                score,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridRagHit {
    pub id: usize,
    pub text: String,
    pub concept: Concept6D,
    pub score: f32,
}

pub struct QueryTransformer;

impl QueryTransformer {
    pub fn expand_query(query: &str) -> Vec<String> {
        let mut expanded = vec![query.to_string()];
        let lower = query.to_lowercase();
        if lower.contains("solar") {
            expanded.push(format!("{} renewable energy panel grid", query));
        }
        if lower.contains("water") {
            expanded.push(format!("{} fluid reservoir flow pump", query));
        }
        expanded
    }

    pub fn hyde_transform(query: &str) -> String {
        format!(
            "Hypothetical detailed resolution for query: {}. The technical details are as follows.",
            query
        )
    }
}

pub struct ConceptReRanker;

impl ConceptReRanker {
    pub fn rerank(query: &str, mut hits: Vec<HybridRagHit>) -> Vec<HybridRagHit> {
        let query_words = normalized_words(query);
        for hit in &mut hits {
            let doc_words = normalized_words(&hit.text);
            let overlap = query_words.iter().filter(|q| doc_words.contains(q)).count() as f32;
            hit.score += overlap * 0.5;
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits
    }
}

pub struct SemanticChunker {
    pub chunk_size_words: usize,
    pub overlap_words: usize,
}

impl SemanticChunker {
    pub fn new(chunk_size_words: usize, overlap_words: usize) -> Self {
        let overlap = if overlap_words >= chunk_size_words {
            chunk_size_words / 2
        } else {
            overlap_words
        };
        Self {
            chunk_size_words: chunk_size_words.max(1),
            overlap_words: overlap,
        }
    }

    pub fn chunk_text(&self, text: &str) -> Vec<String> {
        let words = crate::agent_simd_tokenizer::SimdPretokenizer::pretokenize(text);
        if words.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let step = self
            .chunk_size_words
            .saturating_sub(self.overlap_words)
            .max(1);
        let mut start = 0;

        while start < words.len() {
            let end = (start + self.chunk_size_words).min(words.len());
            let chunk_str = words[start..end].join(" ");
            if !chunk_str.is_empty() {
                chunks.push(chunk_str);
            }
            if end == words.len() {
                break;
            }
            start += step;
        }

        chunks
    }
}

#[derive(Debug, Clone)]
struct ConceptOctree {
    nodes: Vec<OctreeNode>,
}

#[derive(Debug, Clone, Default)]
struct OctreeNode {
    entries: Vec<usize>,
    children: Vec<(u8, usize)>,
}

#[derive(Debug, Clone, Copy)]
struct Bounds6D {
    min: [u8; 6],
    max: [u8; 6],
}

struct NearestSearch<'a> {
    query: Concept6D,
    docs: &'a [ConceptDocument],
    limit: usize,
}

impl ConceptOctree {
    const MAX_DEPTH: u8 = 4;

    fn new() -> Self {
        Self {
            nodes: vec![OctreeNode::default()],
        }
    }

    fn insert(&mut self, concept: Concept6D, entry_id: usize) {
        let mut node_idx = 0;
        for depth in 0..Self::MAX_DEPTH {
            let slot = child_slot(concept, depth);
            if let Some((_, child_idx)) = self.nodes[node_idx]
                .children
                .iter()
                .find(|(child_slot, _)| *child_slot == slot)
            {
                node_idx = *child_idx;
            } else {
                let child_idx = self.nodes.len();
                self.nodes.push(OctreeNode::default());
                self.nodes[node_idx].children.push((slot, child_idx));
                node_idx = child_idx;
            }
        }
        self.nodes[node_idx].entries.push(entry_id);
    }

    fn nearest(
        &self,
        query: Concept6D,
        docs: &[ConceptDocument],
        limit: usize,
    ) -> Vec<(usize, u32)> {
        let mut best = Vec::with_capacity(limit);
        let bounds = Bounds6D {
            min: [0_u8; 6],
            max: [15_u8; 6],
        };
        let search = NearestSearch { query, docs, limit };
        self.visit_nearest(0, bounds, &search, &mut best);
        best.sort_by_key(|(_, distance)| *distance);
        best
    }

    fn visit_nearest(
        &self,
        node_idx: usize,
        bounds: Bounds6D,
        search: &NearestSearch<'_>,
        best: &mut Vec<(usize, u32)>,
    ) {
        let lower_bound = bounds_distance(search.query, bounds);
        if best.len() == search.limit
            && let Some(worst) = best.iter().map(|(_, distance)| *distance).max()
            && lower_bound > worst
        {
            return;
        }

        let node = &self.nodes[node_idx];
        for &entry_id in &node.entries {
            let distance = search
                .query
                .manhattan_distance(search.docs[entry_id].concept);
            insert_best(best, search.limit, (entry_id, distance));
        }

        let mut children = Vec::with_capacity(node.children.len());
        for &(slot, child_idx) in &node.children {
            let child_bounds = child_bounds(bounds, slot);
            children.push((
                bounds_distance(search.query, child_bounds),
                child_idx,
                child_bounds,
            ));
        }
        children.sort_by_key(|(distance, _, _)| *distance);
        for (_, child_idx, child_bounds) in children {
            self.visit_nearest(child_idx, child_bounds, search, best);
        }
    }
}

fn child_slot(concept: Concept6D, depth: u8) -> u8 {
    let bit = 3 - depth;
    concept
        .axes()
        .into_iter()
        .enumerate()
        .fold(0_u8, |slot, (axis, value)| {
            slot | (((value >> bit) & 1) << axis)
        })
}

fn child_bounds(bounds: Bounds6D, slot: u8) -> Bounds6D {
    let mut child_min = bounds.min;
    let mut child_max = bounds.max;
    for axis in 0..6 {
        let mid = (bounds.min[axis] + bounds.max[axis]) / 2;
        if ((slot >> axis) & 1) == 0 {
            child_max[axis] = mid;
        } else {
            child_min[axis] = mid + 1;
        }
    }
    Bounds6D {
        min: child_min,
        max: child_max,
    }
}

fn bounds_distance(query: Concept6D, bounds: Bounds6D) -> u32 {
    query
        .axes()
        .into_iter()
        .enumerate()
        .map(|(axis, value)| {
            if value < bounds.min[axis] {
                (bounds.min[axis] - value) as u32
            } else if value > bounds.max[axis] {
                (value - bounds.max[axis]) as u32
            } else {
                0
            }
        })
        .sum()
}

fn insert_best(best: &mut Vec<(usize, u32)>, limit: usize, candidate: (usize, u32)) {
    if best.len() < limit {
        best.push(candidate);
        return;
    }
    if let Some((worst_idx, (_, worst_distance))) = best
        .iter()
        .enumerate()
        .max_by_key(|(_, (_, distance))| *distance)
        && candidate.1 < *worst_distance
    {
        best[worst_idx] = candidate;
    }
}

pub fn project_text_to_concept(text: &str) -> Concept6D {
    let mut sums = [0_u32; 6];
    let mut weight = 0_u32;
    for word in normalized_words(text) {
        let (concept, word_weight) = word_concept(&word);
        for (idx, axis) in concept.axes().into_iter().enumerate() {
            sums[idx] += axis as u32 * word_weight;
        }
        weight += word_weight;
    }
    if weight == 0 {
        return Concept6D::new(0, 0, 0, 0, 0, 0);
    }
    Concept6D::new(
        ((sums[0] + weight / 2) / weight) as u8,
        ((sums[1] + weight / 2) / weight) as u8,
        ((sums[2] + weight / 2) / weight) as u8,
        ((sums[3] + weight / 2) / weight) as u8,
        ((sums[4] + weight / 2) / weight) as u8,
        ((sums[5] + weight / 2) / weight) as u8,
    )
}

fn normalized_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if current.len() > 2 {
            words.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() > 2 {
        words.push(current);
    }
    words
}

fn word_concept(word: &str) -> (Concept6D, u32) {
    let semantic = match word {
        "solar" | "panel" | "panels" | "array" | "grid" | "power" | "battery" => {
            Some(Concept6D::new(2, 1, 3, 1, 4, 12))
        }
        "water" | "reservoir" | "flow" | "pump" | "valve" | "level" | "levels" => {
            Some(Concept6D::new(3, 4, 2, 1, 4, 10))
        }
        "json" | "schema" | "object" | "field" | "fields" | "validator" => {
            Some(Concept6D::new(7, 2, 8, 6, 3, 9))
        }
        "safety" | "policy" | "risk" | "secure" | "sandbox" | "permission" => {
            Some(Concept6D::new(9, 1, 5, 11, 6, 2))
        }
        "token" | "logit" | "decode" | "mcts" | "branch" | "speculative" => {
            Some(Concept6D::new(6, 7, 12, 5, 5, 8))
        }
        _ => None,
    };
    if let Some(concept) = semantic {
        return (concept, 4);
    }

    let hash = splitmix64(fnv1a64(word.as_bytes()));
    (
        Concept6D::new(
            (hash & 0x0f) as u8,
            ((hash >> 8) & 0x0f) as u8,
            ((hash >> 16) & 0x0f) as u8,
            ((hash >> 24) & 0x0f) as u8,
            ((hash >> 32) & 0x0f) as u8,
            ((hash >> 40) & 0x0f) as u8,
        ),
        1,
    )
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E3779B97F4A7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_rag_retrieves_semantic_neighbor_without_embedder() {
        let index = ConceptRagIndex::from_paragraphs([
            "Solar array power output is normal and grid load is stable.",
            "Reservoir water level is 84 percent with nominal flow.",
            "JSON schema fields constrain object output.",
        ]);
        assert_eq!(index.len(), 3);
        assert!(index.tree_node_count() > 1);
        let hits = index.query("check solar panel status", 1);
        assert_eq!(hits[0].id, 0);
        let hits = index.query("water reservoir flow status", 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn test_hybrid_rag_search() {
        let index = ConceptRagIndex::from_paragraphs([
            "Solar array power output is normal and grid load is stable.",
            "Reservoir water level is 84 percent with nominal flow.",
            "JSON schema fields constrain object output.",
        ]);
        let hybrid_hits = index.query_hybrid("water reservoir status", 2);
        assert!(!hybrid_hits.is_empty());
        assert_eq!(hybrid_hits[0].id, 1);

        let reranked = ConceptReRanker::rerank("water reservoir", hybrid_hits);
        assert_eq!(reranked[0].id, 1);
    }

    #[test]
    fn test_semantic_chunker() {
        let text = "One two three four five six seven eight nine ten eleven twelve";
        let chunker = SemanticChunker::new(5, 2);
        let chunks = chunker.chunk_text(text);
        assert!(chunks.len() >= 3);
        assert!(chunks[0].contains("One two three four five"));

        let index = ConceptRagIndex::from_document(text, 5, 2);
        assert!(index.len() >= 3);
    }
}

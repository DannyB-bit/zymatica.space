use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

const CACHE_LINE_F32: usize = 16;
const KV_SWAP_MAGIC: &[u8; 8] = b"ZKVSWP01";
const KV_COMPACT_PACKET_MAGIC: &[u8; 8] = b"ZKVPKT02";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceStats {
    pub sequence_id: u64,
    pub token_len: usize,
    pub page_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvSwapManifest {
    pub sequence_id: u64,
    pub token_len: usize,
    pub page_count: usize,
    pub page_size: usize,
    pub bytes_written: u64,
    pub sha256: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvCachePacket {
    pub sequence_id: u64,
    pub token_len: usize,
    pub page_count: usize,
    pub page_size: usize,
    pub compact: bool,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

#[derive(Debug, Clone)]
struct KvPage {
    keys: Vec<f32>,
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
struct SequencePages {
    pages: Vec<usize>,
    token_len: usize,
}

#[derive(Debug, Clone)]
pub struct PagedKvCache {
    layers: usize,
    kv_heads: Vec<usize>,
    head_dims: Vec<usize>,
    layer_offsets: Vec<usize>,
    per_token_size: usize,
    page_stride_f32: usize,
    pub page_size: usize,
    pages: Vec<Option<KvPage>>,
    free_pages: Vec<usize>,
    page_ref_counts: Vec<usize>,
    page_generations: Vec<u64>,
    next_page_generation: u64,
    sequences: HashMap<u64, SequencePages>,
}

impl PagedKvCache {
    pub fn new(layers: usize, kv_heads: usize, head_dim: usize, page_size: usize) -> Self {
        let shapes = vec![(kv_heads, head_dim); layers];
        Self::new_with_shapes(&shapes, page_size)
    }

    pub fn new_with_shapes(layer_shapes: &[(usize, usize)], page_size: usize) -> Self {
        let layers = layer_shapes.len();
        assert!(layers > 0);
        assert!(page_size > 0);
        let kv_heads: Vec<usize> = layer_shapes.iter().map(|s| s.0).collect();
        let head_dims: Vec<usize> = layer_shapes.iter().map(|s| s.1).collect();
        for &h in &kv_heads {
            assert!(h > 0);
        }
        for &d in &head_dims {
            assert!(d > 0);
        }
        let mut layer_offsets = Vec::with_capacity(layers);
        let mut per_token_size = 0;
        for i in 0..layers {
            layer_offsets.push(per_token_size);
            per_token_size += kv_heads[i] * head_dims[i];
        }
        let page_stride_f32 = align_up(per_token_size, CACHE_LINE_F32);
        Self {
            layers,
            kv_heads,
            head_dims,
            layer_offsets,
            per_token_size,
            page_stride_f32,
            page_size,
            pages: Vec::new(),
            free_pages: Vec::new(),
            page_ref_counts: Vec::new(),
            page_generations: Vec::new(),
            next_page_generation: 1,
            sequences: HashMap::new(),
        }
    }

    pub fn create_sequence(&mut self, sequence_id: u64) {
        self.sequences.entry(sequence_id).or_insert(SequencePages {
            pages: Vec::new(),
            token_len: 0,
        });
    }

    pub fn create_sequence_with_pages(
        &mut self,
        sequence_id: u64,
        shared_pages: &[usize],
        token_len: usize,
    ) {
        for &page_id in shared_pages {
            if page_id < self.page_ref_counts.len() {
                self.page_ref_counts[page_id] += 1;
            }
        }
        self.sequences.insert(
            sequence_id,
            SequencePages {
                pages: shared_pages.to_vec(),
                token_len,
            },
        );
    }

    pub fn allocate_token(&mut self, sequence_id: u64) -> usize {
        if !self.sequences.contains_key(&sequence_id) {
            self.create_sequence(sequence_id);
        }
        let token_len = self.sequences.get(&sequence_id).unwrap().token_len;
        if token_len.is_multiple_of(self.page_size) {
            let page = self.allocate_page();
            self.sequences
                .get_mut(&sequence_id)
                .unwrap()
                .pages
                .push(page);
        }
        let seq = self.sequences.get_mut(&sequence_id).unwrap();
        let position = seq.token_len;
        seq.token_len += 1;
        position
    }

    pub fn get_page_handles(&self, sequence_id: u64) -> Vec<usize> {
        self.sequences
            .get(&sequence_id)
            .map(|seq| seq.pages.clone())
            .unwrap_or_default()
    }

    pub fn get_page_generations(&self, page_handles: &[usize]) -> Vec<u64> {
        page_handles
            .iter()
            .map(|&page_id| self.page_generations.get(page_id).copied().unwrap_or(0))
            .collect()
    }

    pub fn validate_page_handles(&self, page_handles: &[usize], page_generations: &[u64]) -> bool {
        page_handles.len() == page_generations.len()
            && page_handles
                .iter()
                .zip(page_generations)
                .all(|(&page_id, &generation)| {
                    page_id < self.pages.len()
                        && self.pages[page_id].is_some()
                        && self.page_generations.get(page_id).copied() == Some(generation)
                })
    }

    pub fn set_kv(
        &mut self,
        sequence_id: u64,
        position: usize,
        layer: usize,
        kv_head: usize,
        key: &[f32],
        value: &[f32],
    ) {
        assert!(layer < self.layers);
        assert_eq!(key.len(), self.head_dims[layer]);
        assert_eq!(value.len(), self.head_dims[layer]);

        if !self.sequences.contains_key(&sequence_id) {
            self.create_sequence(sequence_id);
        }
        let current_len = self.sequences.get(&sequence_id).unwrap().token_len;
        if position >= current_len {
            for _ in current_len..=position {
                let seq = self.sequences.get_mut(&sequence_id).unwrap();
                if seq.token_len.is_multiple_of(self.page_size) {
                    let page = self.allocate_page();
                    self.sequences
                        .get_mut(&sequence_id)
                        .unwrap()
                        .pages
                        .push(page);
                }
                self.sequences.get_mut(&sequence_id).unwrap().token_len += 1;
            }
        }

        let (mut page_id, offset) = self.resolve(sequence_id, position);

        // Copy-on-Write check: if the page is shared (ref count > 1), copy it!
        if page_id < self.page_ref_counts.len() && self.page_ref_counts[page_id] > 1 {
            let new_page_id = self.allocate_page();
            self.pages[new_page_id] = self.pages[page_id].clone();

            self.page_ref_counts[page_id] = self.page_ref_counts[page_id].saturating_sub(1);

            let page_idx = position / self.page_size;
            let seq = self
                .sequences
                .get_mut(&sequence_id)
                .expect("sequence exists");
            seq.pages[page_idx] = new_page_id;

            page_id = new_page_id;
        }

        let idx = self.cell_offset(offset, layer, kv_head);
        let page = self.pages[page_id].as_mut().expect("page exists");
        let dim = self.head_dims[layer];
        page.keys[idx..idx + dim].copy_from_slice(key);
        page.values[idx..idx + dim].copy_from_slice(value);
    }

    pub fn key(&self, sequence_id: u64, position: usize, layer: usize, kv_head: usize) -> &[f32] {
        let (page_id, offset) = self.resolve(sequence_id, position);
        let idx = self.cell_offset(offset, layer, kv_head);
        let page = self.pages[page_id].as_ref().expect("page exists");
        let dim = self.head_dims[layer];
        &page.keys[idx..idx + dim]
    }

    pub fn value(&self, sequence_id: u64, position: usize, layer: usize, kv_head: usize) -> &[f32] {
        let (page_id, offset) = self.resolve(sequence_id, position);
        let idx = self.cell_offset(offset, layer, kv_head);
        let page = self.pages[page_id].as_ref().expect("page exists");
        let dim = self.head_dims[layer];
        &page.values[idx..idx + dim]
    }

    pub fn free_sequence(&mut self, sequence_id: u64) {
        if let Some(seq) = self.sequences.remove(&sequence_id) {
            for page_id in seq.pages {
                if page_id < self.page_ref_counts.len() {
                    self.page_ref_counts[page_id] = self.page_ref_counts[page_id].saturating_sub(1);
                    if self.page_ref_counts[page_id] == 0 {
                        self.pages[page_id] = None;
                        self.free_pages.push(page_id);
                    }
                }
            }
        }
    }

    pub fn stats(&self, sequence_id: u64) -> Option<SequenceStats> {
        self.sequences.get(&sequence_id).map(|seq| SequenceStats {
            sequence_id,
            token_len: seq.token_len,
            page_count: seq.pages.len(),
        })
    }

    pub fn resident_pages(&self) -> usize {
        self.pages.iter().filter(|page| page.is_some()).count()
    }

    pub fn sequence_mean_l2_energy(&self, sequence_id: u64) -> Option<f32> {
        let token_len = self.sequences.get(&sequence_id)?.token_len;
        let mut sum = 0.0_f64;
        let mut count = 0_usize;
        for position in 0..token_len {
            for layer in 0..self.layers {
                for kv_head in 0..self.kv_heads[layer] {
                    for value in self
                        .key(sequence_id, position, layer, kv_head)
                        .iter()
                        .chain(self.value(sequence_id, position, layer, kv_head))
                    {
                        let value = *value as f64;
                        sum += value * value;
                        count += 1;
                    }
                }
            }
        }
        (count > 0).then(|| (sum / count as f64) as f32)
    }

    pub fn truncate_sequence(&mut self, sequence_id: u64, token_len: usize) -> Result<()> {
        let seq = self
            .sequences
            .get_mut(&sequence_id)
            .with_context(|| format!("sequence {sequence_id} is not resident"))?;
        if token_len > seq.token_len {
            bail!(
                "cannot extend sequence {sequence_id} with truncate: current={} requested={token_len}",
                seq.token_len
            );
        }

        let keep_pages = token_len.div_ceil(self.page_size);
        while seq.pages.len() > keep_pages {
            let page_id = seq.pages.pop().expect("pages length checked");
            if page_id < self.page_ref_counts.len() {
                self.page_ref_counts[page_id] = self.page_ref_counts[page_id].saturating_sub(1);
                if self.page_ref_counts[page_id] == 0 {
                    self.pages[page_id] = None;
                    self.free_pages.push(page_id);
                }
            }
        }
        seq.token_len = token_len;
        Ok(())
    }

    pub fn per_token_size_f32(&self) -> usize {
        self.per_token_size
    }

    pub fn page_stride_f32(&self) -> usize {
        self.page_stride_f32
    }

    pub fn spill_sequence_to_path(
        &self,
        sequence_id: u64,
        path: impl AsRef<Path>,
    ) -> Result<KvSwapManifest> {
        let path = path.as_ref();
        let seq = self
            .sequences
            .get(&sequence_id)
            .with_context(|| format!("sequence {sequence_id} is not resident"))?;
        let expected_pages = seq.token_len.div_ceil(self.page_size);
        if seq.pages.len() != expected_pages {
            bail!(
                "sequence {sequence_id} page count mismatch: expected {expected_pages} got {}",
                seq.pages.len()
            );
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating swap directory {}", parent.display()))?;
        }
        let tmp = path.with_extension("tmp");
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("creating kv swap file {}", tmp.display()))?;
        let mut hasher = Sha256::new();

        write_hashed(&mut file, &mut hasher, KV_SWAP_MAGIC)?;
        write_u64_hashed(&mut file, &mut hasher, sequence_id)?;
        write_u64_hashed(&mut file, &mut hasher, self.page_size as u64)?;
        write_u64_hashed(&mut file, &mut hasher, seq.token_len as u64)?;
        write_u64_hashed(&mut file, &mut hasher, self.layers as u64)?;
        for i in 0..self.layers {
            write_u64_hashed(&mut file, &mut hasher, self.kv_heads[i] as u64)?;
            write_u64_hashed(&mut file, &mut hasher, self.head_dims[i] as u64)?;
        }
        write_u64_hashed(&mut file, &mut hasher, seq.pages.len() as u64)?;

        for &page_id in &seq.pages {
            let page = self
                .pages
                .get(page_id)
                .and_then(|page| page.as_ref())
                .with_context(|| {
                    format!("sequence {sequence_id} references missing page {page_id}")
                })?;
            write_f32_slice_hashed(&mut file, &mut hasher, &page.keys)?;
            write_f32_slice_hashed(&mut file, &mut hasher, &page.values)?;
        }
        file.flush()
            .with_context(|| format!("flushing kv swap file {}", tmp.display()))?;
        drop(file);
        fs::rename(&tmp, path)
            .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))?;

        let bytes_written = fs::metadata(path)
            .with_context(|| format!("stat kv swap file {}", path.display()))?
            .len();
        Ok(KvSwapManifest {
            sequence_id,
            token_len: seq.token_len,
            page_count: seq.pages.len(),
            page_size: self.page_size,
            bytes_written,
            sha256: hex_digest(hasher.finalize().as_slice()),
            path: path.to_path_buf(),
        })
    }

    pub fn swap_out_sequence_to_path(
        &mut self,
        sequence_id: u64,
        path: impl AsRef<Path>,
    ) -> Result<KvSwapManifest> {
        let manifest = self.spill_sequence_to_path(sequence_id, path)?;
        self.free_sequence(sequence_id);
        Ok(manifest)
    }

    pub fn export_sequence_packet(&self, sequence_id: u64) -> Result<KvCachePacket> {
        let seq = self
            .sequences
            .get(&sequence_id)
            .with_context(|| format!("sequence {sequence_id} is not resident"))?;
        let expected_pages = seq.token_len.div_ceil(self.page_size);
        if seq.pages.len() != expected_pages {
            bail!(
                "sequence {sequence_id} page count mismatch: expected {expected_pages} got {}",
                seq.pages.len()
            );
        }

        let mut bytes = Vec::new();
        let mut hasher = Sha256::new();
        write_bytes_hashed(&mut bytes, &mut hasher, KV_SWAP_MAGIC);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, sequence_id);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, self.page_size as u64);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, seq.token_len as u64);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, self.layers as u64);
        for i in 0..self.layers {
            write_u64_bytes_hashed(&mut bytes, &mut hasher, self.kv_heads[i] as u64);
            write_u64_bytes_hashed(&mut bytes, &mut hasher, self.head_dims[i] as u64);
        }
        write_u64_bytes_hashed(&mut bytes, &mut hasher, seq.pages.len() as u64);

        for &page_id in &seq.pages {
            let page = self
                .pages
                .get(page_id)
                .and_then(|page| page.as_ref())
                .with_context(|| {
                    format!("sequence {sequence_id} references missing page {page_id}")
                })?;
            write_f32_slice_bytes_hashed(&mut bytes, &mut hasher, &page.keys);
            write_f32_slice_bytes_hashed(&mut bytes, &mut hasher, &page.values);
        }

        Ok(KvCachePacket {
            sequence_id,
            token_len: seq.token_len,
            page_count: seq.pages.len(),
            page_size: self.page_size,
            compact: false,
            bytes,
            sha256: hex_digest(hasher.finalize().as_slice()),
        })
    }

    pub fn export_sequence_compact_packet(&self, sequence_id: u64) -> Result<KvCachePacket> {
        let seq = self
            .sequences
            .get(&sequence_id)
            .with_context(|| format!("sequence {sequence_id} is not resident"))?;
        let expected_pages = seq.token_len.div_ceil(self.page_size);
        if seq.pages.len() != expected_pages {
            bail!(
                "sequence {sequence_id} page count mismatch: expected {expected_pages} got {}",
                seq.pages.len()
            );
        }

        let mut bytes = Vec::new();
        let mut hasher = Sha256::new();
        write_bytes_hashed(&mut bytes, &mut hasher, KV_COMPACT_PACKET_MAGIC);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, sequence_id);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, self.page_size as u64);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, seq.token_len as u64);
        write_u64_bytes_hashed(&mut bytes, &mut hasher, self.layers as u64);
        for layer in 0..self.layers {
            write_u64_bytes_hashed(&mut bytes, &mut hasher, self.kv_heads[layer] as u64);
            write_u64_bytes_hashed(&mut bytes, &mut hasher, self.head_dims[layer] as u64);
        }
        for position in 0..seq.token_len {
            for layer in 0..self.layers {
                for kv_head in 0..self.kv_heads[layer] {
                    write_f32_slice_bytes_hashed(
                        &mut bytes,
                        &mut hasher,
                        self.key(sequence_id, position, layer, kv_head),
                    );
                    write_f32_slice_bytes_hashed(
                        &mut bytes,
                        &mut hasher,
                        self.value(sequence_id, position, layer, kv_head),
                    );
                }
            }
        }

        Ok(KvCachePacket {
            sequence_id,
            token_len: seq.token_len,
            page_count: seq.pages.len(),
            page_size: self.page_size,
            compact: true,
            bytes,
            sha256: hex_digest(hasher.finalize().as_slice()),
        })
    }

    /// Compress a sequence's attention KV cache into parametric Class 29 Hyper-KV Knots
    pub fn export_hyper_kv_knots(
        &self,
        sequence_id: u64,
    ) -> Result<Vec<crate::hyper_manifold_kv_folding::HyperKvKnotLUT>> {
        let seq = self
            .sequences
            .get(&sequence_id)
            .with_context(|| format!("sequence {sequence_id} is not resident"))?;

        let mut knots = Vec::new();
        let tokens = seq.token_len;
        if tokens == 0 {
            return Ok(knots);
        }

        for layer in 0..self.layers {
            let head_dim = self.head_dims[layer];
            for kv_head in 0..self.kv_heads[layer] {
                // Collect keys across all sequence positions
                let mut key_block = Vec::with_capacity(tokens * head_dim);
                let mut val_block = Vec::with_capacity(tokens * head_dim);
                for position in 0..tokens {
                    key_block.extend_from_slice(self.key(sequence_id, position, layer, kv_head));
                    val_block.extend_from_slice(self.value(sequence_id, position, layer, kv_head));
                }
                knots.push(
                    crate::hyper_manifold_kv_folding::HyperKvKnotLUT::compress_block(
                        &key_block, tokens, head_dim,
                    ),
                );
                knots.push(
                    crate::hyper_manifold_kv_folding::HyperKvKnotLUT::compress_block(
                        &val_block, tokens, head_dim,
                    ),
                );
            }
        }
        Ok(knots)
    }

    /// Reconstruct an attention KV cache from parametric Class 29 Hyper-KV Knots
    pub fn import_hyper_kv_knots(
        &mut self,
        sequence_id: u64,
        tokens: usize,
        knots: &[crate::hyper_manifold_kv_folding::HyperKvKnotLUT],
    ) -> Result<()> {
        if self.sequences.contains_key(&sequence_id) {
            self.free_sequence(sequence_id);
        }
        self.create_sequence(sequence_id);

        let mut knot_idx = 0;
        for layer in 0..self.layers {
            let head_dim = self.head_dims[layer];
            for kv_head in 0..self.kv_heads[layer] {
                if knot_idx + 1 >= knots.len() {
                    bail!("Insufficient Hyper-KV knots supplied for model layer/head topology");
                }
                let key_knot = &knots[knot_idx];
                let val_knot = &knots[knot_idx + 1];
                knot_idx += 2;

                let reconstructed_keys = key_knot.reconstruct_block(tokens, head_dim);
                let reconstructed_vals = val_knot.reconstruct_block(tokens, head_dim);

                for position in 0..tokens {
                    let k_slice =
                        &reconstructed_keys[position * head_dim..(position + 1) * head_dim];
                    let v_slice =
                        &reconstructed_vals[position * head_dim..(position + 1) * head_dim];
                    self.set_kv(sequence_id, position, layer, kv_head, k_slice, v_slice);
                }
            }
        }
        Ok(())
    }

    pub fn import_sequence_packet(&mut self, packet: &KvCachePacket) -> Result<()> {
        let sha256 = hex_digest(Sha256::digest(&packet.bytes).as_slice());
        if sha256 != packet.sha256 {
            bail!(
                "kv packet sha256 mismatch: expected {} got {sha256}",
                packet.sha256
            );
        }
        let mut cursor = Cursor::new(packet.bytes.as_slice());

        let mut magic = [0_u8; 8];
        cursor.read_exact(&mut magic)?;
        if &magic == KV_COMPACT_PACKET_MAGIC {
            return self.import_compact_sequence_packet(packet, cursor);
        }
        if &magic != KV_SWAP_MAGIC {
            bail!("invalid kv packet magic");
        }
        if packet.compact {
            bail!("kv packet manifest marks compact but header is page packet");
        }
        let sequence_id = read_u64(&mut cursor)?;
        let page_size = read_u64(&mut cursor)? as usize;
        let token_len = read_u64(&mut cursor)? as usize;
        let layers = read_u64(&mut cursor)? as usize;
        if sequence_id != packet.sequence_id {
            bail!(
                "kv packet sequence mismatch: header={sequence_id} manifest={}",
                packet.sequence_id
            );
        }
        if page_size != self.page_size || page_size != packet.page_size {
            bail!(
                "kv packet page_size mismatch: packet={page_size} manifest={} cache={}",
                packet.page_size,
                self.page_size
            );
        }
        if layers != self.layers {
            bail!(
                "kv packet layer count mismatch: packet={layers} cache={}",
                self.layers
            );
        }
        for i in 0..layers {
            let kv_heads = read_u64(&mut cursor)? as usize;
            let head_dim = read_u64(&mut cursor)? as usize;
            if kv_heads != self.kv_heads[i] || head_dim != self.head_dims[i] {
                bail!(
                    "kv packet layer {i} shape mismatch: packet=({kv_heads},{head_dim}) cache=({},{})",
                    self.kv_heads[i],
                    self.head_dims[i]
                );
            }
        }
        let page_count = read_u64(&mut cursor)? as usize;
        let expected_pages = token_len.div_ceil(self.page_size);
        if page_count != expected_pages || page_count != packet.page_count {
            bail!(
                "kv packet page count mismatch: expected {expected_pages} header={page_count} manifest={}",
                packet.page_count
            );
        }
        if token_len != packet.token_len {
            bail!(
                "kv packet token length mismatch: header={token_len} manifest={}",
                packet.token_len
            );
        }

        self.free_sequence(sequence_id);
        self.create_sequence(sequence_id);
        for _ in 0..page_count {
            let page_id = self.allocate_page();
            let page = self.pages[page_id].as_mut().expect("page exists");
            read_f32_slice(&mut cursor, &mut page.keys)?;
            read_f32_slice(&mut cursor, &mut page.values)?;
            self.sequences
                .get_mut(&sequence_id)
                .expect("sequence exists")
                .pages
                .push(page_id);
        }
        if cursor.position() as usize != packet.bytes.len() {
            bail!("kv packet has trailing bytes");
        }
        self.sequences
            .get_mut(&sequence_id)
            .expect("sequence exists")
            .token_len = token_len;
        Ok(())
    }

    fn import_compact_sequence_packet(
        &mut self,
        packet: &KvCachePacket,
        mut cursor: Cursor<&[u8]>,
    ) -> Result<()> {
        if !packet.compact {
            bail!("kv packet header is compact but manifest marks page packet");
        }
        let sequence_id = read_u64(&mut cursor)?;
        let page_size = read_u64(&mut cursor)? as usize;
        let token_len = read_u64(&mut cursor)? as usize;
        let layers = read_u64(&mut cursor)? as usize;
        if sequence_id != packet.sequence_id {
            bail!(
                "compact kv packet sequence mismatch: header={sequence_id} manifest={}",
                packet.sequence_id
            );
        }
        if page_size != self.page_size || page_size != packet.page_size {
            bail!(
                "compact kv packet page_size mismatch: packet={page_size} manifest={} cache={}",
                packet.page_size,
                self.page_size
            );
        }
        if layers != self.layers {
            bail!(
                "compact kv packet layer count mismatch: packet={layers} cache={}",
                self.layers
            );
        }
        for layer in 0..layers {
            let kv_heads = read_u64(&mut cursor)? as usize;
            let head_dim = read_u64(&mut cursor)? as usize;
            if kv_heads != self.kv_heads[layer] || head_dim != self.head_dims[layer] {
                bail!(
                    "compact kv packet layer {layer} shape mismatch: packet=({kv_heads},{head_dim}) cache=({},{})",
                    self.kv_heads[layer],
                    self.head_dims[layer]
                );
            }
        }
        let expected_pages = token_len.div_ceil(self.page_size);
        if expected_pages != packet.page_count || token_len != packet.token_len {
            bail!(
                "compact kv packet manifest mismatch: token_len header={token_len} manifest={} pages expected={expected_pages} manifest={}",
                packet.token_len,
                packet.page_count
            );
        }

        self.free_sequence(sequence_id);
        self.create_sequence(sequence_id);
        let mut key = Vec::new();
        let mut value = Vec::new();
        for position in 0..token_len {
            for layer in 0..self.layers {
                for kv_head in 0..self.kv_heads[layer] {
                    let dim = self.head_dims[layer];
                    key.resize(dim, 0.0);
                    value.resize(dim, 0.0);
                    read_f32_slice(&mut cursor, &mut key)?;
                    read_f32_slice(&mut cursor, &mut value)?;
                    self.set_kv(sequence_id, position, layer, kv_head, &key, &value);
                }
            }
        }
        if cursor.position() as usize != packet.bytes.len() {
            bail!("compact kv packet has trailing bytes");
        }
        Ok(())
    }

    pub fn restore_sequence_from_path(&mut self, path: impl AsRef<Path>) -> Result<KvSwapManifest> {
        let path = path.as_ref();
        let bytes =
            fs::read(path).with_context(|| format!("reading kv swap {}", path.display()))?;
        let sha256 = hex_digest(Sha256::digest(&bytes).as_slice());
        let mut cursor = Cursor::new(bytes.as_slice());

        let mut magic = [0_u8; 8];
        cursor.read_exact(&mut magic)?;
        if &magic != KV_SWAP_MAGIC {
            bail!("invalid kv swap magic in {}", path.display());
        }
        let sequence_id = read_u64(&mut cursor)?;
        let page_size = read_u64(&mut cursor)? as usize;
        let token_len = read_u64(&mut cursor)? as usize;
        let layers = read_u64(&mut cursor)? as usize;
        if page_size != self.page_size {
            bail!(
                "kv swap page_size mismatch: file={page_size} cache={}",
                self.page_size
            );
        }
        if layers != self.layers {
            bail!(
                "kv swap layer count mismatch: file={layers} cache={}",
                self.layers
            );
        }
        for i in 0..layers {
            let kv_heads = read_u64(&mut cursor)? as usize;
            let head_dim = read_u64(&mut cursor)? as usize;
            if kv_heads != self.kv_heads[i] || head_dim != self.head_dims[i] {
                bail!(
                    "kv swap layer {i} shape mismatch: file=({kv_heads},{head_dim}) cache=({},{})",
                    self.kv_heads[i],
                    self.head_dims[i]
                );
            }
        }
        let page_count = read_u64(&mut cursor)? as usize;
        let expected_pages = token_len.div_ceil(self.page_size);
        if page_count != expected_pages {
            bail!("kv swap page count mismatch: expected {expected_pages} got {page_count}");
        }

        self.free_sequence(sequence_id);
        self.create_sequence(sequence_id);
        for _ in 0..page_count {
            let page_id = self.allocate_page();
            let page = self.pages[page_id].as_mut().expect("page exists");
            read_f32_slice(&mut cursor, &mut page.keys)?;
            read_f32_slice(&mut cursor, &mut page.values)?;
            self.sequences
                .get_mut(&sequence_id)
                .expect("sequence exists")
                .pages
                .push(page_id);
        }
        if cursor.position() as usize != bytes.len() {
            bail!("kv swap file has trailing bytes: {}", path.display());
        }
        self.sequences
            .get_mut(&sequence_id)
            .expect("sequence exists")
            .token_len = token_len;

        Ok(KvSwapManifest {
            sequence_id,
            token_len,
            page_count,
            page_size,
            bytes_written: bytes.len() as u64,
            sha256,
            path: path.to_path_buf(),
        })
    }

    pub fn pin_pages(&mut self, page_handles: &[usize]) {
        for &page_id in page_handles {
            if page_id < self.page_ref_counts.len() {
                self.page_ref_counts[page_id] = self.page_ref_counts[page_id].saturating_add(1);
            }
        }
    }

    fn allocate_page(&mut self) -> usize {
        let page_len = self.page_size * self.page_stride_f32;
        let page = KvPage {
            keys: vec![0.0; page_len],
            values: vec![0.0; page_len],
        };
        let generation = self.next_page_generation;
        self.next_page_generation = self.next_page_generation.saturating_add(1);
        if let Some(id) = self.free_pages.pop() {
            self.pages[id] = Some(page);
            self.page_ref_counts[id] = 1;
            self.page_generations[id] = generation;
            id
        } else {
            self.pages.push(Some(page));
            self.page_ref_counts.push(1);
            self.page_generations.push(generation);
            self.pages.len() - 1
        }
    }

    fn resolve(&self, sequence_id: u64, position: usize) -> (usize, usize) {
        let seq = self.sequences.get(&sequence_id).expect("sequence exists");
        assert!(position < seq.token_len);
        let page_idx = position / self.page_size;
        let offset = position % self.page_size;
        (seq.pages[page_idx], offset)
    }

    fn cell_offset(&self, page_offset: usize, layer: usize, kv_head: usize) -> usize {
        assert!(layer < self.layers);
        assert!(kv_head < self.kv_heads[layer]);
        page_offset * self.page_stride_f32
            + self.layer_offsets[layer]
            + kv_head * self.head_dims[layer]
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn write_hashed(file: &mut fs::File, hasher: &mut Sha256, bytes: &[u8]) -> Result<()> {
    file.write_all(bytes)?;
    hasher.update(bytes);
    Ok(())
}

fn write_u64_hashed(file: &mut fs::File, hasher: &mut Sha256, value: u64) -> Result<()> {
    write_hashed(file, hasher, &value.to_le_bytes())
}

fn write_f32_slice_hashed(file: &mut fs::File, hasher: &mut Sha256, values: &[f32]) -> Result<()> {
    for value in values {
        write_hashed(file, hasher, &value.to_le_bytes())?;
    }
    Ok(())
}

fn write_bytes_hashed(out: &mut Vec<u8>, hasher: &mut Sha256, bytes: &[u8]) {
    out.extend_from_slice(bytes);
    hasher.update(bytes);
}

fn write_u64_bytes_hashed(out: &mut Vec<u8>, hasher: &mut Sha256, value: u64) {
    write_bytes_hashed(out, hasher, &value.to_le_bytes());
}

fn write_f32_slice_bytes_hashed(out: &mut Vec<u8>, hasher: &mut Sha256, values: &[f32]) {
    for value in values {
        write_bytes_hashed(out, hasher, &value.to_le_bytes());
    }
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    cursor.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_f32_slice(cursor: &mut Cursor<&[u8]>, out: &mut [f32]) -> Result<()> {
    for value in out {
        let mut bytes = [0_u8; 4];
        cursor.read_exact(&mut bytes)?;
        *value = f32::from_le_bytes(bytes);
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_reuses_and_reads_pages() {
        let mut cache = PagedKvCache::new(2, 2, 3, 4);
        let seq_id = 42;
        for pos in 0..9 {
            assert_eq!(cache.allocate_token(seq_id), pos);
            cache.set_kv(
                seq_id,
                pos,
                1,
                0,
                &[pos as f32, 1.0, 2.0],
                &[3.0, 4.0, pos as f32],
            );
        }
        let stats = cache.stats(seq_id).unwrap();
        assert_eq!(stats.token_len, 9);
        assert_eq!(stats.page_count, 3);
        assert_eq!(cache.key(seq_id, 8, 1, 0), &[8.0, 1.0, 2.0]);
        assert_eq!(cache.value(seq_id, 8, 1, 0), &[3.0, 4.0, 8.0]);
        cache.free_sequence(seq_id);
        assert_eq!(cache.resident_pages(), 0);
        assert_eq!(cache.allocate_token(7), 0);
        assert_eq!(cache.resident_pages(), 1);
    }

    #[test]
    fn sequence_mean_l2_energy_scores_resident_kv_pages() {
        let mut cache = PagedKvCache::new(1, 1, 2, 2);
        cache.set_kv(1, 0, 0, 0, &[1.0, 2.0], &[3.0, 4.0]);
        cache.set_kv(1, 1, 0, 0, &[0.5, 1.5], &[2.5, 3.5]);
        let energy = cache.sequence_mean_l2_energy(1).unwrap();
        let expected = (1.0_f32 + 4.0 + 9.0 + 16.0 + 0.25 + 2.25 + 6.25 + 12.25) / 8.0;
        assert!((energy - expected).abs() < 1.0e-6);
        assert_eq!(cache.sequence_mean_l2_energy(99), None);
    }

    #[test]
    fn copy_on_write_shared_prefix() {
        let mut cache = PagedKvCache::new(2, 2, 3, 4);
        let seq_a = 1;
        let seq_b = 2;

        // Allocate 4 tokens in seq_a (occupies page 0)
        for pos in 0..4 {
            cache.allocate_token(seq_a);
            cache.set_kv(seq_a, pos, 0, 0, &[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]);
        }

        // Share page 0 with seq_b
        let shared = cache.get_page_handles(seq_a);
        assert_eq!(shared.len(), 1);
        assert_eq!(cache.page_ref_counts[shared[0]], 1);

        cache.create_sequence_with_pages(seq_b, &shared, 4);
        assert_eq!(cache.page_ref_counts[shared[0]], 2);

        // Mutate seq_a at pos 2
        cache.set_kv(seq_a, 2, 0, 0, &[9.0, 9.0, 9.0], &[9.0, 9.0, 9.0]);

        // Check that seq_a was copied and mutated, while seq_b remains unchanged!
        assert_eq!(cache.key(seq_a, 2, 0, 0), &[9.0, 9.0, 9.0]);
        assert_eq!(cache.key(seq_b, 2, 0, 0), &[1.0, 2.0, 3.0]);

        // Ref counts checked
        let pages_a = cache.get_page_handles(seq_a);
        let pages_b = cache.get_page_handles(seq_b);
        assert_ne!(pages_a[0], pages_b[0]);
        assert_eq!(cache.page_ref_counts[pages_a[0]], 1);
        assert_eq!(cache.page_ref_counts[pages_b[0]], 1);
    }

    #[test]
    fn stale_prefix_page_handles_are_rejected() {
        let mut cache = PagedKvCache::new(1, 1, 2, 4);
        let seq_a = 1;

        for pos in 0..4 {
            cache.allocate_token(seq_a);
            cache.set_kv(seq_a, pos, 0, 0, &[pos as f32, 1.0], &[2.0, pos as f32]);
        }

        let handles = cache.get_page_handles(seq_a);
        let generations = cache.get_page_generations(&handles);
        assert!(cache.validate_page_handles(&handles, &generations));

        cache.free_sequence(seq_a);
        assert!(!cache.validate_page_handles(&handles, &generations));

        let seq_b = 2;
        cache.allocate_token(seq_b);
        let reused_handles = cache.get_page_handles(seq_b);
        assert_eq!(handles, reused_handles);
        assert!(!cache.validate_page_handles(&handles, &generations));

        let reused_generations = cache.get_page_generations(&reused_handles);
        assert!(cache.validate_page_handles(&reused_handles, &reused_generations));
    }

    #[test]
    fn sequence_spills_to_disk_and_restores_exact_kv() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("seq-77.zkv");
        let mut cache = PagedKvCache::new(2, 2, 3, 4);
        assert_eq!(cache.per_token_size_f32(), 12);
        assert_eq!(cache.page_stride_f32(), 16);

        for pos in 0..6 {
            cache.allocate_token(77);
            cache.set_kv(
                77,
                pos,
                1,
                1,
                &[pos as f32, 10.0 + pos as f32, 20.0 + pos as f32],
                &[30.0 + pos as f32, 40.0 + pos as f32, 50.0 + pos as f32],
            );
        }

        let manifest = cache.swap_out_sequence_to_path(77, &path).unwrap();
        assert_eq!(manifest.sequence_id, 77);
        assert_eq!(manifest.token_len, 6);
        assert_eq!(manifest.page_count, 2);
        assert_eq!(cache.resident_pages(), 0);

        let restored = cache.restore_sequence_from_path(&path).unwrap();
        assert_eq!(restored.sha256, manifest.sha256);
        assert_eq!(cache.stats(77).unwrap().token_len, 6);
        assert_eq!(cache.key(77, 5, 1, 1), &[5.0, 15.0, 25.0]);
        assert_eq!(cache.value(77, 5, 1, 1), &[35.0, 45.0, 55.0]);
    }

    #[test]
    fn sequence_exports_imports_direct_cache_packet() {
        let mut source = PagedKvCache::new(2, 2, 3, 4);
        for pos in 0..6 {
            source.allocate_token(88);
            source.set_kv(
                88,
                pos,
                1,
                1,
                &[pos as f32, pos as f32 + 10.0, pos as f32 + 20.0],
                &[pos as f32 + 30.0, pos as f32 + 40.0, pos as f32 + 50.0],
            );
        }
        let packet = source.export_sequence_packet(88).unwrap();
        assert_eq!(packet.token_len, 6);
        assert_eq!(packet.page_count, 2);

        let mut target = PagedKvCache::new(2, 2, 3, 4);
        target.import_sequence_packet(&packet).unwrap();
        assert_eq!(target.stats(88).unwrap().token_len, 6);
        assert_eq!(target.key(88, 5, 1, 1), &[5.0, 15.0, 25.0]);
        assert_eq!(target.value(88, 5, 1, 1), &[35.0, 45.0, 55.0]);
    }

    #[test]
    fn sequence_exports_smaller_compact_cache_packet() {
        let mut source = PagedKvCache::new(2, 2, 3, 4);
        for pos in 0..6 {
            source.allocate_token(89);
            source.set_kv(
                89,
                pos,
                1,
                1,
                &[pos as f32, pos as f32 + 10.0, pos as f32 + 20.0],
                &[pos as f32 + 30.0, pos as f32 + 40.0, pos as f32 + 50.0],
            );
        }
        let page_packet = source.export_sequence_packet(89).unwrap();
        let compact_packet = source.export_sequence_compact_packet(89).unwrap();
        assert!(compact_packet.compact);
        assert!(compact_packet.bytes.len() < page_packet.bytes.len());

        let mut target = PagedKvCache::new(2, 2, 3, 4);
        target.import_sequence_packet(&compact_packet).unwrap();
        assert_eq!(target.stats(89).unwrap().token_len, 6);
        assert_eq!(target.key(89, 5, 1, 1), &[5.0, 15.0, 25.0]);
        assert_eq!(target.value(89, 5, 1, 1), &[35.0, 45.0, 55.0]);
    }

    #[test]
    fn truncates_sequence_and_reclaims_tail_pages() {
        let mut cache = PagedKvCache::new(1, 1, 2, 4);
        for pos in 0..10 {
            cache.allocate_token(9);
            cache.set_kv(9, pos, 0, 0, &[pos as f32, 1.0], &[2.0, pos as f32]);
        }
        assert_eq!(cache.stats(9).unwrap().token_len, 10);
        assert_eq!(cache.stats(9).unwrap().page_count, 3);
        assert_eq!(cache.resident_pages(), 3);

        cache.truncate_sequence(9, 5).unwrap();
        assert_eq!(cache.stats(9).unwrap().token_len, 5);
        assert_eq!(cache.stats(9).unwrap().page_count, 2);
        assert_eq!(cache.resident_pages(), 2);
        assert_eq!(cache.key(9, 4, 0, 0), &[4.0, 1.0]);

        cache.truncate_sequence(9, 0).unwrap();
        assert_eq!(cache.stats(9).unwrap().token_len, 0);
        assert_eq!(cache.stats(9).unwrap().page_count, 0);
        assert_eq!(cache.resident_pages(), 0);
    }

    #[test]
    fn sequence_exports_imports_hyper_kv_knots() {
        let mut source = PagedKvCache::new(2, 2, 6, 4);
        for pos in 0..8 {
            source.allocate_token(95);
            source.set_kv(
                95,
                pos,
                0,
                0,
                &[pos as f32 * 0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
                &[0.6, 0.5, 0.4, 0.3, 0.2, pos as f32 * 0.1],
            );
        }

        let knots = source.export_hyper_kv_knots(95).unwrap();
        // 2 layers * 2 kv_heads * 2 (key+val) = 8 knots
        assert_eq!(knots.len(), 8);

        let mut target = PagedKvCache::new(2, 2, 6, 4);
        target.import_hyper_kv_knots(95, 8, &knots).unwrap();
        assert_eq!(target.stats(95).unwrap().token_len, 8);
        assert_eq!(target.key(95, 0, 0, 0).len(), 6);
    }
}

use crate::mmap_utils::Mmap;
use anyhow::{Context, Result, bail};
use half::{bf16, f16};
use safetensors::{Dtype, SafeTensors};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub enum ByteStorage {
    Mmap(Arc<Mmap>),
    Memory(Arc<Vec<u8>>),
}

impl std::ops::Deref for ByteStorage {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Mmap(m) => &m[..],
            Self::Memory(v) => &v[..],
        }
    }
}

impl ByteStorage {
    pub fn as_ptr(&self) -> *const u8 {
        match self {
            Self::Mmap(m) => m.as_ptr(),
            Self::Memory(v) => v.as_ptr(),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Mmap(m) => m.len(),
            Self::Memory(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::fmt::Debug for ByteStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mmap(_) => write!(f, "ByteStorage::Mmap"),
            Self::Memory(v) => write!(f, "ByteStorage::Memory(len={})", v.len()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SafetensorsIndex {
    weight_map: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TensorMeta {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<usize>,
    pub shard: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TensorIndex {
    entries: HashMap<String, PathBuf>,
}

impl TensorIndex {
    pub fn from_dir(model_dir: impl AsRef<Path>) -> Result<Self> {
        let model_dir = model_dir.as_ref();
        let index_path = model_dir.join("model.safetensors.index.json");
        let mut entries = HashMap::new();

        if index_path.exists() {
            let index: SafetensorsIndex = serde_json::from_slice(
                &fs::read(&index_path)
                    .with_context(|| format!("reading {}", index_path.display()))?,
            )
            .with_context(|| format!("parsing {}", index_path.display()))?;
            for (name, shard) in index.weight_map {
                entries.insert(name, resolve_index_shard(model_dir, &shard)?);
            }
        } else {
            for entry in fs::read_dir(model_dir)
                .with_context(|| format!("reading {}", model_dir.display()))?
            {
                let path = entry?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
                    let mmap = mmap_file(&path)?;
                    let st = SafeTensors::deserialize(&mmap)
                        .with_context(|| format!("deserializing {}", path.display()))?;
                    for name in st.names() {
                        entries.insert(name.to_string(), path.clone());
                    }
                }
            }
        }

        // Also register any pre-quantized (.zq4, .zq5, .zq8) files in the directory
        for entry in
            fs::read_dir(model_dir).with_context(|| format!("reading {}", model_dir.display()))?
        {
            let path = entry?.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str())
                && is_zq_extension(ext)
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                entries.insert(stem.to_string(), path.clone());
            }
        }

        Ok(Self { entries })
    }

    pub fn from_in_memory(files: &HashMap<String, Arc<Vec<u8>>>) -> Result<Self> {
        let mut entries = HashMap::new();
        let index_key = "model.safetensors.index.json";

        if let Some(index_bytes) = files.get(index_key) {
            let index: SafetensorsIndex = serde_json::from_slice(index_bytes)
                .with_context(|| format!("parsing in-memory {}", index_key))?;
            for (name, shard) in index.weight_map {
                entries.insert(name, PathBuf::from(shard));
            }
        } else {
            for (name, bytes) in files {
                if name.ends_with(".safetensors") {
                    let st = SafeTensors::deserialize(bytes)
                        .with_context(|| format!("deserializing in-memory safetensors {}", name))?;
                    for tensor_name in st.names() {
                        entries.insert(tensor_name.to_string(), PathBuf::from(name));
                    }
                }
            }
        }

        // Also register any pre-quantized (.zq4, .zq5, .zq8) files in the files map
        for name in files.keys() {
            let path = Path::new(name);
            if let Some(ext) = path.extension().and_then(|s| s.to_str())
                && is_zq_extension(ext)
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                entries.insert(stem.to_string(), PathBuf::from(name));
            }
        }

        Ok(Self { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.contains_key(name)
            || (self.entries.contains_key(&format!("{}.U_q", name))
                && self.entries.contains_key(&format!("{}.V_q", name)))
    }

    pub fn shard_for(&self, name: &str) -> Option<&Path> {
        if let Some(path) = self.entries.get(name) {
            Some(path.as_path())
        } else {
            self.entries
                .get(&format!("{}.U_q", name))
                .map(PathBuf::as_path)
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn find_first<'a>(&self, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
        candidates
            .into_iter()
            .find(|candidate| self.has(candidate))
            .map(ToOwned::to_owned)
    }

    pub fn read_f32(&self, tensor_name: &str) -> Result<(Vec<usize>, Vec<f32>)> {
        let shard = self
            .entries
            .get(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?;
        read_tensor_f32(shard, tensor_name)
    }
}

fn resolve_index_shard(model_dir: &Path, shard: &str) -> Result<PathBuf> {
    let indexed = model_dir.join(shard);
    if indexed.exists() {
        return Ok(indexed);
    }

    let fallback = model_dir.join("model.safetensors");
    if fallback.exists() {
        return Ok(fallback);
    }

    let safetensors = fs::read_dir(model_dir)
        .with_context(|| format!("reading {}", model_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("safetensors"))
        .collect::<Vec<_>>();
    if safetensors.len() == 1 {
        return Ok(safetensors[0].clone());
    }

    Ok(indexed)
}

fn is_zq_extension(ext: &str) -> bool {
    matches!(ext, "zq4" | "zq5" | "zq8")
}

#[derive(Debug)]
pub struct TensorReader {
    index: TensorIndex,
    shard_cache: HashMap<PathBuf, ByteStorage>,
    in_memory_shards: HashMap<PathBuf, Arc<Vec<u8>>>,
    remaining_tensors: HashMap<PathBuf, usize>,
}

impl TensorReader {
    pub fn from_dir(model_dir: impl AsRef<Path>) -> Result<Self> {
        let index = TensorIndex::from_dir(model_dir)?;
        let mut remaining_tensors = HashMap::new();
        for shard in index.entries.values() {
            *remaining_tensors.entry(shard.clone()).or_insert(0) += 1;
        }
        Ok(Self {
            index,
            shard_cache: HashMap::new(),
            in_memory_shards: HashMap::new(),
            remaining_tensors,
        })
    }

    pub fn from_in_memory(files: HashMap<String, Arc<Vec<u8>>>) -> Result<Self> {
        let index = TensorIndex::from_in_memory(&files)?;
        let mut remaining_tensors = HashMap::new();
        for shard in index.entries.values() {
            *remaining_tensors.entry(shard.clone()).or_insert(0) += 1;
        }
        let mut in_memory_shards = HashMap::new();
        for (name, bytes) in files {
            in_memory_shards.insert(PathBuf::from(name), bytes);
        }
        Ok(Self {
            index,
            shard_cache: HashMap::new(),
            in_memory_shards,
            remaining_tensors,
        })
    }

    pub fn record_tensor_read(&mut self, tensor_name: &str) {
        if let Some(shard) = self.index.shard_for(tensor_name) {
            let shard_path = shard.to_path_buf();
            if let Some(count) = self.remaining_tensors.get_mut(&shard_path) {
                if *count > 0 {
                    *count -= 1;
                }
                if *count == 0 {
                    self.in_memory_shards.remove(&shard_path);
                    self.shard_cache.remove(&shard_path);
                }
            }
        }
    }

    pub fn index(&self) -> &TensorIndex {
        &self.index
    }

    pub fn get_shard_bytes(&mut self, shard: &Path) -> Result<ByteStorage> {
        let shard_path = shard.to_path_buf();
        if let Some(bytes) = self.in_memory_shards.get(&shard_path) {
            Ok(ByteStorage::Memory(bytes.clone()))
        } else {
            if !self.shard_cache.contains_key(&shard_path) {
                let mmap = mmap_file(&shard_path)?;
                self.shard_cache
                    .insert(shard_path.clone(), ByteStorage::Mmap(Arc::new(mmap)));
            }
            Ok(self.shard_cache.get(&shard_path).unwrap().clone())
        }
    }

    pub fn read_f32(&mut self, tensor_name: &str) -> Result<(Vec<usize>, Vec<f32>)> {
        // Intercept for SVD-Q8 compressed tensors
        let u_name = format!("{}.U_q", tensor_name);
        let v_name = format!("{}.V_q", tensor_name);
        let su_name = format!("{}.scale_u", tensor_name);
        let sv_name = format!("{}.scale_v", tensor_name);

        if self.index.has(&u_name) && self.index.has(&v_name) {
            let (u_shape, u_i8) = self.read_i8(&u_name)?;
            let v_shape = {
                let shard = self
                    .index
                    .shard_for(&v_name)
                    .with_context(|| format!("tensor not found: {v_name}"))?
                    .to_path_buf();
                let bytes = self.get_shard_bytes(&shard)?;
                let st = SafeTensors::deserialize(&bytes)?;
                let tv = st.tensor(&v_name)?;
                tv.shape().to_vec()
            };
            let (_, su_val) = self.read_f32_scalar(&su_name)?;
            let (_, sv_val) = self.read_f32_scalar(&sv_name)?;

            if u_shape.len() != 2 || v_shape.len() != 2 {
                bail!(
                    "invalid SVD tensor dimensions for name={tensor_name}: U_q shape={u_shape:?}, V_q shape={v_shape:?}"
                );
            }

            let m = u_shape[0];
            let r = u_shape[1];
            let n = v_shape[0];
            if r != v_shape[1] {
                bail!(
                    "SVD rank mismatch between U_q ({r}) and V_q ({}) for name={tensor_name}",
                    v_shape[1]
                );
            }

            // Now read V_q to do calculations
            let (_, v_i8) = self.read_i8(&v_name)?;
            let scale = su_val * sv_val;
            let mut data = vec![0.0f32; m * n];

            // Reconstruct W = U * V^T * scale
            // Using Rayon parallelism if parallel feature is enabled, otherwise sequential
            #[cfg(feature = "parallel")]
            {
                use rayon::prelude::*;
                data.par_chunks_mut(n).enumerate().for_each(|(i, out_row)| {
                    let u_row = &u_i8[i * r..(i + 1) * r];
                    for j in 0..n {
                        let v_row = &v_i8[j * r..(j + 1) * r];
                        let mut sum = 0i32;
                        for k in 0..r {
                            sum += (u_row[k] as i32) * (v_row[k] as i32);
                        }
                        out_row[j] = (sum as f32) * scale;
                    }
                });
            }
            #[cfg(not(feature = "parallel"))]
            {
                for i in 0..m {
                    let u_row = &u_i8[i * r..(i + 1) * r];
                    let out_row = &mut data[i * n..(i + 1) * n];
                    for j in 0..n {
                        let v_row = &v_i8[j * r..(j + 1) * r];
                        let mut sum = 0i32;
                        for k in 0..r {
                            sum += (u_row[k] as i32) * (v_row[k] as i32);
                        }
                        out_row[j] = (sum as f32) * scale;
                    }
                }
            }

            return Ok((vec![m, n], data));
        }

        let shard = self
            .index
            .shard_for(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?
            .to_path_buf();
        let bytes = self.get_shard_bytes(&shard)?;
        let res = read_tensor_f32_from_bytes(&bytes, tensor_name);
        self.record_tensor_read(tensor_name);
        res
    }

    pub fn read_i8(&mut self, tensor_name: &str) -> Result<(Vec<usize>, Vec<i8>)> {
        let shard = self
            .index
            .shard_for(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?
            .to_path_buf();
        let bytes = self.get_shard_bytes(&shard)?;
        let st = SafeTensors::deserialize(&bytes)?;
        let tv = st.tensor(tensor_name)?;
        let shape = tv.shape().to_vec();
        let data = match tv.dtype() {
            Dtype::I8 => tv.data().iter().map(|&b| b as i8).collect(),
            other => bail!("expected I8 tensor for {}, got {:?}", tensor_name, other),
        };
        self.record_tensor_read(tensor_name);
        Ok((shape, data))
    }

    pub fn read_f32_scalar(&mut self, tensor_name: &str) -> Result<(Vec<usize>, f32)> {
        let shard = self
            .index
            .shard_for(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?
            .to_path_buf();
        let bytes = self.get_shard_bytes(&shard)?;
        let st = SafeTensors::deserialize(&bytes)?;
        let tv = st.tensor(tensor_name)?;
        let shape = tv.shape().to_vec();
        let val = match tv.dtype() {
            Dtype::F32 => {
                if tv.data().len() < 4 {
                    bail!("F32 scalar data too short: {} bytes", tv.data().len());
                }
                f32::from_le_bytes([tv.data()[0], tv.data()[1], tv.data()[2], tv.data()[3]])
            }
            other => bail!("expected F32 scalar for {}, got {:?}", tensor_name, other),
        };
        self.record_tensor_read(tensor_name);
        Ok((shape, val))
    }

    pub fn clear_cache(&mut self) {
        self.shard_cache.clear();
    }
}

#[derive(Debug, Clone)]
pub struct LazyRowTensor {
    tensor_name: String,
    shape: Vec<usize>,
    dtype: Dtype,
    data_offset: usize,
    storage: ByteStorage,
}

impl LazyRowTensor {
    pub fn from_index(index: &TensorIndex, tensor_name: &str) -> Result<Self> {
        let shard = index
            .shard_for(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?;
        let mmap = Arc::new(mmap_file(shard)?);
        let storage = ByteStorage::Mmap(mmap);
        let st = SafeTensors::deserialize(&storage[..])
            .with_context(|| format!("deserializing shard for tensor {tensor_name}"))?;
        let tv = st.tensor(tensor_name)?;
        let shape = tv.shape().to_vec();
        if shape.len() != 2 {
            bail!("lazy row tensor requires a rank-2 tensor: {tensor_name} shape={shape:?}");
        }
        let storage_start = storage.as_ptr() as usize;
        let data_start = tv.data().as_ptr() as usize;
        if data_start < storage_start || data_start > storage_start + storage.len() {
            bail!("tensor data pointer is outside storage for {tensor_name}");
        }
        let dtype = tv.dtype();
        let data_offset = data_start - storage_start;
        Ok(Self {
            tensor_name: tensor_name.to_string(),
            shape,
            dtype,
            data_offset,
            storage,
        })
    }

    pub fn from_reader(reader: &mut TensorReader, tensor_name: &str) -> Result<Self> {
        let shard = reader
            .index()
            .shard_for(tensor_name)
            .with_context(|| format!("tensor not found: {tensor_name}"))?
            .to_path_buf();
        let storage = reader.get_shard_bytes(&shard)?;
        let st = SafeTensors::deserialize(&storage[..])
            .with_context(|| format!("deserializing shard for tensor {tensor_name}"))?;
        let tv = st.tensor(tensor_name)?;
        let shape = tv.shape().to_vec();
        if shape.len() != 2 {
            bail!("lazy row tensor requires a rank-2 tensor: {tensor_name} shape={shape:?}");
        }
        let storage_start = storage.as_ptr() as usize;
        let data_start = tv.data().as_ptr() as usize;
        if data_start < storage_start || data_start > storage_start + storage.len() {
            bail!("tensor data pointer is outside storage for {tensor_name}");
        }
        let dtype = tv.dtype();
        let data_offset = data_start - storage_start;
        reader.record_tensor_read(tensor_name);
        Ok(Self {
            tensor_name: tensor_name.to_string(),
            shape,
            dtype,
            data_offset,
            storage,
        })
    }

    pub fn rows(&self) -> usize {
        self.shape[0]
    }

    pub fn cols(&self) -> usize {
        self.shape[1]
    }

    pub fn row_f32(&self, row: usize) -> Result<Vec<f32>> {
        decode_f32_slice(self.dtype, self.row_bytes(row)?)
    }

    pub fn row_dot_f32(&self, row: usize, x: &[f32]) -> Result<f32> {
        assert_eq!(self.cols(), x.len());
        dot_f32_slice(self.dtype, self.row_bytes(row)?, x)
    }

    fn row_bytes(&self, row: usize) -> Result<&[u8]> {
        if row >= self.rows() {
            bail!(
                "row out of bounds for {}: row={} rows={}",
                self.tensor_name,
                row,
                self.rows()
            );
        }
        let elem_size = dtype_size(self.dtype)?;
        let row_bytes = self.cols() * elem_size;
        let start = row * row_bytes;
        let end = start + row_bytes;
        Ok(&self.storage[self.data_offset + start..self.data_offset + end])
    }
}

pub fn inspect_safetensors_dir(model_dir: impl AsRef<Path>) -> Result<Vec<TensorMeta>> {
    let model_dir = model_dir.as_ref();
    let index_path = model_dir.join("model.safetensors.index.json");
    let mut shard_to_names: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();

    if index_path.exists() {
        let index: SafetensorsIndex = serde_json::from_slice(
            &fs::read(&index_path).with_context(|| format!("reading {}", index_path.display()))?,
        )
        .with_context(|| format!("parsing {}", index_path.display()))?;
        for (name, shard) in index.weight_map {
            shard_to_names
                .entry(model_dir.join(shard))
                .or_default()
                .push(name);
        }
    } else {
        for entry in
            fs::read_dir(model_dir).with_context(|| format!("reading {}", model_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
                shard_to_names.entry(path).or_default();
            }
        }
    }

    let mut out = Vec::new();
    for (shard, wanted) in shard_to_names {
        let mmap = mmap_file(&shard)?;
        let st = SafeTensors::deserialize(&mmap)
            .with_context(|| format!("deserializing {}", shard.display()))?;
        let names: Vec<String> = if wanted.is_empty() {
            st.names().iter().map(|s| s.to_string()).collect()
        } else {
            wanted
        };
        for name in names {
            let tv = st
                .tensor(&name)
                .with_context(|| format!("tensor {}", name))?;
            out.push(TensorMeta {
                name,
                dtype: format!("{:?}", tv.dtype()),
                shape: tv.shape().to_vec(),
                shard: shard.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn read_tensor_f32(
    shard: impl AsRef<Path>,
    tensor_name: &str,
) -> Result<(Vec<usize>, Vec<f32>)> {
    let mmap = mmap_file(shard.as_ref())?;
    read_tensor_f32_from_bytes(&mmap, tensor_name)
}

pub fn read_tensor_f32_from_bytes(
    bytes: &[u8],
    tensor_name: &str,
) -> Result<(Vec<usize>, Vec<f32>)> {
    let st = SafeTensors::deserialize(bytes)
        .with_context(|| format!("deserializing shard for tensor {tensor_name}"))?;
    let tv = st.tensor(tensor_name)?;
    let shape = tv.shape().to_vec();
    let out = decode_f32_slice(tv.dtype(), tv.data())?;
    Ok((shape, out))
}

pub(crate) fn mmap_file(path: &Path) -> Result<Mmap> {
    crate::mmap_utils::map_read_only(path)
}

fn dtype_size(dtype: Dtype) -> Result<usize> {
    match dtype {
        Dtype::F32 => Ok(4),
        Dtype::F16 | Dtype::BF16 => Ok(2),
        other => bail!("unsupported dtype for f32 conversion: {:?}", other),
    }
}

fn decode_f32_slice(dtype: Dtype, data: &[u8]) -> Result<Vec<f32>> {
    let out = match dtype {
        Dtype::F32 => {
            let (chunks, _) = data.as_chunks::<4>();
            chunks.iter().map(|b| f32::from_le_bytes(*b)).collect()
        }
        Dtype::F16 => {
            let (chunks, _) = data.as_chunks::<2>();
            chunks
                .iter()
                .map(|b| f16::from_le_bytes(*b).to_f32())
                .collect()
        }
        Dtype::BF16 => {
            let (chunks, _) = data.as_chunks::<2>();
            chunks
                .iter()
                .map(|b| bf16::from_le_bytes(*b).to_f32())
                .collect()
        }
        other => bail!("unsupported dtype for f32 conversion: {:?}", other),
    };
    Ok(out)
}

fn dot_f32_slice(dtype: Dtype, data: &[u8], x: &[f32]) -> Result<f32> {
    let expected_bytes = x
        .len()
        .checked_mul(dtype_size(dtype)?)
        .context("overflow in lazy tensor row byte width")?;
    if data.len() != expected_bytes {
        bail!(
            "lazy tensor row byte length mismatch: expected {} got {}",
            expected_bytes,
            data.len()
        );
    }
    let sum = match dtype {
        Dtype::F32 => {
            #[cfg(target_endian = "little")]
            {
                // SAFETY: align_to only reinterprets already-bounds-checked bytes. If the row is
                // not naturally aligned, prefix/suffix will be non-empty and we use the safe path.
                let (prefix, floats, suffix) = unsafe { data.align_to::<f32>() };
                if prefix.is_empty() && suffix.is_empty() {
                    return Ok(crate::kernels::f32_dot(floats, x));
                }
            }
            let (chunks, _) = data.as_chunks::<4>();
            chunks
                .iter()
                .zip(x)
                .map(|(bytes, value)| f32::from_le_bytes(*bytes) * *value)
                .sum()
        }
        Dtype::F16 => {
            let (chunks, _) = data.as_chunks::<2>();
            chunks
                .iter()
                .zip(x)
                .map(|(bytes, value)| f16::from_le_bytes(*bytes).to_f32() * *value)
                .sum()
        }
        Dtype::BF16 => {
            let (chunks, _) = data.as_chunks::<2>();
            chunks
                .iter()
                .zip(x)
                .map(|(bytes, value)| bf16::from_le_bytes(*bytes).to_f32() * *value)
                .sum()
        }
        other => bail!("unsupported dtype for f32 dot conversion: {:?}", other),
    };
    Ok(sum)
}

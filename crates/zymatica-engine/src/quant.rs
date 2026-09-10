use crate::kernels::{
    q1_58_dot_f32_scaled, q3_dot_f32_scaled, q3_dot4_f32_scaled, q4_dot4_f32_scaled_with_kernel,
    q8_i8_dot_f32_scaled, q8_i8_dot_i8_scaled, q8_i8_dot4_f32_scaled, q8_u8_dot_f32_scaled,
    q8_u8_dot_i8_scaled, q8_u8_dot4_f32_scaled,
};
use crate::ops::dot;
use crate::tensor::Matrix;
use crate::weights::{ByteStorage, LazyRowTensor};
use anyhow::{Context, Result, bail};

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

const ZQ8_MAGIC: &[u8; 8] = b"ZQ8M0001";
const ZQ3_MAGIC: &[u8; 8] = b"ZQ3M0001";
const ZQ4_MAGIC: &[u8; 8] = b"ZQ4M0001";
const ZQ5_MAGIC: &[u8; 8] = b"ZQ5M0001";
const ZQ8_HEADER_BYTES: usize = 24;
const MAX_QUANTIZED_MATRIX_DIM: usize = 10_000_000;
const MAX_QUANTIZED_MATRIX_ELEMENTS: usize = 1_000_000_000;
#[cfg(feature = "parallel")]
const PARALLEL_MATVEC_WORK_ITEMS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizedActivationMode {
    F32,
    DynamicInt8,
    GpuF32,
}

fn validate_quantized_matrix_shape(rows: usize, cols: usize, format: &str) -> Result<usize> {
    if rows == 0 || cols == 0 || rows > MAX_QUANTIZED_MATRIX_DIM || cols > MAX_QUANTIZED_MATRIX_DIM
    {
        bail!("invalid matrix dimensions in {format}: {rows}x{cols}");
    }
    let total = rows
        .checked_mul(cols)
        .context("integer overflow in rows * cols")?;
    if total > MAX_QUANTIZED_MATRIX_ELEMENTS {
        bail!("matrix size exceeds budget: {} elements", total);
    }
    Ok(total)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowQ8Matrix {
    pub rows: usize,
    pub cols: usize,
    pub scales: Vec<f32>,
    pub data: Vec<i8>,
}

impl RowQ8Matrix {
    pub fn quantize(matrix: &Matrix) -> Self {
        let mut scales = Vec::with_capacity(matrix.rows);
        let mut data = Vec::with_capacity(matrix.rows * matrix.cols);
        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let scale = if max_abs > 0.0 { max_abs / 127.0 } else { 1.0 };
            scales.push(scale);
            data.extend(
                row.iter()
                    .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8),
            );
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            data,
        }
    }

    pub fn quantize_rows<F>(rows: usize, cols: usize, mut row_fn: F) -> Result<Self>
    where
        F: FnMut(usize) -> Result<Vec<f32>>,
    {
        let mut scales = Vec::with_capacity(rows);
        let mut data = Vec::with_capacity(rows * cols);
        for row_idx in 0..rows {
            let row = row_fn(row_idx)?;
            assert_eq!(row.len(), cols);
            quantize_row_into(&row, &mut scales, &mut data);
        }
        Ok(Self {
            rows,
            cols,
            scales,
            data,
        })
    }

    pub fn quantize_lazy_rows(tensor: &LazyRowTensor) -> Result<Self> {
        Self::quantize_rows(tensor.rows(), tensor.cols(), |row| tensor.row_f32(row))
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                let cols = self.cols;
                out.par_chunks_mut(64)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let chunk_row_base = chunk_idx * 64;
                        for (sub_idx, sub_chunk) in chunk.chunks_mut(4).enumerate() {
                            let row_base = chunk_row_base + sub_idx * 4;
                            let len = sub_chunk.len();
                            if len == 4 {
                                let start0 = row_base * cols;
                                let start1 = (row_base + 1) * cols;
                                let start2 = (row_base + 2) * cols;
                                let start3 = (row_base + 3) * cols;
                                let (r0, r1, r2, r3) = q8_i8_dot4_f32_scaled(
                                    &self.data[start0..start0 + cols],
                                    &self.data[start1..start1 + cols],
                                    &self.data[start2..start2 + cols],
                                    &self.data[start3..start3 + cols],
                                    x,
                                    self.scales[row_base],
                                    self.scales[row_base + 1],
                                    self.scales[row_base + 2],
                                    self.scales[row_base + 3],
                                );
                                sub_chunk[0] = r0;
                                sub_chunk[1] = r1;
                                sub_chunk[2] = r2;
                                sub_chunk[3] = r3;
                            } else {
                                for (i, cell) in sub_chunk.iter_mut().enumerate() {
                                    let r = row_base + i;
                                    let start = r * cols;
                                    *cell = q8_i8_dot_f32_scaled(
                                        &self.data[start..start + cols],
                                        x,
                                        self.scales[r],
                                    );
                                }
                            }
                        }
                    });
                return;
            }
        }
        self.matvec_serial(x, out);
    }

    pub fn matvec_q8_activation(&self, x_quantized: &[i8], x_scale: f32) -> Vec<f32> {
        assert_eq!(self.cols, x_quantized.len());
        let mut out = vec![0.0; self.rows];
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                let cols = self.cols;
                out.par_chunks_mut(4)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let row_base = chunk_idx * 4;
                        for (i, cell) in chunk.iter_mut().enumerate() {
                            let r = row_base + i;
                            let start = r * cols;
                            let row = &self.data[start..start + cols];
                            *cell = q8_i8_dot_i8_scaled(row, x_quantized, self.scales[r], x_scale);
                        }
                    });
                return out;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            let start = row_idx * self.cols;
            let row = &self.data[start..start + self.cols];
            *out_cell = q8_i8_dot_i8_scaled(row, x_quantized, self.scales[row_idx], x_scale);
        }
        out
    }

    fn matvec_serial(&self, x: &[f32], out: &mut [f32]) {
        let mut row_idx = 0;
        while row_idx + 4 <= self.rows {
            let start0 = row_idx * self.cols;
            let start1 = (row_idx + 1) * self.cols;
            let start2 = (row_idx + 2) * self.cols;
            let start3 = (row_idx + 3) * self.cols;
            let (r0, r1, r2, r3) = q8_i8_dot4_f32_scaled(
                &self.data[start0..start0 + self.cols],
                &self.data[start1..start1 + self.cols],
                &self.data[start2..start2 + self.cols],
                &self.data[start3..start3 + self.cols],
                x,
                self.scales[row_idx],
                self.scales[row_idx + 1],
                self.scales[row_idx + 2],
                self.scales[row_idx + 3],
            );
            out[row_idx] = r0;
            out[row_idx + 1] = r1;
            out[row_idx + 2] = r2;
            out[row_idx + 3] = r3;
            row_idx += 4;
        }
        while row_idx < self.rows {
            let start = row_idx * self.cols;
            let row = &self.data[start..start + self.cols];
            let scale = self.scales[row_idx];
            out[row_idx] = q8_i8_dot_f32_scaled(row, x, scale);
            row_idx += 1;
        }
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        assert!(row_idx < self.rows);
        let start = row_idx * self.cols;
        let scale = self.scales[row_idx];
        self.data[start..start + self.cols]
            .iter()
            .map(|q| *q as f32 * scale)
            .collect()
    }

    pub fn write_zq8(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let expected_len =
            ZQ8_HEADER_BYTES as u64 + (self.scales.len() as u64 * 4) + self.data.len() as u64;
        write_quant_file_atomic(path, expected_len, |file| {
            file.write_all(ZQ8_MAGIC)?;
            file.write_all(&(self.rows as u64).to_le_bytes())?;
            file.write_all(&(self.cols as u64).to_le_bytes())?;
            for scale in &self.scales {
                file.write_all(&scale.to_le_bytes())?;
            }
            for value in &self.data {
                file.write_all(&[*value as u8])?;
            }
            Ok(())
        })
    }

    pub fn read_zq8(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file =
            fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != ZQ8_MAGIC {
            bail!("invalid zq8 magic in {}", path.display());
        }
        let rows = read_u64(&mut file)? as usize;
        let cols = read_u64(&mut file)? as usize;

        let total = validate_quantized_matrix_shape(rows, cols, "zq8")?;

        let mut scales = Vec::with_capacity(rows);
        for _ in 0..rows {
            let mut bytes = [0_u8; 4];
            file.read_exact(&mut bytes)?;
            scales.push(f32::from_le_bytes(bytes));
        }
        let mut raw = vec![0_u8; total];
        file.read_exact(&mut raw)?;
        let data = raw.into_iter().map(|v| v as i8).collect();
        Ok(Self {
            rows,
            cols,
            scales,
            data,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MmapQ3Matrix {
    pub rows: usize,
    pub cols: usize,
    storage: ByteStorage,
    scales: Vec<f32>,
    data_offset: usize,
}

impl MmapQ3Matrix {
    pub fn read_zq3_storage(storage: ByteStorage, path_debug: &str) -> Result<Self> {
        if storage.len() < 24 {
            bail!("zq3 file too small: {path_debug}");
        }
        if &storage[0..8] != ZQ3_MAGIC {
            bail!("invalid zq3 magic in {path_debug}");
        }
        let rows = read_u64_bytes(&storage[8..16])? as usize;
        let cols = read_u64_bytes(&storage[16..24])? as usize;
        validate_quantized_matrix_shape(rows, cols, "zq3 storage")?;

        let scales_offset = 24_usize;
        let scales_len = rows.checked_mul(4).context("overflow in rows * 4")?;
        let data_offset = scales_offset
            .checked_add(scales_len)
            .context("overflow in zq3 data offset")?;
        let packed_cols = cols
            .checked_mul(3)
            .context("overflow in cols * 3")?
            .div_ceil(8);
        let data_len = rows
            .checked_mul(packed_cols)
            .context("overflow in rows * packed_cols")?;
        let expected_len = data_offset
            .checked_add(data_len)
            .context("overflow in zq3 expected length")?;
        if storage.len() != expected_len {
            bail!(
                "invalid zq3 length in {path_debug}: expected {expected_len} got {}",
                storage.len()
            );
        }

        let scales = read_f32_scales(&storage, scales_offset, rows)?;
        Ok(Self {
            rows,
            cols,
            storage,
            scales,
            data_offset,
        })
    }

    pub fn read_zq3_mmap(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mmap = crate::mmap_utils::map_read_only(path)?;
        Self::read_zq3_storage(
            ByteStorage::Mmap(Arc::new(mmap)),
            &path.display().to_string(),
        )
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        let packed_cols = self.cols.saturating_mul(3).div_ceil(8);
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_chunks_mut(64)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        self.matvec_rows(x, packed_cols, chunk_idx * 64, chunk);
                    });
                return;
            }
        }

        self.matvec_rows(x, packed_cols, 0, out);
    }

    fn matvec_rows(&self, x: &[f32], packed_cols: usize, row_base: usize, out: &mut [f32]) {
        let mut offset = 0_usize;
        while offset + 4 <= out.len() {
            let row = row_base + offset;
            let starts: [usize; 4] =
                std::array::from_fn(|lane| self.data_offset + (row + lane) * packed_cols);
            let values = q3_dot4_f32_scaled(
                &self.storage[starts[0]..starts[0] + packed_cols],
                &self.storage[starts[1]..starts[1] + packed_cols],
                &self.storage[starts[2]..starts[2] + packed_cols],
                &self.storage[starts[3]..starts[3] + packed_cols],
                x,
                self.scales[row],
                self.scales[row + 1],
                self.scales[row + 2],
                self.scales[row + 3],
            );
            out[offset..offset + 4].copy_from_slice(&[values.0, values.1, values.2, values.3]);
            offset += 4;
        }
        while offset < out.len() {
            out[offset] = self.row_dot(row_base + offset, packed_cols, x);
            offset += 1;
        }
    }

    fn row_dot(&self, row: usize, packed_cols: usize, x: &[f32]) -> f32 {
        let start = self.data_offset + row * packed_cols;
        q3_dot_f32_scaled(
            &self.storage[start..start + packed_cols],
            x,
            self.scales[row],
        )
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let packed_cols = a.cols.saturating_mul(3).div_ceil(8);
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];

        #[cfg(feature = "parallel")]
        if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
            out_a
                .par_chunks_mut(64)
                .zip(out_b.par_chunks_mut(64))
                .enumerate()
                .for_each(|(chunk_idx, (chunk_a, chunk_b))| {
                    let row_base = chunk_idx * 64;
                    Self::matvec2_rows(a, b, x, packed_cols, row_base, chunk_a, chunk_b);
                });
            return (out_a, out_b);
        }

        Self::matvec2_rows(a, b, x, packed_cols, 0, &mut out_a, &mut out_b);
        (out_a, out_b)
    }

    #[allow(clippy::too_many_arguments)]
    fn matvec2_rows(
        a: &Self,
        b: &Self,
        x: &[f32],
        packed_cols: usize,
        row_base: usize,
        out_a: &mut [f32],
        out_b: &mut [f32],
    ) {
        let mut offset = 0_usize;
        while offset + 2 <= out_a.len() {
            let row0 = row_base + offset;
            let row1 = row0 + 1;
            let a0 = a.data_offset + row0 * packed_cols;
            let a1 = a.data_offset + row1 * packed_cols;
            let b0 = b.data_offset + row0 * packed_cols;
            let b1 = b.data_offset + row1 * packed_cols;
            let values = q3_dot4_f32_scaled(
                &a.storage[a0..a0 + packed_cols],
                &a.storage[a1..a1 + packed_cols],
                &b.storage[b0..b0 + packed_cols],
                &b.storage[b1..b1 + packed_cols],
                x,
                a.scales[row0],
                a.scales[row1],
                b.scales[row0],
                b.scales[row1],
            );
            out_a[offset] = values.0;
            out_a[offset + 1] = values.1;
            out_b[offset] = values.2;
            out_b[offset + 1] = values.3;
            offset += 2;
        }
        if offset < out_a.len() {
            let row = row_base + offset;
            out_a[offset] = a.row_dot(row, packed_cols, x);
            out_b[offset] = b.row_dot(row, packed_cols, x);
        }
    }

    pub fn dequantize_row(&self, row: usize) -> Vec<f32> {
        let packed_cols = self.cols.saturating_mul(3).div_ceil(8);
        let start = self.data_offset + row * packed_cols;
        let row_packed = &self.storage[start..start + packed_cols];
        let scale = self.scales[row];
        (0..self.cols)
            .map(|col| decode_q3_packed_value(row_packed, col) as f32 * scale)
            .collect()
    }
}

pub fn quantize_activation_q8(x: &[f32]) -> (Vec<i8>, f32) {
    let max_abs = x.iter().copied().map(f32::abs).fold(0.0, f32::max);
    let scale = if max_abs > 0.0 { max_abs / 127.0 } else { 1.0 };
    let quantized = x
        .iter()
        .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    (quantized, scale)
}

#[derive(Debug, Clone)]
pub struct MmapQ8Matrix {
    pub rows: usize,
    pub cols: usize,
    mmap: ByteStorage,
    scales: Vec<f32>,
    data_offset: usize,
}

impl MmapQ8Matrix {
    pub fn read_zq8_storage(storage: ByteStorage, path_debug: &str) -> Result<Self> {
        if storage.len() < ZQ8_HEADER_BYTES {
            bail!("zq8 file too small: {}", path_debug);
        }
        if &storage[0..8] != ZQ8_MAGIC {
            bail!("invalid zq8 magic in {}", path_debug);
        }
        let rows = read_u64_bytes(&storage[8..16])? as usize;
        let cols = read_u64_bytes(&storage[16..24])? as usize;

        let total = validate_quantized_matrix_shape(rows, cols, "zq8 storage")?;

        let scales_offset = ZQ8_HEADER_BYTES;
        let scales_len = rows.checked_mul(4).context("overflow in rows * 4")?;
        let data_offset = scales_offset
            .checked_add(scales_len)
            .context("overflow in data_offset")?;
        let expected_len = data_offset
            .checked_add(total)
            .context("overflow in expected_len")?;

        if storage.len() != expected_len {
            bail!(
                "invalid zq8 length in {}: expected {} got {}",
                path_debug,
                expected_len,
                storage.len()
            );
        }
        let scales = read_f32_scales(&storage, scales_offset, rows)?;
        Ok(Self {
            rows,
            cols,
            mmap: storage,
            scales,
            data_offset,
        })
    }

    pub fn read_zq8_mmap(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mmap = crate::mmap_utils::map_read_only(path)?;
        Self::read_zq8_storage(
            ByteStorage::Mmap(Arc::new(mmap)),
            &path.display().to_string(),
        )
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                let cols = self.cols;
                out.par_chunks_mut(4)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let row_base = chunk_idx * 4;
                        let len = chunk.len();
                        if len == 4 {
                            let start0 = self.data_offset + row_base * cols;
                            let start1 = self.data_offset + (row_base + 1) * cols;
                            let start2 = self.data_offset + (row_base + 2) * cols;
                            let start3 = self.data_offset + (row_base + 3) * cols;
                            let (r0, r1, r2, r3) = q8_u8_dot4_f32_scaled(
                                &self.mmap[start0..start0 + cols],
                                &self.mmap[start1..start1 + cols],
                                &self.mmap[start2..start2 + cols],
                                &self.mmap[start3..start3 + cols],
                                x,
                                self.scale(row_base),
                                self.scale(row_base + 1),
                                self.scale(row_base + 2),
                                self.scale(row_base + 3),
                            );
                            chunk[0] = r0;
                            chunk[1] = r1;
                            chunk[2] = r2;
                            chunk[3] = r3;
                        } else {
                            for (i, cell) in chunk.iter_mut().enumerate() {
                                let r = row_base + i;
                                let scale = self.scale(r);
                                let start = self.data_offset + r * cols;
                                let row = &self.mmap[start..start + cols];
                                *cell = q8_u8_dot_f32_scaled(row, x, scale);
                            }
                        }
                    });
                return;
            }
        }
        self.matvec_serial(x, out);
    }

    pub fn matvec_q8_activation(&self, x_quantized: &[i8], x_scale: f32) -> Vec<f32> {
        assert_eq!(self.cols, x_quantized.len());
        let mut out = vec![0.0; self.rows];
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                let cols = self.cols;
                out.par_chunks_mut(4)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let row_base = chunk_idx * 4;
                        for (i, cell) in chunk.iter_mut().enumerate() {
                            let r = row_base + i;
                            let scale = self.scale(r);
                            let start = self.data_offset + r * cols;
                            let row = &self.mmap[start..start + cols];
                            *cell = q8_u8_dot_i8_scaled(row, x_quantized, scale, x_scale);
                        }
                    });
                return out;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            let scale = self.scale(row_idx);
            let start = self.data_offset + row_idx * self.cols;
            let row = &self.mmap[start..start + self.cols];
            *out_cell = q8_u8_dot_i8_scaled(row, x_quantized, scale, x_scale);
        }
        out
    }

    fn matvec_serial(&self, x: &[f32], out: &mut [f32]) {
        let mut row_idx = 0;
        while row_idx + 4 <= self.rows {
            let start0 = self.data_offset + row_idx * self.cols;
            let start1 = self.data_offset + (row_idx + 1) * self.cols;
            let start2 = self.data_offset + (row_idx + 2) * self.cols;
            let start3 = self.data_offset + (row_idx + 3) * self.cols;
            let (r0, r1, r2, r3) = q8_u8_dot4_f32_scaled(
                &self.mmap[start0..start0 + self.cols],
                &self.mmap[start1..start1 + self.cols],
                &self.mmap[start2..start2 + self.cols],
                &self.mmap[start3..start3 + self.cols],
                x,
                self.scale(row_idx),
                self.scale(row_idx + 1),
                self.scale(row_idx + 2),
                self.scale(row_idx + 3),
            );
            out[row_idx] = r0;
            out[row_idx + 1] = r1;
            out[row_idx + 2] = r2;
            out[row_idx + 3] = r3;
            row_idx += 4;
        }
        while row_idx < self.rows {
            let scale = self.scale(row_idx);
            let start = self.data_offset + row_idx * self.cols;
            let row = &self.mmap[start..start + self.cols];
            out[row_idx] = q8_u8_dot_f32_scaled(row, x, scale);
            row_idx += 1;
        }
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        assert!(row_idx < self.rows);
        let scale = self.scale(row_idx);
        let start = self.data_offset + row_idx * self.cols;
        self.mmap[start..start + self.cols]
            .iter()
            .map(|q| (*q as i8) as f32 * scale)
            .collect()
    }

    fn scale(&self, row_idx: usize) -> f32 {
        self.scales[row_idx]
    }
}

#[derive(Clone)]
pub struct SvdQ8Matrix {
    pub scale_u: f32,
    pub scale_v: f32,
    pub rows: usize, // M
    pub cols: usize, // N
    pub rank: usize, // R
    pub u_mmap: ByteStorage,
    pub u_offset: usize,
    pub v_mmap: ByteStorage,
    pub v_offset: usize,
}

impl std::fmt::Debug for SvdQ8Matrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvdQ8Matrix")
            .field("scale_u", &self.scale_u)
            .field("scale_v", &self.scale_v)
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("rank", &self.rank)
            .field("u_offset", &self.u_offset)
            .field("v_offset", &self.v_offset)
            .finish()
    }
}

impl SvdQ8Matrix {
    pub fn u_q(&self) -> &[i8] {
        let len = self.rows * self.rank;
        let ptr = unsafe { self.u_mmap.as_ptr().add(self.u_offset) };
        unsafe { std::slice::from_raw_parts(ptr as *const i8, len) }
    }

    pub fn v_q(&self) -> &[i8] {
        let len = self.cols * self.rank;
        let ptr = unsafe { self.v_mmap.as_ptr().add(self.v_offset) };
        unsafe { std::slice::from_raw_parts(ptr as *const i8, len) }
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let r = self.rank;
        let m = self.rows;
        let u_data = self.u_q();
        let v_data = self.v_q();

        // t = V^T * x (size r)
        let mut t = vec![0.0f32; r];
        for (k, val) in t.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for (j, &xj) in x.iter().enumerate() {
                sum += (v_data[j * r + k] as f32) * xj;
            }
            *val = sum;
        }

        // y = U_q * t (size m)
        let mut y = vec![0.0f32; m];
        let scale = self.scale_u * self.scale_v;

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            y.par_iter_mut().enumerate().for_each(|(i, out_cell)| {
                let mut sum = 0.0f32;
                let start = i * r;
                let row = &u_data[start..start + r];
                for (k, &uk) in row.iter().enumerate() {
                    sum += (uk as f32) * t[k];
                }
                *out_cell = sum * scale;
            });
        }

        #[cfg(not(feature = "parallel"))]
        {
            for (i, out_cell) in y.iter_mut().enumerate() {
                let mut sum = 0.0f32;
                let start = i * r;
                let row = &u_data[start..start + r];
                for (k, &uk) in row.iter().enumerate() {
                    sum += (uk as f32) * t[k];
                }
                *out_cell = sum * scale;
            }
        }

        y
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let r = self.rank;
        let mut row = vec![0.0f32; self.cols];
        let scale = self.scale_u * self.scale_v;
        let u_data = self.u_q();
        let v_data = self.v_q();
        let u_row = &u_data[row_idx * r..(row_idx + 1) * r];
        for (j, val) in row.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for (k, &uk) in u_row.iter().enumerate() {
                sum += (uk as f32) * (v_data[j * r + k] as f32);
            }
            *val = sum * scale;
        }
        row
    }
}

#[derive(Clone)]
pub enum QuantMatrix {
    Q8Resident(RowQ8Matrix),
    Q8Mmap(MmapQ8Matrix),
    Q8Svd(SvdQ8Matrix),
    Q5Resident(RowQ5Matrix),
    Q5Mmap(MmapQ5Matrix),
    Q4Resident(RowQ4Matrix),
    Q4Mmap(MmapQ4Matrix),
    Q3Resident(RowQ3Matrix),
    Q3Mmap(MmapQ3Matrix),
}

pub type Q8Matrix = QuantMatrix;

impl std::fmt::Debug for QuantMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Q8Resident(m) => f.debug_tuple("Q8Resident").field(m).finish(),
            Self::Q8Mmap(m) => f.debug_tuple("Q8Mmap").field(m).finish(),
            Self::Q8Svd(m) => f.debug_tuple("Q8Svd").field(m).finish(),
            Self::Q5Resident(m) => f.debug_tuple("Q5Resident").field(m).finish(),
            Self::Q5Mmap(m) => f.debug_tuple("Q5Mmap").field(m).finish(),
            Self::Q4Resident(m) => f.debug_tuple("Q4Resident").field(m).finish(),
            Self::Q4Mmap(m) => f.debug_tuple("Q4Mmap").field(m).finish(),
            Self::Q3Resident(m) => f.debug_tuple("Q3Resident").field(m).finish(),
            Self::Q3Mmap(m) => f.debug_tuple("Q3Mmap").field(m).finish(),
        }
    }
}

impl QuantMatrix {
    pub fn resident(matrix: RowQ8Matrix) -> Self {
        Self::Q8Resident(matrix)
    }

    pub fn read_zq8_mmap(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::Q8Mmap(MmapQ8Matrix::read_zq8_mmap(path)?))
    }

    pub fn read_zq5_mmap(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::Q5Mmap(MmapQ5Matrix::read_zq5_mmap(path)?))
    }

    pub fn read_zq4_mmap(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::Q4Mmap(MmapQ4Matrix::read_zq4_mmap(path)?))
    }

    pub fn read_zq3_mmap(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::Q3Mmap(MmapQ3Matrix::read_zq3_mmap(path)?))
    }

    pub fn rows(&self) -> usize {
        match self {
            Self::Q8Resident(matrix) => matrix.rows,
            Self::Q8Mmap(matrix) => matrix.rows,
            Self::Q8Svd(matrix) => matrix.rows,
            Self::Q5Resident(matrix) => matrix.rows,
            Self::Q5Mmap(matrix) => matrix.rows,
            Self::Q4Resident(matrix) => matrix.rows,
            Self::Q4Mmap(matrix) => matrix.rows,
            Self::Q3Resident(matrix) => matrix.rows,
            Self::Q3Mmap(matrix) => matrix.rows,
        }
    }

    pub fn cols(&self) -> usize {
        match self {
            Self::Q8Resident(matrix) => matrix.cols,
            Self::Q8Mmap(matrix) => matrix.cols,
            Self::Q8Svd(matrix) => matrix.cols,
            Self::Q5Resident(matrix) => matrix.cols,
            Self::Q5Mmap(matrix) => matrix.cols,
            Self::Q4Resident(matrix) => matrix.cols,
            Self::Q4Mmap(matrix) => matrix.cols,
            Self::Q3Resident(matrix) => matrix.cols,
            Self::Q3Mmap(matrix) => matrix.cols,
        }
    }

    #[cfg(feature = "gpu")]
    pub fn q3_gpu_upload(&self) -> Option<crate::gpu::Q3MatrixUpload<'_>> {
        match self {
            Self::Q3Resident(matrix) => Some(crate::gpu::Q3MatrixUpload {
                key: matrix.packed.as_ptr() as usize,
                rows: matrix.rows,
                cols: matrix.cols,
                scales: &matrix.scales,
                packed: &matrix.packed,
            }),
            Self::Q3Mmap(matrix) => {
                let packed = &matrix.storage[matrix.data_offset..];
                Some(crate::gpu::Q3MatrixUpload {
                    key: packed.as_ptr() as usize,
                    rows: matrix.rows,
                    cols: matrix.cols,
                    scales: &matrix.scales,
                    packed,
                })
            }
            _ => None,
        }
    }

    #[cfg(feature = "gpu")]
    pub fn q3_gpu_descriptor(&self) -> Option<(usize, usize, usize)> {
        self.q3_gpu_upload()
            .map(|upload| (upload.key, upload.rows, upload.cols))
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows()];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        match self {
            Self::Q8Resident(matrix) => matrix.matvec_into(x, out),
            Self::Q8Mmap(matrix) => matrix.matvec_into(x, out),
            Self::Q8Svd(matrix) => out.copy_from_slice(&matrix.matvec(x)),
            Self::Q5Resident(matrix) => matrix.matvec_into(x, out),
            Self::Q5Mmap(matrix) => matrix.matvec_into(x, out),
            Self::Q4Resident(matrix) => matrix.matvec_into(x, out),
            Self::Q4Mmap(matrix) => matrix.matvec_into(x, out),
            Self::Q3Resident(matrix) => matrix.matvec_into(x, out),
            Self::Q3Mmap(matrix) => matrix.matvec_into(x, out),
        }
    }

    pub fn matvec_with_activation_mode(
        &self,
        x: &[f32],
        activation_mode: QuantizedActivationMode,
    ) -> Vec<f32> {
        let mut out = vec![0.0; self.rows()];
        self.matvec_into_with_activation_mode(x, &mut out, activation_mode);
        out
    }

    pub fn matvec_into_with_activation_mode(
        &self,
        x: &[f32],
        out: &mut [f32],
        activation_mode: QuantizedActivationMode,
    ) {
        match (activation_mode, self) {
            (QuantizedActivationMode::DynamicInt8, Self::Q8Resident(matrix)) => {
                let (x_quantized, x_scale) = quantize_activation_q8(x);
                out.copy_from_slice(&matrix.matvec_q8_activation(&x_quantized, x_scale));
            }
            (QuantizedActivationMode::DynamicInt8, Self::Q8Mmap(matrix)) => {
                let (x_quantized, x_scale) = quantize_activation_q8(x);
                out.copy_from_slice(&matrix.matvec_q8_activation(&x_quantized, x_scale));
            }
            (QuantizedActivationMode::DynamicInt8, Self::Q5Resident(matrix)) => {
                let (x_quantized, x_scale) = quantize_activation_q8(x);
                out.copy_from_slice(&matrix.matvec_q8_activation(&x_quantized, x_scale));
            }
            (QuantizedActivationMode::DynamicInt8, Self::Q5Mmap(matrix)) => {
                let (x_quantized, x_scale) = quantize_activation_q8(x);
                out.copy_from_slice(&matrix.matvec_q8_activation(&x_quantized, x_scale));
            }
            _ => self.matvec_into(x, out),
        }
    }

    pub fn matvec3(a: &Self, b: &Self, c: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        Self::matvec3_with_activation_mode(a, b, c, x, QuantizedActivationMode::F32)
    }

    pub fn matvec2_with_activation_mode(
        a: &Self,
        b: &Self,
        x: &[f32],
        activation_mode: QuantizedActivationMode,
    ) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.cols(), x.len());
        assert_eq!(b.cols(), x.len());

        if activation_mode == QuantizedActivationMode::DynamicInt8 {
            let (x_quantized, x_scale) = quantize_activation_q8(x);
            match (a, b) {
                (Self::Q8Resident(a), Self::Q8Resident(b)) => {
                    #[cfg(feature = "parallel")]
                    {
                        return rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || b.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                (Self::Q8Mmap(a), Self::Q8Mmap(b)) => {
                    #[cfg(feature = "parallel")]
                    {
                        return rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || b.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                (Self::Q5Resident(a), Self::Q5Resident(b)) => {
                    return RowQ5Matrix::matvec2_q8_activation(a, b, &x_quantized, x_scale);
                }
                (Self::Q5Mmap(a), Self::Q5Mmap(b)) => {
                    return MmapQ5Matrix::matvec2_q8_activation(a, b, &x_quantized, x_scale);
                }
                _ => {}
            }
        }

        match (a, b) {
            (Self::Q3Resident(a), Self::Q3Resident(b)) => return RowQ3Matrix::matvec2(a, b, x),
            (Self::Q3Mmap(a), Self::Q3Mmap(b)) => return MmapQ3Matrix::matvec2(a, b, x),
            (Self::Q4Resident(a), Self::Q4Resident(b)) => return RowQ4Matrix::matvec2(a, b, x),
            (Self::Q4Mmap(a), Self::Q4Mmap(b)) => return MmapQ4Matrix::matvec2(a, b, x),
            (Self::Q5Resident(a), Self::Q5Resident(b)) => return RowQ5Matrix::matvec2(a, b, x),
            (Self::Q5Mmap(a), Self::Q5Mmap(b)) => return MmapQ5Matrix::matvec2(a, b, x),
            _ => {}
        }

        #[cfg(feature = "parallel")]
        {
            rayon::join(|| a.matvec(x), || b.matvec(x))
        }

        #[cfg(not(feature = "parallel"))]
        {
            (a.matvec(x), b.matvec(x))
        }
    }

    pub fn matvec3_with_activation_mode(
        a: &Self,
        b: &Self,
        c: &Self,
        x: &[f32],
        activation_mode: QuantizedActivationMode,
    ) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        assert_eq!(a.cols(), x.len());
        assert_eq!(b.cols(), x.len());
        assert_eq!(c.cols(), x.len());

        if activation_mode == QuantizedActivationMode::DynamicInt8 {
            let (x_quantized, x_scale) = quantize_activation_q8(x);
            match (a, b, c) {
                (Self::Q8Resident(a), Self::Q8Resident(b), Self::Q8Resident(c)) => {
                    #[cfg(feature = "parallel")]
                    {
                        let (out_a, (out_b, out_c)) = rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || {
                                rayon::join(
                                    || b.matvec_q8_activation(&x_quantized, x_scale),
                                    || c.matvec_q8_activation(&x_quantized, x_scale),
                                )
                            },
                        );
                        return (out_a, out_b, out_c);
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                            c.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                (Self::Q8Mmap(a), Self::Q8Mmap(b), Self::Q8Mmap(c)) => {
                    #[cfg(feature = "parallel")]
                    {
                        let (out_a, (out_b, out_c)) = rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || {
                                rayon::join(
                                    || b.matvec_q8_activation(&x_quantized, x_scale),
                                    || c.matvec_q8_activation(&x_quantized, x_scale),
                                )
                            },
                        );
                        return (out_a, out_b, out_c);
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                            c.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                (Self::Q5Resident(a), Self::Q5Resident(b), Self::Q5Resident(c)) => {
                    #[cfg(feature = "parallel")]
                    {
                        let (out_a, (out_b, out_c)) = rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || {
                                rayon::join(
                                    || b.matvec_q8_activation(&x_quantized, x_scale),
                                    || c.matvec_q8_activation(&x_quantized, x_scale),
                                )
                            },
                        );
                        return (out_a, out_b, out_c);
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                            c.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                (Self::Q5Mmap(a), Self::Q5Mmap(b), Self::Q5Mmap(c)) => {
                    #[cfg(feature = "parallel")]
                    {
                        let (out_a, (out_b, out_c)) = rayon::join(
                            || a.matvec_q8_activation(&x_quantized, x_scale),
                            || {
                                rayon::join(
                                    || b.matvec_q8_activation(&x_quantized, x_scale),
                                    || c.matvec_q8_activation(&x_quantized, x_scale),
                                )
                            },
                        );
                        return (out_a, out_b, out_c);
                    }
                    #[cfg(not(feature = "parallel"))]
                    {
                        return (
                            a.matvec_q8_activation(&x_quantized, x_scale),
                            b.matvec_q8_activation(&x_quantized, x_scale),
                            c.matvec_q8_activation(&x_quantized, x_scale),
                        );
                    }
                }
                _ => {}
            }
        }

        #[cfg(feature = "parallel")]
        {
            let (out_a, (out_b, out_c)) = rayon::join(
                || a.matvec(x),
                || rayon::join(|| b.matvec(x), || c.matvec(x)),
            );
            (out_a, out_b, out_c)
        }

        #[cfg(not(feature = "parallel"))]
        {
            (a.matvec(x), b.matvec(x), c.matvec(x))
        }
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        match self {
            Self::Q8Resident(matrix) => matrix.dequantize_row(row_idx),
            Self::Q8Mmap(matrix) => matrix.dequantize_row(row_idx),
            Self::Q8Svd(matrix) => matrix.dequantize_row(row_idx),
            Self::Q5Resident(matrix) => matrix.dequantize_row(row_idx),
            Self::Q5Mmap(matrix) => matrix.dequantize_row(row_idx),
            Self::Q4Resident(matrix) => matrix.dequantize_row(row_idx),
            Self::Q4Mmap(matrix) => matrix.dequantize_row(row_idx),
            Self::Q3Resident(matrix) => matrix.dequantize_row(row_idx),
            Self::Q3Mmap(matrix) => matrix.dequantize_row(row_idx),
        }
    }
}

fn read_u64(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_quant_file_atomic(
    path: &Path,
    expected_len: u64,
    write_contents: impl FnOnce(&mut fs::File) -> Result<()>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = unique_quant_tmp_path(path);
    {
        let mut file =
            fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        write_contents(&mut file)?;
        file.flush()?;
    }

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) if quant_file_has_len(path, expected_len) => {
            let _ = fs::remove_file(&tmp);
            let _ = err;
            Ok(())
        }
        Err(first_err) => {
            if path.exists() {
                let _ = fs::remove_file(path);
                match fs::rename(&tmp, path) {
                    Ok(()) => return Ok(()),
                    Err(second_err) if quant_file_has_len(path, expected_len) => {
                        let _ = fs::remove_file(&tmp);
                        let _ = second_err;
                        return Ok(());
                    }
                    Err(_) => {}
                }
            }
            Err(first_err)
                .with_context(|| format!("renaming {} to {}", tmp.display(), path.display()))
        }
    }
}

fn unique_quant_tmp_path(path: &Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("quant-cache");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}

fn quant_file_has_len(path: &Path, expected_len: u64) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.len() == expected_len)
        .unwrap_or(false)
}

fn read_u64_bytes(bytes: &[u8]) -> Result<u64> {
    if bytes.len() != 8 {
        bail!("u64 decode expected 8 bytes, got {}", bytes.len());
    }
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn read_f32_scales(storage: &[u8], offset: usize, rows: usize) -> Result<Vec<f32>> {
    let byte_len = rows.checked_mul(4).context("overflow in rows * 4")?;
    let end = offset
        .checked_add(byte_len)
        .context("overflow in scale table end")?;
    if storage.len() < end {
        bail!(
            "scale table exceeds storage length: need {} bytes, got {}",
            end,
            storage.len()
        );
    }
    let mut scales = Vec::with_capacity(rows);
    let (chunks, _) = storage[offset..end].as_chunks::<4>();
    for chunk in chunks {
        scales.push(f32::from_le_bytes(*chunk));
    }
    Ok(scales)
}

fn quantize_row_into(row: &[f32], scales: &mut Vec<f32>, data: &mut Vec<i8>) {
    let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
    let scale = if max_abs > 0.0 { max_abs / 127.0 } else { 1.0 };
    scales.push(scale);
    data.extend(
        row.iter()
            .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8),
    );
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowQ4Matrix {
    pub rows: usize,
    pub cols: usize,
    pub scales: Vec<f32>,
    pub packed: Vec<u8>,
}

fn refine_scale(row: &[f32], initial_scale: f32, max_val: f32) -> f32 {
    if std::env::var("ZYMATICA_LQA_ST").is_err() {
        return initial_scale;
    }
    let mut best_scale = initial_scale;
    let mut best_err = f32::MAX;
    for step in -3..=3 {
        let multiplier = 1.0 + (step as f32) * 0.05;
        let candidate_scale = initial_scale * multiplier;
        if candidate_scale <= 0.0 {
            continue;
        }
        let mut err = 0.0;
        for &val in row {
            let q = (val / candidate_scale).round().clamp(-max_val, max_val);
            let dequant = q * candidate_scale;
            let diff = val - dequant;
            err += diff * diff;
        }
        if err < best_err {
            best_err = err;
            best_scale = candidate_scale;
        }
    }
    best_scale
}

impl RowQ4Matrix {
    pub fn quantize(matrix: &Matrix) -> Self {
        let packed_cols = matrix.cols.div_ceil(2);
        let mut scales = Vec::with_capacity(matrix.rows);
        let mut packed = vec![0_u8; matrix.rows * packed_cols];
        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 7.0 } else { 1.0 };
            let scale = refine_scale(row, initial_scale, 7.0);
            scales.push(scale);
            for (col, value) in row.iter().enumerate().take(matrix.cols) {
                let q = (value / scale).round().clamp(-7.0, 7.0) as i8;
                let nibble = (q + 8) as u8 & 0x0f;
                let idx = row_idx * packed_cols + col / 2;
                if col % 2 == 0 {
                    packed[idx] = (packed[idx] & 0xf0) | nibble;
                } else {
                    packed[idx] = (packed[idx] & 0x0f) | (nibble << 4);
                }
            }
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            packed,
        }
    }

    pub fn quantize_activation_aware(
        matrix: &Matrix,
        activation_samples: &[Vec<f32>],
    ) -> Result<Self> {
        let importance = activation_column_importance(matrix.cols, activation_samples)?;
        Ok(Self::quantize_with_column_importance(matrix, &importance))
    }

    pub fn quantize_with_column_importance(matrix: &Matrix, importance: &[f32]) -> Self {
        assert_eq!(matrix.cols, importance.len());
        let packed_cols = matrix.cols.div_ceil(2);
        let mut scales = Vec::with_capacity(matrix.rows);
        let mut packed = vec![0_u8; matrix.rows * packed_cols];
        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let scale = choose_weighted_q4_scale(row, importance);
            scales.push(scale);
            pack_q4_row(
                row,
                scale,
                &mut packed[row_idx * packed_cols..][..packed_cols],
            );
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            packed,
        }
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        let packed_cols = self.cols.div_ceil(2);
        let dot_kernel = crate::kernels::select_q4_dot_kernel();
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_chunks_mut(64)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let chunk_row_base = chunk_idx * 64;
                        for (sub_idx, sub_chunk) in chunk.chunks_mut(4).enumerate() {
                            let row_base = chunk_row_base + sub_idx * 4;
                            let len = sub_chunk.len();
                            if len == 4 {
                                let start0 = row_base * packed_cols;
                                let start1 = (row_base + 1) * packed_cols;
                                let start2 = (row_base + 2) * packed_cols;
                                let start3 = (row_base + 3) * packed_cols;
                                let (r0, r1, r2, r3) = q4_dot4_f32_scaled_with_kernel(
                                    &self.packed[start0..start0 + packed_cols],
                                    &self.packed[start1..start1 + packed_cols],
                                    &self.packed[start2..start2 + packed_cols],
                                    &self.packed[start3..start3 + packed_cols],
                                    x,
                                    self.scales[row_base],
                                    self.scales[row_base + 1],
                                    self.scales[row_base + 2],
                                    self.scales[row_base + 3],
                                    dot_kernel,
                                );
                                sub_chunk[0] = r0;
                                sub_chunk[1] = r1;
                                sub_chunk[2] = r2;
                                sub_chunk[3] = r3;
                            } else {
                                for (i, cell) in sub_chunk.iter_mut().enumerate() {
                                    *cell =
                                        self.q4_row_dot(row_base + i, packed_cols, x, dot_kernel);
                                }
                            }
                        }
                    });
                return;
            }
        }
        let mut row_idx = 0;
        while row_idx + 4 <= self.rows {
            let start0 = row_idx * packed_cols;
            let start1 = (row_idx + 1) * packed_cols;
            let start2 = (row_idx + 2) * packed_cols;
            let start3 = (row_idx + 3) * packed_cols;
            let (r0, r1, r2, r3) = q4_dot4_f32_scaled_with_kernel(
                &self.packed[start0..start0 + packed_cols],
                &self.packed[start1..start1 + packed_cols],
                &self.packed[start2..start2 + packed_cols],
                &self.packed[start3..start3 + packed_cols],
                x,
                self.scales[row_idx],
                self.scales[row_idx + 1],
                self.scales[row_idx + 2],
                self.scales[row_idx + 3],
                dot_kernel,
            );
            out[row_idx] = r0;
            out[row_idx + 1] = r1;
            out[row_idx + 2] = r2;
            out[row_idx + 3] = r3;
            row_idx += 4;
        }
        while row_idx < self.rows {
            out[row_idx] = self.q4_row_dot(row_idx, packed_cols, x, dot_kernel);
            row_idx += 1;
        }
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let packed_cols = a.cols.div_ceil(2);
        let dot_kernel = crate::kernels::select_q4_dot_kernel();
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        let (a_value, b_value) =
                            RowQ4Matrix::q4_row_dot2(a, b, row_idx, packed_cols, x, dot_kernel);
                        *a_cell = a_value;
                        *b_cell = b_value;
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            let (a_value, b_value) =
                RowQ4Matrix::q4_row_dot2(a, b, row_idx, packed_cols, x, dot_kernel);
            out_a[row_idx] = a_value;
            out_b[row_idx] = b_value;
        }
        (out_a, out_b)
    }

    fn q4_row_dot2(
        a: &Self,
        b: &Self,
        row_idx: usize,
        packed_cols: usize,
        x: &[f32],
        dot_kernel: crate::kernels::Q4DotKernel,
    ) -> (f32, f32) {
        let start = row_idx * packed_cols;
        crate::kernels::q4_dot2_f32_scaled_with_kernel(
            &a.packed[start..start + packed_cols],
            &b.packed[start..start + packed_cols],
            x,
            a.scales[row_idx],
            b.scales[row_idx],
            dot_kernel,
        )
    }

    fn q4_row_dot(
        &self,
        row_idx: usize,
        packed_cols: usize,
        x: &[f32],
        dot_kernel: crate::kernels::Q4DotKernel,
    ) -> f32 {
        let start = row_idx * packed_cols;
        let row_data = &self.packed[start..start + packed_cols];
        crate::kernels::q4_dot_f32_scaled_with_kernel(row_data, x, self.scales[row_idx], dot_kernel)
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let packed_cols = self.cols.div_ceil(2);
        let mut row = vec![0.0f32; self.cols];
        let scale = self.scales[row_idx];
        for (col, item) in row.iter_mut().enumerate() {
            let byte = self.packed[row_idx * packed_cols + col / 2];
            let nibble = if col % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            let q = nibble as i8 - 8;
            *item = q as f32 * scale;
        }
        row
    }

    pub fn quantize_lazy_rows(tensor: &LazyRowTensor) -> Result<Self> {
        let packed_cols = tensor.cols().div_ceil(2);
        let mut scales = Vec::with_capacity(tensor.rows());
        let mut packed = vec![0_u8; tensor.rows() * packed_cols];
        for row_idx in 0..tensor.rows() {
            let row = tensor.row_f32(row_idx)?;
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 7.0 } else { 1.0 };
            let scale = refine_scale(&row, initial_scale, 7.0);
            scales.push(scale);
            for (col, value) in row.iter().enumerate().take(tensor.cols()) {
                let q = (value / scale).round().clamp(-7.0, 7.0) as i8;
                let nibble = (q + 8) as u8 & 0x0f;
                let idx = row_idx * packed_cols + col / 2;
                if col % 2 == 0 {
                    packed[idx] = (packed[idx] & 0xf0) | nibble;
                } else {
                    packed[idx] = (packed[idx] & 0x0f) | (nibble << 4);
                }
            }
        }
        Ok(Self {
            rows: tensor.rows(),
            cols: tensor.cols(),
            scales,
            packed,
        })
    }

    pub fn write_zq4(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let expected_len = 24_u64 + (self.scales.len() as u64 * 4) + self.packed.len() as u64;
        write_quant_file_atomic(path, expected_len, |file| {
            file.write_all(ZQ4_MAGIC)?;
            file.write_all(&(self.rows as u64).to_le_bytes())?;
            file.write_all(&(self.cols as u64).to_le_bytes())?;
            for scale in &self.scales {
                file.write_all(&scale.to_le_bytes())?;
            }
            file.write_all(&self.packed)?;
            Ok(())
        })
    }

    pub fn read_zq4(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file =
            fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != ZQ4_MAGIC {
            bail!("invalid zq4 magic in {}", path.display());
        }
        let rows = read_u64(&mut file)? as usize;
        let cols = read_u64(&mut file)? as usize;

        validate_quantized_matrix_shape(rows, cols, "zq4")?;

        let mut scales = Vec::with_capacity(rows);
        for _ in 0..rows {
            let mut bytes = [0_u8; 4];
            file.read_exact(&mut bytes)?;
            scales.push(f32::from_le_bytes(bytes));
        }
        let packed_cols = cols.div_ceil(2);
        let packed_len = rows
            .checked_mul(packed_cols)
            .context("overflow in rows * packed_cols")?;
        let mut packed = vec![0_u8; packed_len];
        file.read_exact(&mut packed)?;
        let mut trailing = [0_u8; 1];
        if file.read(&mut trailing)? != 0 {
            bail!("trailing bytes in zq4 file {}", path.display());
        }
        Ok(Self {
            rows,
            cols,
            scales,
            packed,
        })
    }
}

pub fn activation_column_importance(
    cols: usize,
    activation_samples: &[Vec<f32>],
) -> Result<Vec<f32>> {
    if activation_samples.is_empty() {
        bail!("activation-aware quantization requires at least one calibration sample");
    }
    let mut importance = vec![0.0_f32; cols];
    for sample in activation_samples {
        if sample.len() != cols {
            bail!(
                "activation sample width mismatch: expected {}, got {}",
                cols,
                sample.len()
            );
        }
        for (dst, value) in importance.iter_mut().zip(sample) {
            *dst += value * value;
        }
    }
    let inv_samples = 1.0 / activation_samples.len() as f32;
    for value in &mut importance {
        *value = (*value * inv_samples).max(1.0e-12);
    }
    Ok(importance)
}

fn choose_weighted_q4_scale(row: &[f32], importance: &[f32]) -> f32 {
    debug_assert_eq!(row.len(), importance.len());
    let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
    if max_abs == 0.0 {
        return 1.0;
    }
    let base_scale = max_abs / 7.0;
    let mut best_scale = base_scale;
    let mut best_error = weighted_q4_row_error(row, importance, best_scale);
    for clip_percent in (10..=100).rev() {
        let scale = base_scale * clip_percent as f32 / 100.0;
        let error = weighted_q4_row_error(row, importance, scale);
        if error < best_error {
            best_error = error;
            best_scale = scale;
        }
    }
    best_scale
}

fn weighted_q4_row_error(row: &[f32], importance: &[f32], scale: f32) -> f32 {
    row.iter()
        .zip(importance)
        .map(|(value, weight)| {
            let q = (*value / scale).round().clamp(-7.0, 7.0);
            let err = *value - q * scale;
            err * err * *weight
        })
        .sum()
}

fn pack_q4_row(row: &[f32], scale: f32, packed: &mut [u8]) {
    packed.fill(0);
    for (col, value) in row.iter().enumerate() {
        let q = (value / scale).round().clamp(-7.0, 7.0) as i8;
        let nibble = (q + 8) as u8 & 0x0f;
        let idx = col / 2;
        if col % 2 == 0 {
            packed[idx] = (packed[idx] & 0xf0) | nibble;
        } else {
            packed[idx] = (packed[idx] & 0x0f) | (nibble << 4);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowQ5Matrix {
    pub rows: usize,
    pub cols: usize,
    pub scales: Vec<f32>,
    pub packed: Vec<u8>,
    pub unpacked: Vec<i8>,
}

impl RowQ5Matrix {
    pub fn quantize(matrix: &Matrix) -> Self {
        let mut scales = Vec::with_capacity(matrix.rows);
        let mut packed = vec![0_u8; q5_packed_len(matrix.rows, matrix.cols)];
        let mut unpacked = vec![0_i8; matrix.rows * matrix.cols];
        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 15.0 } else { 1.0 };
            let scale = refine_scale(row, initial_scale, 15.0);
            scales.push(scale);
            for (col, value) in row.iter().enumerate().take(matrix.cols) {
                let q = (value / scale).round().clamp(-15.0, 15.0) as i8;
                unpacked[row_idx * matrix.cols + col] = q;
                let code = (q + 16) as u8 & 0x1f;
                write_bits(&mut packed, (row_idx * matrix.cols + col) * 5, 5, code);
            }
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            packed,
            unpacked,
        }
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_iter_mut()
                    .enumerate()
                    .for_each(|(row_idx, out_cell)| {
                        *out_cell = self.q5_row_dot(row_idx, x);
                    });
                return;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            *out_cell = self.q5_row_dot(row_idx, x);
        }
    }

    pub fn matvec_q8_activation(&self, x_quantized: &[i8], x_scale: f32) -> Vec<f32> {
        assert_eq!(self.cols, x_quantized.len());
        let mut out = vec![0.0; self.rows];
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_iter_mut()
                    .enumerate()
                    .for_each(|(row_idx, out_cell)| {
                        *out_cell = self.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                    });
                return out;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            *out_cell = self.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
        }
        out
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        let (a_value, b_value) = RowQ5Matrix::q5_row_dot2(a, b, row_idx, x);
                        *a_cell = a_value;
                        *b_cell = b_value;
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            let (a_value, b_value) = RowQ5Matrix::q5_row_dot2(a, b, row_idx, x);
            out_a[row_idx] = a_value;
            out_b[row_idx] = b_value;
        }
        (out_a, out_b)
    }

    fn q5_row_dot2(a: &Self, b: &Self, row_idx: usize, x: &[f32]) -> (f32, f32) {
        let start = row_idx * a.cols;
        crate::kernels::q8_i8_dot2_f32_scaled(
            &a.unpacked[start..start + a.cols],
            &b.unpacked[start..start + b.cols],
            x,
            a.scales[row_idx],
            b.scales[row_idx],
        )
    }

    pub fn matvec2_q8_activation(
        a: &Self,
        b: &Self,
        x_quantized: &[i8],
        x_scale: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x_quantized.len());
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        *a_cell = a.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                        *b_cell = b.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            out_a[row_idx] = a.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
            out_b[row_idx] = b.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
        }
        (out_a, out_b)
    }

    fn q5_row_dot(&self, row_idx: usize, x: &[f32]) -> f32 {
        let start = row_idx * self.cols;
        crate::kernels::q8_i8_dot_f32_scaled(
            &self.unpacked[start..start + self.cols],
            x,
            self.scales[row_idx],
        )
    }

    fn q5_row_dot_q8_activation(&self, row_idx: usize, x_quantized: &[i8], x_scale: f32) -> f32 {
        let start = row_idx * self.cols;
        crate::kernels::q8_i8_dot_i8_scaled(
            &self.unpacked[start..start + self.cols],
            x_quantized,
            self.scales[row_idx],
            x_scale,
        )
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let scale = self.scales[row_idx];
        let start = row_idx * self.cols;
        self.unpacked[start..start + self.cols]
            .iter()
            .map(|q| *q as f32 * scale)
            .collect()
    }

    pub fn quantize_lazy_rows(tensor: &LazyRowTensor) -> Result<Self> {
        let mut scales = Vec::with_capacity(tensor.rows());
        let mut packed = vec![0_u8; q5_packed_len(tensor.rows(), tensor.cols())];
        let mut unpacked = vec![0_i8; tensor.rows() * tensor.cols()];
        for row_idx in 0..tensor.rows() {
            let row = tensor.row_f32(row_idx)?;
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 15.0 } else { 1.0 };
            let scale = refine_scale(&row, initial_scale, 15.0);
            scales.push(scale);
            for (col, value) in row.iter().enumerate().take(tensor.cols()) {
                let q = (value / scale).round().clamp(-15.0, 15.0) as i8;
                unpacked[row_idx * tensor.cols() + col] = q;
                let code = (q + 16) as u8 & 0x1f;
                write_bits(&mut packed, (row_idx * tensor.cols() + col) * 5, 5, code);
            }
        }
        Ok(Self {
            rows: tensor.rows(),
            cols: tensor.cols(),
            scales,
            packed,
            unpacked,
        })
    }

    pub fn write_zq5(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let expected_len = 24_u64 + (self.scales.len() as u64 * 4) + self.packed.len() as u64;
        write_quant_file_atomic(path, expected_len, |file| {
            file.write_all(ZQ5_MAGIC)?;
            file.write_all(&(self.rows as u64).to_le_bytes())?;
            file.write_all(&(self.cols as u64).to_le_bytes())?;
            for scale in &self.scales {
                file.write_all(&scale.to_le_bytes())?;
            }
            file.write_all(&self.packed)?;
            Ok(())
        })
    }

    pub fn read_zq5(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file =
            fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != ZQ5_MAGIC {
            bail!("invalid zq5 magic in {}", path.display());
        }
        let rows = read_u64(&mut file)? as usize;
        let cols = read_u64(&mut file)? as usize;

        validate_quantized_matrix_shape(rows, cols, "zq5")?;

        let mut scales = Vec::with_capacity(rows);
        for _ in 0..rows {
            let mut bytes = [0_u8; 4];
            file.read_exact(&mut bytes)?;
            scales.push(f32::from_le_bytes(bytes));
        }
        let packed_len = q5_packed_len(rows, cols);
        let mut packed = vec![0_u8; packed_len];
        file.read_exact(&mut packed)?;
        let unpacked = unpack_q5_codes(rows, cols, &packed);
        let mut trailing = [0_u8; 1];
        if file.read(&mut trailing)? != 0 {
            bail!("trailing bytes in zq5 file {}", path.display());
        }
        Ok(Self {
            rows,
            cols,
            scales,
            packed,
            unpacked,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MmapQ4Matrix {
    pub rows: usize,
    pub cols: usize,
    mmap: ByteStorage,
    scales: Vec<f32>,
    data_offset: usize,
}

impl MmapQ4Matrix {
    pub fn read_zq4_storage(storage: ByteStorage, path_debug: &str) -> Result<Self> {
        if storage.len() < 24 {
            bail!("zq4 file too small: {}", path_debug);
        }
        if &storage[0..8] != ZQ4_MAGIC {
            bail!("invalid zq4 magic in {}", path_debug);
        }
        let rows = read_u64_bytes(&storage[8..16])? as usize;
        let cols = read_u64_bytes(&storage[16..24])? as usize;

        validate_quantized_matrix_shape(rows, cols, "zq4 storage")?;

        let scales_offset: usize = 24;
        let packed_cols = cols.div_ceil(2);
        let scales_len = rows.checked_mul(4).context("overflow in rows * 4")?;
        let data_offset = scales_offset
            .checked_add(scales_len)
            .context("overflow in data_offset")?;
        let data_len = rows
            .checked_mul(packed_cols)
            .context("overflow in rows * packed_cols")?;
        let expected_len = data_offset
            .checked_add(data_len)
            .context("overflow in expected_len")?;

        if storage.len() != expected_len {
            bail!(
                "invalid zq4 length in {}: expected {} got {}",
                path_debug,
                expected_len,
                storage.len()
            );
        }
        let scales = read_f32_scales(&storage, scales_offset, rows)?;
        Ok(Self {
            rows,
            cols,
            mmap: storage,
            scales,
            data_offset,
        })
    }

    pub fn read_zq4_mmap(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mmap = crate::mmap_utils::map_read_only(path)?;
        Self::read_zq4_storage(
            ByteStorage::Mmap(Arc::new(mmap)),
            &path.display().to_string(),
        )
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        let packed_cols = self.cols.div_ceil(2);
        let dot_kernel = crate::kernels::select_q4_dot_kernel();
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_chunks_mut(64)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        let chunk_row_base = chunk_idx * 64;
                        for (sub_idx, sub_chunk) in chunk.chunks_mut(4).enumerate() {
                            let row_base = chunk_row_base + sub_idx * 4;
                            let len = sub_chunk.len();
                            if len == 4 {
                                let start0 = self.data_offset + row_base * packed_cols;
                                let start1 = self.data_offset + (row_base + 1) * packed_cols;
                                let start2 = self.data_offset + (row_base + 2) * packed_cols;
                                let start3 = self.data_offset + (row_base + 3) * packed_cols;
                                let (r0, r1, r2, r3) = q4_dot4_f32_scaled_with_kernel(
                                    &self.mmap[start0..start0 + packed_cols],
                                    &self.mmap[start1..start1 + packed_cols],
                                    &self.mmap[start2..start2 + packed_cols],
                                    &self.mmap[start3..start3 + packed_cols],
                                    x,
                                    self.scale(row_base),
                                    self.scale(row_base + 1),
                                    self.scale(row_base + 2),
                                    self.scale(row_base + 3),
                                    dot_kernel,
                                );
                                sub_chunk[0] = r0;
                                sub_chunk[1] = r1;
                                sub_chunk[2] = r2;
                                sub_chunk[3] = r3;
                            } else {
                                for (i, cell) in sub_chunk.iter_mut().enumerate() {
                                    *cell =
                                        self.q4_row_dot(row_base + i, packed_cols, x, dot_kernel);
                                }
                            }
                        }
                    });
                return;
            }
        }
        let mut row_idx = 0;
        while row_idx + 4 <= self.rows {
            let start0 = self.data_offset + row_idx * packed_cols;
            let start1 = self.data_offset + (row_idx + 1) * packed_cols;
            let start2 = self.data_offset + (row_idx + 2) * packed_cols;
            let start3 = self.data_offset + (row_idx + 3) * packed_cols;
            let (r0, r1, r2, r3) = q4_dot4_f32_scaled_with_kernel(
                &self.mmap[start0..start0 + packed_cols],
                &self.mmap[start1..start1 + packed_cols],
                &self.mmap[start2..start2 + packed_cols],
                &self.mmap[start3..start3 + packed_cols],
                x,
                self.scale(row_idx),
                self.scale(row_idx + 1),
                self.scale(row_idx + 2),
                self.scale(row_idx + 3),
                dot_kernel,
            );
            out[row_idx] = r0;
            out[row_idx + 1] = r1;
            out[row_idx + 2] = r2;
            out[row_idx + 3] = r3;
            row_idx += 4;
        }
        while row_idx < self.rows {
            out[row_idx] = self.q4_row_dot(row_idx, packed_cols, x, dot_kernel);
            row_idx += 1;
        }
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let packed_cols = a.cols.div_ceil(2);
        let dot_kernel = crate::kernels::select_q4_dot_kernel();
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        let (a_value, b_value) =
                            MmapQ4Matrix::q4_row_dot2(a, b, row_idx, packed_cols, x, dot_kernel);
                        *a_cell = a_value;
                        *b_cell = b_value;
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            let (a_value, b_value) =
                MmapQ4Matrix::q4_row_dot2(a, b, row_idx, packed_cols, x, dot_kernel);
            out_a[row_idx] = a_value;
            out_b[row_idx] = b_value;
        }
        (out_a, out_b)
    }

    fn q4_row_dot2(
        a: &Self,
        b: &Self,
        row_idx: usize,
        packed_cols: usize,
        x: &[f32],
        dot_kernel: crate::kernels::Q4DotKernel,
    ) -> (f32, f32) {
        let a_start = a.data_offset + row_idx * packed_cols;
        let b_start = b.data_offset + row_idx * packed_cols;
        let a_row = &a.mmap[a_start..a_start + packed_cols];
        let b_row = &b.mmap[b_start..b_start + packed_cols];
        crate::kernels::q4_dot2_f32_scaled_with_kernel(
            a_row,
            b_row,
            x,
            a.scale(row_idx),
            b.scale(row_idx),
            dot_kernel,
        )
    }

    fn q4_row_dot(
        &self,
        row_idx: usize,
        packed_cols: usize,
        x: &[f32],
        dot_kernel: crate::kernels::Q4DotKernel,
    ) -> f32 {
        let start = self.data_offset + row_idx * packed_cols;
        let row_data = &self.mmap[start..start + packed_cols];
        crate::kernels::q4_dot_f32_scaled_with_kernel(row_data, x, self.scale(row_idx), dot_kernel)
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let packed_cols = self.cols.div_ceil(2);
        let mut row = vec![0.0f32; self.cols];
        let scale = self.scale(row_idx);
        let start = self.data_offset + row_idx * packed_cols;
        let row_data = &self.mmap[start..start + packed_cols];
        for (col, item) in row.iter_mut().enumerate() {
            let byte = row_data[col / 2];
            let nibble = if col % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            let q = nibble as i8 - 8;
            *item = q as f32 * scale;
        }
        row
    }

    fn scale(&self, row_idx: usize) -> f32 {
        self.scales[row_idx]
    }
}
#[derive(Debug, Clone)]
pub struct MmapQ5Matrix {
    pub rows: usize,
    pub cols: usize,
    scales: Vec<f32>,
    unpacked: Vec<i8>,
}

impl MmapQ5Matrix {
    pub fn read_zq5_storage(storage: ByteStorage, path_debug: &str) -> Result<Self> {
        if storage.len() < 24 {
            bail!("zq5 file too small: {}", path_debug);
        }
        if &storage[0..8] != ZQ5_MAGIC {
            bail!("invalid zq5 magic in {}", path_debug);
        }
        let rows = read_u64_bytes(&storage[8..16])? as usize;
        let cols = read_u64_bytes(&storage[16..24])? as usize;

        validate_quantized_matrix_shape(rows, cols, "zq5 storage")?;

        let scales_offset: usize = 24;
        let scales_len = rows.checked_mul(4).context("overflow in rows * 4")?;
        let data_offset = scales_offset
            .checked_add(scales_len)
            .context("overflow in data_offset")?;

        let total_bits = (rows as u128)
            .checked_mul(cols as u128)
            .context("overflow in rows * cols")?
            .checked_mul(5)
            .context("overflow in total bits")?;
        let packed_len = total_bits.div_ceil(8) as usize;
        let expected_len = data_offset
            .checked_add(packed_len)
            .context("overflow in expected_len")?;

        if storage.len() != expected_len {
            bail!(
                "invalid zq5 length in {}: expected {} got {}",
                path_debug,
                expected_len,
                storage.len()
            );
        }
        let scales = read_f32_scales(&storage, scales_offset, rows)?;
        let unpacked = unpack_q5_codes(rows, cols, &storage[data_offset..expected_len]);
        Ok(Self {
            rows,
            cols,
            scales,
            unpacked,
        })
    }

    pub fn read_zq5_mmap(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mmap = crate::mmap_utils::map_read_only(path)?;
        Self::read_zq5_storage(
            ByteStorage::Mmap(Arc::new(mmap)),
            &path.display().to_string(),
        )
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_iter_mut()
                    .enumerate()
                    .for_each(|(row_idx, out_cell)| {
                        *out_cell = self.q5_row_dot(row_idx, x);
                    });
                return;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            *out_cell = self.q5_row_dot(row_idx, x);
        }
    }

    pub fn matvec_q8_activation(&self, x_quantized: &[i8], x_scale: f32) -> Vec<f32> {
        assert_eq!(self.cols, x_quantized.len());
        let mut out = vec![0.0; self.rows];
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_iter_mut()
                    .enumerate()
                    .for_each(|(row_idx, out_cell)| {
                        *out_cell = self.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                    });
                return out;
            }
        }
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            *out_cell = self.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
        }
        out
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        let (a_value, b_value) = MmapQ5Matrix::q5_row_dot2(a, b, row_idx, x);
                        *a_cell = a_value;
                        *b_cell = b_value;
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            let (a_value, b_value) = MmapQ5Matrix::q5_row_dot2(a, b, row_idx, x);
            out_a[row_idx] = a_value;
            out_b[row_idx] = b_value;
        }
        (out_a, out_b)
    }

    fn q5_row_dot2(a: &Self, b: &Self, row_idx: usize, x: &[f32]) -> (f32, f32) {
        let start = row_idx * a.cols;
        crate::kernels::q8_i8_dot2_f32_scaled(
            &a.unpacked[start..start + a.cols],
            &b.unpacked[start..start + b.cols],
            x,
            a.scale(row_idx),
            b.scale(row_idx),
        )
    }

    pub fn matvec2_q8_activation(
        a: &Self,
        b: &Self,
        x_quantized: &[i8],
        x_scale: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x_quantized.len());
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];
        #[cfg(feature = "parallel")]
        {
            if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out_a
                    .par_iter_mut()
                    .zip(out_b.par_iter_mut())
                    .enumerate()
                    .for_each(|(row_idx, (a_cell, b_cell))| {
                        *a_cell = a.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                        *b_cell = b.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
                    });
                return (out_a, out_b);
            }
        }
        for row_idx in 0..a.rows {
            out_a[row_idx] = a.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
            out_b[row_idx] = b.q5_row_dot_q8_activation(row_idx, x_quantized, x_scale);
        }
        (out_a, out_b)
    }

    fn q5_row_dot(&self, row_idx: usize, x: &[f32]) -> f32 {
        let start = row_idx * self.cols;
        crate::kernels::q8_i8_dot_f32_scaled(
            &self.unpacked[start..start + self.cols],
            x,
            self.scale(row_idx),
        )
    }

    fn q5_row_dot_q8_activation(&self, row_idx: usize, x_quantized: &[i8], x_scale: f32) -> f32 {
        let start = row_idx * self.cols;
        crate::kernels::q8_i8_dot_i8_scaled(
            &self.unpacked[start..start + self.cols],
            x_quantized,
            self.scale(row_idx),
            x_scale,
        )
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let scale = self.scale(row_idx);
        let start = row_idx * self.cols;
        self.unpacked[start..start + self.cols]
            .iter()
            .map(|q| *q as f32 * scale)
            .collect()
    }

    fn scale(&self, row_idx: usize) -> f32 {
        self.scales[row_idx]
    }
}

fn q5_packed_len(rows: usize, cols: usize) -> usize {
    (rows * cols * 5).div_ceil(8)
}

fn unpack_q5_codes(rows: usize, cols: usize, packed: &[u8]) -> Vec<i8> {
    let mut unpacked = vec![0_i8; rows * cols];
    let row_bits = cols * 5;
    if row_bits.is_multiple_of(8) {
        let row_bytes = row_bits / 8;
        #[cfg(feature = "parallel")]
        {
            if rows * cols >= PARALLEL_MATVEC_WORK_ITEMS {
                unpacked
                    .par_chunks_mut(cols)
                    .enumerate()
                    .for_each(|(row_idx, row_out)| {
                        let row_packed = &packed[row_idx * row_bytes..(row_idx + 1) * row_bytes];
                        unpack_q5_aligned_row(cols, row_packed, row_out);
                    });
                return unpacked;
            }
        }
        for row_idx in 0..rows {
            let row_packed = &packed[row_idx * row_bytes..(row_idx + 1) * row_bytes];
            let row_out = &mut unpacked[row_idx * cols..(row_idx + 1) * cols];
            unpack_q5_aligned_row(cols, row_packed, row_out);
        }
    } else {
        for (idx, item) in unpacked.iter_mut().enumerate() {
            *item = read_bits(packed, idx * 5, 5) as i8 - 16;
        }
    }
    unpacked
}

fn unpack_q5_aligned_row(cols: usize, row_packed: &[u8], row_out: &mut [i8]) {
    let mut col = 0_usize;
    let mut byte = 0_usize;
    while col + 8 <= cols {
        let codes = decode_q5_aligned_8(row_packed, byte);
        row_out[col..col + 8].copy_from_slice(&codes);
        col += 8;
        byte += 5;
    }
    while col < cols {
        row_out[col] = read_bits(row_packed, col * 5, 5) as i8 - 16;
        col += 1;
    }
}

fn decode_q5_aligned_8(src: &[u8], offset: usize) -> [i8; 8] {
    let b0 = src[offset];
    let b1 = src[offset + 1];
    let b2 = src[offset + 2];
    let b3 = src[offset + 3];
    let b4 = src[offset + 4];
    [
        ((b0 & 0x1f) as i8) - 16,
        (((b0 >> 5) | (b1 << 3)) & 0x1f) as i8 - 16,
        ((b1 >> 2) & 0x1f) as i8 - 16,
        (((b1 >> 7) | (b2 << 1)) & 0x1f) as i8 - 16,
        (((b2 >> 4) | (b3 << 4)) & 0x1f) as i8 - 16,
        ((b3 >> 1) & 0x1f) as i8 - 16,
        (((b3 >> 6) | (b4 << 2)) & 0x1f) as i8 - 16,
        ((b4 >> 3) & 0x1f) as i8 - 16,
    ]
}

fn write_bits(buf: &mut [u8], bit_index: usize, bit_count: usize, value: u8) {
    for bit in 0..bit_count {
        let bit_value = (value >> bit) & 1;
        let dst = bit_index + bit;
        let byte_idx = dst / 8;
        let bit_idx = dst % 8;
        if bit_value == 1 {
            buf[byte_idx] |= 1 << bit_idx;
        } else {
            buf[byte_idx] &= !(1 << bit_idx);
        }
    }
}

fn read_bits(buf: &[u8], bit_index: usize, bit_count: usize) -> u8 {
    let mut value = 0_u8;
    for bit in 0..bit_count {
        let src = bit_index + bit;
        let byte_idx = src / 8;
        let bit_idx = src % 8;
        value |= ((buf[byte_idx] >> bit_idx) & 1) << bit;
    }
    value
}

#[inline]
fn decode_q3_packed_value(row_packed: &[u8], col: usize) -> i8 {
    let byte_index = (col / 8) * 3;
    let code = match col % 8 {
        0 => row_packed[byte_index] & 0x07,
        1 => (row_packed[byte_index] >> 3) & 0x07,
        2 => ((row_packed[byte_index] >> 6) & 0x03) | ((row_packed[byte_index + 1] & 0x01) << 2),
        3 => (row_packed[byte_index + 1] >> 1) & 0x07,
        4 => (row_packed[byte_index + 1] >> 4) & 0x07,
        5 => {
            ((row_packed[byte_index + 1] >> 7) & 0x01) | ((row_packed[byte_index + 2] & 0x03) << 1)
        }
        6 => (row_packed[byte_index + 2] >> 2) & 0x07,
        7 => (row_packed[byte_index + 2] >> 5) & 0x07,
        _ => unreachable!(),
    };
    code as i8 - 4
}

fn pack_q3_row(row: &[f32], scale: f32, row_packed: &mut [u8]) {
    row_packed.fill(0);
    for (col, &value) in row.iter().enumerate() {
        let code = ((value / scale).round().clamp(-4.0, 3.0) as i8 + 4) as u8;
        let byte_index = (col / 8) * 3;
        match col % 8 {
            0 => row_packed[byte_index] |= code,
            1 => row_packed[byte_index] |= code << 3,
            2 => {
                row_packed[byte_index] |= (code & 0x03) << 6;
                row_packed[byte_index + 1] |= code >> 2;
            }
            3 => row_packed[byte_index + 1] |= code << 1,
            4 => row_packed[byte_index + 1] |= code << 4,
            5 => {
                row_packed[byte_index + 1] |= (code & 0x01) << 7;
                row_packed[byte_index + 2] |= code >> 1;
            }
            6 => row_packed[byte_index + 2] |= code << 2,
            7 => row_packed[byte_index + 2] |= code << 5,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowQ3Matrix {
    pub rows: usize,
    pub cols: usize,
    pub scales: Vec<f32>,
    pub packed: Vec<u8>,
}

impl RowQ3Matrix {
    pub fn quantize(matrix: &Matrix) -> Self {
        let packed_cols = (matrix.cols * 3).div_ceil(8);
        let mut scales = Vec::with_capacity(matrix.rows);
        let mut packed = vec![0_u8; matrix.rows * packed_cols];
        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 3.5 } else { 1.0 };
            let scale = refine_scale(row, initial_scale, 3.5);
            scales.push(scale);

            let row_packed_start = row_idx * packed_cols;
            let row_packed = &mut packed[row_packed_start..row_packed_start + packed_cols];
            pack_q3_row(row, scale, row_packed);
        }
        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            packed,
        }
    }

    pub fn quantize_rows<F>(rows: usize, cols: usize, mut row_fn: F) -> Result<Self>
    where
        F: FnMut(usize) -> Result<Vec<f32>>,
    {
        validate_quantized_matrix_shape(rows, cols, "q3 rows")?;
        let packed_cols = cols
            .checked_mul(3)
            .context("overflow in cols * 3")?
            .div_ceil(8);
        let packed_len = rows
            .checked_mul(packed_cols)
            .context("overflow in rows * packed_cols")?;
        let mut scales = Vec::with_capacity(rows);
        let mut packed = vec![0_u8; packed_len];
        for row_idx in 0..rows {
            let row = row_fn(row_idx)?;
            if row.len() != cols {
                bail!(
                    "q3 row width mismatch at row {row_idx}: expected {cols}, got {}",
                    row.len()
                );
            }
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let initial_scale = if max_abs > 0.0 { max_abs / 3.5 } else { 1.0 };
            let scale = refine_scale(&row, initial_scale, 3.5);
            scales.push(scale);
            let start = row_idx * packed_cols;
            pack_q3_row(&row, scale, &mut packed[start..start + packed_cols]);
        }
        Ok(Self {
            rows,
            cols,
            scales,
            packed,
        })
    }

    pub fn quantize_lazy_rows(tensor: &LazyRowTensor) -> Result<Self> {
        Self::quantize_rows(tensor.rows(), tensor.cols(), |row| tensor.row_f32(row))
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        let packed_cols = (self.cols * 3).div_ceil(8);
        #[cfg(feature = "parallel")]
        {
            if self.rows * self.cols >= PARALLEL_MATVEC_WORK_ITEMS {
                out.par_chunks_mut(64)
                    .enumerate()
                    .for_each(|(chunk_idx, chunk)| {
                        self.matvec_rows(x, packed_cols, chunk_idx * 64, chunk);
                    });
                return;
            }
        }

        self.matvec_rows(x, packed_cols, 0, out);
    }

    fn matvec_rows(&self, x: &[f32], packed_cols: usize, row_base: usize, out: &mut [f32]) {
        let mut offset = 0_usize;
        while offset + 4 <= out.len() {
            let row = row_base + offset;
            let starts: [usize; 4] = std::array::from_fn(|lane| (row + lane) * packed_cols);
            let values = q3_dot4_f32_scaled(
                &self.packed[starts[0]..starts[0] + packed_cols],
                &self.packed[starts[1]..starts[1] + packed_cols],
                &self.packed[starts[2]..starts[2] + packed_cols],
                &self.packed[starts[3]..starts[3] + packed_cols],
                x,
                self.scales[row],
                self.scales[row + 1],
                self.scales[row + 2],
                self.scales[row + 3],
            );
            out[offset..offset + 4].copy_from_slice(&[values.0, values.1, values.2, values.3]);
            offset += 4;
        }
        while offset < out.len() {
            let row = row_base + offset;
            let start = row * packed_cols;
            out[offset] = q3_dot_f32_scaled(
                &self.packed[start..start + packed_cols],
                x,
                self.scales[row],
            );
            offset += 1;
        }
    }

    pub fn matvec2(a: &Self, b: &Self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.cols, b.cols);
        assert_eq!(a.cols, x.len());
        let packed_cols = a.cols.saturating_mul(3).div_ceil(8);
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];

        #[cfg(feature = "parallel")]
        if a.rows * a.cols >= PARALLEL_MATVEC_WORK_ITEMS {
            out_a
                .par_chunks_mut(64)
                .zip(out_b.par_chunks_mut(64))
                .enumerate()
                .for_each(|(chunk_idx, (chunk_a, chunk_b))| {
                    let row_base = chunk_idx * 64;
                    Self::matvec2_rows(a, b, x, packed_cols, row_base, chunk_a, chunk_b);
                });
            return (out_a, out_b);
        }

        Self::matvec2_rows(a, b, x, packed_cols, 0, &mut out_a, &mut out_b);
        (out_a, out_b)
    }

    #[allow(clippy::too_many_arguments)]
    fn matvec2_rows(
        a: &Self,
        b: &Self,
        x: &[f32],
        packed_cols: usize,
        row_base: usize,
        out_a: &mut [f32],
        out_b: &mut [f32],
    ) {
        let mut offset = 0_usize;
        while offset + 2 <= out_a.len() {
            let row0 = row_base + offset;
            let row1 = row0 + 1;
            let a0 = row0 * packed_cols;
            let a1 = row1 * packed_cols;
            let b0 = row0 * packed_cols;
            let b1 = row1 * packed_cols;
            let values = q3_dot4_f32_scaled(
                &a.packed[a0..a0 + packed_cols],
                &a.packed[a1..a1 + packed_cols],
                &b.packed[b0..b0 + packed_cols],
                &b.packed[b1..b1 + packed_cols],
                x,
                a.scales[row0],
                a.scales[row1],
                b.scales[row0],
                b.scales[row1],
            );
            out_a[offset] = values.0;
            out_a[offset + 1] = values.1;
            out_b[offset] = values.2;
            out_b[offset + 1] = values.3;
            offset += 2;
        }
        if offset < out_a.len() {
            let row = row_base + offset;
            let a_start = row * packed_cols;
            let b_start = row * packed_cols;
            out_a[offset] =
                q3_dot_f32_scaled(&a.packed[a_start..a_start + packed_cols], x, a.scales[row]);
            out_b[offset] =
                q3_dot_f32_scaled(&b.packed[b_start..b_start + packed_cols], x, b.scales[row]);
        }
    }

    pub fn write_zq3(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let expected_len = 24_u64 + (self.scales.len() as u64 * 4) + self.packed.len() as u64;
        write_quant_file_atomic(path, expected_len, |file| {
            file.write_all(ZQ3_MAGIC)?;
            file.write_all(&(self.rows as u64).to_le_bytes())?;
            file.write_all(&(self.cols as u64).to_le_bytes())?;
            for scale in &self.scales {
                file.write_all(&scale.to_le_bytes())?;
            }
            file.write_all(&self.packed)?;
            Ok(())
        })
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let packed_cols = (self.cols * 3).div_ceil(8);
        let start = row_idx * packed_cols;
        let scale = self.scales[row_idx];
        let row_packed = &self.packed[start..start + packed_cols];
        let mut row = vec![0.0_f32; self.cols];
        for (col, item) in row.iter_mut().enumerate().take(self.cols) {
            *item = decode_q3_packed_value(row_packed, col) as f32 * scale;
        }
        row
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowQ1_58Matrix {
    pub rows: usize,
    pub cols: usize,
    pub scales: Vec<f32>,
    pub packed: Vec<u8>,
}

impl RowQ1_58Matrix {
    pub fn quantize(matrix: &Matrix) -> Self {
        let mut scales = Vec::with_capacity(matrix.rows);
        let packed_cols = matrix.cols.div_ceil(4);
        let mut packed = vec![0_u8; matrix.rows * packed_cols];

        for row_idx in 0..matrix.rows {
            let row = matrix.row(row_idx);
            let max_abs = row.iter().copied().map(f32::abs).fold(0.0, f32::max);
            let scale = if max_abs > 0.0 { max_abs } else { 1.0 };
            scales.push(scale);

            let row_packed_start = row_idx * packed_cols;
            let row_packed = &mut packed[row_packed_start..row_packed_start + packed_cols];
            for (col, &v) in row.iter().enumerate() {
                let norm = v / scale;
                let code = if norm > 0.33 {
                    2_u8
                } else if norm < -0.33 {
                    0_u8
                } else {
                    1_u8
                };
                let byte_idx = col / 4;
                let shift = (col % 4) * 2;
                row_packed[byte_idx] |= code << shift;
            }
        }

        Self {
            rows: matrix.rows,
            cols: matrix.cols,
            scales,
            packed,
        }
    }

    pub fn matvec_into(&self, x: &[f32], out: &mut [f32]) {
        assert_eq!(self.cols, x.len());
        assert_eq!(self.rows, out.len());
        let packed_cols = self.cols.div_ceil(4);
        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            let scale = self.scales[row_idx];
            let start = row_idx * packed_cols;
            let row_packed = &self.packed[start..start + packed_cols];
            *out_cell = q1_58_dot_f32_scaled(row_packed, x, scale, self.cols);
        }
    }

    pub fn matvec(&self, x: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; self.rows];
        self.matvec_into(x, &mut out);
        out
    }

    pub fn dequantize_row(&self, row_idx: usize) -> Vec<f32> {
        let packed_cols = self.cols.div_ceil(4);
        let start = row_idx * packed_cols;
        let scale = self.scales[row_idx];
        let row_packed = &self.packed[start..start + packed_cols];
        let mut row = vec![0.0_f32; self.cols];
        for (col, item) in row.iter_mut().enumerate().take(self.cols) {
            let byte = row_packed[col / 4];
            let code = (byte >> ((col % 4) * 2)) & 0x03;
            let val = code as i8 - 1;
            *item = val as f32 * scale;
        }
        row
    }
}

pub fn relative_l2_error(reference: &[f32], candidate: &[f32]) -> f32 {
    assert_eq!(reference.len(), candidate.len());
    let numerator = reference
        .iter()
        .zip(candidate)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        .sqrt();
    let denominator = dot(reference, reference).sqrt().max(1e-9);
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn quantized_shape_budget_allows_e4b_embedding_matrix() {
        let rows = 262_144;
        let cols = 2_560;
        let total = validate_quantized_matrix_shape(rows, cols, "zq5 storage").unwrap();

        assert_eq!(total, 671_088_640);
        assert_eq!(q5_packed_len(rows, cols), 419_430_400);
    }

    #[test]
    fn q1_58_matvec_and_quantization_roundtrip() {
        let rows = 4;
        let cols = 8;
        let matrix = Matrix::from_row_major(
            rows,
            cols,
            vec![
                0.9, -0.8, 0.1, 0.0, -0.9, 0.85, -0.05, 0.0, 0.2, -0.3, 0.9, -0.85, 0.0, -0.1, 0.7,
                -0.9, -0.9, 0.0, 0.8, -0.1, 0.9, -0.85, 0.05, -0.2, 0.8, -0.9, 0.0, 0.1, -0.85,
                0.9, -0.1, 0.0,
            ],
        );
        let x = vec![1.0, -1.0, 0.5, -0.5, 2.0, -2.0, 0.25, -0.25];
        let resident = RowQ1_58Matrix::quantize(&matrix);
        assert_eq!(resident.rows, rows);
        assert_eq!(resident.cols, cols);

        let output = resident.matvec(&x);
        assert_eq!(output.len(), rows);
        for row in 0..rows {
            let dequant = resident.dequantize_row(row);
            assert_eq!(dequant.len(), cols);
        }
    }

    #[test]
    fn quantized_shape_budget_still_rejects_unbounded_matrix_headers() {
        let err = validate_quantized_matrix_shape(1_000_001, 1_000, "zq5 storage").unwrap_err();

        assert!(err.to_string().contains("matrix size exceeds budget"));
    }

    #[test]
    fn q3_tail_matvec_and_mmap_cache_match_resident_matrix() {
        let rows = 5;
        let cols = 11;
        let matrix = Matrix::from_row_major(
            rows,
            cols,
            (0..rows * cols)
                .map(|index| ((index as f32 * 0.31).sin() * 1.7) - 0.25)
                .collect(),
        );
        let x = (0..cols)
            .map(|index| ((index as f32 * 0.19).cos() * 0.8) + 0.1)
            .collect::<Vec<_>>();
        let resident = RowQ3Matrix::quantize(&matrix);
        let streamed =
            RowQ3Matrix::quantize_rows(rows, cols, |row| Ok(matrix.row(row).to_vec())).unwrap();
        assert_eq!(streamed, resident);
        let resident_output = resident.matvec(&x);
        for (row, &output) in resident_output.iter().enumerate() {
            let expected = dot(&resident.dequantize_row(row), &x);
            assert!((output - expected).abs() < 1.0e-5);
        }

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("matrix.zq3");
        resident.write_zq3(&path).unwrap();
        let mapped = MmapQ3Matrix::read_zq3_mmap(&path).unwrap();

        assert_eq!(mapped.rows, rows);
        assert_eq!(mapped.cols, cols);
        assert_eq!(mapped.matvec(&x), resident_output);
        for row in 0..rows {
            assert_eq!(mapped.dequantize_row(row), resident.dequantize_row(row));
        }

        let matrix_b = Matrix::from_row_major(
            rows,
            cols,
            (0..rows * cols)
                .map(|index| ((index as f32 * 0.23).cos() * 1.4) + 0.15)
                .collect(),
        );
        let resident_b = RowQ3Matrix::quantize(&matrix_b);
        let (pair_a, pair_b) = RowQ3Matrix::matvec2(&resident, &resident_b, &x);
        assert_eq!(pair_a, resident_output);
        assert_eq!(pair_b, resident_b.matvec(&x));

        let path_b = temp.path().join("matrix-b.zq3");
        resident_b.write_zq3(&path_b).unwrap();
        let mapped_b = MmapQ3Matrix::read_zq3_mmap(&path_b).unwrap();
        let (mapped_pair_a, mapped_pair_b) = MmapQ3Matrix::matvec2(&mapped, &mapped_b, &x);
        assert_eq!(mapped_pair_a, resident_output);
        assert_eq!(mapped_pair_b, resident_b.matvec(&x));
    }

    #[test]
    fn q8_matvec_tracks_f32_closely() {
        let m = Matrix::from_row_major(
            3,
            4,
            vec![
                0.1, -0.2, 0.3, -0.4, 1.0, 2.0, 3.0, 4.0, -2.0, 0.5, 0.25, 0.125,
            ],
        );
        let x = [0.7, -1.2, 0.3, 2.1];
        let reference = crate::ops::matvec(&m, &x);
        let q = RowQ8Matrix::quantize(&m);
        let got = q.matvec(&x);
        assert!(relative_l2_error(&reference, &got) < 0.01);
    }

    #[test]
    fn q8_activation_pipeline_tracks_f32_activation_path() {
        let m = Matrix::from_row_major(2, 4, vec![0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25]);
        let x = vec![0.4, -0.9, 1.7, 2.2];
        let q = RowQ8Matrix::quantize(&m);
        let f32_path = q.matvec(&x);
        let (xq, x_scale) = quantize_activation_q8(&x);
        let int8_path = q.matvec_q8_activation(&xq, x_scale);
        assert!(relative_l2_error(&f32_path, &int8_path) < 0.02);
    }

    #[test]
    fn mmap_q8_activation_pipeline_matches_resident_path() {
        let m = Matrix::from_row_major(
            3,
            4,
            vec![
                0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25, 0.125, -0.5, 0.875, -1.5,
            ],
        );
        let x = vec![0.4, -0.9, 1.7, 2.2];
        let q = RowQ8Matrix::quantize(&m);
        let (xq, x_scale) = quantize_activation_q8(&x);
        let resident = q.matvec_q8_activation(&xq, x_scale);

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("matrix.zq8");
        q.write_zq8(&path).unwrap();
        let mmap = MmapQ8Matrix::read_zq8_mmap(&path).unwrap();

        assert_eq!(mmap.matvec_q8_activation(&xq, x_scale), resident);
    }

    #[test]
    fn q8_dynamic_activation_matvec3_tracks_f32_activation_path() {
        let a = QuantMatrix::Q8Resident(RowQ8Matrix::quantize(&Matrix::from_row_major(
            2,
            4,
            vec![0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25],
        )));
        let b = QuantMatrix::Q8Resident(RowQ8Matrix::quantize(&Matrix::from_row_major(
            1,
            4,
            vec![0.5, -1.0, 2.0, -0.25],
        )));
        let c = QuantMatrix::Q8Resident(RowQ8Matrix::quantize(&Matrix::from_row_major(
            2,
            4,
            vec![-2.0, 1.0, 0.25, 0.75, 1.25, -0.5, 0.375, -1.0],
        )));
        let x = [0.25, 0.5, 0.75, -1.25];
        let reference = QuantMatrix::matvec3(&a, &b, &c, &x);
        let got = QuantMatrix::matvec3_with_activation_mode(
            &a,
            &b,
            &c,
            &x,
            QuantizedActivationMode::DynamicInt8,
        );

        assert!(relative_l2_error(&reference.0, &got.0) < 0.02);
        assert!(relative_l2_error(&reference.1, &got.1) < 0.02);
        assert!(relative_l2_error(&reference.2, &got.2) < 0.02);
    }

    #[test]
    fn q5_activation_pipeline_tracks_f32_activation_path() {
        let m = Matrix::from_row_major(
            3,
            6,
            vec![
                0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25, 0.125, -0.5, 0.875, -1.5, 1.25,
                0.25, -0.375, 0.75, -0.625, 1.125,
            ],
        );
        let x = vec![0.4, -0.9, 1.7, 2.2, -0.6, 0.35];
        let q = RowQ5Matrix::quantize(&m);
        let f32_path = q.matvec(&x);
        let (xq, x_scale) = quantize_activation_q8(&x);
        let int8_path = q.matvec_q8_activation(&xq, x_scale);
        assert!(relative_l2_error(&f32_path, &int8_path) < 0.03);
    }

    #[test]
    fn mmap_q5_activation_pipeline_matches_resident_path() {
        let m = Matrix::from_row_major(
            3,
            6,
            vec![
                0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25, 0.125, -0.5, 0.875, -1.5, 1.25,
                0.25, -0.375, 0.75, -0.625, 1.125,
            ],
        );
        let x = vec![0.4, -0.9, 1.7, 2.2, -0.6, 0.35];
        let q = RowQ5Matrix::quantize(&m);
        let (xq, x_scale) = quantize_activation_q8(&x);
        let resident = q.matvec_q8_activation(&xq, x_scale);

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("matrix.zq5");
        q.write_zq5(&path).unwrap();
        let mmap = MmapQ5Matrix::read_zq5_mmap(&path).unwrap();

        assert_eq!(mmap.matvec_q8_activation(&xq, x_scale), resident);
    }

    #[test]
    fn q5_dynamic_activation_fused_projections_track_f32_activation_path() {
        let a = QuantMatrix::Q5Resident(RowQ5Matrix::quantize(&Matrix::from_row_major(
            2,
            4,
            vec![0.25, -0.75, 1.5, 2.0, -1.25, 0.5, 0.75, -0.25],
        )));
        let b = QuantMatrix::Q5Resident(RowQ5Matrix::quantize(&Matrix::from_row_major(
            1,
            4,
            vec![0.5, -1.0, 2.0, -0.25],
        )));
        let c = QuantMatrix::Q5Resident(RowQ5Matrix::quantize(&Matrix::from_row_major(
            2,
            4,
            vec![-2.0, 1.0, 0.25, 0.75, 1.25, -0.5, 0.375, -1.0],
        )));
        let x = [0.25, 0.5, 0.75, -1.25];
        let reference2 =
            QuantMatrix::matvec2_with_activation_mode(&a, &c, &x, QuantizedActivationMode::F32);
        let got2 = QuantMatrix::matvec2_with_activation_mode(
            &a,
            &c,
            &x,
            QuantizedActivationMode::DynamicInt8,
        );
        let reference3 = QuantMatrix::matvec3(&a, &b, &c, &x);
        let got3 = QuantMatrix::matvec3_with_activation_mode(
            &a,
            &b,
            &c,
            &x,
            QuantizedActivationMode::DynamicInt8,
        );

        assert!(relative_l2_error(&reference2.0, &got2.0) < 0.03);
        assert!(relative_l2_error(&reference2.1, &got2.1) < 0.03);
        assert!(relative_l2_error(&reference3.0, &got3.0) < 0.03);
        assert!(relative_l2_error(&reference3.1, &got3.1) < 0.03);
        assert!(relative_l2_error(&reference3.2, &got3.2) < 0.03);
    }

    #[test]
    fn quant_matrix_matvec3_matches_individual_outputs() {
        let a = QuantMatrix::Q8Resident(RowQ8Matrix::quantize(&Matrix::from_row_major(
            1,
            3,
            vec![1.0, 2.0, 3.0],
        )));
        let b = QuantMatrix::Q5Resident(RowQ5Matrix::quantize(&Matrix::from_row_major(
            1,
            3,
            vec![0.5, -1.0, 2.0],
        )));
        let c = QuantMatrix::Q4Resident(RowQ4Matrix::quantize(&Matrix::from_row_major(
            1,
            3,
            vec![-2.0, 1.0, 0.25],
        )));
        let x = [0.25, 0.5, 0.75];
        let (got_a, got_b, got_c) = QuantMatrix::matvec3(&a, &b, &c, &x);
        assert_eq!(got_a, a.matvec(&x));
        assert_eq!(got_b, b.matvec(&x));
        assert_eq!(got_c, c.matvec(&x));
    }

    #[test]
    fn quant_matrix_matvec2_matches_individual_outputs() {
        let a = QuantMatrix::Q8Resident(RowQ8Matrix::quantize(&Matrix::from_row_major(
            1,
            3,
            vec![1.0, 2.0, 3.0],
        )));
        let b = QuantMatrix::Q4Resident(RowQ4Matrix::quantize(&Matrix::from_row_major(
            1,
            3,
            vec![-2.0, 1.0, 0.25],
        )));
        let x = [0.25, 0.5, 0.75];
        let (got_a, got_b) =
            QuantMatrix::matvec2_with_activation_mode(&a, &b, &x, QuantizedActivationMode::F32);
        assert_eq!(got_a, a.matvec(&x));
        assert_eq!(got_b, b.matvec(&x));
    }

    #[test]
    fn q4_matvec_is_usable_low_precision_path() {
        let m = Matrix::from_row_major(
            2,
            5,
            vec![0.1, -0.2, 0.3, -0.4, 0.5, 1.0, -2.0, 3.0, -4.0, 5.0],
        );
        let x = [0.7, -1.2, 0.3, 2.1, -0.8];
        let reference = crate::ops::matvec(&m, &x);
        let q = RowQ4Matrix::quantize(&m);
        let got = q.matvec(&x);
        assert!(relative_l2_error(&reference, &got) < 0.20);
    }

    #[test]
    fn activation_aware_q4_reduces_calibrated_output_error() {
        let m = Matrix::from_row_major(1, 4, vec![10.0, 1.0, 1.0, 1.0]);
        let samples = vec![
            vec![0.01, 1.0, 1.0, 1.0],
            vec![-0.01, 0.5, 1.5, -1.0],
            vec![0.0, -1.0, 0.75, 1.25],
        ];
        let standard = RowQ4Matrix::quantize(&m);
        let aware = RowQ4Matrix::quantize_activation_aware(&m, &samples).unwrap();

        let standard_error = samples
            .iter()
            .map(|x| {
                let reference = crate::ops::matvec(&m, x);
                let got = standard.matvec(x);
                (reference[0] - got[0]).powi(2)
            })
            .sum::<f32>();
        let aware_error = samples
            .iter()
            .map(|x| {
                let reference = crate::ops::matvec(&m, x);
                let got = aware.matvec(x);
                (reference[0] - got[0]).powi(2)
            })
            .sum::<f32>();

        assert!(aware.scales[0] < standard.scales[0]);
        assert!(aware_error < standard_error * 0.25);
    }

    #[test]
    fn activation_aware_q4_rejects_bad_calibration_width() {
        let m = Matrix::from_row_major(1, 4, vec![10.0, 1.0, 1.0, 1.0]);
        let bad_samples = vec![vec![1.0, 2.0, 3.0]];
        assert!(RowQ4Matrix::quantize_activation_aware(&m, &bad_samples).is_err());
    }

    #[test]
    fn q5_matvec_is_middle_precision_path() {
        let m = Matrix::from_row_major(
            2,
            5,
            vec![0.1, -0.2, 0.3, -0.4, 0.5, 1.0, -2.0, 3.0, -4.0, 5.0],
        );
        let x = [0.7, -1.2, 0.3, 2.1, -0.8];
        let reference = crate::ops::matvec(&m, &x);
        let q = RowQ5Matrix::quantize(&m);
        let got = q.matvec(&x);
        assert!(relative_l2_error(&reference, &got) < 0.10);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn large_quantized_matvec_parallel_paths_are_usable() {
        let rows = 64;
        let cols = 4096;
        let values = (0..rows * cols)
            .map(|idx| ((idx % 251) as f32 - 125.0) / 64.0)
            .collect();
        let m = Matrix::from_row_major(rows, cols, values);
        let x = (0..cols)
            .map(|idx| ((idx % 97) as f32 - 48.0) / 128.0)
            .collect::<Vec<_>>();
        let reference = crate::ops::matvec(&m, &x);

        let q8 = RowQ8Matrix::quantize(&m).matvec(&x);
        let q5 = RowQ5Matrix::quantize(&m).matvec(&x);
        let q4 = RowQ4Matrix::quantize(&m).matvec(&x);

        assert!(relative_l2_error(&reference, &q8) < 0.01);
        assert!(relative_l2_error(&reference, &q5) < 0.08);
        assert!(relative_l2_error(&reference, &q4) < 0.20);
    }

    #[test]
    fn q8_zq8_cache_round_trips() {
        let m = Matrix::from_row_major(2, 4, vec![0.25, -0.5, 1.0, -2.0, 3.0, -4.0, 5.0, -6.0]);
        let q = RowQ8Matrix::quantize(&m);
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("matrix.zq8");
        q.write_zq8(&path).unwrap();
        let loaded = RowQ8Matrix::read_zq8(&path).unwrap();
        assert_eq!(loaded, q);
        assert_eq!(
            loaded.matvec(&[1.0, 2.0, 3.0, 4.0]),
            q.matvec(&[1.0, 2.0, 3.0, 4.0])
        );
        let mmap = Q8Matrix::read_zq8_mmap(&path).unwrap();
        assert!(matches!(mmap, QuantMatrix::Q8Mmap(_)));
        assert_eq!(
            mmap.matvec(&[1.0, 2.0, 3.0, 4.0]),
            q.matvec(&[1.0, 2.0, 3.0, 4.0])
        );
    }

    #[test]
    fn zq4_and_zq5_artifacts_round_trip() {
        let m = Matrix::from_row_major(
            3,
            5,
            vec![
                0.1, -0.2, 0.3, -0.4, 0.5, 1.0, -2.0, 3.0, -4.0, 5.0, 0.9, 0.8, 0.7, 0.6, -0.5,
            ],
        );
        let x = [0.7, -1.2, 0.3, 2.1, -0.8];
        let temp = TempDir::new().unwrap();

        let q4 = RowQ4Matrix::quantize(&m);
        let q4_path = temp.path().join("matrix.zq4");
        q4.write_zq4(&q4_path).unwrap();
        let q4_loaded = RowQ4Matrix::read_zq4(&q4_path).unwrap();
        assert_eq!(q4_loaded, q4);
        assert_eq!(q4_loaded.matvec(&x), q4.matvec(&x));

        let q5 = RowQ5Matrix::quantize(&m);
        let q5_path = temp.path().join("matrix.zq5");
        q5.write_zq5(&q5_path).unwrap();
        let q5_loaded = RowQ5Matrix::read_zq5(&q5_path).unwrap();
        assert_eq!(q5_loaded, q5);
        assert_eq!(q5_loaded.matvec(&x), q5.matvec(&x));
    }

    #[test]
    fn test_svd_q8_reconstruction_and_loading_works() -> Result<(), Box<dyn std::error::Error>> {
        use crate::weights::TensorReader;
        use safetensors::Dtype;
        use safetensors::tensor::{View, serialize_to_file};
        use std::borrow::Cow;

        struct TestTensor {
            shape: Vec<usize>,
            bytes: Vec<u8>,
            dtype: Dtype,
        }

        impl View for TestTensor {
            fn dtype(&self) -> Dtype {
                self.dtype
            }
            fn shape(&self) -> &[usize] {
                &self.shape
            }
            fn data(&self) -> Cow<'_, [u8]> {
                Cow::Borrowed(&self.bytes)
            }
            fn data_len(&self) -> usize {
                self.bytes.len()
            }
        }

        let temp = TempDir::new()?;

        // Let's create dummy SVD-Q8 tensors for "layer.0.q_proj.weight"
        // W = U * V^T * scale_u * scale_v
        // U_q: 4 x 2, V_q: 4 x 2
        let u_q_bytes = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let v_q_bytes = vec![9u8, 10, 11, 12, 13, 14, 15, 16];
        let scale_u_val = 0.5f32;
        let scale_v_val = 0.25f32;

        let scale_u_bytes = scale_u_val.to_le_bytes().to_vec();
        let scale_v_bytes = scale_v_val.to_le_bytes().to_vec();

        let tensors = vec![
            (
                "layer.0.q_proj.weight.U_q".to_string(),
                TestTensor {
                    shape: vec![4, 2],
                    bytes: u_q_bytes,
                    dtype: Dtype::I8,
                },
            ),
            (
                "layer.0.q_proj.weight.V_q".to_string(),
                TestTensor {
                    shape: vec![4, 2],
                    bytes: v_q_bytes,
                    dtype: Dtype::I8,
                },
            ),
            (
                "layer.0.q_proj.weight.scale_u".to_string(),
                TestTensor {
                    shape: vec![1],
                    bytes: scale_u_bytes,
                    dtype: Dtype::F32,
                },
            ),
            (
                "layer.0.q_proj.weight.scale_v".to_string(),
                TestTensor {
                    shape: vec![1],
                    bytes: scale_v_bytes,
                    dtype: Dtype::F32,
                },
            ),
        ];

        let model_path = temp.path().join("model.safetensors");
        serialize_to_file(tensors, &None, &model_path)?;

        // Now initialize TensorReader
        let mut reader = TensorReader::from_dir(temp.path())?;

        // 1. Verify read_f32 performs transparent reconstruction
        let (shape, data) = reader.read_f32("layer.0.q_proj.weight")?;
        assert_eq!(shape, vec![4, 4]);

        // Compute expected cell values
        // W_{i,j} = (U_{i,0}*V_{j,0} + U_{i,1}*V_{j,1}) * scale_u * scale_v
        // For i=0, j=0: (1*9 + 2*10) * 0.125 = 29 * 0.125 = 3.625
        assert_eq!(data[0], 3.625f32);
        // For i=1, j=2: (3*13 + 4*14) * 0.125 = (39 + 56) * 0.125 = 95 * 0.125 = 11.875
        assert_eq!(data[6], 11.875f32);

        // 2. Verify load_q8_matrix loads it as Q8Matrix::Svd and matvec/dequantize_row works
        let q8_matrix = crate::model::load_quant_matrix(
            &mut reader,
            "layer.0.q_proj.weight",
            4,
            4,
            "q_proj",
            crate::model::QuantMode::Q8,
        )?;

        assert!(matches!(q8_matrix, QuantMatrix::Q8Svd(_)));
        assert_eq!(q8_matrix.rows(), 4);
        assert_eq!(q8_matrix.cols(), 4);

        // Verify matvec
        let x = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
        let y = q8_matrix.matvec(&x);
        assert_eq!(y.len(), 4);

        // Let's check dequantize_row
        let row1 = q8_matrix.dequantize_row(1);
        assert_eq!(row1.len(), 4);
        assert_eq!(row1[2], 11.875f32);

        Ok(())
    }

    #[test]
    fn test_corrupted_magic_fails_gracefully() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("corrupted.zq8");
        fs::write(&path, b"NOT_MAGIC_BYTES_123").unwrap();

        let res_resident = RowQ8Matrix::read_zq8(&path);
        assert!(res_resident.is_err());
        let res_mmap = MmapQ8Matrix::read_zq8_mmap(&path);
        assert!(res_mmap.is_err());
    }

    #[test]
    fn test_truncated_zq8_fails_gracefully() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("truncated.zq8");
        let mut data = ZQ8_MAGIC.to_vec();
        data.extend_from_slice(&10u64.to_le_bytes());
        fs::write(&path, &data).unwrap();

        let res_resident = RowQ8Matrix::read_zq8(&path);
        assert!(res_resident.is_err());
        let res_mmap = MmapQ8Matrix::read_zq8_mmap(&path);
        assert!(res_mmap.is_err());
    }

    #[test]
    fn test_malformed_gguf_fails_gracefully() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("malformed.gguf");
        fs::write(&path, b"NOTGGUFMAGIC").unwrap();
        let reader = crate::gguf::GgufReader::open(&path);
        assert!(reader.is_err());

        let mut data = b"GGUF".to_vec();
        data.extend_from_slice(&99u32.to_le_bytes());
        fs::write(&path, &data).unwrap();
        let reader = crate::gguf::GgufReader::open(&path);
        assert!(reader.is_err());
    }
}

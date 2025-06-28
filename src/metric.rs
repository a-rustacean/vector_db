use core::{
    cmp::Ordering,
    f32,
    simd::{Simd, num::SimdFloat},
};

use crate::storage::{QuantVec, Quantization, RawVec};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum DistanceMetricKind {
    Cosine,
    Euclidean,
    Hamming,
    DotProduct,
}

pub struct DistanceMetric {
    kind: DistanceMetricKind,
    quantization: Quantization,
}

impl DistanceMetric {
    pub fn new(kind: DistanceMetricKind, quantization: Quantization) -> Self {
        Self { kind, quantization }
    }

    pub fn calculate(&self, a: &QuantVec, b: &QuantVec) -> f32 {
        use DistanceMetricKind::*;
        use Quantization::*;

        match (self.quantization, self.kind) {
            (SignedByte, Cosine) => {
                let dot_product = dot_product_i8(a.as_signed_byte(), b.as_signed_byte());
                cosine_similarity_from_dot_procut(dot_product, a.mag, b.mag)
            }
            (UnsignedByte, Cosine) => {
                let dot_product = dot_product_u8(a.as_unsigned_byte(), b.as_unsigned_byte());
                cosine_similarity_from_dot_procut(dot_product, a.mag, b.mag)
            }
            (FullPrecisionFP, Cosine) => {
                let dot_product =
                    dot_product_f32(a.as_full_precision_fp(), b.as_full_precision_fp());
                cosine_similarity_from_dot_procut(dot_product, a.mag, b.mag)
            }
            (SignedByte, DotProduct) => dot_product_i8(a.as_signed_byte(), b.as_signed_byte()),
            (UnsignedByte, DotProduct) => {
                dot_product_u8(a.as_unsigned_byte(), b.as_unsigned_byte())
            }
            (FullPrecisionFP, DotProduct) => {
                dot_product_f32(a.as_full_precision_fp(), b.as_full_precision_fp())
            }
            _ => todo!(),
        }
    }

    pub fn calculate_raw(&self, a: &RawVec, mag_a: f32, b: &RawVec, mag_b: f32) -> f32 {
        use DistanceMetricKind::*;
        match self.kind {
            Cosine => {
                let dot_product = dot_product_f32(&a.vec, &b.vec);
                cosine_similarity_from_dot_procut(dot_product, mag_a, mag_b)
            }
            DotProduct => dot_product_f32(&a.vec, &b.vec),
            _ => todo!(),
        }
    }

    pub fn cmp_score(&self, a: f32, b: f32) -> Ordering {
        use DistanceMetricKind::*;
        match self.kind {
            Cosine => a.total_cmp(&b),
            Euclidean => b.total_cmp(&a),
            Hamming => b.total_cmp(&a),
            DotProduct => a.total_cmp(&b),
        }
    }

    pub fn max_value(&self) -> f32 {
        use DistanceMetricKind::*;
        match self.kind {
            Cosine => 2.0,
            Euclidean => 0.0,
            Hamming => 0.0,
            DotProduct => f32::INFINITY,
        }
    }
}

const LANES: usize = 16;

pub(crate) fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let len = a.len();
    let mut sum = Simd::<f32, LANES>::splat(0.0);
    let mut i = 0;
    while i + LANES <= len {
        let a_chunk = Simd::from_slice(&a[i..]);
        let b_chunk = Simd::from_slice(&b[i..]);
        sum += a_chunk * b_chunk;
        i += LANES;
    }
    let mut total = sum.reduce_sum();
    for j in i..len {
        total += a[j] * b[j];
    }
    total
}

pub fn dot_product_u8(a: &[u8], b: &[u8]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut sum: u32 = 0;
    for i in 0..a.len() {
        sum += a[i] as u32 * b[i] as u32;
    }
    sum as f32 / (65025.0)
}

pub fn dot_product_i8(a: &[i8], b: &[i8]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut sum: i32 = 0;
    for i in 0..a.len() {
        sum += a[i] as i32 * b[i] as i32;
    }
    sum as f32 / (16384.0)
}

pub fn cosine_similarity_from_dot_procut(dot_product: f32, mag_a: f32, mag_b: f32) -> f32 {
    let denominator = mag_a * mag_b;

    if denominator == 0.0 {
        0.0
    } else {
        dot_product / denominator
    }
}

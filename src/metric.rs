use core::{cmp::Ordering, f32};

use crate::storage::{QuantVec, Quantization};

#[derive(Debug, Clone, Copy)]
pub enum DistanceMetricKind {
    Cosine,
    Euclidean,
    Hamming,
    DotProduct,
}

#[allow(unused)]
pub struct DistanceMetric {
    kind: DistanceMetricKind,
    quantization: Quantization,
    len: u16,
}

impl DistanceMetric {
    pub fn new(kind: DistanceMetricKind, quantization: Quantization, len: u16) -> Self {
        Self {
            kind,
            quantization,
            len,
        }
    }

    pub fn calculate(&self, _a: &QuantVec, _b: &QuantVec) -> f32 {
        todo!()
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
            Cosine => 1.0,
            Euclidean => 0.0,
            Hamming => 0.0,
            DotProduct => f32::INFINITY,
        }
    }
}

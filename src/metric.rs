use crate::storage::{QuantVec, Quantization};

pub trait MetricResult: Ord + Clone + Copy {
    const MIN: Self;
    const MAX: Self;
}

pub trait DistanceMetric<const DIMS: u16, Q: Quantization<DIMS>>
where
    [(); DIMS as usize]: Sized,
{
    type Result: MetricResult;

    fn calculate(a: &QuantVec<DIMS, Q>, b: &QuantVec<DIMS, Q>) -> Self::Result;
}

macro_rules! define_distance_metrics {
    ($($name:ident -> $result:ident($ord:ident, $min:expr => $max:expr) { $($quantization:ident($a: ident, $b: ident) { $body: expr })+ })+) => {
        $(define_distance_metrics!(@define_metric $name -> $result($ord, $min => $max) { $($quantization($a, $b) { $body })+ });)+
    };
    (@define_metric $name:ident -> $result:ident($ord:ident, $min:expr => $max:expr) { $($quantization:ident($a: ident, $b: ident) { $body: expr })+ }) => {
        define_distance_metrics!(@define_structs $name -> $result($min => $max));
        define_distance_metrics!(@impl_ord $result($ord));
        define_distance_metrics!(@trait_impls $name -> $result { $($quantization($a, $b) { $body })+ });
    };
    (@impl_ord $result:ident(asc)) => {
        impl Ord for $result {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.0.total_cmp(&other.0)
            }
        }
    };
    (@impl_ord $result:ident(dsc)) => {
        impl Ord for $result {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                other.0.total_cmp(&self.0)
            }
        }
    };
    (@define_structs $name:ident -> $result:ident($min:expr => $max:expr)) => {
        pub struct $name;

        #[derive(Debug, Clone, Copy)]
        pub struct $result(f32);

        impl PartialOrd for $result {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Eq for $result {}

        impl PartialEq for $result {
            fn eq(&self, other: &Self) -> bool {
                self.0.total_cmp(&other.0) == core::cmp::Ordering::Equal
            }
        }

        impl MetricResult for $result {
            const MIN: Self = Self($min);
            const MAX: Self = Self($max);
        }
    };
    (@trait_impls $name:ident -> $result:ident { $($quantization:ident($a: ident, $b: ident) { $body: expr })+ }) => {
        $(define_distance_metrics!(@trait_impl $name<$quantization>($a, $b) -> $result { $body });)+
    };
    (@trait_impl $name:ident<$quantization:ident>($a: ident, $b: ident) -> $result:ident { $body: expr }) => {
        impl<const DIMS: u16> DistanceMetric<DIMS, crate::storage::$quantization> for $name
        where
            [(); DIMS as usize]: Sized,
        {
            type Result = $result;

            fn calculate($a: &QuantVec<DIMS, crate::storage::$quantization>, $b: &QuantVec<DIMS, crate::storage::$quantization>) -> Self::Result {
                $body
            }
        }
    };
}

define_distance_metrics! {
    Cosine -> CosineSimilarity(asc, -1.0 => 1.0) {
        SignedByte(a, b) {
            todo!()
        }
        HalfPrecisionFP(a, b) {
            todo!()
        }
        FullPrecisionFP(a, b) {
            todo!()
        }
    }
    Euclidean -> EuclideanDistance(dsc, f32::INFINITY => 0.0) {
        SignedByte(a, b) {
            todo!()
        }
        HalfPrecisionFP(a, b) {
            todo!()
        }
        FullPrecisionFP(a, b) {
            todo!()
        }
    }
    Hamming -> HammingDistance(dsc, f32::MAX => 0.0) {
        SignedByte(a, b) {
            todo!()
        }
        HalfPrecisionFP(a, b) {
            todo!()
        }
        FullPrecisionFP(a, b) {
            todo!()
        }
    }
    DotProduct -> DotProductSimilarity(asc, f32::NEG_INFINITY => f32::INFINITY) {
        SignedByte(a, b) {
            todo!()
        }
        HalfPrecisionFP(a, b) {
            todo!()
        }
        FullPrecisionFP(a, b) {
            todo!()
        }
    }
}

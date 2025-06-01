use alloc::boxed::Box;

pub struct RawVec<const DIMS: u16>
where
    [(); DIMS as usize]: Sized,
{
    pub vec: [f32; DIMS as usize],
}

pub struct QuantVec<const DIMS: u16, Q>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
{
    mag: f32,
    vec: Q::QuantVec,
}

pub trait Quantization<const DIMS: u16>
where
    [(); DIMS as usize]: Sized,
{
    type QuantVec;

    fn quantize(vec: RawVec<DIMS>) -> Self::QuantVec;
}

impl<const DIMS: u16, Q> From<RawVec<DIMS>> for QuantVec<DIMS, Q>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
{
    fn from(vec: RawVec<DIMS>) -> Self {
        let mag = vec.mag();
        let vec = Q::quantize(vec);
        Self { mag, vec }
    }
}

impl<const DIM: u16> RawVec<DIM>
where
    [(); DIM as usize]: Sized,
{
    pub fn mag(&self) -> f32 {
        self.vec.iter().map(|dim| dim * dim).sum::<f32>().sqrt()
    }
}

impl<const DIM: u16> From<[f32; DIM as usize]> for RawVec<DIM>
where
    [(); DIM as usize]: Sized,
{
    fn from(vec: [f32; DIM as usize]) -> Self {
        Self { vec }
    }
}

macro_rules! define_quantizations {
    ($($name:ident($vec:ident) -> [$result:ty] { $body:expr })+) => {
        $(
            pub struct $name;

            impl<const DIMS: u16> Quantization<DIMS> for $name
            where
                [(); DIMS as usize]: Sized,
            {
                type QuantVec = [$result; DIMS as usize];

                fn quantize($vec: RawVec<DIMS>) -> Self::QuantVec {
                    $body
                }
            }
        )+
    };
}

define_quantizations! {
    SignedByte(vec) -> [i8] {
        vec.vec.map(|dim| (dim * 128.0).clamp(-128.0, 127.0) as i8)
    }
    HalfPrecisionFP(vec) -> [f16] {
        vec.vec.map(|dim| dim as f16)
    }
    FullPrecisionFP(vec) -> [f32] {
        vec.vec
    }
}

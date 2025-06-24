use core::ptr::{self, Pointee};

use crate::arena::{DynAlloc, DynInit};

#[derive(Debug, Clone, Copy)]
pub enum Quantization {
    SignedByte,
    UnsignedByte,
    HalfPrecisionFP,
    FullPrecisionFP,
}

impl Quantization {
    #[inline]
    fn size(&self) -> usize {
        match self {
            Self::SignedByte | Self::UnsignedByte => 1,
            Self::HalfPrecisionFP => 2,
            Self::FullPrecisionFP => 4,
        }
    }
}

#[repr(C, align(4))]
pub struct QuantVec {
    mag: f32,
    vec: [u8],
}

#[repr(C, align(4))]
pub struct RawVec {
    vec: [f32],
}

impl DynAlloc for QuantVec {
    type Metadata = (Quantization, u16);

    const ALIGN: usize = 4;

    #[inline]
    fn size((quantization, len): Self::Metadata) -> usize {
        let multiplier = quantization.size();
        4 + len as usize * multiplier
    }

    #[inline]
    fn ptr_metadata((quantization, len): Self::Metadata) -> <Self as Pointee>::Metadata {
        let multiplier = quantization.size();
        len as usize * multiplier
    }
}

impl DynAlloc for RawVec {
    type Metadata = u16;

    const ALIGN: usize = 4;

    #[inline]
    fn size(len: Self::Metadata) -> usize {
        4 * len as usize
    }

    #[inline]
    fn ptr_metadata(len: Self::Metadata) -> <Self as Pointee>::Metadata {
        len as usize
    }
}

impl DynInit for QuantVec {
    type Args = *const f32;

    unsafe fn new_at(ptr: *mut u8, (quantization, len): Self::Metadata, raw_vec_ptr: Self::Args) {
        let raw_vec_ref: &[f32] = &*ptr::from_raw_parts(raw_vec_ptr, len as usize);
        let mag = raw_vec_ref.iter().map(|dim| dim * dim).sum::<f32>().sqrt();
        (ptr as *mut f32).write(mag);

        let vec_ptr = ptr.add(4);

        match quantization {
            Quantization::SignedByte => {
                let vec_ptr = vec_ptr as *mut i8;
                for (i, dim) in raw_vec_ref.iter().enumerate() {
                    vec_ptr
                        .add(i)
                        .write((dim * 127.0).clamp(-128.0, 127.0) as i8);
                }
            }
            Quantization::UnsignedByte => {
                for (i, dim) in raw_vec_ref.iter().enumerate() {
                    vec_ptr.add(i).write((dim * 255.0).clamp(0.0, 255.0) as u8);
                }
            }
            Quantization::HalfPrecisionFP => {
                let vec_ptr = vec_ptr as *mut f16;
                for (i, dim) in raw_vec_ref.iter().enumerate() {
                    vec_ptr.add(i).write(*dim as f16);
                }
            }
            Quantization::FullPrecisionFP => {
                let vec_ptr = vec_ptr as *mut f32;
                ptr::copy_nonoverlapping(raw_vec_ptr, vec_ptr, len as usize);
            }
        }
    }
}

impl DynInit for RawVec {
    type Args = *const f32;

    unsafe fn new_at(ptr: *mut u8, metadata: Self::Metadata, args: Self::Args) {
        ptr::copy_nonoverlapping(args, ptr as *mut f32, metadata as usize);
    }
}

impl QuantVec {
    pub fn as_signed_byte(&self) -> &[i8] {
        unsafe { &*(&self.vec as *const [u8] as *const [i8]) }
    }

    pub fn as_unsigned_byte(&self) -> &[u8] {
        &self.vec
    }

    pub fn as_half_precision_fp(&self) -> &[f16] {
        unsafe { &*ptr::from_raw_parts(&self.vec as *const [u8] as *const f16, self.vec.len() / 2) }
    }

    pub fn as_full_precision_fp(&self) -> &[f32] {
        unsafe { &*ptr::from_raw_parts(&self.vec as *const [u8] as *const f32, self.vec.len() / 4) }
    }
}

pub const fn next_pow2_u16(mut x: u16) -> usize {
    if x == 0 {
        return 1;
    }

    x -= 1;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    (x + 1) as usize
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FixedSet<const M: u16>
where
    [(); next_pow2_u16(M)]: Sized,
{
    buckets: [u64; next_pow2_u16(M)],
}

impl<const M: u16> FixedSet<M>
where
    [(); next_pow2_u16(M)]: Sized,
{
    const MASK: u32 = next_pow2_u16(M) as u32 - 1;

    #[inline]
    pub fn new() -> Self {
        Self { buckets: [0; _] }
    }

    #[inline]
    pub fn insert(&mut self, value: u32) {
        let bucket = (value >> 6) & Self::MASK;
        let bit_pos = value & 0x3f;
        self.buckets[bucket as usize] |= 1u64 << bit_pos;
    }

    #[inline]
    pub fn is_member(&self, value: u32) -> bool {
        let bucket = (value >> 6) & Self::MASK;
        let bit_pos = value & 0x3f;
        (self.buckets[bucket as usize] & (1u64 << bit_pos)) != 0
    }
}

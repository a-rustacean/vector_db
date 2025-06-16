#![no_std]
// silent unstable warnings
#![allow(incomplete_features)]
// unstable features
#![feature(generic_const_exprs)]
#![feature(generic_arg_infer)]
#![feature(f16)]
#![feature(allocator_api)]
#![feature(duration_constants)]

use crate::fixedset::next_pow2_u16;

extern crate alloc;

pub mod arena;
pub mod fixedset;
pub mod graph;
pub mod metric;
pub mod node;
pub mod storage;

pub type MConstraints<const M: u16> = (
    [(); M as usize],
    [(); next_pow2_u16(M)],
    [(); (M * 2) as usize],
);

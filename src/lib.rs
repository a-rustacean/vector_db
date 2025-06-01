#![no_std]
// silent unstable warnings
#![allow(incomplete_features)]
// unstable features
#![feature(generic_const_exprs)]
#![feature(f16)]
#![feature(allocator_api)]

extern crate alloc;

pub mod arena;
pub mod graph;
pub mod metric;
pub mod node;
pub mod storage;
pub mod types;

#![no_std]
#![feature(ptr_metadata, f16, new_zeroed_alloc)]

extern crate alloc;

mod arena;
mod fixedset;
mod graph;
mod handle;
mod mem_project;
mod metric;
mod node;
mod random;
mod rwlock;
mod storage;
mod util;

pub use graph::{Graph, InternalSearchResult};
pub use mem_project::mem_project;
pub use metric::DistanceMetricKind;
pub use storage::Quantization;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u32);

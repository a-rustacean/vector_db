#![allow(unsafe_op_in_unsafe_fn)]
#![feature(ptr_metadata, f16, new_zeroed_alloc)]

extern crate alloc;

mod arena;
mod fixedset;
mod graph;
mod metric;
mod node;
mod rwlock;
mod storage;
mod util;

pub use arena::Handle;
pub use graph::{Graph, SearchResult};
pub use metric::DistanceMetricKind;
pub use storage::Quantization;

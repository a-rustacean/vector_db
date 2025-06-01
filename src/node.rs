use alloc::boxed::Box;
use parking_lot::RwLock;

use crate::{
    arena::Handle,
    metric::{DistanceMetric, MetricResult},
    storage::{QuantVec, Quantization},
    types::{HNSWLevel, NeighborIndex},
};

pub struct Node<const M: u16, const DIMS: u16, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    hnsw_level: HNSWLevel,
    vec: VecHandle<DIMS, Q>,
    neighbors: RwLock<Neighbors<M, DIMS, Q, D>>,
    child: NodeHandle<M, DIMS, Q, D>,
}

impl<const M: u16, const DIMS: u16, Q, D> Node<M, DIMS, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub fn new(
        hnsw_level: HNSWLevel,
        vec: VecHandle<DIMS, Q>,
        child: NodeHandle<M, DIMS, Q, D>,
    ) -> Self {
        Self {
            hnsw_level,
            vec,
            neighbors: RwLock::new(Neighbors::default()),
            child,
        }
    }
}

pub type NodeHandle<const M: u16, const DIMS: u16, Q, D> = Handle<Node<M, DIMS, Q, D>>;
pub type VecHandle<const DIMS: u16, Q> = Handle<QuantVec<DIMS, Q>>;

pub struct Neighbors<const M: u16, const DIMS: u16, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    lowest_index: NeighborIndex,
    lowest_score: D::Result,
    neighbors: [Option<Box<Neighbor<M, DIMS, Q, D>>>; M as usize],
}

impl<const M: u16, const DIMS: u16, Q, D> Default for Neighbors<M, DIMS, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn default() -> Self {
        Self {
            lowest_index: NeighborIndex::from(0),
            lowest_score: D::Result::MIN,
            neighbors: core::array::from_fn(|_| None),
        }
    }
}

pub struct Neighbor<const M: u16, const DIMS: u16, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub node: NodeHandle<M, DIMS, Q, D>,
    pub score: D::Result,
}

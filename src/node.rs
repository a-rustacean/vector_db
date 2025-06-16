use core::{array, mem, time::Duration};

use arrayvec::ArrayVec;
use parking_lot::RwLock;

use crate::{
    MConstraints,
    arena::Handle,
    graph::{Graph, SearchResult},
    metric::{DistanceMetric, MetricResult},
    storage::{QuantVec, Quantization},
    types::{HNSWLevel, NeighborIndex},
};

pub struct Node<const M: u16, const DIMS: u16, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub(crate) hnsw_level: HNSWLevel,
    pub(crate) vec: VecHandle<DIMS, Q>,
    pub(crate) neighbors: RwLock<Neighbors<M, DIMS, Q, D>>,
    pub(crate) child: NodeHandle<M, DIMS, Q, D>,
}

pub struct Node0<const M0: u16, const DIMS: u16, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub(crate) hnsw_level: HNSWLevel,
    pub(crate) vec: VecHandle<DIMS, Q>,
    pub(crate) neighbors: RwLock<Neighbors0<M0, DIMS, Q, D>>,
}

impl<const M: u16, const DIMS: u16, Q, D> Node<M, DIMS, Q, D>
where
    MConstraints<M>: Sized,
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

impl<const M0: u16, const DIMS: u16, Q, D> Node0<M0, DIMS, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub fn new(hnsw_level: HNSWLevel, vec: VecHandle<DIMS, Q>) -> Self {
        Self {
            hnsw_level,
            vec,
            neighbors: RwLock::new(Neighbors0::default()),
        }
    }
}

pub type NodeHandle<const M: u16, const DIMS: u16, Q, D> = Handle<Node<M, DIMS, Q, D>>;
pub type Node0Handle<const M0: u16, const DIMS: u16, Q, D> = Handle<Node0<M0, DIMS, Q, D>>;
pub type VecHandle<const DIMS: u16, Q> = Handle<QuantVec<DIMS, Q>>;

pub struct Neighbors<const M: u16, const DIMS: u16, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub(crate) lowest_index: NeighborIndex,
    pub(crate) lowest_score: D::Result,
    pub(crate) neighbors: [Option<Neighbor<M, DIMS, Q, D>>; M as usize],
}

pub struct Neighbors0<const M0: u16, const DIMS: u16, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub(crate) lowest_index: NeighborIndex,
    pub(crate) lowest_score: D::Result,
    pub(crate) neighbors: [Option<Neighbor0<M0, DIMS, Q, D>>; M0 as usize],
}

impl<const M: u16, const DIMS: u16, Q, D> Default for Neighbors<M, DIMS, Q, D>
where
    MConstraints<M>: Sized,
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

impl<const M: u16, const DIMS: u16, Q, D> Neighbors<M, DIMS, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub fn insert_neighbor<const M0: u16, const LEVELS: u8>(
        &mut self,
        node_handle: NodeHandle<M, DIMS, Q, D>,
        neighbor: Neighbor<M, DIMS, Q, D>,
        graph: &Graph<M, M0, DIMS, LEVELS, Q, D>,
    ) -> bool
    where
        MConstraints<M0>: Sized,
    {
        let neighbor_slot = &mut self.neighbors[*self.lowest_index as usize];

        if let Some(existing_neighbor) = neighbor_slot {
            if neighbor.score > existing_neighbor.score {
                let existing_neighbor = mem::replace(existing_neighbor, neighbor);
                let existing_neighbor_node = &graph.nodes_arena[existing_neighbor.node];
                if let Some(mut neighbors) = existing_neighbor_node
                    .neighbors
                    .try_write_for(Duration::SECOND)
                {
                    neighbors.remove_neighbor(node_handle);
                }
                self.recompute_lowest_index();
                true
            } else {
                false
            }
        } else {
            *neighbor_slot = Some(neighbor);
            self.recompute_lowest_index();
            true
        }
    }

    fn remove_neighbor(&mut self, neighbor_node: NodeHandle<M, DIMS, Q, D>) {
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;
        let mut neighbor_index = None;

        for i in (0..M).rev() {
            let Some(neighbor) = &self.neighbors[i as usize] else {
                lowest_index = i;
                lowest_score = D::Result::MIN;
                break;
            };

            if neighbor.node == neighbor_node {
                neighbor_index = Some(i as usize);
            }

            if neighbor.score < lowest_score {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        if let Some(neighbor_index) = neighbor_index {
            self.neighbors[neighbor_index] = None;

            self.lowest_index = NeighborIndex::from(lowest_index);
            self.lowest_score = lowest_score;
        }
    }

    fn recompute_lowest_index(&mut self) {
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;

        for i in (0..M).rev() {
            let Some(neighbor) = &self.neighbors[i as usize] else {
                lowest_index = i;
                lowest_score = D::Result::MIN;
                break;
            };

            if neighbor.score < lowest_score {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        self.lowest_index = NeighborIndex::from(lowest_index);
        self.lowest_score = lowest_score;
    }
}

impl<const M0: u16, const DIMS: u16, Q, D> Default for Neighbors0<M0, DIMS, Q, D>
where
    MConstraints<M0>: Sized,
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

impl<const M0: u16, const DIMS: u16, Q, D> Neighbors0<M0, DIMS, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub fn insert_neighbor<const M: u16, const LEVELS: u8>(
        &mut self,
        node_handle: Node0Handle<M0, DIMS, Q, D>,
        neighbor: Neighbor0<M0, DIMS, Q, D>,
        graph: &Graph<M, M0, DIMS, LEVELS, Q, D>,
    ) -> bool
    where
        MConstraints<M>: Sized,
    {
        let neighbor_slot = &mut self.neighbors[*self.lowest_index as usize];

        if let Some(existing_neighbor) = neighbor_slot {
            if neighbor.score > existing_neighbor.score {
                let existing_neighbor = mem::replace(existing_neighbor, neighbor);
                let existing_neighbor_node = &graph.nodes0_arena[existing_neighbor.node];
                if let Some(mut neighbors) = existing_neighbor_node
                    .neighbors
                    .try_write_for(Duration::SECOND)
                {
                    neighbors.remove_neighbor(node_handle);
                }
                self.recompute_lowest_index();
                true
            } else {
                false
            }
        } else {
            *neighbor_slot = Some(neighbor);
            self.recompute_lowest_index();
            true
        }
    }

    fn remove_neighbor(&mut self, neighbor_node: Node0Handle<M0, DIMS, Q, D>) {
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;
        let mut neighbor_index = None;

        for i in (0..M0).rev() {
            let Some(neighbor) = &self.neighbors[i as usize] else {
                lowest_index = i;
                lowest_score = D::Result::MIN;
                break;
            };

            if neighbor.node == neighbor_node {
                neighbor_index = Some(i as usize);
            }

            if neighbor.score < lowest_score {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        if let Some(neighbor_index) = neighbor_index {
            self.neighbors[neighbor_index] = None;

            self.lowest_index = NeighborIndex::from(lowest_index);
            self.lowest_score = lowest_score;
        }
    }

    fn recompute_lowest_index(&mut self) {
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;

        for i in (0..M0).rev() {
            let Some(neighbor) = &self.neighbors[i as usize] else {
                lowest_index = i;
                lowest_score = D::Result::MIN;
                break;
            };

            if neighbor.score < lowest_score {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        self.lowest_index = NeighborIndex::from(lowest_index);
        self.lowest_score = lowest_score;
    }
}

#[derive(Clone, Copy)]
pub struct Neighbor<const M: u16, const DIMS: u16, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub node: NodeHandle<M, DIMS, Q, D>,
    pub score: D::Result,
}

pub struct Neighbor0<const M0: u16, const DIMS: u16, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub node: Node0Handle<M0, DIMS, Q, D>,
    pub score: D::Result,
}

impl<const M: u16, const DIMS: u16, Q, D> From<SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>>
    for Neighbor<M, DIMS, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn from(result: SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>) -> Self {
        Self {
            node: result.node,
            score: result.score,
        }
    }
}

impl<const M: u16, const DIMS: u16, Q, D>
    From<ArrayVec<SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>, { M as usize }>>
    for Neighbors<M, DIMS, Q, D>
where
    MConstraints<M>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn from(
        results: ArrayVec<SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>, { M as usize }>,
    ) -> Self {
        let mut iter = results.into_iter();
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;

        let neighbors = array::from_fn(|i| {
            let neighbor = iter.next();

            if let Some(neighbor) = neighbor {
                if neighbor.score < lowest_score {
                    lowest_score = neighbor.score;
                    lowest_index = i as u16;
                }

                Some(neighbor.into())
            } else {
                if lowest_score > D::Result::MIN {
                    lowest_score = D::Result::MIN;
                    lowest_index = i as u16;
                }

                None
            }
        });

        Self {
            lowest_index: NeighborIndex::from(lowest_index),
            lowest_score,
            neighbors,
        }
    }
}

impl<const M0: u16, const DIMS: u16, Q, D> From<SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>>
    for Neighbor0<M0, DIMS, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn from(result: SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>) -> Self {
        Self {
            node: result.node,
            score: result.score,
        }
    }
}

impl<const M0: u16, const DIMS: u16, Q, D>
    From<ArrayVec<SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>, { M0 as usize }>>
    for Neighbors0<M0, DIMS, Q, D>
where
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn from(
        results: ArrayVec<SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>, { M0 as usize }>,
    ) -> Self {
        let mut iter = results.into_iter();
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;

        let neighbors = array::from_fn(|i| {
            let neighbor = iter.next();

            if let Some(neighbor) = neighbor {
                if neighbor.score < lowest_score {
                    lowest_score = neighbor.score;
                    lowest_index = i as u16;
                }

                Some(neighbor.into())
            } else {
                if lowest_score > D::Result::MIN {
                    lowest_score = D::Result::MIN;
                    lowest_index = i as u16;
                }

                None
            }
        });

        Self {
            lowest_index: NeighborIndex::from(lowest_index),
            lowest_score,
            neighbors,
        }
    }
}

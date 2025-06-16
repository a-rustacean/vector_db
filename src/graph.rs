use alloc::{collections::binary_heap::BinaryHeap, vec::Vec};
use arrayvec::ArrayVec;
use parking_lot::RwLock;

use crate::{
    MConstraints,
    arena::{Arena, Handle},
    fixedset::FixedSet,
    metric::{DistanceMetric, MetricResult},
    node::{
        Neighbor, Neighbor0, Neighbors, Neighbors0, Node, Node0, Node0Handle, NodeHandle, VecHandle,
    },
    storage::{QuantVec, Quantization, RawVec},
    types::{HNSWLevel, NeighborIndex},
};

pub struct Graph<const M: u16, const M0: u16, const DIMS: u16, const LEVELS: u8, Q, D>
where
    MConstraints<M>: Sized,
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub(crate) nodes_arena: Arena<Node<M, DIMS, Q, D>>,
    pub(crate) nodes0_arena: Arena<Node0<M0, DIMS, Q, D>>,
    vec_arena: Arena<QuantVec<DIMS, Q>>,
    top_level_root_node: NodeHandle<M, DIMS, Q, D>,
}

pub struct SearchResult<const DIMS: u16, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub node: Handle<T>,
    pub score: D::Result,
}

impl<const DIMS: u16, Q, D, T> Clone for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn clone(&self) -> Self {
        Self {
            node: self.node,
            score: self.score,
        }
    }
}

impl<const DIMS: u16, Q, D, T> Copy for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
}

impl<const DIMS: u16, Q, D, T> Ord for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl<const DIMS: u16, Q, D, T> PartialOrd for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const DIMS: u16, Q, D, T> PartialEq for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl<const DIMS: u16, Q, D, T> Eq for SearchResult<DIMS, Q, D, T>
where
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
}

impl<const M: u16, const M0: u16, const DIMS: u16, const LEVELS: u8, Q, D> Default
    for Graph<M, M0, DIMS, LEVELS, Q, D>
where
    MConstraints<M>: Sized,
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn default() -> Self {
        let nodes_arena = Arena::new();
        let nodes0_arena = Arena::new();
        let vec_arena = Arena::new();
        let root_vec_raw = RawVec::from([0.0; DIMS as usize]);
        let root_vec_quant = QuantVec::from(root_vec_raw);
        let vec_handle = vec_arena.alloc(root_vec_quant);
        let node0 = Node0::new(HNSWLevel::from(0), vec_handle);
        let node0_handle = nodes0_arena.alloc(node0);
        let mut prev_node = node0_handle.cast();
        for level in 1..LEVELS {
            let level = HNSWLevel::from(level);
            let node = Node::new(level, vec_handle, prev_node);
            let node_handle = nodes_arena.alloc(node);
            prev_node = node_handle;
        }
        Self {
            nodes_arena,
            nodes0_arena,
            vec_arena,
            top_level_root_node: prev_node,
        }
    }
}

impl<const M: u16, const M0: u16, const DIMS: u16, const LEVELS: u8, Q, D>
    Graph<M, M0, DIMS, LEVELS, Q, D>
where
    MConstraints<M>: Sized,
    MConstraints<M0>: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    pub fn index(&self, vec: RawVec<DIMS>, ef: u16) -> Handle<QuantVec<DIMS, Q>> {
        let vec = QuantVec::from(vec);
        let vec_handle = self.vec_arena.alloc(vec);
        let vec = &self.vec_arena[vec_handle];
        let max_level = LEVELS;

        self.index_level(
            vec_handle,
            vec,
            self.top_level_root_node,
            LEVELS - 1,
            max_level,
            ef,
        );
        vec_handle
    }

    fn index_level(
        &self,
        vec_handle: VecHandle<DIMS, Q>,
        vec: &QuantVec<DIMS, Q>,
        entry_node: NodeHandle<M, DIMS, Q, D>,
        current_level: u8,
        max_level: u8,
        ef: u16,
    ) -> NodeHandle<M, DIMS, Q, D> {
        if current_level > max_level {
            let results = self.search_level::<1>(entry_node, vec, ef);
            let child = self.nodes_arena[results[0].node].child;

            self.index_level(vec_handle, vec, child, current_level - 1, max_level, ef);
            NodeHandle::invalid()
        } else if current_level == 0 {
            self.index_level0(vec_handle, vec, entry_node.cast(), ef)
                .cast()
        } else {
            let results = self.search_level::<{ M * 2 }>(entry_node, vec, ef);
            let child = self.nodes_arena[results[0].node].child;

            let child = self.index_level(vec_handle, vec, child, current_level - 1, max_level, ef);

            self.create_node(vec_handle, results, child)
        }
    }

    fn index_level0(
        &self,
        vec_handle: VecHandle<DIMS, Q>,
        vec: &QuantVec<DIMS, Q>,
        entry_node: Node0Handle<M0, DIMS, Q, D>,
        ef: u16,
    ) -> Node0Handle<M0, DIMS, Q, D> {
        let results = self.search_level0::<{ M0 * 2 }>(entry_node, vec, ef);
        self.create_node0(vec_handle, results)
    }

    fn create_node(
        &self,
        vec: VecHandle<DIMS, Q>,
        results: ArrayVec<SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>, { (M * 2) as usize }>,
        child: NodeHandle<M, DIMS, Q, D>,
    ) -> NodeHandle<M, DIMS, Q, D> {
        let neighbors = Neighbors::default();
        let node = Node {
            hnsw_level: HNSWLevel::from(0),
            vec,
            neighbors: RwLock::new(neighbors),
            child,
        };
        let node_handle = self.nodes_arena.alloc(node);
        let node = &self.nodes_arena[node_handle];
        let mut neighbors_guard = node.neighbors.write();
        let mut neighbors_connected = 0;
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;
        for neighbor in results {
            let neighbor_node = &self.nodes_arena[neighbor.node];
            let node_inserted = neighbor_node.neighbors.write().insert_neighbor(
                neighbor.node,
                Neighbor {
                    node: node_handle,
                    score: neighbor.score,
                },
                self,
            );
            if node_inserted {
                neighbors_guard.neighbors[neighbors_connected as usize] = Some(neighbor.into());

                if neighbor.score < lowest_score {
                    lowest_score = neighbor.score;
                    lowest_index = neighbors_connected;
                }

                neighbors_connected += 1;
                if neighbors_connected >= M0 {
                    break;
                }
            }
        }
        neighbors_guard.lowest_score = lowest_score;
        neighbors_guard.lowest_index = NeighborIndex::from(lowest_index);
        node_handle
    }

    fn create_node0(
        &self,
        vec: VecHandle<DIMS, Q>,
        results: ArrayVec<SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>, { (M0 * 2) as usize }>,
    ) -> Node0Handle<M0, DIMS, Q, D> {
        let neighbors = Neighbors0::default();
        let node = Node0 {
            hnsw_level: HNSWLevel::from(0),
            vec,
            neighbors: RwLock::new(neighbors),
        };
        let node_handle = self.nodes0_arena.alloc(node);
        let node = &self.nodes0_arena[node_handle];
        let mut neighbors_guard = node.neighbors.write();
        let mut neighbors_connected = 0;
        let mut lowest_index = 0;
        let mut lowest_score = D::Result::MAX;
        for neighbor in results {
            let neighbor_node = &self.nodes0_arena[neighbor.node];
            let node_inserted = neighbor_node.neighbors.write().insert_neighbor(
                neighbor.node,
                Neighbor0 {
                    node: node_handle,
                    score: neighbor.score,
                },
                self,
            );
            if node_inserted {
                neighbors_guard.neighbors[neighbors_connected as usize] = Some(neighbor.into());

                if neighbor.score < lowest_score {
                    lowest_score = neighbor.score;
                    lowest_index = neighbors_connected;
                }

                neighbors_connected += 1;
                if neighbors_connected >= M0 {
                    break;
                }
            }
        }
        neighbors_guard.lowest_score = lowest_score;
        neighbors_guard.lowest_index = NeighborIndex::from(lowest_index);
        node_handle
    }

    pub fn search<const TOP_K: u16>(
        &self,
        query: RawVec<DIMS>,
        ef: u16,
    ) -> ArrayVec<SearchResult<DIMS, Q, D, QuantVec<DIMS, Q>>, { TOP_K as usize }> {
        let query = QuantVec::from(query);
        let mut entry_node = self.top_level_root_node;

        // range = (LEVELS, 1]
        for _ in 1..LEVELS {
            let results = self.search_level::<1>(entry_node, &query, ef);
            let result = if results.is_empty() {
                results[0].node.cast()
            } else {
                self.nodes_arena[entry_node].child
            };
            entry_node = result;
        }

        let entry_node = entry_node.cast();

        let results = self.search_level0::<TOP_K>(entry_node, &query, ef);

        ArrayVec::from_iter(results.iter().map(|result| SearchResult {
            node: self.nodes0_arena[result.node].vec,
            score: result.score,
        }))
    }

    fn search_level<const TOP_K: u16>(
        &self,
        entry_node: NodeHandle<M, DIMS, Q, D>,
        query: &QuantVec<DIMS, Q>,
        ef: u16,
    ) -> ArrayVec<SearchResult<DIMS, Q, D, Node<M, DIMS, Q, D>>, { TOP_K as usize }> {
        let mut candidate_queue = BinaryHeap::new();
        let mut results = Vec::new();
        let mut set = FixedSet::<M>::new();

        let node = &self.nodes_arena[entry_node];
        let vec = &self.vec_arena[node.vec];

        let score = D::calculate(query, vec);

        set.insert(*entry_node);
        candidate_queue.push(SearchResult {
            node: entry_node,
            score,
        });

        let mut nodes_visited = 0;

        while let Some(entry) = candidate_queue.pop() {
            if nodes_visited >= ef {
                break;
            }
            nodes_visited += 1;

            results.push(entry);

            let node = &self.nodes_arena[entry_node];

            for neighbor in &node.neighbors.read().neighbors {
                let Some(neighbor) = neighbor else {
                    continue;
                };

                if set.is_member(*neighbor.node) {
                    let neighbor_node = &self.nodes_arena[neighbor.node];
                    let neighbor_vec = &self.vec_arena[neighbor_node.vec];
                    let score = D::calculate(query, neighbor_vec);

                    set.insert(*neighbor.node);
                    candidate_queue.push(SearchResult {
                        node: neighbor.node,
                        score,
                    });
                }
            }
        }

        if results.len() > TOP_K as usize {
            results.select_nth_unstable_by(TOP_K as usize, |a, b| b.cmp(a));
            results.truncate(TOP_K as usize);
        }

        results.sort_unstable_by(|a, b| b.cmp(a));

        ArrayVec::from_iter(results.into_iter())
    }

    fn search_level0<const TOP_K: u16>(
        &self,
        entry_node: Node0Handle<M0, DIMS, Q, D>,
        query: &QuantVec<DIMS, Q>,
        ef: u16,
    ) -> ArrayVec<SearchResult<DIMS, Q, D, Node0<M0, DIMS, Q, D>>, { TOP_K as usize }> {
        let mut candidate_queue = BinaryHeap::new();
        let mut results = Vec::new();
        let mut set = FixedSet::<M0>::new();

        let node = &self.nodes0_arena[entry_node];
        let vec = &self.vec_arena[node.vec];

        let score = D::calculate(query, vec);
        candidate_queue.push(SearchResult {
            node: entry_node,
            score,
        });

        let mut nodes_visited = 0;

        while let Some(entry) = candidate_queue.pop() {
            if nodes_visited >= ef {
                break;
            }
            nodes_visited += 1;

            results.push(entry);

            let node = &self.nodes0_arena[entry_node];

            for neighbor in &node.neighbors.read().neighbors {
                let Some(neighbor) = neighbor else {
                    continue;
                };

                if set.is_member(*neighbor.node) {
                    let neighbor_node = &self.nodes0_arena[neighbor.node];
                    let neighbor_vec = &self.vec_arena[neighbor_node.vec];
                    let score = D::calculate(query, neighbor_vec);

                    set.insert(*neighbor.node);

                    candidate_queue.push(SearchResult {
                        node: neighbor.node,
                        score,
                    });
                }
            }
        }

        if results.len() > TOP_K as usize {
            results.select_nth_unstable_by(TOP_K as usize, |a, b| b.cmp(a));
            results.truncate(TOP_K as usize);
        }

        results.sort_unstable_by(|a, b| b.cmp(a));

        ArrayVec::from_iter(results.into_iter())
    }
}

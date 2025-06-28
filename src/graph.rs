use core::{alloc::Layout, cmp::Ordering, mem, ptr};

use alloc::{
    alloc::{alloc, dealloc, handle_alloc_error},
    boxed::Box,
    vec::Vec,
};
use binary_heap_plus::BinaryHeap;

use crate::{
    NodeId,
    arena::{Arena, DoubleArena, DynAlloc},
    fixedset::FixedSet,
    handle::{Handle, HandleA},
    metric::{DistanceMetric, DistanceMetricKind, dot_product_f32},
    node::{Neighbor, Neighbor0, Node, Node0, Node0Handle, NodeHandle, VecHandle},
    random::{AtomicRng, exponential_random},
    storage::{QuantVec, Quantization, RawVec},
    util::map_boxed_slice,
};

pub struct Graph {
    m: u16,
    m0: u16,
    dims: u16,
    levels: u8,
    quantization: Quantization,
    distance_metric: DistanceMetric,
    nodes_arena: Arena<Node>,
    nodes0_arena: Arena<Node0>,
    vec_arena: DoubleArena<RawVec, QuantVec>,
    top_level_root_node: NodeHandle,
    rng: AtomicRng,
}

#[repr(C, align(4))]
pub struct InternalSearchResult<T: ?Sized> {
    pub node: Handle<T>,
    pub score: f32,
}

impl<T: ?Sized> Clone for InternalSearchResult<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for InternalSearchResult<T> {}

#[repr(C, align(4))]
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    pub node: NodeId,
    pub score: f32,
}

impl Graph {
    pub fn new(
        m: u16,
        m0: u16,
        dims: u16,
        levels: u8,
        quantization: Quantization,
        metric: DistanceMetricKind,
    ) -> Self {
        let nodes_arena = Arena::new(1024, m);
        let nodes0_arena = Arena::new(1024, m0);
        let vec_arena = DoubleArena::new(1024, dims, (quantization, dims));

        let root_vec_raw: Box<[f32]> =
            unsafe { Box::new_zeroed_slice(dims as usize).assume_init() };

        let vec_handle = vec_arena.alloc(root_vec_raw.as_ptr(), root_vec_raw.as_ptr());

        let node0_handle = nodes0_arena.alloc(vec_handle);

        let mut prev_node = node0_handle.cast();

        for _ in 1..=levels {
            let node_handle = nodes_arena.alloc((vec_handle, prev_node));
            prev_node = node_handle;
        }

        Self {
            m,
            m0,
            dims,
            levels,
            quantization,
            distance_metric: DistanceMetric::new(metric, quantization),
            nodes_arena,
            nodes0_arena,
            vec_arena,
            top_level_root_node: prev_node,
            rng: AtomicRng::new(42),
        }
    }

    pub fn index(&self, vec: &[f32], ef: u16) -> NodeId {
        let vec_handle = self.vec_arena.alloc(vec.as_ptr(), vec.as_ptr());
        let vec = &self.vec_arena[vec_handle.handle_b()];

        let max_level = exponential_random(&self.rng, 0.4, self.levels);

        self.index_level(
            vec_handle,
            vec,
            self.top_level_root_node,
            self.levels,
            max_level,
            ef,
        );

        NodeId(*vec_handle - 1)
    }

    fn index_level(
        &self,
        vec_handle: VecHandle,
        vec: &QuantVec,
        entry_node: NodeHandle,
        current_level: u8,
        max_level: u8,
        ef: u16,
    ) -> NodeHandle {
        if current_level > max_level {
            let results = self.search_level(entry_node, vec, ef, 1, true);
            let child = self.nodes_arena[results[0].node].child;

            self.index_level(vec_handle, vec, child, current_level - 1, max_level, ef)
        } else if current_level == 0 {
            self.index_level0(vec_handle, vec, entry_node.cast(), ef)
                .cast()
        } else {
            let results = self.search_level(entry_node, vec, ef, self.m, true);
            let child = self.nodes_arena[results[0].node].child;

            let child = self.index_level(vec_handle, vec, child, current_level - 1, max_level, ef);

            self.create_node(vec_handle, results, child)
        }
    }

    fn index_level0(
        &self,
        vec_handle: VecHandle,
        vec: &QuantVec,
        entry_node: Node0Handle,
        ef: u16,
    ) -> Node0Handle {
        let results = self.search_level0(entry_node, vec, ef, self.m0, true);
        self.create_node0(vec_handle, results)
    }

    fn create_node(
        &self,
        vec_handle: VecHandle,
        results: Box<[InternalSearchResult<Node>]>,
        child: NodeHandle,
    ) -> NodeHandle {
        let node_handle = self.nodes_arena.alloc((vec_handle, child));
        let node = &self.nodes_arena[node_handle];
        let mut neighbors_guard = node.neighbors.write();

        unsafe {
            ptr::copy_nonoverlapping(
                results.as_ptr() as *const Neighbor,
                neighbors_guard.neighbors.as_mut_ptr(),
                results.len(),
            );
        }

        if results.len() as u16 == self.m {
            neighbors_guard.neighbors_full = true;
            let mut lowest_index = 0;
            let mut lowest_score = self.distance_metric.max_value();

            for i in 0..self.m {
                let neighbor = &neighbors_guard.neighbors[i as usize];
                if self.distance_metric.cmp_score(neighbor.score, lowest_score) == Ordering::Less {
                    lowest_score = neighbor.score;
                    lowest_index = i;
                }
            }

            neighbors_guard.lowest_index = lowest_index;
            neighbors_guard.lowest_score = lowest_score;
        } else {
            neighbors_guard.lowest_index = results.len() as u16;
        }

        for result in results {
            let neighbor = &self.nodes_arena[result.node];
            neighbor.neighbors.write().insert_neighbor(
                &self.distance_metric,
                node_handle,
                result.score,
            );
        }

        node_handle
    }

    fn create_node0(
        &self,
        vec_handle: VecHandle,
        results: Box<[InternalSearchResult<Node0>]>,
    ) -> Node0Handle {
        let node_handle = self.nodes0_arena.alloc(vec_handle);
        let node = &self.nodes0_arena[node_handle];
        let mut neighbors_guard = node.neighbors.write();

        unsafe {
            ptr::copy_nonoverlapping(
                results.as_ptr() as *const Neighbor0,
                neighbors_guard.neighbors.as_mut_ptr(),
                results.len(),
            );
        }

        if results.len() as u16 == self.m0 {
            neighbors_guard.neighbors_full = true;
            let mut lowest_index = 0;
            let mut lowest_score = self.distance_metric.max_value();

            for i in 0..self.m0 {
                let neighbor = &neighbors_guard.neighbors[i as usize];
                if self.distance_metric.cmp_score(neighbor.score, lowest_score) == Ordering::Less {
                    lowest_score = neighbor.score;
                    lowest_index = i;
                }
            }

            neighbors_guard.lowest_index = lowest_index;
            neighbors_guard.lowest_score = lowest_score;
        } else {
            neighbors_guard.lowest_index = results.len() as u16;
        }

        for result in results {
            let neighbor = &self.nodes0_arena[result.node];
            neighbor.neighbors.write().insert_neighbor(
                &self.distance_metric,
                node_handle,
                result.score,
            );
        }

        node_handle
    }

    pub fn search_quantized(&self, query: &[f32], ef: u16, top_k: u16) -> Box<[SearchResult]> {
        let (query, ptr, layout): (&QuantVec, *mut u8, Layout) = unsafe {
            let metadata = (self.quantization, self.dims);
            let size = QuantVec::size_aligned(metadata);
            let layout = Layout::from_size_align_unchecked(size, QuantVec::ALIGN);
            let ptr = alloc(layout);
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            QuantVec::new_at(ptr, metadata, query.as_ptr());
            let query = &*ptr::from_raw_parts(ptr, QuantVec::ptr_metadata(metadata));
            (query, ptr, layout)
        };
        let mut entry_node = self.top_level_root_node;

        // ignore the `0..self.range`, the actual search range in (0, self.levels]
        for _ in 0..self.levels {
            let results = self.search_level(entry_node, query, ef, top_k, true);
            let child = self.nodes_arena[results[0].node].child;
            entry_node = child;
        }

        let entry_node = entry_node.cast();

        let results = self.search_level0(entry_node, query, ef, top_k, false);

        unsafe {
            dealloc(ptr, layout);
        }

        unsafe {
            map_boxed_slice(results, |result| SearchResult {
                node: NodeId(*self.nodes0_arena[result.node].vec - 1),
                score: result.score,
            })
        }
    }

    pub fn search(&self, query: &[f32], ef: u16, top_k: u16) -> Box<[SearchResult]> {
        debug_assert!((0..8192).contains(&top_k));
        let mag_query = dot_product_f32(query, query);
        let results_quantized = self.search_quantized(query, ef, top_k * 8);
        let results_quantized =
            unsafe { mem::transmute::<Box<[SearchResult]>, Box<[(u32, f32)]>>(results_quantized) };
        let query = unsafe { mem::transmute::<&[f32], &RawVec>(query) };
        let mut results = Vec::with_capacity(results_quantized.len());
        for (handle, _) in results_quantized {
            let handle_a = HandleA::new(handle + 1);
            let vec = &self.vec_arena[handle_a];
            let mag_vec = dot_product_f32(&vec.vec, &vec.vec);
            let score = self
                .distance_metric
                .calculate_raw(query, mag_query, vec, mag_vec);
            results.push((handle, score));
        }

        let top_k = top_k as usize;

        if results.len() > top_k {
            results.select_nth_unstable_by(top_k, |a, b| self.distance_metric.cmp_score(a.1, b.1));
            results.truncate(top_k);
        }

        results.sort_unstable_by(|a, b| self.distance_metric.cmp_score(a.1, b.1));

        unsafe {
            mem::transmute::<Box<[(u32, f32)]>, Box<[SearchResult]>>(results.into_boxed_slice())
        }
    }

    fn search_level(
        &self,
        entry_node: NodeHandle,
        query: &QuantVec,
        ef: u16,
        top_k: u16,
        include_root: bool,
    ) -> Box<[InternalSearchResult<Node>]> {
        let mut candidate_queue = BinaryHeap::new_by(|a: &InternalSearchResult<Node>, b| {
            self.distance_metric.cmp_score(a.score, b.score)
        });
        let mut results = Vec::new();
        let mut set = FixedSet::new(self.m);

        let node = &self.nodes_arena[entry_node];
        let vec = &self.vec_arena[node.vec.handle_b()];

        let score = self.distance_metric.calculate(query, vec);

        set.insert(*entry_node);
        candidate_queue.push(InternalSearchResult {
            node: entry_node,
            score,
        });

        let mut nodes_visisted = 0;

        while let Some(entry) = candidate_queue.pop() {
            if nodes_visisted >= ef {
                break;
            }

            nodes_visisted += 1;
            if include_root || *entry.node != 0 {
                results.push(entry);
            }

            let node = &self.nodes_arena[entry_node];

            for neighbor in node.neighbors.read().neighbors() {
                if !set.is_member(*neighbor.node) {
                    let neighbor_node = &self.nodes_arena[neighbor.node];
                    let neighbor_vec = &self.vec_arena[neighbor_node.vec.handle_b()];
                    let score = self.distance_metric.calculate(query, neighbor_vec);

                    set.insert(*neighbor.node);
                    candidate_queue.push(InternalSearchResult {
                        node: neighbor.node,
                        score,
                    });
                }
            }
        }

        let top_k = top_k as usize;

        if results.len() > top_k {
            results.select_nth_unstable_by(top_k, |a, b| {
                self.distance_metric.cmp_score(a.score, b.score)
            });
            results.truncate(top_k);
        }

        results.sort_unstable_by(|a, b| self.distance_metric.cmp_score(a.score, b.score));

        results.into_boxed_slice()
    }

    fn search_level0(
        &self,
        entry_node: Node0Handle,
        query: &QuantVec,
        ef: u16,
        top_k: u16,
        include_root: bool,
    ) -> Box<[InternalSearchResult<Node0>]> {
        let mut candidate_queue = BinaryHeap::new_by(|a: &InternalSearchResult<Node0>, b| {
            self.distance_metric.cmp_score(a.score, b.score)
        });
        let mut results = Vec::new();
        let mut set = FixedSet::new(self.m0);

        let node = &self.nodes0_arena[entry_node];
        let vec = &self.vec_arena[node.vec.handle_b()];

        let score = self.distance_metric.calculate(query, vec);

        set.insert(*entry_node);
        candidate_queue.push(InternalSearchResult {
            node: entry_node,
            score,
        });

        let mut nodes_visisted = 0;

        while let Some(entry) = candidate_queue.pop() {
            if nodes_visisted >= ef {
                break;
            }

            nodes_visisted += 1;
            if include_root || *entry.node != 0 {
                results.push(entry);
            }

            let node = &self.nodes0_arena[entry_node];

            for neighbor in node.neighbors.read().neighbors() {
                if !set.is_member(*neighbor.node) {
                    let neighbor_node = &self.nodes0_arena[neighbor.node];
                    let neighbor_vec = &self.vec_arena[neighbor_node.vec.handle_b()];
                    let score = self.distance_metric.calculate(query, neighbor_vec);

                    set.insert(*neighbor.node);
                    candidate_queue.push(InternalSearchResult {
                        node: neighbor.node,
                        score,
                    });
                }
            }
        }

        let top_k = top_k as usize;

        if results.len() > top_k {
            results.select_nth_unstable_by(top_k, |a, b| {
                self.distance_metric.cmp_score(a.score, b.score)
            });
            results.truncate(top_k);
        }

        results.sort_unstable_by(|a, b| self.distance_metric.cmp_score(a.score, b.score));

        results.into_boxed_slice()
    }
}

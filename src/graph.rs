use crate::{
    arena::{Arena, Handle},
    metric::DistanceMetric,
    node::{Node, NodeHandle},
    storage::{QuantVec, Quantization, RawVec},
    types::HNSWLevel,
};

pub struct Graph<const M: u16, const DIMS: u16, const LEVELS: u8, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    nodes_arena: Arena<Node<M, DIMS, Q, D>>,
    vec_arena: Arena<QuantVec<DIMS, Q>>,
    top_level_root_node: NodeHandle<M, DIMS, Q, D>,
}

impl<const M: u16, const DIMS: u16, const LEVELS: u8, Q, D> Default for Graph<M, DIMS, LEVELS, Q, D>
where
    [(); M as usize]: Sized,
    [(); DIMS as usize]: Sized,
    Q: Quantization<DIMS>,
    D: DistanceMetric<DIMS, Q>,
{
    fn default() -> Self {
        let nodes_arena = Arena::new();
        let vec_arena = Arena::new();
        let root_vec_raw = RawVec::from([0.0; DIMS as usize]);
        let root_vec_quant = QuantVec::from(root_vec_raw);
        let vec_handle = vec_arena.alloc(root_vec_quant);
        let mut prev_node = Handle::invalid();
        for level in 0..LEVELS {
            let level = HNSWLevel::from(level);
            let node = Node::new(level, vec_handle, prev_node);
            let node_handle = nodes_arena.alloc(node);
            prev_node = node_handle;
        }
        Self {
            nodes_arena,
            vec_arena,
            top_level_root_node: prev_node,
        }
    }
}

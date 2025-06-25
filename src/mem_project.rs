use crate::{
    Quantization,
    arena::DynAlloc,
    node::{Node, Node0},
};

pub fn len_to_cap(mut x: u64) -> u64 {
    if x == 0 {
        return 0;
    }
    x -= 1;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x |= x >> 32;
    x + 1
}

pub fn mem_project(
    m: u16,
    m0: u16,
    dims: u16,
    levels: u8,
    quantization: Quantization,
    dataset_size: u32,
) -> u64 {
    let graph_size_bytes = 232;
    let chunk_size = 1024;
    let node0_size = Node0::size_aligned(m0) as u64;
    let node_size = Node::size_aligned(m) as u64;

    let raw_vec_size = dims as u64 * 4;
    let quant_vec_size = quantization.size() as u64 * dims as u64;
    let vec_size = raw_vec_size + quant_vec_size;
    let mut node_arena_size = 0.0;

    for level in 1..=levels {
        let multiplier = 0.4f64.powi(level as i32);
        node_arena_size += multiplier * dataset_size as f64;
    }

    let node0_arena_len = dataset_size as u64;
    let node_arena_len = node_arena_size as u64;
    let vec_arena_len = dataset_size as u64;

    let node0_arena_vec_len = node0_arena_len.div_ceil(chunk_size);
    let node_arena_vec_len = node_arena_len.div_ceil(chunk_size);
    let vec_arena_vec_len = vec_arena_len.div_ceil(chunk_size);

    let node0_arena_vec_cap = len_to_cap(node0_arena_vec_len);
    let node_arena_vec_cap = len_to_cap(node_arena_vec_len);
    let vec_arena_vec_cap = len_to_cap(vec_arena_vec_len);

    let chunk_size = size_of::<usize>() as u64;

    let node0_arena_heap_size = (node0_arena_vec_cap * chunk_size) + (node0_arena_len * node0_size);
    let node_arena_heap_size = (node_arena_vec_cap * chunk_size) + (node_arena_len * node_size);
    let vec_arena_heap_size = (vec_arena_vec_cap * chunk_size) + (vec_arena_len * vec_size);

    graph_size_bytes + node0_arena_heap_size + node_arena_heap_size + vec_arena_heap_size
}

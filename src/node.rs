use core::cmp::Ordering;

use crate::{
    arena::{DynAlloc, DynDefault, DynInit},
    handle::Handle,
    metric::DistanceMetric,
    rwlock::RwLock,
    storage::QuantVec,
};

pub type VecHandle = Handle<QuantVec>;
pub type NodeHandle = Handle<Node>;
pub type Node0Handle = Handle<Node0>;

#[repr(C, align(4))]
pub struct Node {
    pub(crate) vec: VecHandle,
    pub(crate) child: NodeHandle,
    pub(crate) neighbors: RwLock<Neighbors>,
}

#[repr(C, align(4))]
pub struct Node0 {
    pub(crate) vec: VecHandle,
    pub(crate) neighbors: RwLock<Neighbors0>,
}

#[repr(C, align(4))]
pub struct Neighbors {
    pub(crate) neighbors_full: bool,
    pub(crate) lowest_index: u16,
    pub(crate) lowest_score: f32,
    pub(crate) neighbors: [Neighbor],
}

#[repr(C, align(4))]
pub struct Neighbors0 {
    pub(crate) neighbors_full: bool,
    pub(crate) lowest_index: u16,
    pub(crate) lowest_score: f32,
    pub(crate) neighbors: [Neighbor0],
}

impl Neighbors {
    pub fn neighbors(&self) -> &[Neighbor] {
        if self.neighbors_full {
            &self.neighbors
        } else {
            &self.neighbors[..(self.lowest_index as usize)]
        }
    }

    pub fn insert_neighbor(
        &mut self,
        distance_metric: &DistanceMetric,
        node: NodeHandle,
        score: f32,
    ) {
        if self.neighbors_full {
            if distance_metric.cmp_score(score, self.neighbors[self.lowest_index as usize].score)
                == Ordering::Greater
            {
                self.neighbors[self.lowest_index as usize] = Neighbor { node, score };
                self.recompute_lowest_index(distance_metric);
            }
        } else {
            self.neighbors[self.lowest_index as usize] = Neighbor { node, score };
            if self.lowest_index as usize == self.neighbors.len() {
                self.neighbors_full = true;
                self.recompute_lowest_index(distance_metric);
            } else {
                self.lowest_index += 1;
            }
        }
    }

    fn recompute_lowest_index(&mut self, distance_metric: &DistanceMetric) {
        let mut lowest_index = 0;
        let mut lowest_score = distance_metric.max_value();

        for i in 0..(self.neighbors.len() as u16) {
            let neighbor = &self.neighbors[i as usize];
            if distance_metric.cmp_score(neighbor.score, lowest_score) == Ordering::Less {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        self.lowest_index = lowest_index;
        self.lowest_score = lowest_score;
    }
}

impl Neighbors0 {
    pub fn neighbors(&self) -> &[Neighbor0] {
        if self.neighbors_full {
            &self.neighbors
        } else {
            &self.neighbors[..(self.lowest_index as usize)]
        }
    }

    pub fn insert_neighbor(
        &mut self,
        distance_metric: &DistanceMetric,
        node: Node0Handle,
        score: f32,
    ) {
        if self.neighbors_full {
            if distance_metric.cmp_score(score, self.neighbors[self.lowest_index as usize].score)
                == Ordering::Greater
            {
                self.neighbors[self.lowest_index as usize] = Neighbor0 { node, score };
                self.recompute_lowest_index(distance_metric);
            }
        } else {
            self.neighbors[self.lowest_index as usize] = Neighbor0 { node, score };
            if self.lowest_index as usize == self.neighbors.len() {
                self.neighbors_full = true;
                self.recompute_lowest_index(distance_metric);
            } else {
                self.lowest_index += 1;
            }
        }
    }

    fn recompute_lowest_index(&mut self, distance_metric: &DistanceMetric) {
        let mut lowest_index = 0;
        let mut lowest_score = distance_metric.max_value();

        for i in 0..(self.neighbors.len() as u16) {
            let neighbor = &self.neighbors[i as usize];
            if distance_metric.cmp_score(neighbor.score, lowest_score) == Ordering::Less {
                lowest_score = neighbor.score;
                lowest_index = i;
            }
        }

        self.lowest_index = lowest_index;
        self.lowest_score = lowest_score;
    }
}

#[repr(C, align(4))]
pub struct Neighbor {
    pub node: NodeHandle,
    pub score: f32,
}

#[repr(C, align(4))]
pub struct Neighbor0 {
    pub node: Node0Handle,
    pub score: f32,
}

impl DynAlloc for Node {
    type Metadata = u16;
    const ALIGN: usize = 4;

    fn size(metadata: u16) -> usize {
        12 + Neighbors::size_aligned(metadata)
    }

    fn ptr_metadata(len: u16) -> <Self as core::ptr::Pointee>::Metadata {
        len as usize
    }
}

impl DynInit for Node {
    type Args = (VecHandle, NodeHandle);

    unsafe fn new_at(ptr: *mut u8, len: u16, (vec, child): Self::Args) {
        (ptr as *mut VecHandle).write(vec);
        (ptr.add(4) as *mut NodeHandle).write(child);
        ptr.add(8).write(0);
        Neighbors::default_at(ptr.add(12), len);
    }
}

impl DynAlloc for Node0 {
    type Metadata = u16;
    const ALIGN: usize = 4;

    fn size(metadata: u16) -> usize {
        8 + Neighbors0::size_aligned(metadata)
    }

    fn ptr_metadata(len: u16) -> <Self as core::ptr::Pointee>::Metadata {
        len as usize
    }
}

impl DynInit for Node0 {
    type Args = VecHandle;

    unsafe fn new_at(ptr: *mut u8, len: u16, vec: Self::Args) {
        (ptr as *mut VecHandle).write(vec);
        ptr.add(4).write(0);
        Neighbors0::default_at(ptr.add(8), len);
    }
}

impl DynAlloc for Neighbors {
    type Metadata = u16;
    const ALIGN: usize = 4;

    fn size(len: u16) -> usize {
        8 + (len as usize) * 8
    }

    fn ptr_metadata(len: u16) -> <Self as core::ptr::Pointee>::Metadata {
        len as usize
    }
}

impl DynDefault for Neighbors {
    unsafe fn default_at(ptr: *mut u8, len: u16) {
        ptr.write_bytes(0, Self::size_aligned(len));
    }
}

impl DynAlloc for Neighbors0 {
    type Metadata = u16;
    const ALIGN: usize = 4;

    fn size(len: u16) -> usize {
        8 + (len as usize) * 8
    }

    fn ptr_metadata(len: u16) -> <Self as core::ptr::Pointee>::Metadata {
        len as usize
    }
}

impl DynDefault for Neighbors0 {
    unsafe fn default_at(ptr: *mut u8, metadata: Self::Metadata) {
        ptr.write_bytes(0, Self::size_aligned(metadata));
    }
}

// Add to the end of node.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::Arena;

    #[test]
    fn test_node_allocation() {
        let metadata: u16 = 5; // Number of neighbors
        let arena = Arena::<Node>::new(16, metadata);
        let dummy_vec_handle = VecHandle::invalid();
        let dummy_child_handle = NodeHandle::invalid();

        // Allocate a Node
        let node_handle = arena.alloc((dummy_vec_handle, dummy_child_handle));
        let node = &arena[node_handle];

        // Verify fields
        assert_eq!(node.vec, dummy_vec_handle);
        assert_eq!(node.child, dummy_child_handle);

        // Verify neighbors initialization
        let neighbors = node.neighbors.read();
        assert!(!neighbors.neighbors_full);
        assert_eq!(neighbors.lowest_index, 0);
        assert_eq!(neighbors.lowest_score, 0.0);
        assert_eq!(neighbors.neighbors.len(), metadata as usize);

        for neighbor in &neighbors.neighbors {
            assert_eq!(*neighbor.node, 0);
            assert_eq!(neighbor.score, 0.0);
        }
    }

    #[test]
    fn test_node0_allocation() {
        let metadata: u16 = 3; // Number of neighbors
        let arena = Arena::<Node0>::new(16, metadata);
        let dummy_vec_handle = VecHandle::invalid();

        // Allocate a Node0
        let node0_handle = arena.alloc(dummy_vec_handle);
        let node0 = &arena[node0_handle];

        // Verify fields
        assert_eq!(node0.vec, dummy_vec_handle);

        // Verify neighbors initialization
        let neighbors = node0.neighbors.read();
        assert!(!neighbors.neighbors_full);
        assert_eq!(neighbors.lowest_index, 0);
        assert_eq!(neighbors.lowest_score, 0.0);
        assert_eq!(neighbors.neighbors.len(), metadata as usize);

        for neighbor in &neighbors.neighbors {
            assert_eq!(*neighbor.node, 0);
            assert_eq!(neighbor.score, 0.0);
        }
    }

    #[test]
    fn test_clear_arena() {
        let metadata: u16 = 2;
        let mut arena = Arena::<Node>::new(16, metadata);
        let dummy_vec_handle = VecHandle::invalid();
        let dummy_child_handle = NodeHandle::invalid();

        // Allocate nodes
        let _node1 = arena.alloc((dummy_vec_handle, dummy_child_handle));
        let _node2 = arena.alloc((dummy_vec_handle, dummy_child_handle));
        assert_eq!(arena.len(), 2);

        // Clear arena and verify
        arena.clear();
        assert_eq!(arena.len(), 0);
    }
}

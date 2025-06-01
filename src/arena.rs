use alloc::{boxed::Box, vec::Vec};
use core::{
    cell::UnsafeCell,
    fmt,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Index,
    sync::atomic::{AtomicU32, Ordering},
};
use parking_lot::{RwLock, RwLockUpgradableReadGuard};

const CHUNK_SIZE: usize = 1024;

pub struct Handle<T> {
    index: u32,
    _marker: PhantomData<T>,
}

impl<T> Handle<T> {
    fn new(index: u32) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    pub fn invalid() -> Self {
        Self::new(u32::MAX)
    }

    pub fn is_valid(&self) -> bool {
        self.index != u32::MAX
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Handle").field(&self.index).finish()
    }
}

struct Chunk<T> {
    data: [UnsafeCell<MaybeUninit<T>>; CHUNK_SIZE],
}

impl<T> Default for Chunk<T> {
    fn default() -> Self {
        let data: [UnsafeCell<MaybeUninit<T>>; CHUNK_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        Self { data }
    }
}

impl<T> Chunk<T> {
    unsafe fn write(&self, index: usize, value: T) {
        unsafe { &mut *self.data[index].get() }.write(value);
    }

    unsafe fn get_ref<'a>(&self, index: usize) -> &'a T {
        unsafe { (*self.data[index].get()).assume_init_ref() }
    }

    unsafe fn drop_range(&mut self, count: usize) {
        for i in 0..count {
            unsafe { self.data[i].get_mut().assume_init_drop() };
        }
    }
}

unsafe impl<T: Send> Send for Chunk<T> {}
unsafe impl<T: Sync> Sync for Chunk<T> {}

pub struct Arena<T> {
    chunks: RwLock<Vec<Box<Chunk<T>>>>,
    next_index: AtomicU32,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            chunks: RwLock::new(Vec::new()),
            next_index: AtomicU32::new(0),
        }
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&self, value: T) -> Handle<T> {
        let index = self.next_index.fetch_add(1, Ordering::AcqRel);
        let chunk_index = index as usize / CHUNK_SIZE;
        let offset = index as usize % CHUNK_SIZE;

        let chunks_guard = self.chunks.upgradable_read();
        if chunk_index >= chunks_guard.len() {
            let mut chunks_guard = RwLockUpgradableReadGuard::upgrade(chunks_guard);
            while chunk_index >= chunks_guard.len() {
                chunks_guard.push(Box::new(Chunk::default()));
            }
        }

        let chunk = &self.chunks.read()[chunk_index];
        unsafe {
            chunk.write(offset, value);
        }

        Handle::new(index)
    }

    pub fn get(&self, handle: Handle<T>) -> Option<T>
    where
        T: Clone,
    {
        let (chunk_index, offset) = Self::split_handle(handle);
        let chunks_guard = self.chunks.read();
        let chunk = chunks_guard.get(chunk_index)?;
        Some(unsafe { chunk.get_ref(offset).clone() })
    }

    pub fn clear(&self) {
        let total_allocated = self.next_index.load(Ordering::Acquire) as usize;
        let mut chunks_guard = self.chunks.write();

        let mut remaining = total_allocated;
        for chunk in chunks_guard.iter_mut() {
            let to_drop = remaining.min(CHUNK_SIZE);
            unsafe {
                chunk.drop_range(to_drop);
            }
            remaining -= to_drop;
        }

        chunks_guard.clear();
        self.next_index.store(0, Ordering::Release);
    }

    fn split_handle(handle: Handle<T>) -> (usize, usize) {
        let index = handle.index as usize;
        (index / CHUNK_SIZE, index % CHUNK_SIZE)
    }

    fn get_ref_internal(&self, handle: Handle<T>) -> &T {
        let (chunk_index, offset) = Self::split_handle(handle);
        let chunks_guard = self.chunks.read();
        let chunk = &chunks_guard[chunk_index];
        unsafe { chunk.get_ref(offset) }
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        let total_allocated = self.next_index.load(Ordering::Acquire) as usize;
        let mut chunks_guard = self.chunks.write();

        let mut remaining = total_allocated;
        for mut chunk in chunks_guard.drain(..) {
            let to_drop = remaining.min(CHUNK_SIZE);
            unsafe {
                chunk.drop_range(to_drop);
            }
            remaining -= to_drop;
        }
    }
}

impl<T> Index<Handle<T>> for Arena<T> {
    type Output = T;

    fn index(&self, handle: Handle<T>) -> &Self::Output {
        self.get_ref_internal(handle)
    }
}

// arena.rs (continued)

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::sync::atomic::{AtomicU8, Ordering};

    #[test]
    fn basic_alloc_and_get() {
        let arena = Arena::new();
        let handle = arena.alloc(42);
        assert_eq!(arena[handle], 42);
    }

    #[test]
    fn multiple_allocations() {
        let arena = Arena::new();
        let handles: Vec<Handle<usize>> = (0..5000).map(|i| arena.alloc(i)).collect();

        for (i, handle) in handles.iter().enumerate() {
            assert_eq!(arena[*handle], i);
        }
    }

    #[test]
    fn cross_chunk_allocations() {
        let arena = Arena::new();
        // Allocate enough to span multiple chunks
        let handles: Vec<Handle<usize>> = (0..CHUNK_SIZE * 3).map(|i| arena.alloc(i)).collect();

        for (i, handle) in handles.iter().enumerate() {
            assert_eq!(arena[*handle], i);
        }
    }

    #[test]
    fn clear_arena() {
        let arena = Arena::new();
        let _ = arena.alloc(1);
        let _ = arena.alloc(2);

        arena.clear();

        // Allocate after clear should start from index 0
        let handle = arena.alloc(3);
        assert_eq!(handle.index, 0);
        assert_eq!(arena[handle], 3);
    }

    #[test]
    fn clone_handle() {
        let arena = Arena::new();
        let handle1 = arena.alloc(42);
        let handle2 = handle1;
        assert_eq!(handle1.index, handle2.index);
        assert_eq!(arena[handle1], arena[handle2]);
    }

    #[test]
    fn get_clone() {
        let arena = Arena::new();
        let handle = arena.alloc("test".to_string());
        let value = arena.get(handle).unwrap();
        assert_eq!(&value, "test");
    }

    #[test]
    fn values_dropped_on_clear() {
        static DROP_COUNTER: AtomicU8 = AtomicU8::new(0);

        struct DropCount;
        impl Drop for DropCount {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
            }
        }

        let arena = Arena::new();
        arena.alloc(DropCount);
        arena.alloc(DropCount);

        arena.clear();
        assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn values_dropped_on_drop() {
        static DROP_COUNTER: AtomicU8 = AtomicU8::new(0);

        struct DropCount;
        impl Drop for DropCount {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
            }
        }

        let arena = Arena::new();
        arena.alloc(DropCount);
        arena.alloc(DropCount);

        drop(arena);
        assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn partial_chunk_drop() {
        static DROP_COUNTER: AtomicU8 = AtomicU8::new(0);

        struct DropCount;
        impl Drop for DropCount {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
            }
        }

        let arena = Arena::new();
        // Allocate 1.5 chunks worth
        for _ in 0..(CHUNK_SIZE + CHUNK_SIZE / 2) {
            arena.alloc(DropCount);
        }

        drop(arena);
        assert_eq!(
            DROP_COUNTER.load(Ordering::Relaxed),
            (CHUNK_SIZE + CHUNK_SIZE / 2) as u8
        );
    }
}

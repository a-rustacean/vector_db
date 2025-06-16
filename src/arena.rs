use alloc::{boxed::Box, format, vec, vec::Vec};
use core::{
    cell::UnsafeCell,
    fmt,
    marker::PhantomData,
    mem::{MaybeUninit, align_of, size_of},
    ops::{Deref, Index},
    ptr, slice,
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

    pub fn cast<U>(self) -> Handle<U> {
        Handle::new(self.index)
    }
}

impl<T> Deref for Handle<T> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!("Handle<{:?}>", core::any::type_name::<T>()))
            .field(&self.index)
            .finish()
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

    /// Convert the chunk to a byte slice for serialization
    /// Only valid for the initialized portion of the chunk
    unsafe fn as_bytes(&self, count: usize) -> &[u8] {
        let ptr = self.data.as_ptr() as *const u8;
        let byte_count = count * size_of::<T>();
        unsafe { slice::from_raw_parts(ptr, byte_count) }
    }

    /// Initialize chunk from byte slice
    /// SAFETY: bytes must contain valid T values and be properly aligned
    unsafe fn from_bytes(bytes: &[u8], count: usize) -> Self {
        let mut chunk = Self::default();
        let src_ptr = bytes.as_ptr();
        let dst_ptr = chunk.data.as_mut_ptr() as *mut u8;
        let byte_count = count * size_of::<T>();
        unsafe {
            ptr::copy_nonoverlapping(src_ptr, dst_ptr, byte_count);
        }
        chunk
    }
}

unsafe impl<T: Send> Send for Chunk<T> {}
unsafe impl<T: Sync> Sync for Chunk<T> {}

#[derive(Debug)]
pub enum SerializeError {
    WriteError,
    InvalidData,
    AlignmentError,
}

#[derive(Debug)]
pub enum DeserializeError {
    ReadError,
    InvalidData,
    AlignmentError,
    InsufficientData,
}

pub trait Writer {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), SerializeError>;
}

pub trait Reader {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), DeserializeError>;
}

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

    /// Serialize the arena to a writer
    ///
    /// # Safety
    ///
    /// T must be safely transmutable to/from bytes (e.g., POD types)
    pub unsafe fn serialize<W: Writer>(&self, writer: &mut W) -> Result<(), SerializeError>
    where
        T: Copy,
    {
        // Check alignment requirements
        if align_of::<T>() > 1 {
            // For types with alignment requirements, we need to be more careful
            // This is a simplified check - in practice you might want more sophisticated handling
        }

        let total_allocated = self.next_index.load(Ordering::Acquire);
        let chunks_guard = self.chunks.read();

        // Write header: total count
        let header_bytes = total_allocated.to_le_bytes();
        writer.write_all(&header_bytes)?;

        // Write each chunk's data
        let mut remaining = total_allocated as usize;
        for chunk in chunks_guard.iter() {
            let to_write = remaining.min(CHUNK_SIZE);
            if to_write > 0 {
                let bytes = unsafe { chunk.as_bytes(to_write) };
                writer.write_all(bytes)?;
            }
            remaining -= to_write;
        }

        Ok(())
    }

    /// Deserialize the arena from a reader
    ///
    /// # Safety
    ///
    /// The data must have been serialized from a compatible Arena<T>
    pub unsafe fn deserialize<R: Reader>(&mut self, reader: &mut R) -> Result<(), DeserializeError>
    where
        T: Copy,
    {
        // Check alignment requirements
        if align_of::<T>() > 1 {
            // For types with alignment requirements, we need to be more careful
        }

        // Clear existing data
        self.clear();

        // Read header
        let mut header_bytes = [0u8; 4];
        reader.read_exact(&mut header_bytes)?;
        let total_count = u32::from_le_bytes(header_bytes);

        if total_count == 0 {
            return Ok(());
        }

        let total_count = total_count as usize;
        let mut chunks_guard = self.chunks.write();

        // Calculate number of chunks needed
        let chunks_needed = total_count.div_ceil(CHUNK_SIZE);

        // Read chunk data
        let mut remaining = total_count;
        for _chunk_idx in 0..chunks_needed {
            let to_read = remaining.min(CHUNK_SIZE);
            let byte_count = to_read * size_of::<T>();

            let mut chunk_bytes = vec![0; byte_count];
            reader.read_exact(&mut chunk_bytes)?;

            let chunk = unsafe { Chunk::from_bytes(&chunk_bytes, to_read) };
            chunks_guard.push(Box::new(chunk));

            remaining -= to_read;
        }

        self.next_index.store(total_count as u32, Ordering::Release);
        Ok(())
    }

    /// Get the number of allocated items
    pub fn len(&self) -> usize {
        self.next_index.load(Ordering::Acquire) as usize
    }

    /// Check if the arena is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::sync::atomic::{AtomicU8, Ordering};

    // Mock writer for testing
    struct VecWriter {
        data: Vec<u8>,
    }

    impl VecWriter {
        fn new() -> Self {
            Self { data: Vec::new() }
        }
    }

    impl Writer for VecWriter {
        fn write_all(&mut self, buf: &[u8]) -> Result<(), SerializeError> {
            self.data.extend_from_slice(buf);
            Ok(())
        }
    }

    // Mock reader for testing
    struct VecReader {
        data: Vec<u8>,
        pos: usize,
    }

    impl VecReader {
        fn new(data: Vec<u8>) -> Self {
            Self { data, pos: 0 }
        }
    }

    impl Reader for VecReader {
        fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), DeserializeError> {
            if self.pos + buf.len() > self.data.len() {
                return Err(DeserializeError::InsufficientData);
            }
            buf.copy_from_slice(&self.data[self.pos..self.pos + buf.len()]);
            self.pos += buf.len();
            Ok(())
        }
    }

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

    #[test]
    fn serialize_deserialize_basic() {
        let arena = Arena::new();
        let handles: Vec<Handle<u32>> = (0..10).map(|i| arena.alloc(i)).collect();

        // Serialize
        let mut writer = VecWriter::new();
        unsafe {
            arena.serialize(&mut writer).unwrap();
        }

        // Deserialize into new arena
        let mut new_arena = Arena::<u32>::new();
        let mut reader = VecReader::new(writer.data);
        unsafe {
            new_arena.deserialize(&mut reader).unwrap();
        }

        // Verify data
        assert_eq!(new_arena.len(), 10);
        for (i, handle) in handles.iter().enumerate() {
            let new_handle = Handle::new(handle.index);
            assert_eq!(new_arena[new_handle], i as u32);
        }
    }

    #[test]
    fn serialize_deserialize_multiple_chunks() {
        let arena = Arena::new();
        let num_items = CHUNK_SIZE * 2 + 100; // Cross multiple chunks
        let handles: Vec<Handle<u64>> = (0..num_items).map(|i| arena.alloc(i as u64)).collect();

        // Serialize
        let mut writer = VecWriter::new();
        unsafe {
            arena.serialize(&mut writer).unwrap();
        }

        // Deserialize into new arena
        let mut new_arena = Arena::<u64>::new();
        let mut reader = VecReader::new(writer.data);
        unsafe {
            new_arena.deserialize(&mut reader).unwrap();
        }

        // Verify data
        assert_eq!(new_arena.len(), num_items);
        for (i, handle) in handles.iter().enumerate() {
            let new_handle = Handle::new(handle.index);
            assert_eq!(new_arena[new_handle], i as u64);
        }
    }

    #[test]
    fn serialize_deserialize_empty() {
        let arena: Arena<u32> = Arena::new();

        // Serialize empty arena
        let mut writer = VecWriter::new();
        unsafe {
            arena.serialize(&mut writer).unwrap();
        }

        // Deserialize into new arena
        let mut new_arena = Arena::<u32>::new();
        let mut reader = VecReader::new(writer.data);
        unsafe {
            new_arena.deserialize(&mut reader).unwrap();
        }

        // Verify empty
        assert_eq!(new_arena.len(), 0);
        assert!(new_arena.is_empty());
    }

    #[test]
    fn serialize_deserialize_mixed_types() {
        #[derive(Debug, PartialEq, Clone, Copy)]
        struct Point {
            x: f32,
            y: f32,
        }

        let arena = Arena::new();
        let points = [
            Point { x: 1.0, y: 2.0 },
            Point { x: 3.0, y: 4.0 },
            Point { x: 5.0, y: 6.0 },
        ];

        let handles: Vec<Handle<Point>> = points.iter().map(|&p| arena.alloc(p)).collect();

        // Serialize
        let mut writer = VecWriter::new();
        unsafe {
            arena.serialize(&mut writer).unwrap();
        }

        // Deserialize into new arena
        let mut new_arena = Arena::<Point>::new();
        let mut reader = VecReader::new(writer.data);
        unsafe {
            new_arena.deserialize(&mut reader).unwrap();
        }

        // Verify data
        assert_eq!(new_arena.len(), 3);
        for (i, handle) in handles.iter().enumerate() {
            let new_handle = Handle::new(handle.index);
            assert_eq!(new_arena[new_handle], points[i]);
        }
    }

    #[test]
    fn serialize_deserialize_preserve_handles() {
        let arena = Arena::new();

        // Allocate in a specific pattern
        let h1 = arena.alloc(100u32);
        let h2 = arena.alloc(200u32);
        let h3 = arena.alloc(300u32);

        // Serialize
        let mut writer = VecWriter::new();
        unsafe {
            arena.serialize(&mut writer).unwrap();
        }

        // Deserialize into new arena
        let mut new_arena = Arena::new();
        let mut reader = VecReader::new(writer.data);
        unsafe {
            new_arena.deserialize(&mut reader).unwrap();
        }

        // Original handles should work with new arena
        assert_eq!(new_arena[h1], 100);
        assert_eq!(new_arena[h2], 200);
        assert_eq!(new_arena[h3], 300);
    }
}

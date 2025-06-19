use core::{
    alloc::Layout,
    fmt,
    marker::PhantomData,
    mem,
    ops::{Deref, Index},
    ptr::{self, NonNull, Pointee},
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{
    alloc::{alloc, handle_alloc_error},
    format,
    vec::Vec,
};
use parking_lot::{RwLock, RwLockWriteGuard};

pub struct Handle<T: ?Sized> {
    index: u32,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> Handle<T> {
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

    pub fn cast<U: ?Sized>(self) -> Handle<U> {
        Handle::new(self.index)
    }
}

impl<T: ?Sized> Deref for Handle<T> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<T: ?Sized> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for Handle<T> {}

impl<T: ?Sized> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Handle<T> {}

impl<T: ?Sized> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!("Handle<{:?}>", core::any::type_name::<T>()))
            .field(&self.index)
            .finish()
    }
}

struct Chunk<T: DynAlloc + ?Sized> {
    ptr: NonNull<u8>,
    _marker: PhantomData<T>,
}

impl<T: DynAlloc + ?Sized> Chunk<T> {
    unsafe fn new(item_size: usize, item_align: usize, chunk_size: usize) -> Self {
        let layout = Layout::from_size_align_unchecked(item_size * chunk_size, item_align);
        let ptr = alloc(layout);

        if ptr.is_null() {
            handle_alloc_error(layout)
        }

        Self {
            ptr: NonNull::new_unchecked(ptr),
            _marker: PhantomData,
        }
    }

    unsafe fn get_raw(&self, item_size: usize, index: usize) -> *mut u8 {
        self.ptr.as_ptr().add(item_size * index)
    }

    unsafe fn get_ref<'a>(
        &self,
        item_size: usize,
        index: usize,
        metadata: <T as Pointee>::Metadata,
    ) -> &'a T {
        &*ptr::from_raw_parts(self.ptr.as_ptr().add(item_size * index), metadata)
    }

    unsafe fn init(&self, item_size: usize, index: usize, metadata: T::Metadata, args: T::Args)
    where
        T: DynInit,
    {
        T::new_at(self.get_raw(item_size, index), metadata, args);
    }

    #[allow(unused)]
    unsafe fn init_default(&self, item_size: usize, index: usize, metadata: T::Metadata)
    where
        T: DynDefault,
    {
        T::default_at(self.get_raw(item_size, index), metadata);
    }
}

pub trait DynInit: DynAlloc {
    type Args;

    unsafe fn new_at(ptr: *mut u8, metadata: Self::Metadata, args: Self::Args);
}

pub trait DynDefault: DynAlloc {
    unsafe fn default_at(ptr: *mut u8, metadata: Self::Metadata);
}

fn align_up(size: usize, alignment: usize) -> usize {
    debug_assert!(alignment != 0, "Alignment must be non-zero");
    debug_assert!(
        alignment.is_power_of_two(),
        "Alignment must be a power of two"
    );

    let mask = alignment - 1;
    if size == 0 { 0 } else { (size + mask) & !mask }
}

pub trait DynAlloc {
    type Metadata: Clone + Copy;

    const ALIGN: usize;

    fn size(metadata: Self::Metadata) -> usize;

    #[inline(always)]
    fn size_aligned(metadata: Self::Metadata) -> usize {
        let size = Self::size(metadata);
        align_up(size, Self::ALIGN)
    }

    fn ptr_metadata(metadata: Self::Metadata) -> <Self as Pointee>::Metadata;
}

// impl for all Sized types
impl<T> DynAlloc for T {
    type Metadata = ();
    const ALIGN: usize = align_of::<T>();

    fn size(_metadata: Self::Metadata) -> usize {
        size_of::<T>()
    }

    fn ptr_metadata(_metadata: Self::Metadata) -> <Self as Pointee>::Metadata {}
}

impl<T: Default> DynDefault for T {
    unsafe fn default_at(ptr: *mut u8, _metadata: ()) {
        let default = Self::default();

        (ptr as *mut Self).write(default);
    }
}

pub struct Arena<T: DynAlloc + ?Sized> {
    chunks: RwLock<Vec<Chunk<T>>>,
    chunk_size: usize,
    metadata: T::Metadata,
    next_index: AtomicU32,
}

impl<T: DynAlloc + ?Sized> Arena<T> {
    pub fn new(chunk_size: usize, metadata: T::Metadata) -> Self {
        Self {
            chunks: RwLock::new(Vec::new()),
            chunk_size,
            metadata,
            next_index: AtomicU32::new(0),
        }
    }

    pub fn alloc(&self, args: T::Args) -> Handle<T>
    where
        T: DynInit,
    {
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        let chunk_index = index as usize / self.chunk_size;
        let offset = index as usize % self.chunk_size;

        let chunks_guard = self.chunks.read();

        let chunks_guard = if chunk_index >= chunks_guard.len() {
            drop(chunks_guard);
            let mut chunks_guard = self.chunks.write();
            while chunk_index >= chunks_guard.len() {
                chunks_guard.push(unsafe {
                    Chunk::new(T::size_aligned(self.metadata), T::ALIGN, self.chunk_size)
                });
            }
            RwLockWriteGuard::downgrade(chunks_guard)
        } else {
            chunks_guard
        };

        let chunk = &chunks_guard[chunk_index];
        unsafe {
            chunk.init(T::size_aligned(self.metadata), offset, self.metadata, args);
        }

        Handle::new(index)
    }

    #[allow(unused)]
    pub fn alloc_default(&self) -> Handle<T>
    where
        T: DynDefault,
    {
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        let chunk_index = index as usize / self.chunk_size;
        let offset = index as usize % self.chunk_size;

        let chunks_guard = self.chunks.read();

        let chunks_guard = if chunk_index >= chunks_guard.len() {
            drop(chunks_guard);
            let mut chunks_guard = self.chunks.write();
            while chunk_index >= chunks_guard.len() {
                chunks_guard.push(unsafe {
                    Chunk::new(T::size_aligned(self.metadata), T::ALIGN, self.chunk_size)
                });
            }
            RwLockWriteGuard::downgrade(chunks_guard)
        } else {
            chunks_guard
        };

        let chunk = &chunks_guard[chunk_index];
        unsafe {
            chunk.init_default(T::size_aligned(self.metadata), offset, self.metadata);
        }

        Handle::new(index)
    }

    /// Get the number of allocated items
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.next_index.load(Ordering::Acquire) as usize
    }

    /// Check if the arena is empty
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn split_handle(&self, handle: Handle<T>) -> (usize, usize) {
        let index = handle.index as usize;
        (index / self.chunk_size, index % self.chunk_size)
    }

    #[allow(unused)]
    pub fn clear(&self) {
        let mut chunks_guard = self.chunks.write();
        let chunks = mem::take(&mut *chunks_guard); // Take ownership of the chunks

        let total = self.next_index.load(Ordering::Acquire) as usize;
        if total == 0 {
            return; // No objects allocated
        }

        if chunks.is_empty() {
            return;
        }

        let item_size = T::size_aligned(self.metadata);
        let item_align = T::ALIGN;

        // Drop each allocated object in reverse order (from last to first)
        for i in (0..total).rev() {
            let chunk_index = i / self.chunk_size;
            let offset = i % self.chunk_size;
            let chunk = &chunks[chunk_index];
            let ptr = unsafe { chunk.get_raw(item_size, offset) };
            let ptr_to_t: *mut T =
                ptr::from_raw_parts_mut(ptr as *mut (), T::ptr_metadata(self.metadata));
            unsafe {
                ptr::drop_in_place(ptr_to_t);
            }
        }

        // Deallocate each chunk
        for chunk in chunks {
            let layout = Layout::from_size_align(item_size * self.chunk_size, item_align)
                .expect("Invalid layout");
            unsafe {
                alloc::alloc::dealloc(chunk.ptr.as_ptr(), layout);
            }
        }
        self.next_index.store(0, Ordering::Release);
    }
}

impl<T: DynAlloc + ?Sized> Index<Handle<T>> for Arena<T> {
    type Output = T;

    fn index(&self, handle: Handle<T>) -> &Self::Output {
        let (chunk_index, offset) = self.split_handle(handle);
        let chunks_guard = self.chunks.read();
        let chunk = &chunks_guard[chunk_index];
        unsafe {
            chunk.get_ref(
                T::size_aligned(self.metadata),
                offset,
                T::ptr_metadata(self.metadata),
            )
        }
    }
}

impl<T: DynAlloc + ?Sized> Drop for Arena<T> {
    fn drop(&mut self) {
        let total = self.next_index.load(Ordering::Acquire) as usize;
        if total == 0 {
            return; // No objects allocated
        }

        let mut chunks_guard = self.chunks.write();
        let chunks = mem::take(&mut *chunks_guard); // Take ownership of the chunks

        if chunks.is_empty() {
            return;
        }

        let item_size = T::size_aligned(self.metadata);
        let item_align = T::ALIGN;

        // Drop each allocated object in reverse order (from last to first)
        for i in (0..total).rev() {
            let chunk_index = i / self.chunk_size;
            let offset = i % self.chunk_size;
            let chunk = &chunks[chunk_index];
            let ptr = unsafe { chunk.get_raw(item_size, offset) };
            let ptr_to_t: *mut T =
                ptr::from_raw_parts_mut(ptr as *mut (), T::ptr_metadata(self.metadata));
            unsafe {
                ptr::drop_in_place(ptr_to_t);
            }
        }

        // Deallocate each chunk
        for chunk in chunks {
            let layout = Layout::from_size_align(item_size * self.chunk_size, item_align)
                .expect("Invalid layout");
            unsafe {
                alloc::alloc::dealloc(chunk.ptr.as_ptr(), layout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr;
    use core::sync::atomic::{AtomicUsize, Ordering};

    // Simple test struct
    #[derive(Debug, PartialEq, Eq)]
    struct TestStruct {
        value: u32,
    }

    impl Default for TestStruct {
        fn default() -> Self {
            TestStruct { value: 42 }
        }
    }

    impl DynInit for TestStruct {
        type Args = u32;

        unsafe fn new_at(ptr: *mut u8, _metadata: (), args: Self::Args) {
            ptr::write(ptr as *mut Self, Self { value: args });
        }
    }

    // Struct for drop testing
    #[allow(unused)]
    struct DropTest {
        id: u32,
    }

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    impl DropTest {
        fn new(id: u32) -> Self {
            DropTest { id }
        }
    }

    impl Drop for DropTest {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Default for DropTest {
        fn default() -> Self {
            Self::new(0)
        }
    }

    impl DynInit for DropTest {
        type Args = u32;

        unsafe fn new_at(ptr: *mut u8, _metadata: (), args: Self::Args) {
            ptr::write(ptr as *mut Self, Self::new(args));
        }
    }

    #[test]
    fn basic_allocation() {
        let arena = Arena::<TestStruct>::new(2, ());
        let handle1 = arena.alloc(10);
        let handle2 = arena.alloc(20);

        assert_eq!(arena[handle1].value, 10);
        assert_eq!(arena[handle2].value, 20);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn default_initialization() {
        let arena = Arena::<TestStruct>::new(2, ());
        let handle = arena.alloc_default();

        assert_eq!(arena[handle].value, 42);
    }

    #[test]
    fn chunk_expansion() {
        let arena = Arena::<TestStruct>::new(1, ()); // Small chunk size
        let handle1 = arena.alloc(1);
        let handle2 = arena.alloc(2); // Should trigger new chunk

        assert_eq!(arena[handle1].value, 1);
        assert_eq!(arena[handle2].value, 2);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn handle_operations() {
        let arena = Arena::<TestStruct>::new(2, ());
        let handle = arena.alloc(10);
        let invalid = Handle::<TestStruct>::invalid();

        assert!(handle.is_valid());
        assert!(!invalid.is_valid());
        assert_eq!(handle.clone(), handle); // Test clone
        assert_eq!(*handle, handle.index); // Test deref
    }

    #[test]
    fn clear_operation_and_drop_arena() {
        let arena = Arena::<DropTest>::new(2, ());
        let _ = arena.alloc(1);
        let _ = arena.alloc(2);

        DROP_COUNT.store(0, Ordering::SeqCst);
        arena.clear();

        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
        assert_eq!(arena.len(), 0);

        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            let arena = Arena::<DropTest>::new(2, ());
            let _ = arena.alloc(1);
            let _ = arena.alloc(2);
        } // Arena dropped here

        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn large_allocation() {
        let arena = Arena::<TestStruct>::new(100, ());
        for i in 0..1000 {
            let handle = arena.alloc(i as u32);
            assert_eq!(arena[handle].value, i as u32);
        }
        assert_eq!(arena.len(), 1000);
    }
}

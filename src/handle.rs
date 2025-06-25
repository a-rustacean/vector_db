#![allow(unused)]

use core::{fmt, hash, marker::PhantomData, ops::Deref};

use alloc::format;

pub struct Handle<T: ?Sized> {
    index: u32,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> Handle<T> {
    pub(crate) fn new(index: u32) -> Self {
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

impl<T: ?Sized> core::hash::Hash for Handle<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
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

pub struct DoubleHandle<A: ?Sized, B: ?Sized> {
    index: u32,
    _marker_a: PhantomData<A>,
    _marker_b: PhantomData<B>,
}

impl<A: ?Sized, B: ?Sized> DoubleHandle<A, B> {
    pub(crate) fn new(index: u32) -> Self {
        Self {
            index,
            _marker_a: PhantomData,
            _marker_b: PhantomData,
        }
    }

    pub fn invalid() -> Self {
        Self::new(u32::MAX)
    }

    pub fn is_valid(&self) -> bool {
        self.index != u32::MAX
    }

    pub fn split(self) -> (HandleA<A>, HandleB<B>) {
        (HandleA::new(self.index), HandleB::new(self.index))
    }

    pub fn handle_a(self) -> HandleA<A> {
        HandleA::new(self.index)
    }

    pub fn handle_b(self) -> HandleB<B> {
        HandleB::new(self.index)
    }
}

impl<A: ?Sized, B: ?Sized> Deref for DoubleHandle<A, B> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<A: ?Sized, B: ?Sized> hash::Hash for DoubleHandle<A, B> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<A: ?Sized, B: ?Sized> PartialEq for DoubleHandle<A, B> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<A: ?Sized, B: ?Sized> Eq for DoubleHandle<A, B> {}

impl<A: ?Sized, B: ?Sized> Clone for DoubleHandle<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: ?Sized, B: ?Sized> Copy for DoubleHandle<A, B> {}

impl<A: ?Sized, B: ?Sized> fmt::Debug for DoubleHandle<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!(
            "DoubleHandle<{:?}, {:?}>",
            core::any::type_name::<A>(),
            core::any::type_name::<B>()
        ))
        .field(&self.index)
        .finish()
    }
}

pub struct HandleA<T: ?Sized> {
    index: u32,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> HandleA<T> {
    pub(crate) fn new(index: u32) -> Self {
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

    pub fn into_double<U: ?Sized>(self) -> DoubleHandle<T, U> {
        DoubleHandle::new(self.index)
    }
}

impl<T: ?Sized> Deref for HandleA<T> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<T: ?Sized> core::hash::Hash for HandleA<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T: ?Sized> PartialEq for HandleA<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for HandleA<T> {}

impl<T: ?Sized> Clone for HandleA<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for HandleA<T> {}

impl<T: ?Sized> fmt::Debug for HandleA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!("HandleA<{:?}>", core::any::type_name::<T>()))
            .field(&self.index)
            .finish()
    }
}

pub struct HandleB<T: ?Sized> {
    index: u32,
    _marker: PhantomData<T>,
}

impl<T: ?Sized> HandleB<T> {
    pub(crate) fn new(index: u32) -> Self {
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

    pub fn into_double<U: ?Sized>(self) -> DoubleHandle<T, U> {
        DoubleHandle::new(self.index)
    }
}

impl<T: ?Sized> Deref for HandleB<T> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<T: ?Sized> core::hash::Hash for HandleB<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T: ?Sized> PartialEq for HandleB<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T: ?Sized> Eq for HandleB<T> {}

impl<T: ?Sized> Clone for HandleB<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for HandleB<T> {}

impl<T: ?Sized> fmt::Debug for HandleB<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!("HandleB<{:?}>", core::any::type_name::<T>()))
            .field(&self.index)
            .finish()
    }
}

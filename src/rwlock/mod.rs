pub mod raw_mutex;
pub mod raw_rwlock;

pub type RwLock<T> = parking_lot::lock_api::RwLock<raw_rwlock::RawRwLock, T>;

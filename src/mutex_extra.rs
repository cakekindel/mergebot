use std::sync::{Mutex, MutexGuard};

/// Acquire a lock on a mutex, disregarding whether it was poisoned.
pub fn lock_discard_poison<'a, T>(m: &'a Mutex<T>) -> MutexGuard<'a, T> {
  m.lock().map_err(|e| e.into_inner()).unwrap_or_else(|lock| lock)
}

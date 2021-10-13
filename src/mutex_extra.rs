use std::sync::{Mutex, MutexGuard};

/// Acquire a lock on a mutex, disregarding whether it was poisoned.
pub fn lock_discard_poison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
  m.lock().map_err(|e| e.into_inner()).unwrap_or_else(|lock| lock)
}

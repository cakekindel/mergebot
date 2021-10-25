use std::sync::{Mutex, MutexGuard};

/// Acquire a lock on a mutex, disregarding whether it was poisoned.
pub fn lock_discard_poison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
  m.lock().map_err(|e| e.into_inner()).unwrap_or_else(|lock| lock)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Arc;

  #[test]
  fn ldp() {
    let m = Mutex::new(12);
    assert_eq!(*lock_discard_poison(&m), 12);
  }

  #[test]
  fn ldp_poison() {
    let m = Arc::new(Mutex::new(12));
    let m_copy = Arc::clone(&m);

    let _ = std::thread::spawn(move || {
      let _lock = m_copy.lock().unwrap();
      panic!();
    }).join();

    assert!(m.lock().is_err());
    assert_eq!(*lock_discard_poison(&m), 12);
  }
}

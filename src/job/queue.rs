use std::{collections::VecDeque,
          sync::{Mutex, MutexGuard}};

use super::*;

lazy_static::lazy_static! {
  /// An in-mem thread-safe job queue
  static ref QUEUE: Mutex<VecDeque<Job>> = Mutex::new(VecDeque::new());
}

/// Acquire a lock on the QUEUE static
fn queue_lock() -> MutexGuard<'static, VecDeque<Job>> {
  QUEUE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A FIFO Job queue
pub trait Queue: 'static + Sync + Send + std::fmt::Debug {
  /// Get a copy of a job in the queue with id matching `id`
  fn lookup(&self, id: &str) -> Option<Job>;

  /// Take the next job
  fn dequeue(&self) -> Option<Job>;

  /// Get a copy of the next job
  fn peek(&self) -> Option<Job>;

  /// Queue a new job, yields a copy of the created job.
  fn queue(&self, app: App, command: Command) -> Job;

  /// Update the state of a job
  fn set_state(&self, id: &str, state: State) -> Option<Job>;
}

/// In-memory implementor of the Queue trait.
///
/// Note that this is not persisted across instances of the application
#[derive(Clone, Copy, Debug)]
pub struct MemQueue;

impl Queue for MemQueue {
  fn lookup(&self, id: &str) -> Option<Job> {
    let queue = &queue_lock();
    queue.iter().find(|j| j.id == id).cloned()
  }

  fn dequeue(&self) -> Option<Job> {
    let queue = &mut queue_lock();
    queue.pop_front()
  }

  fn set_state(&self, id: &str, state: State) -> Option<Job> {
    let queue = &mut queue_lock();

    queue.iter_mut()
         .find(|j| j.id == id)
         .map(|j| {
           j.state = state;
           j
         })
         .cloned()
  }

  fn peek(&self) -> Option<Job> {
    let queue = &queue_lock();
    queue.back().cloned()
  }

  fn queue(&self, app: App, command: Command) -> Job {
    let queue = &mut queue_lock();
    let job = Job { id: nanoid::nanoid!(),
                    state: State::Initiated,
                    app,
                    command };

    queue.push_back(job.clone());

    job
  }
}

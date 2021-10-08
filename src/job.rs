use std::{collections::VecDeque,
          sync::{Mutex, MutexGuard}};

use crate::deploy::{App, Command};

/// State a job may be in
#[derive(Clone, Copy, Debug)]
pub enum State {
  /// Job was initiated and awaiting approval
  Initiated,
  /// Job has been approved but not executed (TODO: remove?)
  Approved,
}

/// A deploy job
#[derive(Clone, Debug)]
pub struct Job {
  /// Unique identifier for this job
  pub id: String,
  /// Current state of the deploy job
  pub state: State,
  /// Command issued that triggered the job
  pub command: Command,
  /// Application to deploy
  pub app: App,
}

lazy_static::lazy_static! {
  /// An in-mem thread-safe job queue
  static ref QUEUE: Mutex<VecDeque<Job>> = Mutex::new(VecDeque::new());
}

/// Acquire a lock on the QUEUE static
fn queue_lock() -> MutexGuard<'static, VecDeque<Job>> {
  QUEUE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A FIFO Job queue
pub trait Queue {
  /// Get a copy of a job in the queue with id matching `id`
  fn lookup(&self, id: impl AsRef<str>) -> Option<Job>;

  /// Take the next job
  fn dequeue(&mut self) -> Option<Job>;

  /// Get a copy of the next job
  fn peek(&self) -> Option<Job>;

  /// Queue a new job, yields a copy of the created job.
  fn queue(&mut self, app: App, command: Command) -> Job;
}

/// In-memory implementor of the Queue trait.
///
/// Note that this is not persisted across instances of the application
#[derive(Clone, Copy, Debug)]
pub struct MemQueue;

impl Queue for MemQueue {
  fn lookup(&self, id: impl AsRef<str>) -> Option<Job> {
    let queue = &queue_lock();
    queue.iter().find(|j| &j.id == id.as_ref()).cloned()
  }

  fn dequeue(&mut self) -> Option<Job> {
    let queue = &mut queue_lock();
    queue.pop_front()
  }

  fn peek(&self) -> Option<Job> {
    let queue = &queue_lock();
    queue.back().cloned()
  }

  fn queue(&mut self, app: App, command: Command) -> Job {
    let queue = &mut queue_lock();
    let job = Job { id: nanoid::nanoid!(),
                    state: State::Initiated,
                    app,
                    command };

    queue.push_back(job.clone());

    job
  }
}

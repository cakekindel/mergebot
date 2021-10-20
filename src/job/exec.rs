use job::Job;

use crate::job;

/// Implementation
pub mod r#impl;

/// Execute errors
#[derive(Copy, Clone, Debug)]
pub enum Error {}

/// Execute result
pub type Result<T> = core::result::Result<T, Error>;

/// Transition job states from "Approved" -> "Executing" and "Executing" -> "Done" | "Errored"
///
/// This Transition is queues work asynchronously.
/// As such, all errors are stored on the job rather than returned eagerly
pub trait Executor: 'static + Sync + Send + std::fmt::Debug {
  fn schedule_exec(&self, job: &Job<job::StateApproved>);
}

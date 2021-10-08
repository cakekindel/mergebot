mod queue;
pub use queue::*;

mod messaging;
pub use messaging::*;

use crate::deploy::{App, Command};

/// State a job may be in
#[derive(Clone, Debug)]
pub enum State {
  /// Job was initiated and nobody has been notified
  Initiated,
  /// Approvers have been notified
  Notified {
    /// Unique identifier for sent message
    message_ts: String,
  },
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

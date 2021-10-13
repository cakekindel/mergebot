mod queue;
pub use queue::*;

mod messaging;
pub use messaging::*;

mod exec;
pub use exec::*;

use chrono::{DateTime, Utc};

use crate::{deploy::{App, Command, User},
            slack, git};

/// State a job may be in
#[derive(Clone, Debug)]
pub enum State {
  /// Job was initiated and nobody has been notified
  Initiated,
  /// Approvers have been notified
  Notified {
    /// Unique identifier for sent message
    msg_id: slack::msg::Id,

    /// People who have approved this deploy
    approved_by: Vec<User>,
  },
  /// Job has been approved but not executed (TODO: remove?)
  Approved {
    /// Unique identifier for sent message
    msg_id: slack::msg::Id,

    /// People who have approved this deploy
    approved_by: Vec<User>,
  },
  /// Job is about to be executed
  WorkQueued,
  /// Job execution failed. Will retry.
  Errored {
    /// Number of attempts so far
    attempts: usize,
    /// Next scheduled attempt
    next_attempt: DateTime<Utc>,
    /// Errors encountered during last attempt
    errs: Vec<git::Error>,
  },
  /// Job execution failed more than 5 times. Will not retry.
  Poisoned(Vec<git::Error>),
  /// Job has been executed
  Done,
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

impl Job {
  /// Get all users who have not approved this job
  pub fn outstanding_approvers(&self) -> Vec<User> {
    let approved_by = match &self.state {
      | State::Notified { approved_by, .. } => approved_by.clone(),
      | _ => vec![],
    };
    let hasnt_approved = |u: &User| approved_by.iter().all(|a| a != u);
    let mut users = self.app
                        .repos
                        .iter()
                        .flat_map(|r| r.environments.iter().filter(|env| env.name_eq(&self.command.env_name)))
                        .flat_map(|env| env.users.clone())
                        .filter(hasnt_approved)
                        .collect::<Vec<_>>();
    users.dedup();

    users
  }
}

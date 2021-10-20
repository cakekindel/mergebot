use serde::{Deserialize as De, Serialize as Ser};

mod messaging;
pub use messaging::*;

pub mod event;
pub mod exec;
pub mod hooks;
pub mod store;

use chrono::{DateTime, Utc};
pub use store::Store;

use crate::{deploy::{App, Command, User},
            git,
            slack};

/// Errors that a job can encounter trying to deploy
#[derive(Debug, Clone, Ser, De)]
pub enum Error {
  /// Issue managing app repos
  Git(git::Error),
}

/// Job ID
#[derive(Debug, Hash, PartialOrd, PartialEq, Eq, Clone, Ser, De)]
pub struct Id(String);

impl Default for Id {
  fn default() -> Self {
    Self::new()
  }
}

impl Id {
  pub fn new() -> Self {
    Self(nanoid::nanoid!())
  }
}

impl std::ops::Deref for Id {
  type Target = String;

  fn deref(&self) -> &String {
    &self.0
  }
}

/// State a job may be in
pub trait State: std::fmt::Debug + Clone {
  fn into_states(self) -> States;
}

impl State for StateInit {
  fn into_states(self) -> States {
    States::Init(self)
  }
}
impl State for StateApproved {
  fn into_states(self) -> States {
    States::Approved(self)
  }
}
impl State for StateErrored {
  fn into_states(self) -> States {
    States::Errored(self)
  }
}
impl State for StatePoisoned {
  fn into_states(self) -> States {
    States::Poisoned(self)
  }
}
impl State for StateDone {
  fn into_states(self) -> States {
    States::Done(self)
  }
}
impl State for States {
  fn into_states(self) -> States {
    self
  }
}

/// Sum type over job states
#[derive(Debug, Clone, Ser, De)]
#[serde(tag = "type")]
pub enum States {
  /// Init
  #[serde(rename = "init")]
  Init(StateInit),
  /// Approved
  #[serde(rename = "approved")]
  Approved(StateApproved),
  /// Errored
  #[serde(rename = "errored")]
  Errored(StateErrored),
  /// Poisoned
  #[serde(rename = "poisoned")]
  Poisoned(StatePoisoned),
  /// Done
  #[serde(rename = "done")]
  Done(StateDone),
}

/// Job partially approved
#[derive(Debug, Clone, Ser, De)]
pub struct StateInit {
  /// ID of the slack notification for this deploy
  pub msg_id: Option<slack::msg::Id>,

  /// People who have approved this deploy
  pub approved_by: Vec<User>,
}

/// Job has been fully approved
#[derive(Debug, Clone, Ser, De)]
pub struct StateApproved {
  /// Previous state of the job
  pub prev: StateInit,
}

/// Deploying this job failed. Will retry.
#[derive(Debug, Clone, Ser, De)]
pub struct StateErrored {
  /// Previous state of the job
  pub prev: StateApproved,
  /// Previous errored attempts
  pub prev_attempt: Option<Box<StateErrored>>,
  /// Next scheduled attempt
  pub next_attempt: DateTime<Utc>,
  /// Errors encountered during last attempt
  pub errs: Vec<Error>,
}

/// Failed to deploy more than POISON_THRESHOLD times
#[derive(Debug, Clone, Ser, De)]
pub struct StatePoisoned {
  /// Previous error state
  pub prev: StateErrored,
}

/// Job has been executed. Includes the previous approval state,
/// and if deploy failed but eventually succeeded,
/// includes error state that triggered retry.
#[derive(Debug, Clone, Ser, De)]
pub enum StateDone {
  /// Succeeded right away
  Succeeded(StateApproved),
  /// Failed at least once, but eventually succeeded
  SucceededAfterRetry(StateErrored),
}

/// A deploy job
#[derive(Ser, De, Clone, Debug)]
pub struct Job<S: State> {
  /// Unique identifier for this job
  pub id: Id,
  /// Current state of the deploy job
  pub state: S,
  /// Command issued that triggered the job
  pub command: Command,
  /// Application to deploy
  pub app: App,
}

impl<T: State> Job<T> {
  pub fn map_state<R: State>(&self, f: impl FnOnce(T) -> R) -> Job<R> {
    Job { id: self.id.clone(),
          state: f(self.state.clone()),
          app: self.app.clone(),
          command: self.command.clone() }
  }
}

impl Job<StateInit> {
  /// Get all users who have not approved this job
  pub fn outstanding_approvers(&self) -> Vec<User> {
    let approved_by = &self.state.approved_by.clone();
    let hasnt_approved = |u: &User| approved_by.iter().all(|a| a != u);
    let mut users = self.app
                        .users(&self.command.env_name)
                        .into_iter()
                        .filter(hasnt_approved)
                        .collect::<Vec<_>>();
    users.dedup();

    users
  }
}

impl Job<StateErrored> {
  /// Recursively flatten `prev_attempt`, yielding a flat list of attempts
  pub fn flatten_errors(&self) -> Vec<StateErrored> {
    fn go(err: &Option<Box<StateErrored>>, mut errs: Vec<StateErrored>) -> Vec<StateErrored> {
      if let Some(ref err) = *err {
        errs.push(err.as_ref().clone());
        go(&err.prev_attempt, errs)
      } else {
        errs
      }
    }

    go(&Some(Box::from(self.state.clone())), vec![self.state.clone()])
  }
}

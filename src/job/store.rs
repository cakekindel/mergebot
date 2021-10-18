use serde::{Deserialize as De, Serialize as Ser};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::*;
use event::Event;

use crate::{slack, deploy, mutex_extra::lock_discard_poison};

/// Job store struct
#[derive(Ser, De)]
pub struct StoreData {
  pub created: HashMap<Id, Job<StateInit>>,
  pub approved: HashMap<Id, Job<StateApproved>>,
  pub errored: HashMap<Id, Job<StateErrored>>,
  pub poison: HashMap<Id, Job<StatePoisoned>>,
  pub done: HashMap<Id, Job<StateDone>>,
  #[serde(skip)]
  listeners: Vec<fn(Box<dyn Store>, Event)>,
}

impl std::fmt::Debug for StoreData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StoreData")
     .field("created", &self.created)
     .field("approved", &self.approved)
     .field("errored", &self.errored)
     .field("poison", &self.poison)
     .field("done", &self.done)
     .field("listeners", &self.listeners.iter().map(|_| "Fn(Store, Event)").collect::<Vec<_>>())
     .finish()
  }
}

impl StoreData {
  pub fn new() -> Self {
    Self {
      created: HashMap::new(),
      approved: HashMap::new(),
      errored: HashMap::new(),
      poison: HashMap::new(),
      done: HashMap::new(),
      listeners: Vec::new(),
    }
  }
}

/// Job store & state machine
pub trait Store: 'static + Send + Sync + std::fmt::Debug {
  /// Get a reference to the current state of the store
  fn get_store(&self) -> &StoreData;

  /// Get fresh jobs
  fn get_all_new(&self) -> Vec<Job<StateInit>>;

  /// Get all fully approved jobs
  fn get_all_approved(&self) -> Vec<Job<StateApproved>>;

  /// Get all errored jobs
  fn get_all_errored(&self) -> Vec<Job<StateErrored>>;

  /// Get all poisoned jobs
  fn get_all_poisoned(&self) -> Vec<Job<StatePoisoned>>;

  /// Get all complete jobs
  fn get_all_done(&self) -> Vec<Job<StateDone>>;

  /// Get all jobs
  fn get_all(&self) -> Vec<Job<States>> {
    fn norm<S: State>(v: Vec<Job<S>>) -> impl Iterator<Item = Job<States>> {
      v.into_iter().map(|j| j.map_state(|s| s.to_states()))
    }

    norm(self.get_all_new())
        .chain(norm(self.get_all_approved()))
        .chain(norm(self.get_all_errored()))
        .chain(norm(self.get_all_poisoned()))
        .chain(norm(self.get_all_done()))
  }

  /// Create a new job, returning the created job's id
  fn create(&self, command: deploy::Command, app: deploy::App) -> Id;

  /// Add a slack message id to a job in Init state
  fn notified(&self, job_id: &Id, msg_id: slack::msg::Id) -> Option<Id>;

  /// Mark a job as approved by a user
  fn approved(&self, job_id: &Id, user: &deploy::User) -> Option<Id>;

  /// Get a job of state Init
  fn get_new(&self, job_id: &Id) -> Option<&Job<StateInit>>;

  /// Get a job of state Approved
  fn get_approved(&self, job_id: &Id) -> Option<&Job<StateApproved>>;

  /// Get a job of state Poisoned
  fn get_poisoned(&self, job_id: &Id) -> Option<&Job<StatePoisoned>>;

  /// Get a job of state Errored
  fn get_errored(&self, job_id: &Id) -> Option<&Job<StateErrored>>;

  /// Get a job of state Done
  fn get_done(&self, job_id: &Id) -> Option<&Job<StateDone>>;

  /// Get a job of any state, converting its state from a concrete type to a polymorphic one.
  fn get(&self, job_id: &Id) -> Option<&Job<States>> {
    fn norm<S: State>(j: &Job<S>) -> &Job<States> {
      &j.map_state(|s| s.to_states())
    }

    self.get_new(&job_id)
        .map(norm)
        .or_else(|| self.get_approved(&job_id).map(norm))
        .or_else(|| self.get_errored(&job_id).map(norm))
        .or_else(|| self.get_poisoned(&job_id).map(norm))
        .or_else(|| self.get_done(&job_id).map(norm))
  }

  /// Mark a job as fully approved
  fn state_approved(&self, job_id: &Id) -> Option<Id>;

  /// Mark a job as errored
  fn state_errored(&self, job_id: &Id, errs: Vec<Error>) -> Option<Id>;

  /// Mark a job as poisoned
  fn state_poisoned(&self, job_id: &Id) -> Option<Id>;

  /// Mark a job as done
  fn state_done(&self, job_id: &Id) -> Option<Id>;

  /// Listen for events, allows mutating the store while processing with the provided &Self parameter
  fn attach_listener(&self, f: fn(Box<dyn Store>, Event) -> ());
}

impl Store for Arc<Mutex<StoreData>> {
  /// Get a reference to the current state of the store
  fn get_store(&self) -> &StoreData {
    &*lock_discard_poison(&*self.clone())
  }

  /// Add a slack message id to a job in Init state
  fn notified(&self, job_id: &Id, msg_id: &slack::msg::Id) -> Option<Id> {todo!()}

  /// Create a new job, returning the created job's id
  fn create(&self, app: deploy::App, command: deploy::Command) -> Id {todo!()}

  /// Mark a job as approved by a user
  fn approved(&self, job_id: &Id, user: &deploy::User) -> Option<Id> {todo!()}

  /// Get a job of state Init
  fn get_new(&self, job_id: &Id) -> Option<&Job<StateInit>> {todo!()}

  /// Get a job of state Approved
  fn get_approved(&self, job_id: &Id) -> Option<&Job<StateApproved>> {todo!()}

  /// Get a job of state Poisoned
  fn get_poisoned(&self, job_id: &Id) -> Option<&Job<StatePoisoned>> {todo!()}

  /// Get a job of state Errored
  fn get_errored(&self, job_id: &Id) -> Option<&Job<StateErrored>> {todo!()}

  /// Get a job of state Done
  fn get_done(&self, job_id: &Id) -> Option<&Job<StateDone>> {todo!()}

  /// Mark a job as fully approved
  fn state_approved(&self, job_id: &Id) -> Option<Id> {todo!()}

  /// Mark a job as errored
  fn state_errored(&self, job_id: &Id, errs: Vec<Error>) -> Option<Id> {todo!()}

  /// Mark a job as poisoned
  fn state_poisoned(&self, job_id: &Id) -> Option<Id> {todo!()}

  /// Mark a job as done
  fn state_done(&self, job_id: &Id) -> Option<Id> {todo!()}

  /// Listen for events, allows mutating the store while processing with the provided &Self parameter
  fn attach_listener(&self, f: fn(Box<dyn Store>, Event) -> ()) {todo!()}
}

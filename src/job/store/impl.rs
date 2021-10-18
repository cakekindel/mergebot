use serde::{Deserialize as De, Serialize as Ser};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use nanoid as _;

use super::*;
use event::Event;

use crate::{slack, deploy, mutex_extra::lock_discard_poison};

/// Job store data
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

trait Open<T> {
  fn open(&self) -> MutexGuard<'_, T>;
}

impl<T> Open<T> for Arc<Mutex<T>> {
  fn open(&self) -> MutexGuard<'_, T> {
    lock_discard_poison(&*self)
  }
}

impl super::Store for Arc<Mutex<StoreData>> {
  /// Add a slack message id to a job in Init state
  fn notified(&self, job_id: &Id, msg_id: slack::msg::Id) -> Option<Id> {todo!()}

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
  fn state_poisoned(&self, job_id: &Id) -> Option<Id> {
    let mut store = self.open();
    let prev = store
                 .errored
                 .remove(job_id)
                 .map(|j| j.map_state(|prev| StatePoisoned {prev}));
  }

  /// Mark a job as done
  fn state_done(&self, job_id: &Id) -> Option<Id> {
    let mut store = self.open();
    let retried = store
              .errored
              .remove(job_id)
              .map(|j| j.map_state(StateDone::SucceededAfterRetry));
    let succeeded = store.approved.remove(job_id).map(|j| j.map_state(StateDone::Succeeded));

    if let Some(job) = succeeded.or(retried) {
      store.done.insert(job_id.clone(), job);
      Some(job_id.clone())
    } else {
      None
    }
  }

  /// Listen for events, allows mutating the store while processing with the provided &Self parameter
  fn attach_listener(&self, f: fn(Box<dyn Store>, Event) -> ()) {
    self.open().listeners.push(f)
  }

  /// Get fresh jobs
  fn get_all_new(&self) -> Vec<Job<StateInit>> {
    self.open().created.values().cloned().collect()
  }

  /// Get all fully approved jobs
  fn get_all_approved(&self) -> Vec<Job<StateApproved>> {
    self.open().approved.values().cloned().collect()
  }

  /// Get all errored jobs
  fn get_all_errored(&self) -> Vec<Job<StateErrored>> {
    self.open().errored.values().cloned().collect()
  }

  /// Get all poisoned jobs
  fn get_all_poisoned(&self) -> Vec<Job<StatePoisoned>> {
    self.open().poison.values().cloned().collect()
  }

  /// Get all complete jobs
  fn get_all_done(&self) -> Vec<Job<StateDone>> {
    self.open().done.values().cloned().collect()
  }
}

use std::{collections::HashMap,
          sync::{Arc, Mutex, MutexGuard}};

use event::{Event, Listener};
use serde::{Deserialize as De, Serialize as Ser};

use super::*;
use crate::{deploy, mutex_extra::lock_discard_poison, slack};

lazy_static::lazy_static! {
  static ref LISTENERS: Mutex<Vec<Listener>> = Mutex::new(Vec::new());
}

/// Job store data
#[derive(Ser, De, Debug, Clone)]
pub struct StoreData {
  pub created: HashMap<Id, Job<StateInit>>,
  pub approved: HashMap<Id, Job<StateApproved>>,
  pub errored: HashMap<Id, Job<StateErrored>>,
  pub poison: HashMap<Id, Job<StatePoisoned>>,
  pub done: HashMap<Id, Job<StateDone>>,
}

impl Default for StoreData {
  fn default() -> Self {
    Self::new()
  }
}

impl StoreData {
  pub fn new() -> Self {
    Self { created: HashMap::new(),
           approved: HashMap::new(),
           errored: HashMap::new(),
           poison: HashMap::new(),
           done: HashMap::new() }
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

impl<T> Open<T> for Mutex<T> {
  fn open(&self) -> MutexGuard<'_, T> {
    lock_discard_poison(&self)
  }
}

trait EmitEvent {
  fn emit(&self, lock: MutexGuard<'_, StoreData>, ev: Event);
}

impl EmitEvent for Arc<Mutex<StoreData>> {
  fn emit(&self, lock: MutexGuard<'_, StoreData>, ev: Event) {
    drop(lock);

    LISTENERS.open().iter().for_each(|f| f(ev));
  }
}

impl super::Store for Arc<Mutex<StoreData>> {
  /// Add a slack message id to a job in Init state
  fn notified(&self, job_id: &Id, msg_id: slack::msg::Id) -> Option<Id> {
    self.open().created.get_mut(job_id).map(|j| {
                                         j.state.msg_id = Some(msg_id);
                                         j.id.clone()
                                       })
  }

  /// Create a new job, returning the created job's id
  fn create(&self, app: deploy::App, command: deploy::Command) -> Id {
    let job = Job { id: Id::new(),
                    state: StateInit { approved_by: vec![],
                                       msg_id: None },
                    command,
                    app };

    let mut store = self.open();
    store.created.insert(job.id.clone(), job.clone());
    self.emit(store, Event::Created(&job));

    job.id
  }

  /// Mark a job as approved by a user
  fn approved(&self, job_id: &Id, user: deploy::User) -> Option<Id> {
    let mut state = self.open();

    let job = state.created.get_mut(job_id).map(|j| {
                                             j.state.approved_by.push(user.clone());
                                             j.clone()
                                           });

    if let Some(job) = job {
      self.emit(state, Event::Approved(&job, &user));
      Some(job.id)
    } else {
      None
    }
  }

  /// Get a job of state Init
  fn get_new(&self, job_id: &Id) -> Option<Job<StateInit>> {
    self.open().created.get(job_id).cloned()
  }

  /// Get a job of state Approved
  fn get_approved(&self, job_id: &Id) -> Option<Job<StateApproved>> {
    self.open().approved.get(job_id).cloned()
  }

  /// Get a job of state Poisoned
  fn get_poisoned(&self, job_id: &Id) -> Option<Job<StatePoisoned>> {
    self.open().poison.get(job_id).cloned()
  }

  /// Get a job of state Errored
  fn get_errored(&self, job_id: &Id) -> Option<Job<StateErrored>> {
    self.open().errored.get(job_id).cloned()
  }

  /// Get a job of state Done
  fn get_done(&self, job_id: &Id) -> Option<Job<StateDone>> {
    self.open().done.get(job_id).cloned()
  }

  /// Mark a job as fully approved
  fn fully_approved(&self, job_id: &Id) -> Option<Id> {
    let mut state = self.open();

    let job = state.created
                   .remove(job_id)
                   .map(|j| j.map_state(|s| StateApproved { prev: s }));

    if let Some(j) = job {
      state.approved.insert(job_id.clone(), j.clone());
      self.emit(state, Event::FullyApproved(&j));
      Some(job_id.clone())
    } else {
      None
    }
  }

  /// Mark a job as errored
  fn state_errored(&self, job_id: &Id, errs: Vec<Error>) -> Option<Id> {
    use chrono::Duration as Dur;

    let mut store = self.open();
    let next_attempt = Utc::now() + Dur::seconds(10);

    let errored = store.errored.remove(job_id).map(|j| {
                                                j.map_state(|e| StateErrored { prev: e.prev.clone(),
                                                                               prev_attempt: Some(Box::from(e)),
                                                                               next_attempt,
                                                                               errs: errs.clone() })
                                              });

    let approved = store.approved.remove(job_id).map(|j| {
                                                  j.map_state(|a| StateErrored { prev: a,
                                                                                 prev_attempt: None,
                                                                                 next_attempt,
                                                                                 errs })
                                                });

    if let Some(j) = errored.or(approved) {
      let errs = j.flatten_errors();

      if errs.len() > 4 {
        log::error!("job {:?} poisoned!!1", j.id);
        drop(store);
        self.state_poisoned(&j.id);
      } else {
        store.errored.insert(job_id.clone(), j.clone());
        self.emit(store, Event::Errored(&j));
      }

      Some(job_id.clone())
    } else {
      None
    }
  }

  /// Mark a job as poisoned
  fn state_poisoned(&self, job_id: &Id) -> Option<Id> {
    let mut store = self.open();
    let job = store.errored
                   .remove(job_id)
                   .map(|j| j.map_state(|prev| StatePoisoned { prev }));

    if let Some(j) = job {
      store.poison.insert(job_id.clone(), j.clone());
      self.emit(store, Event::Poisoned(&j));
      Some(job_id.clone())
    } else {
      None
    }
  }

  /// Mark a job as done
  fn state_done(&self, job_id: &Id) -> Option<Id> {
    let mut store = self.open();
    let retried = store.errored
                       .remove(job_id)
                       .map(|j| j.map_state(StateDone::SucceededAfterRetry));
    let succeeded = store.approved.remove(job_id).map(|j| j.map_state(StateDone::Succeeded));

    if let Some(job) = succeeded.or(retried) {
      store.done.insert(job_id.clone(), job.clone());
      self.emit(store, Event::Done(&job));
      Some(job_id.clone())
    } else {
      None
    }
  }

  /// Listen for events, allows mutating the store while processing with the provided &Self parameter
  fn attach_listener(&self, f: Listener) {
    LISTENERS.open().push(f)
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

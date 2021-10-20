use super::*;

/// A closure that does stuff when an event is fired
pub type Listener = Box<dyn for<'a> Fn(&'a dyn Store, Event<'a>) + Send + Sync>;

/// Events available to listen for
#[derive(Copy, Clone, Debug)]
pub enum Event<'a> {
  /// Job created
  Created(&'a Job<StateInit>),
  /// Job approved
  Approved(&'a Job<StateInit>, &'a crate::deploy::User),
  /// Job fully approved
  FullyApproved(&'a Job<StateApproved>),
  /// Job errored
  Errored(&'a Job<StateErrored>),
  /// Job poisoned
  Poisoned(&'a Job<StatePoisoned>),
  /// Job complete
  Done(&'a Job<StateDone>),
}

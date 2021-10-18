use super::*;
use crate::deploy;

/// Events available to listen for
#[derive(Clone, Debug)]
pub enum Event<'a> {
  /// Job created
  Created(&'a Job<StateInit>),
  /// Job approved
  Approved(&'a Job<StateInit>),
  /// Job fully approved
  FullyApproved(&'a Job<StateApproved>),
  /// Job errored
  Errored(&'a Job<StateErrored>),
  /// Job poisoned
  Poisoned(&'a Job<StatePoisoned>),
  /// Job complete
  Done(&'a Job<StateDone>),
}

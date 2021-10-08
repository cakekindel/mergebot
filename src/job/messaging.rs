use super::*;

/// Implements Messenger for slack
#[derive(Clone, Copy, Debug)]
pub struct SlackMessenger;

/// A messenger is able to notify the approvers of an app of a deployment
pub trait Messenger {
  /// Notify approvers of an app for deployment
  fn send_message_for_job(&self, job: &Job) -> Result<(), ()>;
}

impl Messenger for SlackMessenger {
  fn send_message_for_job(&self, job: &Job) -> Result<(), ()> {
    Ok(())
  }
}

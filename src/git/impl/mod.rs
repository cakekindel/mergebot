use crate::mutex_extra::lock_discard_poison;
use std::{path::{PathBuf},};

mod client;
mod repo_context;

pub use client::*;
use repo_context::*;

pub fn init(git_client_homedir: impl Into<PathBuf>) {
  let mut loq = lock_discard_poison(&client::GIT_CLIENT);
  *loq = Some(client::LocalClient::new(git_client_homedir));
}

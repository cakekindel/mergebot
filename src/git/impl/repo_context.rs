use std::{sync::{Mutex, MutexGuard}};

use git::{r#impl::LocalClient, Branch, Error, Output};


use crate::{git};

pub(super) struct RepoContext<'a> {
  lock: MutexGuard<'a, Option<LocalClient>>,
  current_branch: Mutex<Option<Branch>>, // wrap in mutex for interior mutability while preserving Sync impl
}

impl<'a> RepoContext<'a> {
  pub(super) fn new(lock: MutexGuard<'a, Option<LocalClient>>) -> Self {
    let current_branch = Mutex::new(None);
    Self { lock, current_branch }
  }

  fn client<T>(&self, f: impl FnOnce(&LocalClient) -> T) -> T {
    self.lock.as_ref().map(f).unwrap()
  }
}

impl<'a> git::RepoContext for RepoContext<'a> {
  fn upstream(&self, branch: &Branch) -> git::Result<Branch> {
    let config_entry = format!("branch.{}.remote", branch.0);
    self.client(|c| {
          c.git(&["config", "--get", &config_entry])
           .map(|Output(remote)| Branch(format!("{}/{}", remote, branch.0)))
        })
  }

  fn merge(&self, target: &Branch) -> git::Result<()> {
    self.client(|c| c.git(&["merge", &target.0, "--message", "chore: mergebot deploy"]))
        .map(|_| ())
  }

  fn switch(&self, branch: &Branch) -> git::Result<()> {
    self.client(|c| {
          let res = c.git(&["switch", &branch.0]);
          if res.is_ok() {
            let mut cur_branch = self.current_branch.lock().unwrap();
            *cur_branch = Some(branch.clone());
          }
          res
        })
        .map(|_| ())
  }

  fn push(&self) -> git::Result<()> {
    self.client(|c| c.git(&["push", "--no-verify", "--force"])).map(|_| ())
  }

  fn update_branch(&self) -> git::Result<()> {
    let cur_branch = self.current_branch.lock().unwrap();

    // reset --hard, we don't care about merging the upstream into our local
    cur_branch.as_ref()
              .ok_or(Error::NoBranchToUpdate)
              .and_then(|b| self.upstream(b))
              .and_then(|up| self.client(|c| c.git(&["reset", &up.0, "--hard"])))
              .map(|_| ())
  }

  fn fetch_all(&self) -> git::Result<()> {
    self.client(|c| c.git(&["fetch", "--all"])).map(|_| ())
  }
}

impl<'a> Drop for RepoContext<'a> {
  fn drop(&mut self) {
    let homedir = self.client(|c| c.homedir.clone());
    self.client(|c| c.cd(homedir));
  }
}

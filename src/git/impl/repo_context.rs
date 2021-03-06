use std::sync::{Mutex, MutexGuard};

use git::{r#impl::LocalClient, Branch, Error, Output};

use crate::{git, mutex_extra::lock_discard_poison, result_extra::ResultExtra};

pub(super) struct RepoContext<'a> {
  log_prefix: String,
  lock: MutexGuard<'a, Option<LocalClient>>,
  current_branch: Mutex<Option<Branch>>, // wrap in mutex for interior mutability while preserving Sync impl
}

impl<'a> RepoContext<'a> {
  pub(super) fn new(log_prefix: String, lock: MutexGuard<'a, Option<LocalClient>>) -> Self {
    let current_branch = Mutex::new(None);
    Self { log_prefix,
           lock,
           current_branch }
  }

  fn cur_branch(&self) -> Option<Branch> {
    lock_discard_poison(&self.current_branch).clone()
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
           .map(|Output(remote)| remote.strip_suffix('\n').map(String::from).unwrap_or(remote))
           .map(|remote| Branch(format!("{}/{}", remote, branch.0)))
        })
        .tap(|ok| log::info!("{}(upstream) {:?}", self.log_prefix, ok))
        .tap_err(|err| log::error!("{}(upstream) {:?}", self.log_prefix, err))
  }

  fn merge(&self, target: &Branch) -> git::Result<()> {
    self.client(|c| c.git(&["merge", &target.0, "--message", "chore: mergebot deploy"]))
        .tap(|ok| {
          log::info!("{}(merge {:?} -> {:?}) {:?}",
                     self.log_prefix,
                     target,
                     self.cur_branch(),
                     ok)
        })
        .tap_err(|err| {
          log::error!("{}(merge {:?} -> {:?}) {:?}",
                      self.log_prefix,
                      target,
                      self.cur_branch(),
                      err)
        })
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
        .tap(|ok| {
          log::info!("{}(switch {:?} -> {:?}) {:?}",
                     self.log_prefix,
                     self.cur_branch().unwrap_or("unset".into()),
                     branch,
                     ok)
        })
        .tap_err(|err| {
          log::error!("{}(switch {:?} -> {:?}) {:?}",
                      self.log_prefix,
                      self.cur_branch().unwrap_or("unset".into()),
                      branch,
                      err)
        })
        .map(|_| ())
  }

  fn push(&self) -> git::Result<()> {
    self.client(|c| c.git(&["push", "--no-verify", "--force"]))
        .map(|_| ())
        .tap(|ok| log::info!("{}(push {:?}) {:?}", self.log_prefix, self.cur_branch(), ok))
        .tap_err(|err| log::error!("{}(push {:?}) {:?}", self.log_prefix, self.cur_branch(), err))
  }

  fn update_branch(&self) -> git::Result<()> {
    let cur_branch = self.cur_branch();

    // reset --hard, we don't care about merging the upstream into our local
    cur_branch.as_ref()
              .ok_or(Error::NoBranchToUpdate)
              .and_then(|b| self.upstream(b))
              .and_then(|up| self.client(|c| c.git(&["reset", &up.0, "--hard"])))
              .tap(|ok| log::info!("{}(update_branch {:?}) {:?}", self.log_prefix, self.cur_branch(), ok))
              .tap_err(|err| log::error!("{}(update_branch {:?}) {:?}", self.log_prefix, self.cur_branch(), err))
              .map(|_| ())
  }

  fn fetch_all(&self) -> git::Result<()> {
    self.client(|c| c.git(&["fetch", "--all"]))
        .tap(|ok| log::info!("`{}(fetch_all) {:?}", self.log_prefix, ok))
        .tap_err(|err| log::error!("`{}(fetch_all) {:?}", self.log_prefix, err))
        .map(|_| ())
  }
}

impl<'a> Drop for RepoContext<'a> {
  fn drop(&mut self) {
    let homedir = self.client(|c| c.homedir.clone());
    self.client(|c| c.cd(homedir));
  }
}

use std::{path::{Path, PathBuf},
          process::Command,
          sync::{Mutex, MutexGuard}};

use serde::{Deserialize as De, Serialize as Ser};

use crate::{result_extra::ResultExtra, mutex_extra::lock_discard_poison};

lazy_static::lazy_static! {
  /// A mutex capturing the exclusivity of using git on the hosted system.
  ///
  /// If one thread has a lock on this mutex,
  /// other threads need to wait for the lock to release
  /// before interacting with git.
  pub static ref GIT_CLIENT: Mutex<Option<LocalClient>> = Mutex::new(None);
}

/// A git branch
#[derive(PartialEq, Clone, Debug, Ser, De)]
pub struct Branch(pub String);

/// Some raw command output (stdout or stderr)
#[derive(PartialEq, Clone, Debug, Ser, De)]
pub struct Output(String);

impl Output {
  fn from_bytes(b: impl AsRef<[u8]>) -> Self {
    Self(String::from_utf8_lossy(b.as_ref()).to_string())
  }
}

/// Git errors
#[derive(PartialEq, Clone, Debug)]
pub enum Error {
  /// Couldn't spawn git command
  CouldNotSpawnGit(String),
  /// Git command exited not OK with this message
  CommandFailed(Output),
  /// update_branch called before switch
  NoBranchToUpdate,
  /// Other
  Other(String),
}

/// Git result
pub type Result<T> = core::result::Result<T, self::Error>;

/// Hard implementor of Client trait
#[derive(Debug)]
pub struct LocalClient {
  /// Directory that will contain the cloned repos and local git histories
  homedir: PathBuf,
  /// Current directory
  workdir: Mutex<PathBuf>,
}

/// TODO
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct StaticClient;

impl LocalClient {
  /// Create a new LocalClient
  pub fn new(homedir: impl Into<PathBuf>) -> Self {
    let homedir = homedir.into();
    let workdir = Mutex::new(homedir.clone());

    Self { homedir, workdir }
  }

  fn cd(&self, new_path: impl Into<PathBuf>) -> () {
    *lock_discard_poison(&self.workdir) = new_path.into();
  }

  fn clone(&self, url: impl AsRef<str>, dirname: impl AsRef<Path>) -> Result<PathBuf> {
    let workdir: PathBuf = lock_discard_poison(&self.workdir).to_path_buf();
    self.git(&["clone", url.as_ref(), dirname.as_ref().to_string_lossy().as_ref()])
        .map(|_| workdir.join(dirname))
        .and_then_err(|e| match &e {
          | &Error::CommandFailed(Output(ref msg)) => {
            msg.strip_prefix("fatal: destination path \'")
               .and_then(|msg| msg.strip_suffix("\' already exists and is not an empty directory."))
               .map(|dirname| Ok(workdir.join(dirname)))
               .unwrap_or(Err(e))
          },
          | _ => Err(e),
        })
  }

  fn git(&self, args: &[&str]) -> Result<Output> {
    Command::new("git").current_dir(&*lock_discard_poison(&self.workdir))
                       .args(args)
                       .output()
                       .map_err(|e| format!("{:#?}", e))
                       .map_err(Error::CouldNotSpawnGit)
                       .filter(|out| out.status.success(),
                               |out| Error::CommandFailed(Output::from_bytes(out.stderr)))
                       .map(|out| Output::from_bytes(out.stdout))
  }
}

/// A rust trait encapsulating some git functionality
pub trait Client: 'static + Sync + Send + std::fmt::Debug {
  /// Switch repo context.
  /// This will block until any existing repo context is dropped.
  fn repo(&self, url: &str, dirname: &str) -> self::Result<Box<dyn RepoContext>>;
}

impl Client for StaticClient {
  fn repo<'a>(&'a self, url: &str, dirname: &str) -> self::Result<Box<dyn RepoContext>> {
    let mut lock = GIT_CLIENT.lock().map_err(|p| p.into_inner()).unwrap_or_else(|e| e);

    {
      let git =
      lock.as_mut()
            .expect("was initialized");
        git
            .clone(url, dirname)
            .map(|dir| git.cd(dir))
    }.map(|_| Context::new(lock))
    .map(|c| Box::from(c) as Box<dyn RepoContext>)
  }
}

struct Context<'a> {
  lock: MutexGuard<'a, Option<LocalClient>>,
  current_branch: Mutex<Option<Branch>>, // wrap in mutex for interior mutability while preserving Sync impl
}

impl<'a> Context<'a> {
  fn new(lock: MutexGuard<'a, Option<LocalClient>>) -> Self {
    let current_branch = Mutex::new(None);
    Self {lock, current_branch}
  }

  fn client<T>(&self, f: impl FnOnce(&LocalClient) -> T) -> T {
    self.lock.as_ref().map(f).unwrap()
  }
}

/// A git repo context
pub trait RepoContext {
  /// Get the name of a branch's upstream
  fn upstream(&self, branch: &Branch) -> self::Result<Branch>;
  /// Merge a target branch into current
  fn merge(&self, target: &Branch) -> self::Result<()>;
  /// Change current branch
  fn switch(&self, branch: &Branch) -> self::Result<()>;
  /// Push any changes to upstream
  fn push(&self) -> self::Result<()>;
  /// Set HEAD to point to the remote
  fn update_branch(&self) -> self::Result<()>;
  /// Pull any untracked upstream branches
  fn fetch_all(&self) -> self::Result<()>;
}

impl<'a> RepoContext for Context<'a> {
  fn upstream(&self, branch: &Branch) -> Result<Branch> {
    let config_entry = format!("branch.{}.remote", branch.0);
    self.client(|c| {
      c.git(&["config", "--get", &config_entry])
       .map(|Output(remote)| Branch(format!("{}/{}", remote, branch.0)))
    })
  }

  fn merge(&self, target: &Branch) -> Result<()> {
    self.client(|c| {
      c.git(&["merge", &target.0, "--message", "chore: mergebot deploy"])
    }).map(|_| ())
  }

  fn switch(&self, branch: &Branch) -> Result<()> {
    self.client(|c| {
      let res = c.git(&["switch", &branch.0]);
      if let Ok(_) = res {
        let mut cur_branch = self.current_branch.lock().unwrap();
        *cur_branch = Some(branch.clone());
      }
      res
    }).map(|_| ())
  }

  fn push(&self) -> Result<()> {
    self.client(|c| c.git(&["push", "--no-verify", "--force"])).map(|_| ())
  }

  fn update_branch(&self) -> Result<()> {
    let cur_branch = self.current_branch.lock().unwrap();

    // reset --hard, we don't care about merging the upstream into our local
    cur_branch
      .as_ref()
      .ok_or(Error::NoBranchToUpdate)
      .and_then(|b| self.upstream(b))
      .and_then(|up| self.client(|c| c.git(&["reset", &up.0, "--hard"])))
      .map(|_| ())
  }

  fn fetch_all(&self) -> Result<()> {
    self.client(|c| c.git(&["fetch", "--all"])).map(|_| ())
  }
}

impl<'a> Drop for Context<'a> {
  fn drop(&mut self) {
    let homedir = self.client(|c| c.homedir.clone());
    self.client(|c| c.cd(homedir));
  }
}

use std::{io,
          path::{Path, PathBuf},
          process::Command,
          sync::{Mutex, MutexGuard}};

use serde::{Deserialize as De, Serialize as Ser};

use crate::result_extra::ResultExtra;

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
  /// Git hasn't been initialized
  NotInitialized,
  /// Other
  Other(String),
}

/// Git result
pub type Result<T> = core::result::Result<T, self::Error>;

/// Hard implementor of Client trait
#[derive(PartialEq, Clone, Debug)]
pub struct LocalClient {
  /// Directory that will contain the cloned repos and local git histories
  homedir: PathBuf,
  /// Current directory
  workdir: PathBuf,
}

/// TODO
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct StaticClient;

impl LocalClient {
  /// Create a new LocalClient
  pub fn new(homedir: impl Into<PathBuf>) -> Self {
    let homedir = homedir.into();
    let workdir = homedir.clone();

    Self { homedir, workdir }
  }

  fn cd(&mut self, new_path: impl Into<PathBuf>) -> () {
    self.workdir = new_path.into();
  }

  fn clone(&self, url: impl AsRef<str>, dirname: impl AsRef<Path>) -> Result<PathBuf> {
    self.git(&["clone", url.as_ref(), dirname.as_ref().to_string_lossy().as_ref()])
        .map(|_| self.workdir.join(dirname))
        .and_then_err(|e| match &e {
          | &Error::CommandFailed(Output(ref msg)) => {
            msg.strip_prefix("fatal: destination path \'")
               .and_then(|msg| msg.strip_suffix("\' already exists and is not an empty directory."))
               .map(|dirname| Ok(self.workdir.join(dirname)))
               .unwrap_or(Err(e))
          },
          | _ => Err(e),
        })
  }

  fn git(&self, args: &[&str]) -> Result<Output> {
    Command::new("git").current_dir(&self.workdir)
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
pub trait Client {
  /// Switch repo context.
  /// This will block until any existing repo context is dropped.
  fn repo(&mut self, url: &str, dirname: &str) -> self::Result<Box<dyn RepoContext>>;
}

impl Client for StaticClient {
  fn repo<'a>(&'a mut self, url: &str, dirname: &str) -> self::Result<Box<dyn RepoContext>> {
    let mut lock = GIT_CLIENT.lock().map_err(|p| p.into_inner()).unwrap_or_else(|e| e);

    {
      lock.as_mut()
            .ok_or(Error::NotInitialized)
            .and_then(|c| c.clone(url, dirname).map(|dir| c.cd(dir)))
    }.map(|_| Context { lock })
    .map(|c| Box::from(c) as Box<dyn RepoContext>)
  }
}

struct Context<'a> {
  lock: MutexGuard<'a, Option<LocalClient>>,
}

impl<'a> Context<'a> {
  fn client<T>(&mut self, f: impl FnOnce(&mut LocalClient) -> T) -> T {
    self.lock.as_mut().map(f).unwrap()
  }
}

/// A git repo context
pub trait RepoContext {
  /// Merge a target branch into current
  fn merge(&self, target: &Branch) -> self::Result<()>;
  /// Change current branch
  fn switch(&self, branch: &Branch) -> self::Result<()>;
  /// Push any changes to upstream
  fn push(&self) -> self::Result<()>;
  /// Pull any upstream changes into current branch
  fn pull(&self) -> self::Result<()>;
}

impl<'a> Drop for Context<'a> {
  fn drop(&mut self) {
    let homedir = self.client(|c| c.homedir.clone());
    self.client(|c| c.cd(homedir));
  }
}

use serde::{Deserialize as De, Serialize as Ser};
use std::sync::{Mutex, MutexGuard};

static GIT_CONTEXT: Mutex<()> = Mutex::new(());

/// A git branch
#[derive(PartialEq, Clone, Debug, Ser, De)]
pub struct Branch(pub String);

/// Git errors
pub enum Error {
  /// Other
  Other(String),
}

/// Git result
pub type Result<T> = core::result::Result<T, self::Error>;

/// Hard implementor of Client trait
pub struct LocalClient {
  /// Directory that will contain the cloned repos and local git histories
  pub homedir: String,
  /// Current directory
  workdir: String,
}

impl LocalClient {
  fn cd(&mut self, new_path: impl ToString) -> () {
    self.workdir = new_path.to_string();
  }

  fn repo_dir(&self, url: impl AsRef<str>) -> Option<String> {
    // ls self.homedir
    // for each repo, `git remote origin`
    // urls eq?
    todo!()
  }

  fn git(&self, args: &[&str]) -> Result<()> {
    todo!()
  }
}

/// A rust trait encapsulating some git functionality
pub trait Client {
  /// Switch repo context.
  /// This will block until any existing repo context is dropped.
  fn repo(&mut self, url: &str) -> self::Result<Context>;
}

impl Client for LocalClient {
  fn repo(&mut self, url: &str) -> self::Result<Context> {
    let lock = || GIT_CONTEXT.lock().map_err(|p| p.into_inner()).unwrap_or_else(|e| e);

    self.repo_dir(url)
        .map(Ok)
        .unwrap_or_else(|| self.clone(url))
        .map(|dir| self.cd(dir))
        .map(|_| Context {lock: lock()})
  }
}

/// A git repo context
pub struct Context {lock: MutexGuard<'static, ()>}

impl Context {
  /// Merge a target branch into current
  pub fn merge(self, target: &Branch) -> self::Result<Self>;
  /// Change current branch
  pub fn switch(self, branch: &Branch) -> self::Result<Self>;
  /// Push any changes to upstream
  pub fn push(self) -> self::Result<Self>;
  /// Pull any upstream changes into current branch
  pub fn pull(self) -> self::Result<Self>;
}

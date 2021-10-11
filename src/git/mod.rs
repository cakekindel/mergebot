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

/// A rust trait encapsulating some git functionality
pub trait Client {
  /// Switch repo context
  fn repo(&self, url: &str) -> self::Result<Context>;
}

impl Client for LocalClient {
  fn repo(&self, url: &str) -> self::Result<Context> {
    let dir = self.cloned(url)
                  .unwrap_or_else(|| self.git("clone", &[url]));
    self.cd(dir);
    Ok(Context {lock: GIT_CONTEXT.lock()})
  }
}

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

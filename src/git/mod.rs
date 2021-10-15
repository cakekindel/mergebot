use serde::{Deserialize as De, Serialize as Ser};

pub mod r#impl;

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
#[derive(Ser, De, PartialEq, Clone, Debug)]
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

/// A git client
pub trait Client: 'static + Sync + Send + std::fmt::Debug {
  /// Switch repo context.
  /// This will block until any existing repo context is dropped.
  fn repo(&self, url: &str, dirname: &str) -> self::Result<Box<dyn RepoContext>>;
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

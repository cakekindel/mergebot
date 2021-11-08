use std::{path::{Path, PathBuf},
          process::Command,
          sync::Mutex};

use crate::{git,
            git::{Error, Output},
            mutex_extra::lock_discard_poison,
            result_extra::ResultExtra};

lazy_static::lazy_static! {
  /// A mutex capturing the exclusivity of using git on the hosted system.
  ///
  /// If one thread has a lock on this mutex,
  /// other threads need to wait for the lock to release
  /// before interacting with git.
  pub(super) static ref GIT_CLIENT: Mutex<Option<LocalClient>> = Mutex::new(None);
}

/// A wrapper around a git client running on the local machine
#[derive(Debug)]
pub(super) struct LocalClient {
  /// Directory that will contain the cloned repos and local git histories
  pub(super) homedir: PathBuf,
  /// Current directory
  pub(super) workdir: Mutex<PathBuf>,
}

/// A long-living instance of a git client running on the local machine
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct StaticClient;

impl LocalClient {
  /// Create a new LocalClient
  pub(super) fn new(homedir: impl Into<PathBuf>) -> Self {
    let homedir = homedir.into();
    let workdir = Mutex::new(homedir.clone());

    Self { homedir, workdir }
  }

  pub(super) fn cd(&self, new_path: impl Into<PathBuf>) {
    *lock_discard_poison(&self.workdir) = new_path.into();
  }

  fn clone(&self, url: impl AsRef<str>, dirname: impl AsRef<Path>) -> git::Result<PathBuf> {
    let workdir: PathBuf = lock_discard_poison(&self.workdir).to_path_buf();
    self.git(&["clone", url.as_ref(), dirname.as_ref().to_string_lossy().as_ref()])
        .map(|_| workdir.join(dirname))
        .and_then_err(|e| match &e {
          | &Error::CommandFailed(_, Output(ref msg)) => {
            msg.strip_prefix("fatal: destination path \'")
               .and_then(|msg| msg.strip_suffix("\' already exists and is not an empty directory.\n"))
               .map(|dirname| Ok(workdir.join(dirname)))
               .unwrap_or(Err(e))
          },
          | _ => Err(e),
        })
        .tap(|ok| log::info!("cloned repo to {}", ok.to_string_lossy()))
        .tap_err(|err| log::error!("clone failed: {:?}", err))
  }

  pub(super) fn git(&self, args: &[&str]) -> git::Result<Output> {
    let command = format!("git {}", args.join(" "));

    log::info!("executing `{}`", command);

    Command::new("git").current_dir(&*lock_discard_poison(&self.workdir))
                       .args(args)
                       .output()
                       .map_err(|e| format!("{:#?}", e))
                       .map_err(Error::CouldNotSpawnGit)
                       .filter(|out| out.status.success(),
                               |out| Error::CommandFailed(command.clone(), Output::from_bytes(out.stderr)))
                       .map(|out| Output::from_bytes(out.stdout))
  }
}

impl git::Client for StaticClient {
  fn repo<'a>(&'a self, url: &str, dirname: &str) -> git::Result<Box<dyn git::RepoContext>> {
    let lock = lock_discard_poison(&GIT_CLIENT);
    let git = lock.as_ref().unwrap();

    git.git(&["config", "--get", "user.email"])
       .and_then_err(|e| match e {
         | Error::CommandFailed(_, out) => Ok(out),
         | _ => Err(e),
       })
       .and_then(|out| {
         if out.0.is_empty() {
           git.git(&["config",
                     "--global",
                     "user.email",
                     "donotreply@mergebot.orionkindel.com"])
              .map(|_| ())
         } else {
           log::info!("git user email set to {}", out.0);
           Ok(())
         }
       })
       .and_then(|_| {
         git.git(&["config", "--get", "user.name"]).and_then_err(|e| match e {
                                                     | Error::CommandFailed(_, out) => Ok(out),
                                                     | _ => Err(e),
                                                   })
       })
       .and_then(|out| {
         if out.0.is_empty() {
           git.git(&["config", "--global", "user.name", "mergebot"]).map(|_| ())
         } else {
           log::info!("git user name set to {}", out.0);
           Ok(())
         }
       })
       .and_then(|_| git.clone(url, dirname))
       .map(|dir| (dir.to_string_lossy().to_string(), git.cd(dir)))
       .map(|(dir, _)| git::r#impl::RepoContext::new(format!("{}", dir), lock))
       .map(|c| Box::from(c) as Box<dyn git::RepoContext>)
  }
}

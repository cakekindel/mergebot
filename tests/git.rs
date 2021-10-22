use std::{ffi::OsStr,
          path::{Path, PathBuf},
          process::{Command, Output}};

use mergebot::git;

const REPO_URL: &'static str = "git@github.com:cakekindel/mergebot_test";

trait ExpectOk {
  fn expect_ok(self, msg: &str) -> Self;
}

impl ExpectOk for Output {
  fn expect_ok(self, target: &str) -> Self {
    if !self.stderr.is_empty() {
      log::info!("{} stderr: {}", target, String::from_utf8_lossy(&self.stderr));
    }

    if !self.status.success() {
      panic!();
    }

    self
  }
}

struct State {
  workdir: PathBuf,
}

impl State {
  pub fn cd<P: AsRef<str>>(&self, path: P) -> Self {
    let new: PathBuf = match path.as_ref() {
      | "../" => self.workdir.parent().map(PathBuf::from).unwrap_or(PathBuf::from("/")),
      | path => self.workdir.join(path),
    };

    Self { workdir: new }
  }

  pub fn run<P: AsRef<OsStr>, A: AsRef<OsStr>>(&self, program: P, args: impl IntoIterator<Item = A>) -> Output {
    Command::new(program).args(args)
                         .current_dir(&self.workdir)
                         .output()
                         .unwrap()
  }

  pub fn git_tip_head(&self) -> Vec<u8> {
    self.run("git",
             ["--no-pager", "show", "HEAD", "--no-patch", "--pretty=format:%H"])
        .expect_ok("get commit hash of the tip of qa")
        .stdout
  }
}

fn mktemp() -> PathBuf {
  Command::new("mktemp").args(["--directory"])
                        .output()
                        .map(|o| o.stdout)
                        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                        .map(|path| path.strip_suffix('\n').unwrap().to_string())
                        .map(PathBuf::from)
                        .unwrap()
}

mod init {
  use super::*;
  pub(super) fn init() -> State {
    simple_logger::SimpleLogger::new().init().ok();
    let workdir = mktemp();

    log::info!("working in {:?}", workdir.to_str());

    State { workdir }
  }
}

#[test]
fn git() {
  let state = init::init();

  mergebot::git::r#impl::init(&state.workdir);
  let client = mergebot::git::r#impl::StaticClient;

  let repo = test_repo(&state, &client);
  detach_remote(&state);

  test_upstream(&repo);
  test_merge(&state, &repo);
  test_push(&repo);
  test_update(&state, &repo);
  test_fetch(&state, &repo);
}

/// Test that upstream correctly yields the upstream ref for the current branch
fn test_upstream(repo: &Box<dyn git::RepoContext>) {
  let qa = git::Branch::from("qa");
  repo.switch(&qa).unwrap();
  let up = repo.upstream(&qa).unwrap();

  assert_eq!(up, "origin/qa".into());
}

/// Change origin to point to a bare repository
/// on the local filesystem
/// (git-push does not actually push to github)
fn detach_remote(state: &State) {
  state.cd(".git/")
       .run("git", ["clone", REPO_URL, "fake-remote"])
       .expect_ok("make fake remote");

  state.run("git", ["remote", "set-url", "origin", ".git/fake-remote/.git"])
       .expect_ok("replace real remote with fake one");
}

/// Test git::Client.repo clones the repository
fn test_repo<C: git::Client>(state: &State, client: &C) -> Box<dyn git::RepoContext> {
  let workdir_str = state.workdir.to_str().unwrap().to_string();

  client.repo(REPO_URL, &workdir_str).unwrap();

  // Subsequent clone calls should not fail
  client.repo(REPO_URL, &workdir_str).unwrap()
}

/// Make some changes on the "qa" branch
fn change_qa(state: &State, repo: &Box<dyn git::RepoContext>) -> Vec<u8> {
  let qa: git::Branch = "qa".into();
  repo.switch(&qa).unwrap();

  state.run("sh", ["-c", "echo 'foo' > foo.txt"]).expect_ok("make file");

  state.run("git", ["add", "foo.txt"])
       .expect_ok("add file to working tree");

  state.run("git", ["commit", "--no-gpg-sign", "-m", "create foo.txt"])
       .expect_ok("commit");

  state.git_tip_head()
}

/// Test that FF merging qa -> staging succeeds
fn test_merge(state: &State, repo: &Box<dyn git::RepoContext>) {
  let qa: git::Branch = "qa".into();
  let staging: git::Branch = "staging".into();

  let tip_qa = change_qa(&state, &repo);

  repo.switch(&staging).unwrap();
  repo.merge(&qa).unwrap();

  let tip_staging = state.git_tip_head();

  assert_eq!(tip_qa, tip_staging);
}

/// Test that pushing to upstreams succeed
fn test_push(repo: &Box<dyn git::RepoContext>) {
  let qa: git::Branch = "qa".into();
  let staging: git::Branch = "staging".into();

  repo.switch(&staging).unwrap();
  repo.push().unwrap();

  repo.switch(&qa).unwrap();
  repo.push().unwrap();
}

/// Rewind the tip of QA to 1 commit ago
fn rewind_qa(state: &State) {
  state.run("git", ["reset", "--hard", "origin/qa~1"]);
}

/// Test that updating the current branch (qa) to the upstream
/// works and discards local changes / git history
fn test_update(state: &State, repo: &Box<dyn git::RepoContext>) {
  repo.switch(&"qa".into()).unwrap();

  let old_head = state.git_tip_head();

  // make qa dirty and split from origin/qa
  rewind_qa(state);
  change_qa(state, repo);

  // reset qa -> origin/qa
  repo.update_branch().unwrap();
  let new_head = state.git_tip_head();

  assert_eq!(old_head, new_head);
}

/// Test that fetch fetches
fn test_fetch(state: &State, repo: &Box<dyn git::RepoContext>) {
  state.cd(".git/fake-remote")
       .run("git", ["branch", "foobar"])
       .expect_ok("make new branch in remote");

  let get_branches = || {
    let branches = state.run("git", ["branch", "-r"]).expect_ok("list remote branches").stdout;
    let branches = String::from_utf8_lossy(&branches).to_string();
    branches.strip_suffix("\n")
        .unwrap()
        .split('\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.contains("HEAD"))
        .collect::<Vec<_>>()
  };

  assert!(!get_branches().contains(&"origin/foobar".to_string()));

  repo.fetch_all().unwrap();

  assert!(get_branches().contains(&"origin/foobar".to_string()));
}

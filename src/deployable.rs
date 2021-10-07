use serde::{Deserialize as De, Serialize as Ser};

/// A git branch
#[derive(Debug, Ser, De)]
pub struct Branch(pub String);

/// A branch diff that, when merged, triggers a deploy
#[derive(Debug, Ser, De)]
pub struct Mergeable {
  /// Pretty name of the environment. Commands will be matched against this.
  /// Must not contain spaces.
  pub name: String,
  /// Base branch to be merged into `target`
  pub base: Branch,
  /// Target branch that, when merged, triggers a deploy
  pub target: Branch,
  /// Users who can initiate or approve deploys
  pub users: Vec<User>,
}

/// A git repository, containing a branch for each environment
#[derive(Debug, Ser, De)]
pub struct Repo {
  /// Remote URL of the repo (must be SSH)
  pub url: String,
  /// Pretty name of the repo. Not matched against.
  pub name: String,
  /// SSH key for pushing to this repo.
  pub git_ssh_key: String,
  /// The environments contained within the repo
  pub environments: Vec<Mergeable>,
}

/// A user who can initiate or will be asked to approve
#[derive(Debug, Ser, De)]
#[serde(untagged)]
pub enum User {
  /// A single user
  User {
    /// Slack ID of the user
    user_id: String,
    /// Whether they are required to approve a deployment before it can be executed.
    approver: bool,
  },
  /// A user group
  Group {
    /// Slack ID for the group
    group_id: String,
    /// Minimum number of approvers required from this group. Must be greater than `0`.
    min_approvers: u16,
  },
}

/// A deployable application.
#[derive(Debug, Ser, De)]
pub struct Deployable {
  /// Pretty name of the deployable (for displaying and matching commands against).
  /// Must not contain spaces.
  pub name: String,
  /// Slack workspace ID that `/deploy` is allowed in
  pub team_id: String,
  /// Repositories that will be
  pub repos: Vec<Repo>,
}

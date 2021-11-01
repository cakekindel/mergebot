use serde::{Deserialize as De, Serialize as Ser};

use crate::git::Branch;

/// A branch diff that, when merged, triggers a deploy
#[derive(PartialEq, Clone, Debug, Ser, De)]
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

impl Mergeable {
  /// Check if a given name loosely equals the name of this environment
  pub fn name_eq(&self, name: impl AsRef<str>) -> bool {
    self.name.trim().to_lowercase() == name.as_ref().trim().to_lowercase()
  }
}

/// A git repository, containing a branch for each environment
#[derive(PartialEq, Clone, Debug, Ser, De)]
pub struct Repo {
  /// Remote URL of the repo (must be SSH)
  pub url: String,
  /// Pretty name of the repo. Not matched against.
  pub name: String,
  /// The environments contained within the repo
  pub environments: Vec<Mergeable>,
}

/// A user who can initiate or will be asked to approve
#[derive(PartialEq, Clone, Debug, Ser, De)]
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

impl User {
  /// Get user id if this user is not a group
  pub fn user_id(&self) -> Option<&str> {
    match self {
      | User::User { user_id, .. } => Some(user_id),
      | _ => None,
    }
  }

  /// Returns whether this user is a required approver for deployments
  pub fn is_approver(&self) -> bool {
    match self {
      | User::User { approver, .. } => *approver,
      | User::Group { .. } => true,
    }
  }

  /// Get the slack syntax for directly @ing this user
  pub fn to_at(&self) -> String {
    match self {
      | User::User { user_id, .. } => format!("<@{}>", user_id),
      | User::Group { group_id,
                      min_approvers: 1,
                      .. } => format!("1 member of <!subteam^{}>", group_id),
      | User::Group { group_id,
                      min_approvers,
                      .. } => {
        format!("{} members of <!subteam^{}>", min_approvers, group_id)
      },
    }
  }
}

/// A deployable application.
#[derive(PartialEq, Clone, Debug, Ser, De)]
pub struct App {
  /// Pretty name of the app (for displaying and matching commands against).
  /// Must not contain spaces.
  pub name: String,

  /// Slack workspace ID that `/deploy` is allowed in
  pub team_id: String,

  /// Slack channel to send notifications to
  pub notification_channel_id: String,

  /// Repositories that will be
  pub repos: Vec<Repo>,
}

impl App {
  /// Get an iterator yielding clones of all users (approvers or not) for the application.
  /// Will likely contain duplicates.
  pub fn users(&self, env_name: &str) -> Vec<User> {
    self.repos
        .iter()
        .flat_map(|r| r.environments.iter().filter(|env| env.name_eq(env_name)))
        .flat_map(|env| env.users.clone())
        .collect::<Vec<_>>()
  }
}

/// Errors encounterable while trying to read `deployables.json`
#[derive(Debug)]
pub enum ReadError {
  /// Filesystem error
  Io(std::io::Error),
  /// File exists but is not valid json
  Json(serde_json::Error),
}

/// A Reader is capable of producing an array of deployables,
/// presumably from `deployables.json`
pub trait Reader: 'static + Sync + Send + std::fmt::Debug {
  /// Read the deployables from some source
  fn read(&self) -> Result<Vec<App>, ReadError>;

  /// Find app matching a deploy command
  fn get_matching_cmd(&self, cmd: &super::Command) -> Result<App, super::Error> {
    use super::{Error, Error::*};
    use crate::{extra::StrExtra, result_extra::ResultExtra};

    let loose_eq = |a: &str, b: &str| a.trim().to_lowercase() == b.trim().to_lowercase();

    let env_matches = |env: &Mergeable| -> bool {
      env.name.loose_eq(&cmd.env_name)
    };

    let matches_env_and_user = |app: &App| -> bool { app.repos.iter().any(|r| r.environments.iter().any(env_matches)) };

    let matches_app = |app: &App| -> bool { app.team_id == cmd.team_id && loose_eq(&app.name, &cmd.app_name) };

    let matches_team = |apps: Vec<App>| -> Result<App, Error> {
      apps.into_iter()
          .find(matches_app)
          .ok_or_else(|| AppNotFound(cmd.app_name.clone()))
    };

    self.read()
        .map_err(ReadingApps)
        .and_then(matches_team)
        .filter(matches_env_and_user, |_| {
          EnvNotFound(cmd.app_name.clone(), cmd.env_name.clone())
        })
  }
}

/// ZST that implements Reader for `deployables.json`
#[derive(Debug, Clone, Copy)]
pub struct JsonFile;

impl Reader for JsonFile {
  fn read(&self) -> Result<Vec<App>, ReadError> {
    std::fs::read_to_string(std::path::Path::new("./deployables.json"))
            .map_err(ReadError::Io)
            .and_then(|json| serde_json::from_str(&json).map_err(ReadError::Json))
  }
}

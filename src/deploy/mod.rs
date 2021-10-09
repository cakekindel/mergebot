use std::convert::TryFrom;

pub use app::*;

use crate::slack;

/// Models for local configuration file `./deployables.json`
pub mod app;

/// Struct representing a parsed, well-formed /deploy command
#[derive(Clone, Debug)]
pub struct Command {
  /// Application to deploy
  pub app_name: String,
  /// Environment to deploy
  pub env_name: String,
  /// ID of user who initiated deploy
  pub user_id: String,
  /// ID of slack workspace in which deploy was triggered
  pub team_id: String,
}

impl Command {
  /// Given a `deployable::Reader`, try to find a deployable application matching the command.
  pub fn find_app<R: AsRef<impl app::Reader + ?Sized>>(&self, reader: R) -> Result<App, Error> {
    use app::*;
    use Error::*;

    #[allow(clippy::suspicious_operation_groupings)] // clippy is sus
    let matches_app = |app: &App| -> bool {
      app.team_id == self.team_id && app.name.to_lowercase().trim() == self.app_name.to_lowercase().trim()
    };

    let matches_team = |apps: Vec<App>| -> Result<App, Error> {
      match apps.into_iter().find(matches_app) {
        | Some(app) => Ok(app),
        // don't tell users the app exists in a different team
        | None => Err(AppNotFound(self.app_name.clone())),
      }
    };

    let env_matches = |env: &Mergeable| -> bool {
      env.name.to_lowercase().trim() == self.env_name.to_lowercase().trim()
      && env.users.iter().any(|u| u.user_id() == Some(&self.user_id))
    };

    let matches_env_and_user = |app: &App| -> bool { app.repos.iter().any(|r| r.environments.iter().any(env_matches)) };

    reader.as_ref()
          .read()
          .map_err(ReadingApps)
          .and_then(matches_team)
          .and_then(|app| match matches_env_and_user(&app) {
            | true => Ok(app),
            | false => Err(EnvNotFound(self.app_name.clone(), self.env_name.clone())),
          })
  }
}

/// Any error around the /deploy command
#[derive(Debug)]
pub enum Error {
  /// There's a pending deploy already
  JobAlreadyQueued(crate::job::Job),
  /// Slash command sent was not deploy
  CommandNotDeploy,
  /// Error encountered trying to read `deployables.json`
  ReadingApps(app::ReadError),
  /// Slash command was malformed (multiple arguments, not enough)
  CommandMalformed,
  /// Application not found in Apps
  AppNotFound(String),
  /// Environment not found in application
  EnvNotFound(String, String),
  /// Failed to notify approvers
  Notification(crate::job::MessagingError),
}

impl TryFrom<slack::SlashCommand> for Command {
  type Error = Error;

  fn try_from(cmd: slack::SlashCommand) -> Result<Self, Self::Error> {
    Ok(cmd).and_then(|cmd| match cmd.command.as_str() {
             | "/deploy" => Ok(cmd),
             | _ => Err(Error::CommandNotDeploy),
           })
           .and_then(|cmd| match cmd.text.clone().split(' ').collect::<Vec<_>>().as_slice() {
             | [app, env] => Ok((cmd, app.to_string(), env.to_string())),
             | _ => Err(Error::CommandMalformed),
           })
           .map(|(cmd, app_name, env_name)| Command { team_id: cmd.team_id,
                                                      user_id: cmd.user_id,
                                                      app_name,
                                                      env_name })
  }
}

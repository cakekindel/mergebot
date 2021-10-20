use std::convert::TryFrom;

pub use app::*;
use serde::{Deserialize as De, Serialize as Ser};

use crate::{job, slack};

/// Models for local configuration file `./deployables.json`
pub mod app;

/// Struct representing a parsed, well-formed /deploy command
#[derive(Ser, De, Clone, Debug)]
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

/// Any error around the /deploy command
#[derive(Debug)]
pub enum Error {
  /// There's a pending deploy already
  JobAlreadyQueued(job::Job<job::States>),
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
  /// Error interacting with slack
  SlackApi(slack::Error),
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

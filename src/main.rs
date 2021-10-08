//! # mergebot
//! I'm a slack app triggers approvable deployments for multi-repo applications containing per-environment git branches.
//!
//! e.g.
//! - `github.com/todos-app/backend`
//!   - `main` Prod
//!   - `qa` QA
//!   - `staging` Staging
//! - `github.com/todos-app/frontend`
//!   - `main` Prod
//!   - `qa` QA
//!   - `staging` Staging
//!
//! # Flow
//! - User A issues `/deploy foo staging`
//! - mergebot checks Deployables (configured via `./deployables.json`, which is ignored from source control) for name == "foo"
//! - mergebot checks `foo.repos` for `environments` matching the name "staging"
//! - mergebot ensures User A is in `staging.users`
//! - mergebot queues a merge job for all repos who have a "staging" environment
//! - mergebot sends a slack message targeting all users with `approver == true` & all user groups asking for approval
//! - mergebot waits until the users mentioned above have all reacted with :+1:
//! - when approval conditions met, mergebot executes merge job (`git switch <target>; git merge <base> --no-edit --ff-only --no-verify; git push --no-verify;`)
//!
//! # Setup
//! Requirements:
//!  - [`cargo-make`]
//!  - [`ngrok`]
//!  - A git repo with multiple branches (_not_ this one!) for testing
//!  - A `./deployables.json` file that looks something like `./deployables.example.json`
//!
//! 1. Start a tunnel with `ngrok http 3030` - URL yielded will be referred to as `<ngrok>`
//! 1. Create a slack app with:
//!    - Scopes: `['chat:write', 'commands', 'reactions:read']`
//!    - Redirect URI: `<ngrok>/redirect`
//!    - Slash command: `/deploy` -> `<ngrok>/api/v1/command`
//! 1. Install to a slack workspace
//!
//! # cargo-make
//! This crate uses [`cargo-make`] for script consistency, in Makefile.toml you'll find:
//!   - `cargo make fmt`: Format all files according to configured style `rustfmt.toml`
//!   - `cargo make test`: Run all tests
//!   - `cargo make doctest`: Run doc tests only
//!   - `cargo make tdd`: Watch files for changes, and run `cargo make test` on each change
//!   - `cargo make ci`: Run tests, check that code is formatted and no lint violations.
//!                      This is run as a quality gate for all pull requests.
//!   - `cargo make update-readme`: Regenerate README.md based on `src/lib.rs` and `./README.tpl`.
//!
//! [`cargo-make`]: https://github.com/sagiegurari/cargo-make/
//! [`ngrok`]: https://ngrok.com/
//! [`cargo-readme`]: https://github.com/livioribeiro/cargo-readme
//! [`standard-version`]: https://www.npmjs.com/package/standard-version
//! [conventional commits]: https://www.conventionalcommits.org/en/v1.0.0/

#![deny(missing_docs, missing_doc_code_examples)]
#![cfg_attr(not(test),
            forbid(missing_copy_implementations,
                   missing_debug_implementations,
                   unreachable_pub,
                   unsafe_code,
                   unused_crate_dependencies))]

use std::{convert::TryFrom, env};

use serde_json as _;
use warp::Filter;

/// Slack models
pub mod slack;

/// Models for local configuration file `./deployables.json`
pub mod deployable;

/// Entry point
#[tokio::main]
pub async fn main() {
  init_logger();

  let api = filters::api().with(warp::log("mergebot"));

  warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}

/// Struct representing a parsed, well-formed /deploy command
#[derive(Debug)]
pub struct DeployCommand {
  /// Application to deploy
  pub app_name: String,
  /// Environment to deploy
  pub env_name: String,
  /// ID of user who initiated deploy
  pub user_id: String,
  /// ID of slack workspace in which deploy was triggered
  pub team_id: String,
}

impl DeployCommand {
  /// Given a `deployable::Reader`, try to find a deployable application matching the command.
  pub fn find_app(&self,
                  reader: impl deployable::Reader)
                  -> Result<deployable::Deployable, DeployError> {
    use deployable::*;
    use DeployError::*;

    #[allow(clippy::suspicious_operation_groupings)] // clippy is sus
    let matches_app = |app: &Deployable| -> bool {
      app.team_id == self.team_id && app.name == self.app_name
    };

    let matches_team =
      |apps: Vec<Deployable>| -> Result<Deployable, DeployError> {
        match apps.into_iter().find(matches_app) {
          | Some(app) => Ok(app),
          // don't tell users the app exists in a different team
          | None => Err(AppNotFound(self.app_name.clone())),
        }
      };

    let env_matches = |env: &Mergeable| -> bool {
      env.name == self.env_name
      && env.users.iter().any(|u| u.user_id() == Some(&self.user_id))
    };

    let matches_env_and_user = |app: &Deployable| -> bool {
      app.repos
         .iter()
         .any(|r| r.environments.iter().any(env_matches))
    };

    reader.read()
          .map_err(ReadingDeployables)
          .and_then(matches_team)
          .and_then(|app| match matches_env_and_user(&app) {
            | true => Ok(app),
            | false => {
              Err(EnvNotFound(self.app_name.clone(), self.env_name.clone()))
            },
          })
  }
}

/// Any error around the /deploy command
#[derive(Debug)]
pub enum DeployError {
  /// Slash command sent was not deploy
  CommandNotDeploy,
  /// Error encountered trying to read `deployables.json`
  ReadingDeployables(deployable::ReadError),
  /// Slash command was malformed (multiple arguments, not enough)
  CommandMalformed,
  /// Application not found in Deployables
  AppNotFound(String),
  /// Environment not found in application
  EnvNotFound(String, String),
}

impl TryFrom<slack::SlashCommand> for DeployCommand {
  type Error = DeployError;

  fn try_from(cmd: slack::SlashCommand) -> Result<Self, Self::Error> {
    Ok(cmd).and_then(|cmd| match cmd.command.as_str() {
             | "/deploy" => Ok(cmd),
             | _ => Err(DeployError::CommandNotDeploy),
           })
           .and_then(|cmd| {
             match cmd.text.clone().split(' ').collect::<Vec<_>>().as_slice() {
               | [app, env] => Ok((cmd, app.to_string(), env.to_string())),
               | _ => Err(DeployError::CommandMalformed),
             }
           })
           .map(|(cmd, app_name, env_name)| DeployCommand { team_id:
                                                              cmd.team_id,
                                                            user_id:
                                                              cmd.user_id,
                                                            app_name,
                                                            env_name })
  }
}

/// Warp filters
pub mod filters {
  use super::*;

  /// expands to gross filter type
  macro_rules! filter {() => {impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone}}

  /// The composite warp filter that defines our HTTP api
  pub fn api() -> filter!() {
    hello().or(slash_command())
  }

  /// GET api/v1/hello/:name -> 200 "hello, {name}!"
  fn hello() -> filter!() {
    warp::path!("api" / "v1" / "hello" / String).and(warp::get())
                                                .map(|name| {
                                                  format!("hello, {}!", name)
                                                })
  }

  /// Initiate a deployment
  fn slash_command() -> filter!() {
    warp::path!("api" / "v1" / "command")
         .and(warp::post())
         .and(warp::body::form::<slack::SlashCommand>())
         .map(|slash: slack::SlashCommand| {
           let out = DeployCommand::try_from(slash)
                         .and_then(|dep| dep.find_app(deployable::JsonFile))
                         .map(|app| format!("found app: {}", app.name))
                         .map_err(|e| format!("Error processing command: {:#?}", e))
                         .unwrap_or_else(|e| e);
           log::info!("{}", out);
           out
         })
  }
}

fn init_logger() {
  if env::var_os("RUST_LOG").is_none() {
    env::set_var("RUST_LOG", "mergebot=debug");
  }

  pretty_env_logger::init();
}

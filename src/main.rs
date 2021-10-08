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
//! - mergebot checks Apps (configured via `./deployables.json`, which is ignored from source control) for name == "foo"
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
            forbid(missing_debug_implementations,
                   unreachable_pub,
                   unsafe_code,
                   unused_crate_dependencies))]
#![cfg_attr(not(test), deny(missing_copy_implementations))]

use std::env;

use log as _;
use serde_json as _;
use warp::Filter;

/// Slack models
pub mod slack;

/// Deployment stuff
pub mod deploy;

/// Job queue stuff
pub mod job;

/// App environment
#[derive(Clone, Debug)]
pub struct State<JobMsgr: job::Messenger + Sync + Clone + std::fmt::Debug,
 JobQ: job::Queue + Sync + Clone + std::fmt::Debug,
 AppReader: deploy::app::Reader + Sync + Clone + std::fmt::Debug> {
  /// slack signing secret
  pub slack_signing_secret: String,
  /// slack api token
  pub slack_api_token: String,
  /// notifies approvers
  pub job_messenger: JobMsgr,
  /// Job queue
  pub job_queue: JobQ,
  /// Reader for deployable app configuration
  pub app_reader: AppReader,
  /// HTTP request client
  pub reqwest_client: reqwest::blocking::Client,
}

fn init_logger() {
  if env::var_os("RUST_LOG").is_none() {
    env::set_var("RUST_LOG", "mergebot=debug");
  }

  pretty_env_logger::init();
}

fn get_state(
  )
    -> State<job::SlackMessenger, job::MemQueue, deploy::app::JsonFile>
{
  State {
    slack_signing_secret: env::var("SLACK_SIGNING_SECRET").expect("SLACK_SIGNING_SECRET required"),
    slack_api_token: env::var("SLACK_API_TOKEN").expect("SLACK_API_TOKEN required"),
    job_messenger: job::SlackMessenger,
    job_queue: job::MemQueue,
    app_reader: deploy::app::JsonFile,
    reqwest_client: reqwest::blocking::Client::new(),
  }
}

/// Entry point
#[tokio::main]
pub async fn main() {
  init_logger();
  dotenv::dotenv().ok();

  let state = get_state();

  let api = filters::api(state).with(warp::log("mergebot"));

  warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}

/// Warp filters
pub mod filters {
  use std::convert::TryFrom;

  use job::Queue;

  use super::*;

  /// expands to gross filter type
  macro_rules! filter {() => {impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone}}

  /// expands to gross app state type
  macro_rules! state {
    () => {
      State<
        impl job::Messenger + Sync + Send + Clone + std::fmt::Debug,
        impl job::Queue + Sync + Send + Clone + std::fmt::Debug,
        impl deploy::app::Reader + Sync + Send + Clone + std::fmt::Debug,
      >
    }
  }

  /// The composite warp filter that defines our HTTP api
  pub fn api(app_state: state!()) -> filter!() {
    hello().or(slash_command(app_state))
  }

  /// GET api/v1/hello/:name -> 200 "hello, {name}!"
  fn hello() -> filter!() {
    warp::path!("api" / "v1" / "hello" / String).and(warp::get())
                                                .map(|name| {
                                                  format!("hello, {}!", name)
                                                })
  }

  // [1] - User A issues `/deploy foo staging`
  // [2] - mergebot checks Apps (configured via `./deployables.json`, which is ignored from source control) for name == "foo"
  // [3] - mergebot checks `foo.repos` for `environments` matching the name "staging"
  // [4] - mergebot ensures User A is in `staging.users`
  // [5] - mergebot queues a merge job for all repos who have a "staging" environment
  // [6] - mergebot sends a slack message targeting all users with `approver == true` & all user groups asking for approval
  // [7] - mergebot waits until the users mentioned above have all reacted with :+1:
  // [8] - when approval conditions met, mergebot executes merge job (`git switch <target>; git merge <base> --no-edit --ff-only --no-verify; git push --no-verify;`)

  /// Initiate a deployment
  fn slash_command(mergebot: state!()) -> filter!() {
    warp::path!("api" / "v1" / "command")
         .and(warp::post())
         .and(warp::body::form::<slack::SlashCommand>())
         .map(move |slash: slack::SlashCommand| {
           deploy::Command::try_from(slash) // [1]
               .and_then(|cmd| cmd.find_app(&mergebot.app_reader).map(|app| (cmd, app))) // [2], [3], [4]
               .map(|(cmd, app)| mergebot.job_queue.queue(app, cmd)) // [5]
               .and_then(|job| {
                 mergebot.job_messenger.send_message_for_job(&mergebot.reqwest_client, &mergebot.slack_api_token, &job)
                     .map_err(deploy::Error::Notification)
                     .map(|message_ts| mergebot.job_queue.set_state(job.id, job::State::Notified{message_ts}))
               }) // [6]
               .map(|job| format!("```{:#?}```", job))
               .map_err(|e| format!("Error processing command: {:#?}", e))
               .unwrap_or_else(|e| e)
         })
  }
}

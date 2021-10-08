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

/// Spin up the actual app state
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

  use warp::{reject::{Reject, Rejection},
             reply::Reply};

  use super::*;

  /// 401 Unauthorized rejection
  #[derive(Debug)]
  struct Unauthorized;
  impl Reject for Unauthorized {}

  /// expands to gross filter type
  macro_rules! filter {
    () => {impl Filter<Extract = impl Reply, Error = Rejection> + Clone};
    ($reply: ty) => {impl Filter<Extract = $reply, Error = Rejection> + Clone};
    ($reply: ty, $reject: ty) => {impl Filter<Extract = $reply, Error = $reject> + Clone};
  }

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

  async fn handle_unauthorized(err: Rejection)
                               -> Result<impl Reply, Rejection> {
    if err.find::<Unauthorized>().is_some() {
      Ok(warp::reply::with_status("", http::StatusCode::UNAUTHORIZED))
    } else {
      Err(err)
    }
  }

  /// The composite warp filter that defines our HTTP api
  pub fn api(app_state: state!()) -> filter!() {
    let handle_command =
      slack_request_authentic().and(slash_command(app_state));
    hello().or(handle_command).recover(handle_unauthorized)
  }

  /// https://api.slack.com/authentication/verifying-requests-from-slack
  fn slack_request_authentic() -> filter!((), Rejection) {
    warp::filters::body::bytes()
        .and(warp::filters::header::value("X-Slack-Request-Timestamp"))
        .and(warp::filters::header::value("X-Slack-Signature"))
        .and_then(|bytes: bytes::Bytes, ts: http::HeaderValue, inbound_sig: http::HeaderValue| async move {
          use sha2::Digest;

          let ts = ts.to_str().unwrap();
          let inbound_sig = inbound_sig.to_str().unwrap();
          let base_string = [b"v0:", ts.as_bytes(), b":", &bytes].concat();
          let hash = sha2::Sha256::digest(&base_string);
          let sig = [b"v0={}", &hash[..]].concat();

          if sig == inbound_sig.as_bytes() {
            Ok(())
          } else {
            Err(warp::reject::custom(Unauthorized))
          }
        })
        .untuple_one()
  }

  /// GET api/v1/hello/:name -> 200 "hello, {name}!"
  fn hello() -> filter!(impl Reply) {
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
  fn slash_command(mergebot: state!()) -> filter!((impl Reply,)) {
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

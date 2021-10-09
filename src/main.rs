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

// I chose to use dyn boxes rather than generics here for code footprint and code footprint alone.
// If scale was a concern, I would want to change:
//   `State {t: Box<dyn Trait>}`
// to
//   `State<T: Trait> {trait: T}`
/// App environment
#[derive(Debug)]
pub struct State {
  /// slack signing secret
  pub slack_signing_secret: String,
  /// slack api token
  pub slack_api_token: String,
  /// notifies approvers
  pub job_messenger: Box<dyn job::Messenger>,
  /// Job queue
  pub job_queue: Box<dyn job::Queue>,
  /// Reader for deployable app configuration
  pub app_reader: Box<dyn deploy::app::Reader>,
  /// HTTP request client
  pub reqwest_client: reqwest::blocking::Client,
}

lazy_static::lazy_static! {
  static ref STATE: State = {
    State {
      slack_signing_secret: env::var("SLACK_SIGNING_SECRET").expect("SLACK_SIGNING_SECRET required"),
      slack_api_token: env::var("SLACK_API_TOKEN").expect("SLACK_API_TOKEN required"),
      job_messenger: Box::from(job::SlackMessenger),
      job_queue: Box::from(job::MemQueue),
      app_reader: Box::from(deploy::app::JsonFile),
      reqwest_client: reqwest::blocking::Client::new(),
    }
  };
}

type StateFilter = warp::filters::BoxedFilter<(&'static State,)>;

fn init_logger() {
  if env::var_os("RUST_LOG").is_none() {
    env::set_var("RUST_LOG", "mergebot=debug");
  }

  pretty_env_logger::init();
}

fn create_state_filter() -> StateFilter {
  // A note on this filter and dependency injection:
  //
  // Context: It's important to me that I isolate my IO (reading from `./deployables.json`, sending HTTP requests to slack, etc.)
  // from my application code so that I can replace it with mocks during testing.
  //
  // Passing in dependencies to the functions in the `filter` module is rather difficult,
  // since filter closures need to be:
  //   - independent of local state (can't use references to the dep)
  //   - re-runnable (can't move the dep out of the parent scope into the filter)
  //
  // The solution I came up with was a filter that produces a static reference (lives as long as the program)
  // to a STATE static variable.
  //
  // This means that any number of filters can all access (but not mutate)
  // application state, and the filters are isolated from the implementors of the traits

  warp::filters::any::any().map(|| &*STATE).boxed()
}

/// Entry point
#[tokio::main]
pub async fn main() {
  init_logger();

  dotenv::dotenv().ok();

  let api = filters::api(create_state_filter).with(warp::log("mergebot"));

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

  async fn handle_unauthorized(err: Rejection)
                               -> Result<impl Reply, Rejection> {
    if err.find::<Unauthorized>().is_some() {
      Ok(warp::reply::with_status("", http::StatusCode::UNAUTHORIZED))
    } else {
      log::error!("unhandled rejection: {:#?}", err);
      Err(err)
    }
  }

  /// The composite warp filter that defines our HTTP api
  pub fn api(state: fn() -> StateFilter) -> filter!() {
    hello().or(slash_command(state)).recover(handle_unauthorized)
  }

  /// https://api.slack.com/authentication/verifying-requests-from-slack
  fn slack_request_authentic(mergebot_state: StateFilter)
                             -> filter!((bytes::Bytes,), Rejection) {
    mergebot_state
        .and(warp::filters::body::bytes())
        .and(warp::filters::header::value("X-Slack-Request-Timestamp"))
        .and(warp::filters::header::value("X-Slack-Signature"))
        .and_then(|state, body: bytes::Bytes, ts, sig| async move {
          if slack::request_authentic(state, body.clone(), ts, sig) {
            Ok(body)
          } else {
            Err(warp::reject::custom(Unauthorized))
          }
        })
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
  fn slash_command(state: fn() -> StateFilter) -> filter!((impl Reply,)) {
    warp::path!("api" / "v1" / "command")
         .and(warp::post())
         .and(slack_request_authentic(state()))
         .and(state())
         .and_then(|body: bytes::Bytes, mergebot: &'static State| async move {
           serde_urlencoded::from_bytes::<slack::SlashCommand>(&body)
             .map_err(|e| {
               log::error!("{:#?}", e);
               warp::reply::with_status(String::new(), http::StatusCode::BAD_REQUEST)
             })
             .and_then(|slash| {
               deploy::Command::try_from(slash) // [1]
                   .and_then(|cmd| cmd.find_app(&mergebot.app_reader).map(|app| (cmd, app))) // [2], [3], [4]
                   .map(|(cmd, app)| mergebot.job_queue.queue(app, cmd)) // [5]
                   .and_then(|job| {
                     mergebot.job_messenger.send_message_for_job(&mergebot.reqwest_client, &mergebot.slack_api_token, &job)
                         .map_err(deploy::Error::Notification)
                         .map(|message_ts| mergebot.job_queue.set_state(&job.id, job::State::Notified{message_ts}))
                   }) // [6]
                   .map(|job| warp::reply::with_status(format!("```{:#?}```", job), http::StatusCode::OK))
                   .map_err(|e| warp::reply::with_status(format!("Error processing command: {:#?}", e), http::StatusCode::OK))
             })
             .map(|rep| Ok(rep) as Result<warp::reply::WithStatus<String>, warp::reject::Rejection>)
             .unwrap_or_else(|e| Ok(e))
         })
  }
}

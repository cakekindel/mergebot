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

#![cfg_attr(not(test), forbid(missing_debug_implementations, unreachable_pub))]
#![cfg_attr(not(test), deny(unsafe_code, missing_copy_implementations))]

use std::{env, sync::Arc};

use mergebot::*;
use warp::Filter;

type StateFilter = warp::filters::BoxedFilter<(&'static State,)>;

fn init_job_state_hooks(s: &'static State) {
  s.jobs.attach_listener(job::hooks::on_create_notify(&s));
  s.jobs.attach_listener(job::hooks::on_full_approval_change_state(&s));
  s.jobs.attach_listener(job::hooks::on_full_approval_notify(&s));
  s.jobs.attach_listener(job::hooks::on_full_approval_deploy(&s));
  s.jobs.attach_listener(job::hooks::on_failure_log(&s));
  s.jobs.attach_listener(job::hooks::on_failure_poison(&s));
  s.jobs.attach_listener(job::hooks::on_poison_notify(&s));
  s.jobs.attach_listener(job::hooks::on_done_notify(&s));
}

fn init_logger() {
  if env::var_os("RUST_LOG").is_none() {
    env::set_var("RUST_LOG", "mergebot=debug");
  }

  pretty_env_logger::init_timed();
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

  init_job_state_hooks(&STATE);

  Arc::clone(&APP_INIT).wait(); // Wait until worker thread is ready

  warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}

/// Warp filters
pub mod filters {
  use std::convert::TryFrom;

  use extra::StrExtra;
  use warp::{reject::{Reject, Rejection},
             reply::Reply};

  use super::{result_extra::ResultExtra, *};

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

  async fn handle_unauthorized(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.find::<Unauthorized>().is_some() {
      Ok(warp::reply::with_status("", http::StatusCode::UNAUTHORIZED))
    } else {
      log::error!("unhandled rejection: {:#?}", err);
      Err(err)
    }
  }

  /// The composite warp filter that defines our HTTP api
  pub fn api(state: fn() -> StateFilter) -> filter!() {
    hello().or(command_filter(state))
           .or(event_filter(state))
           .or(get_jobs(state))
           .recover(handle_unauthorized)
  }

  fn api_key(state: StateFilter) -> filter!(()) {
    warp::filters::header::value("X-Api-Key").and(state)
                                             .and_then(|t: http::HeaderValue, state: &'static State| async move {
                                               match t == state.api_key {
                                                 | true => Ok(()),
                                                 | false => Err(warp::reject::custom(Unauthorized)),
                                               }
                                             })
                                             .untuple_one()
  }

  fn get_jobs(state: fn() -> StateFilter) -> filter!() {
    state().and(warp::path!("api" / "v1" / "jobs"))
           .and(warp::get())
           .and(api_key(state()))
           .map(|state: &'static State| warp::reply::json(&state.jobs.get_all()))
  }

  /// <https://api.slack.com/authentication/verifying-requests-from-slack>
  fn slack_request_authentic(mergebot_state: StateFilter) -> filter!((bytes::Bytes,), Rejection) {
    mergebot_state.and(warp::filters::body::bytes())
                  .and(warp::filters::header::value("X-Slack-Request-Timestamp"))
                  .and(warp::filters::header::value("X-Slack-Signature"))
                  .and_then(|state: &'static State, body: bytes::Bytes, ts, sig| async move {
                    if slack::request_authentic(&state.slack_signing_secret, body.clone(), ts, sig) {
                      Ok(body)
                    } else {
                      Err(warp::reject::custom(Unauthorized))
                    }
                  })
  }

  /// GET api/v1/hello/:name -> 200 "hello, {name}!"
  fn hello() -> filter!(impl Reply) {
    warp::path!("api" / "v1" / "hello" / String).and(warp::get())
                                                .map(|name| format!("hello, {}!", name))
  }

  fn handle_approval(state: &'static State, job: &job::Job<job::StateInit>, user_id: &str) {
    use deploy::User;

    let user_in_group = |group_id: &str| {
      state.slack_groups
           .contains_user(group_id, user_id)
           .map_err(|e| log::error!("{:#?}", e))
           .unwrap_or(false)
    };

    let user = job.outstanding_approvers().into_iter().find(|u| match u {
                                                        | User::User { user_id: u_id, .. } => u_id == user_id,
                                                        | User::Group { group_id, .. } => user_in_group(&group_id),
                                                      });

    if user.is_none() {
      log::debug!("(job {:?}) user {} approved but isn't an approver", job.id, user_id);
    }

    if let Some(user) = user {
      state.jobs.approved(&job.id, user);
    }
  }

  fn ok<T: Reply>(t: T) -> warp::reply::WithStatus<T> {
    warp::reply::with_status(t, http::StatusCode::OK)
  }

  async fn handle_event(body: bytes::Bytes,
                        state: &'static State)
                        -> Result<warp::reply::WithStatus<String>, warp::reject::Rejection> {
    use slack::event::{Event, EventPayload::ReactionAdded, ReactionAddedItem as Item};

    let ev = match serde_json::from_slice::<Event>(&body) {
      | Ok(b) => b,
      | Err(e) => {
        log::error!("{:#?}", e); // if slack sends us a bad body I need to know about it
        return Ok(warp::reply::with_status(String::new(), http::StatusCode::BAD_REQUEST));
      },
    };

    match ev {
      | Event::Challenge { challenge } => Ok(ok(challenge)),
      | Event::Event { team_id,
                       event:
                         ReactionAdded { user,
                                         reaction,
                                         item: Item::Message { channel, ts }, }, } => {
        if reaction.as_str() != "+1" {
          return Ok(ok(String::new()));
        }

        let matched_job = state.jobs
                               .get_all_new()
                               .into_iter()
                               .find(|j| match j.state.msg_id.as_ref() {
                                 | Some(msg_id) => j.app.team_id == team_id && msg_id.equals(&channel, &ts),
                                 | _ => false,
                               });

        if let Some(j) = matched_job {
          handle_approval(state, &j, &user);
        }

        Ok(ok(String::new()))
      },
      | e => {
        log::info!("not responding to event: {:#?}", e);
        Ok(ok(String::new()))
      },
    }
  }

  fn event_filter(state: fn() -> StateFilter) -> filter!((impl Reply,)) {
    warp::path!("api" / "v1" / "event").and(warp::post())
                                       .and(slack_request_authentic(state()))
                                       .and(state())
                                       .and_then(handle_event)
  }

  // [0] - App ensures slack request is authentic
  // [1] - User A issues `/deploy foo staging`
  // [2] - mergebot checks Apps (configured via `./deployables.json`, which is ignored from source control) for name == "foo"
  // [3] - mergebot checks `foo.repos` for `environments` matching the name "staging"
  // [4] - mergebot ensures User A is in `staging.users`
  // [5] - mergebot queues a merge job for all repos who have a "staging" environment
  // [6] - mergebot sends a slack message targeting all users with `approver == true` & all user groups asking for approval
  // [7] - mergebot waits until the users mentioned above have all reacted with :+1:
  // [8] - when approval conditions met, mergebot executes merge job (`git switch <target>; git merge <base> --no-edit --ff-only --no-verify; git push --no-verify;`)

  /// Initiate a deployment
  fn command_filter(state: fn() -> StateFilter) -> filter!((impl Reply,)) {
    warp::path!("api" / "v1" / "command").and(warp::post())
                                         .and(slack_request_authentic(state())) // [0]
                                         .and(state())
                                         .and_then(handle_command)
  }

  async fn handle_command(body: bytes::Bytes,
                          mergebot: &'static State)
                          -> Result<warp::reply::WithStatus<String>, warp::reject::Rejection> {
    let try_create_job = |(cmd, app): (deploy::Command, _)| {
      let existing = mergebot.jobs.get_all().into_iter().find(|j| {
                                                          j.state.in_progress()
                                                          && j.app == app
                                                          && j.command.env_name.loose_eq(&cmd.env_name)
                                                        });

      if let Some(job) = existing {
        Err(deploy::Error::JobAlreadyQueued(job))
      } else {
        Ok(mergebot.jobs.create(app, cmd)).map(|id| mergebot.jobs.get_new(&id).unwrap())
      }
    };

    let bad_req = || warp::reply::with_status(String::new(), http::StatusCode::BAD_REQUEST);
    let failed = |e| {
      let msg = match e {
        deploy::Error::JobAlreadyQueued(job) => format!("There's already a {} deploy in progress for {}", job.command.env_name, job.app.name),
        _ => "Uh oh :confused: I wasn't able to do that. <https://github.com/cakekindel/mergebot/issues|Please file an issue>!".to_string(),
      };

      warp::reply::with_status(msg, http::StatusCode::OK)
    };

    serde_urlencoded::from_bytes::<slack::SlashCommand>(&body).tap_err(|e| log::error!("{:#?}", e))
                                                              .map(|slash| {
                                                                deploy::Command::try_from(slash).and_then(|cmd| {
                                                                  mergebot.app_reader
                                                                          .get_matching_cmd(&cmd)
                                                                          .map(|app| (cmd, app))
                                                                }) // [2], [3], [4]
                                                                .and_then(try_create_job)
                                                                .map(|_| {
                                                                  warp::reply::with_status(String::new(),
                                                                                           http::StatusCode::OK)
                                                                })
                                                                .tap_err(|e| log::error!("{:?}", e))
                                                                .map_err(failed)
                                                                .unwrap_or_else(|e| e)
                                                              })
                                                              .and_then_err(|_| Ok(bad_req()))
  }
}

#![cfg_attr(not(test), forbid(missing_debug_implementations, unreachable_pub))]
#![cfg_attr(not(test), deny(unsafe_code, missing_copy_implementations))]

use std::{env,
          sync::{Arc, Barrier, Mutex}};

/// Helper result methods
pub mod result_extra;

/// Helper mutex functions
pub mod mutex_extra;

/// Helper functions
pub mod extra;

/// Slack models
pub mod slack;

/// Git stuff
pub mod git;

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
  /// API token used to access jobs api
  pub api_key: String,
  /// notifies approvers
  pub job_messenger: Box<dyn job::Messenger>,
  /// Job queue
  pub jobs: Box<dyn job::Store>,
  /// Reader for deployable app configuration
  pub app_reader: Box<dyn deploy::app::Reader>,
  /// HTTP request client
  pub reqwest_client: &'static reqwest::blocking::Client,
  /// slack groups API
  pub slack_groups: Box<dyn slack::groups::Groups>,
  /// slack msg API
  pub slack_msg: Box<dyn slack::msg::Messages>,
  /// git client
  pub git: Box<dyn git::Client>,
  /// transition jobs from "Approved" -> "Done" | "Poisoned"
  pub job_executor: Box<dyn job::exec::Executor>,
}

lazy_static::lazy_static! {
  pub static ref APP_INIT: Arc<Barrier> = Arc::new(Barrier::new(2));
  pub static ref CLIENT: reqwest::blocking::Client =reqwest::blocking::Client::new();
  pub static ref STATE: State = {
    let slack_token = env::var("SLACK_API_TOKEN").expect("SLACK_API_TOKEN required");
    let slack_api = slack::Api::new(&slack_token, &CLIENT);

    git::r#impl::init(env::var("GIT_WORKDIR").expect("GIT_WORKDIR required"));

    let git = git::r#impl::StaticClient;

    let jobs = Arc::new(Mutex::new(job::store::StoreData::new()));

    job::exec::r#impl::init(Box::from(jobs.clone()), Box::from(git));

    State {
      api_key: env::var("API_KEY").expect("API_KEY required"),
      slack_signing_secret: env::var("SLACK_SIGNING_SECRET").expect("SLACK_SIGNING_SECRET required"),
      slack_api_token: slack_token,
      jobs: Box::from(jobs),
      app_reader: Box::from(deploy::app::JsonFile),
      reqwest_client: &CLIENT,
      slack_groups: Box::from(slack_api.clone()),
      job_messenger: Box::from(slack_api.clone()),
      slack_msg: Box::from(slack_api),
      git: Box::from(git),
      job_executor: Box::from(job::exec::r#impl::Executor),
    }
  };
}

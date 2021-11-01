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
  /// slack app client id
  pub slack_client_id: String,
  /// slack app client secret
  pub slack_client_secret: String,
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
  /// slack Oauth Access
  pub slack_access: Box<dyn slack::access::Access>,
  /// git client
  pub git: Box<dyn git::Client>,
  /// transition jobs from "Approved" -> "Done" | "Poisoned"
  pub job_executor: Box<dyn job::exec::Executor>,
}

lazy_static::lazy_static! {
  pub static ref APP_INIT: Arc<Barrier> = Arc::new(Barrier::new(2));
  pub static ref CLIENT: reqwest::blocking::Client =reqwest::blocking::Client::new();
  pub static ref STATE: State = {
    // Environment
    let api_key = env::var("API_KEY").expect("API_KEY required");
    let slack_signing_secret = env::var("SLACK_SIGNING_SECRET").expect("SLACK_SIGNING_SECRET required");
    let slack_client_id = env::var("SLACK_CLIENT_ID").expect("SLACK_CLIENT_ID required");
    let slack_client_secret = env::var("SLACK_CLIENT_SECRET").expect("SLACK_CLIENT_SECRET required");

    // Slack API
    let slack_api = slack::Api::new("https://www.slack.com", &slack::tokens::Fs, &CLIENT);
    let slack_groups = Box::from(slack_api.clone());
    let job_messenger = Box::from(slack_api.clone());
    let slack_access = Box::from(slack_api.clone());
    let slack_msg = Box::from(slack_api);

    // Git client
    git::r#impl::init(env::var("GIT_WORKDIR").expect("GIT_WORKDIR required"));
    let git = Box::from(git::r#impl::StaticClient);

    // Job store
    let jobs = Box::from(Arc::new(Mutex::new(job::store::StoreData::new())));

    // Job executor
    // TODO(orion): does not need to be at this level, could be implementation detail of job store?
    job::exec::r#impl::init(jobs.clone(), git.clone());
    let job_executor = Box::from(job::exec::r#impl::Executor);

    // App configuration reader
    let app_reader = Box::from(deploy::app::JsonFile);

    State {
      reqwest_client: &CLIENT,
      api_key,
      slack_signing_secret,
      slack_client_id,
      slack_client_secret,
      jobs,
      app_reader,
      slack_groups,
      job_messenger,
      slack_msg,
      slack_access,
      git,
      job_executor,
    }
  };
}

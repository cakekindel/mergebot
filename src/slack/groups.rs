use serde::{Deserialize as De, Serialize as Ser};

#[derive(Ser, De)]
struct Rep {
  pub(self) ok: bool,
  pub(self) error: Option<String>,
  pub(self) users: Option<Vec<String>>,
}

/// Errors encounterable by the slack groups api
#[derive(Debug)]
pub enum Error {
  /// Error sending, establishing http connection, deserializing, etc.
  Http(reqwest::Error),
  /// Slack got our request but didn't like it
  Slack(String),
}

impl Groups for super::Api {
  fn expand(&self, client: &reqwest::blocking::Client, token: &str, group_id: &str) -> Result<Vec<String>, Error> {
    client.get(format!("https://slack.com/api/usergroups.users.list?usergroup={}", group_id))
          .header("authorization", format!("Bearer {}", token))
          .send()
          .and_then(|rep| rep.error_for_status())
          .map_err(Error::Http)
          .and_then(|rep| rep.json::<Rep>().map_err(Error::Http))
          .and_then(|rep| match rep.ok {
            | true => Ok(rep.users.unwrap_or(vec![])),
            | false => Err(Error::Slack(rep.error.unwrap_or("".into()))),
          })
  }
}

/// Trait representing slack api ops around user groups
pub trait Groups: 'static + Sync + Send + std::fmt::Debug {
  /// Expand a group id into user ids
  fn expand(&self, client: &reqwest::blocking::Client, token: &str, group_id: &str) -> Result<Vec<String>, Error>;
}

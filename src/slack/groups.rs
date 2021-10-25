use serde::{Deserialize as De, Serialize as Ser};

use super::{Error, Result};

#[derive(Ser, De)]
struct Rep {
  pub(self) ok: bool,
  pub(self) error: Option<String>,
  pub(self) users: Option<Vec<String>>,
}

/// Trait representing slack api ops around user groups
pub trait Groups: 'static + Sync + Send + std::fmt::Debug {
  /// Expand a group id into user ids
  fn expand(&self, group_id: &str) -> Result<Vec<String>>;

  /// Check if a group contains a user
  fn contains_user(&self, group_id: &str, user_id: &str) -> Result<bool>;
}

impl Groups for super::Api {
  fn expand(&self, group_id: &str) -> Result<Vec<String>> {
    self.client
        .get(format!("{}/api/usergroups.users.list?usergroup={}", self.base_url, group_id))
        .header("authorization", format!("Bearer {}", self.token))
        .send()
        .and_then(|rep| rep.error_for_status())
        .map_err(Error::Http)
        .and_then(|rep| rep.json::<Rep>().map_err(Error::Http))
        .and_then(|rep| match rep.ok {
          | true => Ok(rep.users.unwrap_or_default()),
          | false => Err(Error::Slack(rep.error.unwrap_or_else(|| "".into()))),
        })
  }

  fn contains_user(&self, group_id: &str, user_id: &str) -> Result<bool> {
    self.expand(group_id).map(|g| g.contains(&user_id.to_string()))
  }
}

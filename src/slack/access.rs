use serde::{Deserialize as De, Serialize as Ser};

use crate::result_extra::ResultExtra;

#[derive(Debug, Clone, Ser, De, PartialEq)]
pub struct AccessRep {
  pub access_token: String,
  pub scope: String,
  pub bot_user_id: String,
  pub team: Team,
}

#[derive(Debug, Clone, Ser, De, PartialEq)]
pub struct Team {
  pub id: String,
}

pub trait Access: std::fmt::Debug + Send + Sync + 'static {
  fn access(&self, code: &str, client_id: &str, client_secret: &str) -> super::Result<AccessRep>;
}

impl Access for super::Api {
  fn access(&self, code: &str, client_id: &str, client_secret: &str) -> super::Result<AccessRep> {
    let basic = base64::encode(format!("{}:{}", client_id, client_secret));

    self.client
        .post(format!("{}/api/oauth.v2.access?code={}", self.base_url, code))
        .header("authorization", format!("Basic {}", basic))
        .send()
        .and_then(|rep| rep.error_for_status())
        .and_then(|rep| rep.json::<AccessRep>())
        .tap(|rep| self.tokens.register(&rep))
        .map_err(super::Error::Http)
  }
}

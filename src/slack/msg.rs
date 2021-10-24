use serde::{Deserialize as De, Serialize as Ser};
use slack_blocks::Block;

use super::{Api, Error, Result};

/// A timestamp / channel id pair that uniquely
/// identifies a message
#[derive(PartialEq, Debug, Clone, Ser, De)]
pub struct Id {
  /// Channel ID
  pub channel: String,
  /// Message timestamp
  pub ts: String,
}

impl Id {
  /// Is this id equal to another channel+ts pair?
  pub fn equals(&self, channel: impl AsRef<str>, ts: impl AsRef<str>) -> bool {
    self.channel == channel.as_ref() && self.ts == ts.as_ref()
  }
}

#[derive(Debug, Clone, Ser, De)]
struct RepRaw {
  ok: bool,
  error: Option<String>,
  channel: Option<String>,
  ts: Option<String>,
}

/// Send message OK response
#[derive(Debug, PartialEq, Clone, Ser, De)]
pub struct Rep {
  /// Id of sent message
  pub id: Id,
}

impl Rep {
  fn try_from_raw(raw: RepRaw) -> Result<Rep> {
    let RepRaw { ts, channel, ok, error } = raw;

    match ok {
      | true => ts.ok_or_else(|| Error::Other("expected ts to be present".into()))
                  .and_then(|ts| {
                    channel.ok_or_else(|| Error::Other("expected channel to be present".into()))
                           .map(|channel| (channel, ts))
                  })
                  .map(|(channel, ts)| Id { channel, ts })
                  .map(|id| Rep { id }),
      | false => Err(Error::Slack(error.unwrap_or_else(|| "no error".into()))),
    }
  }
}

/// Send messages
pub trait Messages: 'static + Sync + Send + std::fmt::Debug {
  /// Send message
  fn send(&self, channel_id: &str, blocks: &[Block]) -> Result<Rep>;

  /// Send a message in a thread
  fn send_thread(&self, thread_parent: &Id, blocks: &[Block]) -> Result<Rep>;
}

fn send_body(channel: Option<&str>,
             blocks: &[Block],
             thread_parent: Option<&Id>)
             -> serde_json::value::Map<String, serde_json::Value> {
  let mut map = serde_json::Map::new();
  let channel = thread_parent.map(|id| id.channel.as_str())
                             .or(channel)
                             .expect("channel or thread_parent should be set");
  let blocks = serde_json::to_value(blocks).expect("blocks should serialize");

  map.insert("channel".into(), channel.into());
  map.insert("blocks".into(), blocks);

  if let Some(Id { ts, .. }) = thread_parent {
    map.insert("thread_ts".into(), ts.as_str().into());
  }

  map
}

fn send_base(base_url: &str,
             token: &str,
             client: &reqwest::blocking::Client,
             channel_id: Option<&str>,
             thread_parent: Option<&Id>,
             blocks: &[Block])
             -> Result<Rep> {
  client.post(format!("{}/api/chat.postMessage", base_url))
        .json(&send_body(channel_id, blocks, thread_parent))
        .header("authorization", format!("Bearer {}", token))
        .send()
        .and_then(|rep| rep.error_for_status())
        .and_then(|rep| rep.json::<RepRaw>())
        .map_err(Error::Http)
        .and_then(Rep::try_from_raw)
}

impl Messages for Api {
  fn send(&self, channel_id: &str, blocks: &[Block]) -> Result<Rep> {
    send_base(&self.base_url, &self.token, self.client, Some(channel_id), None, blocks)
  }

  fn send_thread(&self, thread_parent: &Id, blocks: &[Block]) -> Result<Rep> {
    send_base(&self.base_url,
              &self.token,
              self.client,
              None,
              Some(thread_parent),
              blocks)
  }
}

use serde::{Deserialize as De, Serialize as Ser};

/// An incoming event
#[derive(Ser, De, Debug)]
#[serde(tag = "type")]
pub enum Event {
  /// Slack sends us this to make sure we're ready to handle events.
  #[serde(rename = "url_verification")]
  Challenge {
    /// Text we need to respond with
    challenge: String,
  },
  /// A slack event
  #[serde(rename = "event_callback")]
  Event {
    /// Slack workspace ID that event occurred in
    team_id: String,
    /// The event
    event: EventPayload,
  },
}

/// An payload for an incoming event
#[derive(Ser, De, Debug)]
#[serde(tag = "type")]
pub enum EventPayload {
  /// A reaction was added to a message
  #[serde(rename = "reaction_added")]
  ReactionAdded {
    /// The user who reacted
    user: String,
    /// The emoji that was reacted with
    reaction: String,
    /// The item that was reacted to
    item: ReactionAddedItem,
  },
  /// Any other kind of event
  #[serde(other)]
  Other,
}

/// A reaction was added to a message, file, file comment
#[derive(Ser, De, Debug)]
#[serde(tag = "type")]
pub enum ReactionAddedItem {
  /// Some info about the message that was reacted to
  #[serde(rename = "message")]
  Message {
    /// Channel id
    channel: String,
    /// Message timestamp
    ts: String,
  },

  /// Something other than a message
  #[serde(other)]
  Other,
}

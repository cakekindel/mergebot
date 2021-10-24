use serde::{Deserialize as De, Serialize as Ser};

/// An incoming event
#[derive(Ser, De, Debug, PartialEq)]
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
#[derive(Ser, De, Debug, PartialEq)]
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
#[derive(Ser, De, Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn challenge_de() {
    let json = r#"{
      "token": "Jhj5dZrVaK7ZwHHjRyZWjbDl",
      "challenge": "3eZbrw1aBm2rZgRNFdxV2595E9CY3gmdALWMmHkvFXO7tYXAYM8P",
      "type": "url_verification"
    }"#;

    let expected = Event::Challenge { challenge: "3eZbrw1aBm2rZgRNFdxV2595E9CY3gmdALWMmHkvFXO7tYXAYM8P".into() };

    let actual = serde_json::from_str::<Event>(json).unwrap();

    assert_eq!(expected, actual);
  }

  #[test]
  pub fn reaction_de() {
    let json = r#"{
      "token": "XXYYZZ",
      "team_id": "TXXXXXXXX",
      "api_app_id": "AXXXXXXXXX",
      "event": {
        "type": "reaction_added",
        "user": "U024BE7LH",
        "reaction": "thumbsup",
        "item_user": "U0G9QF9C6",
        "item": {
          "type": "message",
          "channel": "C0G9QF9GZ",
          "ts": "1360782400.498405"
        },
        "event_ts": "1360782804.083113"
      },
      "type": "event_callback",
      "authed_users": [
        "UXXXXXXX1"
      ],
      "authed_teams": [
        "TXXXXXXXX"
      ],
      "authorizations": [
        {
          "enterprise_id": "E12345",
          "team_id": "T12345",
          "user_id": "U12345",
          "is_bot": false
        }
      ],
      "event_context": "EC12345",
      "event_id": "Ev08MFMKH6",
      "event_time": 1234567890
    }"#;

    let team_id = "TXXXXXXXX".to_string();
    let user = "U024BE7LH".to_string();
    let channel = "C0G9QF9GZ".to_string();
    let ts = "1360782400.498405".to_string();

    let item = ReactionAddedItem::Message { channel, ts };
    let event = EventPayload::ReactionAdded { user,
                                              reaction: "thumbsup".to_string(),
                                              item };

    let expected = Event::Event { team_id, event };

    let actual = serde_json::from_str::<Event>(json).unwrap();

    assert_eq!(expected, actual);
  }
}

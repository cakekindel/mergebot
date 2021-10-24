use mergebot::slack;
use mockito::{mock, Matcher as Match};
use reqwest::blocking::Client;

fn pretend_static<T>(t: &T) -> &'static T {
  unsafe { std::mem::transmute::<&T, &'static T>(t) }
}

fn mk_api(client: &'static Client) -> slack::Api {
  slack::Api::new(mockito::server_url(), "xoxb", client)
}

#[test]
pub fn groups() {
  use slack::groups::Groups;

  let rep = serde_json::json!({
    "ok": true,
    "users": [
      "user_a",
      "user_b"
    ]
  });

  let moq = mock("GET", "/api/usergroups.users.list?usergroup=ABC123").match_header("authorization",
                                                                                    Match::Exact("Bearer xoxb".into()))
                                                                      .with_status(200)
                                                                      .with_header("Content-Type", "application/json")
                                                                      .with_body(serde_json::to_string(&rep).unwrap())
                                                                      .create();

  let client = Client::new();
  let client_ref = &client;
  let api = mk_api(pretend_static(client_ref));

  let res = api.expand("ABC123");

  moq.assert();

  assert_eq!(res.unwrap(), vec!["user_a".to_string(), "user_b".to_string()])
}

#[test]
pub fn messages_send() {
  use slack::msg::Messages;

  let body_expected = serde_json::json!({
    "channel": "C1234",
    "blocks": [],
  });

  let rep = serde_json::json!({
    "ok": true,
    "channel": "C1234",
    "ts": "1503435956.000247",
    "message": {
      "text": "Here's a message for you",
      "username": "ecto1",
      "bot_id": "B19LU7CSY",
      "attachments": [
        {
          "text": "This is an attachment",
          "id": 1,
          "fallback": "This is an attachment's fallback"
        }
      ],
      "type": "message",
      "subtype": "bot_message",
      "ts": "1503435956.000247"
    }
  });

  let moq = mock("POST", "/api/chat.postMessage").match_header("authorization", Match::Exact("Bearer xoxb".into()))
                                                 .match_body(Match::Json(body_expected))
                                                 .with_status(200)
                                                 .with_header("Content-Type", "application/json")
                                                 .with_body(serde_json::to_string(&rep).unwrap())
                                                 .create();

  let client = Client::new();
  let client_ref = &client;
  let api = mk_api(pretend_static(client_ref));

  let res = api.send("C1234", &[]);

  moq.assert();

  assert_eq!(res.unwrap().id,
             slack::msg::Id { ts: "1503435956.000247".to_string(),
                              channel: "C1234".to_string() })
}
#[test]
pub fn messages_send_thread() {
  use slack::msg::Messages;

  let body_expected = serde_json::json!({
    "channel": "C1234",
    "thread_ts": "z1234",
    "blocks": [],
  });

  let rep = serde_json::json!({
    "ok": true,
    "channel": "C1234",
    "ts": "1503435956.000247",
    "message": {
      "text": "Here's a message for you",
      "username": "ecto1",
      "bot_id": "B19LU7CSY",
      "attachments": [
        {
          "text": "This is an attachment",
          "id": 1,
          "fallback": "This is an attachment's fallback"
        }
      ],
      "type": "message",
      "subtype": "bot_message",
      "ts": "1503435956.000247"
    }
  });

  let moq = mock("POST", "/api/chat.postMessage").match_header("authorization", Match::Exact("Bearer xoxb".into()))
                                                 .match_body(Match::Json(body_expected))
                                                 .with_status(200)
                                                 .with_header("Content-Type", "application/json")
                                                 .with_body(serde_json::to_string(&rep).unwrap())
                                                 .create();

  let client = Client::new();
  let client_ref = &client;
  let api = mk_api(pretend_static(client_ref));

  let res = api.send_thread(&slack::msg::Id { ts: "z1234".to_string(),
                                              channel: "C1234".to_string() },
                            &[]);

  moq.assert();

  assert_eq!(res.unwrap().id,
             slack::msg::Id { ts: "1503435956.000247".to_string(),
                              channel: "C1234".to_string() })
}

#[test]
pub fn authentic() {
  // from slack examples
  let signing_secret = "8f742231b10e8888abcd99yyyzzz85a5";
  let body = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";
  let timestamp = "1531420618";
  let inbound_sig = "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503";
  assert!(mergebot::slack::request_authentic(signing_secret,
                                             bytes::Bytes::from(body),
                                             http::HeaderValue::from_str(timestamp).unwrap(),
                                             http::HeaderValue::from_str(inbound_sig).unwrap()));
}

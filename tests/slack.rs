use mergebot::slack;
use mockito::{mock, Matcher as Match};
use reqwest::blocking::Client;

fn pretend_static<T>(t: &T) -> &'static T {
  unsafe { std::mem::transmute::<&T, &'static T>(t) }
}

fn mk_api(client: &'static Client) -> slack::Api {
  slack::Api::new(mockito::server_url(), "xoxb", client)
}

pub fn groups() {
  use slack::groups::Groups;

  let rep = serde_json::json!({
    "ok": true,
    "users": [
      "user_a",
      "user_b"
    ]
  });

  let _ = mock("GET", "/api/usergroups.users.list?usergroup=ABC123").match_header("authorization",
                                                                                  Match::Exact("xoxb".into()))
                                                                    .with_status(200)
                                                                    .with_header("Content-Type", "application/json")
                                                                    .with_body(serde_json::to_string(&rep).unwrap())
                                                                    .create();

  let client = Client::new();
  let client_ref = &client;
  let api = mk_api(pretend_static(client_ref));

  let res = api.expand("ABC123");

  assert!(res.is_ok());
  assert_eq!(res.unwrap(), vec!["user_a".to_string(), "user_b".to_string()])
}

pub fn messages_send() {
  use slack::msg::Messages;

  let body_expected = serde_json::json!({
    "channel": "C1234",
    "blocks": [],
  });

  let rep = serde_json::json!({
    "ok": true,
    "channel": "C1H9RESGL",
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

  let _ = mock("POST", "/api/chat.postMessage").match_header("authorization", Match::Exact("xoxb".into()))
                                               .match_body(Match::Json(body_expected))
                                               .with_status(200)
                                               .with_header("Content-Type", "application/json")
                                               .with_body(serde_json::to_string(&rep).unwrap())
                                               .create();

  let client = Client::new();
  let client_ref = &client;
  let api = mk_api(pretend_static(client_ref));

  let res = api.send("C1234", &[]);

  assert!(res.is_ok());
  assert_eq!(res.unwrap().id,
             slack::msg::Id { ts: "1503435956.000247".to_string(),
                              channel: "C1234".to_string() })
}
pub fn messages_send_thread() {
  use slack::msg::Messages;

  let body_expected = serde_json::json!({
    "channel": "C1234",
    "thread_ts": "z1234",
    "blocks": [],
  });

  let rep = serde_json::json!({
    "ok": true,
    "channel": "C1H9RESGL",
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

  let _ = mock("POST", "/api/chat.postMessage").match_header("authorization", Match::Exact("xoxb".into()))
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

  assert!(res.is_ok());
  assert_eq!(res.unwrap().id,
             slack::msg::Id { ts: "1503435956.000247".to_string(),
                              channel: "C1234".to_string() })
}

use serde::{Deserialize as De, Serialize as Ser};

use super::*;
use crate::deploy;

/// Implements Messenger for slack
#[derive(Clone, Copy, Debug)]
pub struct SlackMessenger;

/// A messenger is able to notify the approvers of an app of a deployment
pub trait Messenger {
  /// Notify approvers of an app for deployment
  fn send_message_for_job(&self,
                          client: &reqwest::blocking::Client,
                          slack_token: impl AsRef<str>,
                          job: &Job)
                          -> Result<String, MessagingError>;
}

fn fmt_approvers(approvers: &[&deploy::app::User]) -> String {
  if approvers.len() == 1 {
    let usr = approvers.get(0).unwrap();
    return usr.to_at();
  }

  approvers.into_iter().map(|user| user.to_at()).rfold(
                                                       String::new(),
                                                       |msg, at| {
                                                         if &msg == "" {
               format!("& {}", at)
             } else if msg.starts_with('&') {
               format!("{} {}", at, msg)
             } else {
               format!("{}, {}", at, msg)
             }
                                                       },
  )
}

#[derive(Clone, Ser, De)]
struct Rep {
  ok: bool,
  error: Option<String>,
  ts: Option<String>,
}

/// Errors encounterable during sending a slack message
#[derive(Debug)]
pub enum MessagingError {
  /// There was an issue establishing a connection,
  /// status code was not 200,
  /// or response body couldn't be deserialized
  Http(reqwest::Error),
  /// Slack received our request fine, but didn't like it
  Slack(String),
}

impl Messenger for SlackMessenger {
  fn send_message_for_job(&self,
                          client: &reqwest::blocking::Client,
                          slack_token: impl AsRef<str>,
                          job: &Job)
                          -> Result<String, MessagingError> {
    let users = job.app
                   .repos
                   .iter()
                   .flat_map(|repo| {
                     repo.environments.iter().flat_map(|env| env.users.iter())
                   })
                   .collect::<Vec<_>>();
    let approvers = users.iter()
                         .map(|u| *u)
                         .filter(|u| u.is_approver())
                         .collect::<Vec<_>>();

    let blocks: Vec<slack_blocks::Block> = {
      use slack_blocks::blox::*;

      vec![
        blox! {
          <section_block>
            <text kind=mrkdwn>{format!("<!here> <@{}> has requested a deployment for {} to {}.", job.command.user_id, job.app.name, job.command.env_name)}</text>
          </section_block>
        }.into(),
        blox!{
          <section_block>
            <text kind=mrkdwn>{format!("I need {} to this message with :+1: in order to continue.", fmt_approvers(&approvers))}</text>
          </section_block>
        }.into(),
      ]
    };

    client.post("https://slack.com/api/chat.postMessage")
          .json(&serde_json::json!({
                  "token": slack_token.as_ref(),
                  "channel": job.app.notification_channel_id,
                  "blocks": serde_json::to_value(blocks).unwrap(),
                }))
          .header("authorization", format!("Bearer {}", slack_token.as_ref()))
          .send()
          .and_then(|rep| rep.error_for_status())
          .and_then(|rep| rep.json::<Rep>())
          .map_err(MessagingError::Http)
          .and_then(|rep| match rep.ok {
            | true => Ok(rep.ts.unwrap()),
            | false => Err(MessagingError::Slack(rep.error.unwrap())),
          })
  }
}

use super::*;
use crate::{deploy, slack};

/// Implements Messenger for slack
#[derive(Clone, Copy, Debug)]
pub struct SlackMessenger;

/// A messenger is able to notify the approvers of an app of a deployment
pub trait Messenger: 'static + Sync + Send + std::fmt::Debug {
  /// Notify approvers of an app for deployment
  fn send_message_for_job(&self, job: &Job) -> Result<slack::msg::Id, slack::Error>;
}

fn fmt_approvers(approvers: &[&deploy::app::User]) -> String {
  if approvers.len() == 1 {
    let usr = approvers.get(0).unwrap();
    return usr.to_at();
  }

  approvers.iter()
           .map(|user| user.to_at())
           .rfold(String::new(), |msg, at| {
             if msg.is_empty() {
               format!("& {}", at)
             } else if msg.starts_with('&') {
               format!("{} {}", at, msg)
             } else {
               format!("{}, {}", at, msg)
             }
           })
}

impl<T: slack::msg::Messages> Messenger for T {
  fn send_message_for_job(&self, job: &Job) -> Result<slack::msg::Id, slack::Error> {
    let users = job.app
                   .repos
                   .iter()
                   .flat_map(|repo| repo.environments.iter().flat_map(|env| env.users.iter()))
                   .collect::<Vec<_>>();

    let approvers = users.iter().copied().filter(|u| u.is_approver()).collect::<Vec<_>>();

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

    self.send(&job.app.notification_channel_id, &blocks).map(|rep| rep.id)
  }
}

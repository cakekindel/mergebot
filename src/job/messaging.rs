use super::*;
use crate::{deploy, job, slack};

/// A messenger is able to notify the approvers of an app of a deployment
pub trait Messenger: 'static + Sync + Send + std::fmt::Debug {
  /// Notify approvers of an app for deployment
  fn send_job_created(&self, job: &Job<job::StateInit>) -> slack::Result<slack::msg::Id>;

  /// Notify that the job has been approved
  fn send_job_approved(&self, job: &Job<job::StateApproved>) -> slack::Result<slack::msg::Id>;

  /// Notify that the job has failed
  fn send_job_failed(&self, job: &Job<job::StatePoisoned>) -> slack::Result<slack::msg::Id>;

  /// Notify that the job has been executed
  fn send_job_done(&self, job: &Job<job::StateDone>) -> slack::Result<slack::msg::Id>;
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
  fn send_job_created(&self, job: &Job<job::StateInit>) -> slack::Result<slack::msg::Id> {
    let users = job.app
                   .repos
                   .iter()
                   .flat_map(|repo| {
                     repo.environments
                         .iter()
                         .filter(|env| env.name_eq(&job.command.env_name))
                         .flat_map(|env| env.users.iter())
                   })
                   .collect::<Vec<_>>();

    let mut approvers = users.iter().copied().filter(|u| u.is_approver()).collect::<Vec<_>>();
    approvers.dedup();

    let blocks: Vec<slack_blocks::Block> = {
      use slack_blocks::blox::*;

      vec![
        blox! {
          <section_block>
            <text kind=mrkdwn>{format!("<!here> <@{}> has requested a deploy merge for {} to {}.", job.command.user_id, job.app.name, job.command.env_name)}</text>
          </section_block>
        }.into(),
        blox!{
          <section_block>
            <text kind=mrkdwn>{format!("I need {} to react to this message with :+1: in order to continue.", fmt_approvers(&approvers))}</text>
          </section_block>
        }.into(),
      ]
    };

    self.send(&job.app.team_id, &job.app.notification_channel_id, &blocks)
        .map(|rep| rep.id)
  }

  /// Notify that the job has been executed
  fn send_job_approved(&self, job: &Job<job::StateApproved>) -> slack::Result<slack::msg::Id> {
    let id_missing = slack::Error::Other(String::from("no message to respond to"));
    let id = job.state.prev.msg_id.as_ref().ok_or(id_missing)?;

    let blocks: Vec<slack_blocks::Block> = {
      use slack_blocks::blox::*;
      vec![blox! {
             <section_block>
               <text kind=mrkdwn>
                 {format!("Merge approved! :sunglasses: Let's go to {} :rocket:", job.command.env_name)}
               </text>
             </section_block>
           }.into()]
    };

    self.send_thread(&job.app.team_id, id, &blocks).map(|rep| rep.id)
  }

  /// Notify that job has failed (poison)
  fn send_job_failed(&self, job: &Job<job::StatePoisoned>) -> slack::Result<slack::msg::Id> {
    let id_missing = slack::Error::Other(String::from("no message to respond to"));
    let id = job.state
                .prev // errored
                .prev // approved
                .prev // init
                .msg_id
                .as_ref()
                .ok_or(id_missing)?;

    let blocks: Vec<slack_blocks::Block> = {
      use slack_blocks::blox::*;
      vec![blox! {
             <section_block>
               <text kind=mrkdwn>
                 {"Merge failed :skull_and_crossbones:"}
               </text>
             </section_block>
           }.into()]
    };

    self.send_thread(&job.app.team_id, id, &blocks).map(|rep| rep.id)
  }

  fn send_job_done(&self, job: &job::Job<job::StateDone>) -> slack::Result<slack::msg::Id> {
    let id_missing = slack::Error::Other(String::from("no message to respond to"));
    let id = match &job.state {
               | job::StateDone::Succeeded(ref app) => &app.prev.msg_id,
               | job::StateDone::SucceededAfterRetry(ref app) => &app.prev.prev.msg_id,
             }.as_ref()
              .ok_or(id_missing)?;

    let blocks: Vec<slack_blocks::Block> = {
      use slack_blocks::blox::*;
      vec![blox! {
             <section_block>
               <text kind=mrkdwn>
                 {"Deploy merge succeeded! :rocket:"}
               </text>
             </section_block>
           }.into()]
    };

    self.send_thread(&job.app.team_id, id, &blocks).map(|rep| rep.id)
  }
}

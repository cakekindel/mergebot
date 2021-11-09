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

fn fmt_approvers(approvers: &[deploy::app::User]) -> String {
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

fn job_created_msg(job: &Job<job::StateInit>) -> Vec<slack_blocks::Block<'static>> {
  use slack_blocks::{blox::*, Block};

  struct RepoContext {
    repo: deploy::Repo,
    env: deploy::Mergeable,
  }

  let (mut changes, mut ctas) =
    job.clone()
       .app
       .repos
       .iter()
       .map(|repo| {
         let env = repo.environments
                       .iter()
                       .find(|env| env.name_eq(&job.command.env_name))
                       .expect(&format!("env of name {} should exist", job.command.env_name))
                       .clone();
         RepoContext { repo: repo.clone(),
                       env }
       })
       .enumerate()
       .fold((Vec::<Block>::new(), Vec::<Block>::new()),
             move |(mut changes, mut ctas), (ix, ctx)| {
               let change: Block = blox! {
                                     <context_block>
                                       <text kind=mrkdwn>{
                                         format!(
                                           "{} changes: {}/compare/{}..{}",
                                           ctx.repo.name,
                                           ctx.repo.human_url,
                                           ctx.env.target.0,
                                           ctx.env.base.0
                                         )
                                       }</text>
                                     </context_block>
                                   }.into();
               let cta_text = if ix == 0 {
                 format!("In order to merge {}, I need {} to react to this message with :+1:.",
                         ctx.repo.name,
                         fmt_approvers(&ctx.env.users))
               } else {
                 format!("For {}, I need {} to approve.",
                         ctx.repo.name,
                         fmt_approvers(&ctx.env.users))
               };
               let cta: Block = blox! {
                                  <section_block>
                                    <text kind=mrkdwn>{cta_text}</text>
                                  </section_block>
                                }.into();
               changes.push(change);
               ctas.push(cta);
               (changes, ctas)
             });

  let mut blocks = vec![
    blox! {
      <section_block>
        <text kind=mrkdwn>{format!("<!here> <@{}> has requested a deploy merge for {} to {}.", job.command.user_id, job.app.name, job.command.env_name)}</text>
      </section_block>
    }.into(),
  ];

  blocks.append(&mut changes);
  blocks.append(&mut ctas);

  blocks
}

impl<T: slack::msg::Messages> Messenger for T {
  fn send_job_created(&self, job: &Job<job::StateInit>) -> slack::Result<slack::msg::Id> {
    let blocks = job_created_msg(job);

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_job_created_msg() {
    // json here is much more concise than struct initializers
    let job = serde_json::json!({
      "id": job::Id::new(),
      "state": {
        "msg_id": null,
        "approved_by": []
      },
      "command": {
        "app_name": "my_app",
        "env_name": "prod",
        "user_id": "U123",
        "team_id": "T123"
      },
      "app": {
        "name": "my_app",
        "team_id": "T123",
        "notification_channel_id": "C123",
        "repos": [
          {
            "url": "git@foo.com:my/repo",
            "human_url": "foo.com/my/repo",
            "name": "ui",
            "environments": [
              {
                "name": "prod",
                "base": "staging",
                "target": "prod",
                "users": [
                  {
                    "user_id": "U123",
                    "approver": true
                  }
                ]
              }
            ]
          },
          {
            "url": "git@foo.com:my/repo2",
            "human_url": "foo.com/my/repo2",
            "name": "backend",
            "environments": [
              {
                "name": "prod",
                "base": "staging",
                "target": "prod",
                "users": [
                  {
                    "user_id": "U123",
                    "approver": true
                  },
                  {
                    "group_id": "G123",
                    "min_approvers": 2
                  }
                ]
              }
            ]
          }
        ]
      }
    });

    let job = serde_json::from_value::<Job<job::StateInit>>(job).unwrap();

    println!("{}", serde_json::to_string_pretty(&job).unwrap());

    let msg = job_created_msg(&job);

    let expected = {
      use slack_blocks::blox::*;

      vec![
        blox!{<section_block><text kind=mrkdwn>{"<!here> <@U123> has requested a deploy merge for my_app to prod."}</text></section_block>}.into(),
        blox!{<context_block><text kind=mrkdwn>{"ui changes: foo.com/my/repo/compare/prod..staging"}</text></context_block>}.into(),
        blox!{<context_block><text kind=mrkdwn>{"backend changes: foo.com/my/repo2/compare/prod..staging"}</text></context_block>}.into(),
        blox!{<section_block><text kind=mrkdwn>{"In order to merge ui, I need <@U123> to react to this message with :+1:."}</text></section_block>}.into(),
        blox!{<section_block><text kind=mrkdwn>{"For backend, I need <@U123> & 2 members of <!subteam^G123> to approve."}</text></section_block>}.into(),
      ]
    };

    assert_eq!(msg, expected)
  }
}

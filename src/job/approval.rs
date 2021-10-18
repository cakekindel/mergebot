use super::*;
use super::event::*;

/// On approval, check if fully approved, change state, and log
pub fn on_approval(_: &'static State) -> impl Fn(Box<dyn JobStore>, Event) {
  |jobs, ev| {
    match ev {
      Event::Approved(job) => {
        log::info!("(job {}) approved by {:#?}", job.id, user);

        let need_approvers = job.outstanding_approvers();
        if need_approvers.is_empty() {
          log::info!("(job {}) fully approved", job.id);
          jobs.fully_approved(&job.id);
        } else {
          log::info!("(job {}) still needs approvers: {:?}", job.id, need_approvers);
        }
      },
      _ => (),
    }
  }
}

/// Send message on full approval
pub fn on_full_approval_notify(state: &'static State) -> impl Fn(Box<dyn JobStore>, Event) {
  |jobs, ev| {
    let fully_approved = |job| job.outstanding_approvers().is_empty();

    match ev {
      Event::FullyApproved(job) => {
        if let Err(e) = state.job_messenger.send_job_approved(&job) {
          log::error!("{:#?}", e);
        }
      },
      _ => (),
    }
  }
}

/// Deploy on full approval
pub fn on_full_approval_deploy(state: &'static State) -> impl Fn(Box<dyn JobStore>, Event) {
  |jobs, ev| {
    let fully_approved = |job| job.outstanding_approvers().is_empty();

    match ev {
      Event::FullyApproved(job) => {
        if let Err(e) = state.job_executor.schedule_exec(&job) {
          log::error!("{:#?}", e);
        }
      },
      _ => (),
    }
  }
}

use super::{event::*, *};

/// On approval, check if fully approved, change state, and log
pub fn on_approval(_: &'static crate::State) -> Listener {
  fn cloj<'a>(jobs: &'a dyn Store, ev: Event<'a>) {
    match ev {
      | Event::Approved(job, user) => {
        log::info!("job {:?} approved by {:#?}", job.id, user);

        let need_approvers = job.outstanding_approvers();
        if need_approvers.is_empty() {
          log::info!("job {:?} fully approved", job.id);

          if let None = jobs.fully_approved(&job.id) {
            log::error!("job {:?} was not marked approved", job.id);
          }
        } else {
          log::info!("job {:?} still needs approvers: {:?}", job.id, need_approvers);
        }
      },
      | _ => (),
    }
  }

  Box::from(cloj)
}

/// Send message on full approval
pub fn on_full_approval_notify(state: &'static crate::State) -> Listener {
  let f = move |_: &dyn Store, ev: Event| match ev {
    | Event::FullyApproved(job) => {
      log::info!("job {:?}: sending approval message...", job.id);

      if let Err(e) = state.job_messenger.send_job_approved(&job) {
        log::error!("{:#?}", e);
      }

      log::info!("job {:?}: approval message sent", job.id);
    },
    | _ => (),
  };

  Box::from(f)
}

/// Deploy on full approval
pub fn on_full_approval_deploy(state: &'static crate::State) -> Listener {
  let f = move |_: &dyn Store, ev: Event| match ev {
    | Event::FullyApproved(job) => {
      log::info!("job {:?}: deploying", job.id);
      state.job_executor.schedule_exec(&job);
    },
    | _ => (),
  };

  Box::from(f)
}

/// If failed beyond threshold, mark as poisoned
pub fn on_failure_poison(_: &'static crate::State) -> Listener {
  let f = move |jobs: &dyn Store, ev: Event| match ev {
    | Event::Errored(j) => {
      let errs = j.flatten_errors();
      if errs.len() > 4 {
        log::error!("job {:?} poisoned!!1", j.id);
        jobs.state_poisoned(&j.id);
      }
    },
    | _ => (),
  };

  Box::from(f)
}

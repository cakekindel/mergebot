use super::event::*;
use crate::result_extra::*;

pub fn on_create_notify(state: &'static crate::State) -> Listener {
  let cloj = move |ev: Event| {
    if let Event::Created(job) = ev {
      log::info!("job {:?} created", job.id);
      state.job_messenger
           .send_job_created(&job)
           .tap_err(|e| log::error!("job {:?}: error notifying create {:?}", job.id, e))
           .tap(|msg_id| {
             state.jobs.notified(&job.id, msg_id.clone());
           })
           .ok();
    }
  };

  Box::from(cloj)
}

/// On approval, check if fully approved, change state, and log
pub fn on_full_approval_change_state(state: &'static crate::State) -> Listener {
  let cloj = move |ev: Event| {
    if let Event::Approved(job, user) = ev {
      log::info!("job {:?} approved by {:#?}", job.id, user);

      let need_approvers = job.outstanding_approvers();
      if need_approvers.is_empty() {
        log::info!("job {:?} fully approved", job.id);

        let id = job.id.clone();

        // nested event emissions cause deadlock
        // so we essentially queue the op in another thread
        std::thread::spawn(move || {
          state.jobs.fully_approved(&id);
        });
      } else {
        log::info!("job {:?} still needs approvers: {:?}", job.id, need_approvers);
      }
    }
  };

  Box::from(cloj)
}

/// Send message on full approval
pub fn on_full_approval_notify(state: &'static crate::State) -> Listener {
  let f = move |ev: Event| match ev {
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
  let f = move |ev: Event| match ev {
    | Event::FullyApproved(job) => {
      log::info!("job {:?}: deploying", job.id);
      state.job_executor.schedule_exec(&job);
    },
    | _ => (),
  };

  Box::from(f)
}

/// If failed beyond threshold, mark as poisoned
pub fn on_failure_poison(state: &'static crate::State) -> Listener {
  let f = move |ev: Event| match ev {
    | Event::Errored(j) => {
      let errs = j.flatten_errors();
      if errs.len() > 4 {
        log::error!("job {:?} poisoned!!1", j.id);
        let id = j.id.clone();

        std::thread::spawn(move || {
          state.jobs.state_poisoned(&id);
        });
      }
    },
    | _ => (),
  };

  Box::from(f)
}

/// If poisoned, send slack message
pub fn on_poison_notify(state: &'static crate::State) -> Listener {
  let f = move |ev: Event| match ev {
    | Event::Poisoned(j) => {
      if let Err(e) = state.job_messenger.send_job_failed(&j) {
        log::error!("job {:?}: failed to send 'job failed' message {:?}", j.id, e);
      }
    },
    | _ => (),
  };

  Box::from(f)
}

/// If done, send slack message
pub fn on_done_notify(state: &'static crate::State) -> Listener {
  let f = move |ev: Event| match ev {
    | Event::Done(j) => {
      if let Err(e) = state.job_messenger.send_job_done(&j) {
        log::error!("job {:?}: failed to send 'job done' message {:?}", j.id, e);
      }
    },
    | _ => (),
  };

  Box::from(f)
}

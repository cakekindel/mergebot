use std::{sync::{Condvar, Mutex, MutexGuard},
          time::Duration};

use chrono::Utc;

use crate::{git, job, job::Job, mutex_extra::lock_discard_poison};

/// Initialize executor worker thread
pub fn init(job_q: Box<dyn job::Queue>, git: Box<dyn crate::git::Client>) {
  *lock_discard_poison(&JOB_QUEUE) = Some(job_q);
  *lock_discard_poison(&GIT_CLIENT) = Some(git);
}

// Dependencies
lazy_static::lazy_static! {
  pub(super) static ref JOB_QUEUE: Mutex<Option<Box<dyn job::Queue>>> = Mutex::new(None);
  pub(super) static ref GIT_CLIENT: Mutex<Option<Box<dyn git::Client>>> = Mutex::new(None);
}

// Worker variables
lazy_static::lazy_static! {
  /// Worker thread queue
  static ref QUEUE: Mutex<Vec<Work>> = Mutex::new(Vec::new());
  /// Notifies worker thread to wake up if it slept on empty queue
  static ref WORK_QUEUED: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());
  /// Handle for worker thread
  static ref WORKER: std::thread::JoinHandle<()> = std::thread::spawn(worker);
}

/// Work to be picked up by worker thread
struct Work {
  job: Job,
}

fn exec(job: &Job) {
  // trust someone above us to make sure these are set before a job gets here
  let job_q_lock = lock_discard_poison(&JOB_QUEUE);
  let git_lock = lock_discard_poison(&GIT_CLIENT);

  let job_q = job_q_lock.as_ref().unwrap();
  let git = git_lock.as_ref().unwrap();

  let errs = job.app
                   .repos
                   .iter()
                   .map(|app_repo| {
                     let env = app_repo.environments
                                       .iter()
                                       .find(|env| env.name_eq(&job.command.env_name))
                                       .expect("Environment was already matched against command");

                     git.repo(&app_repo.url, &job.app.name).and_then(|repo| {
                                                             // fetch all upstreams
                                                             repo.fetch_all()?;
                                                             // switch base
                                                             repo.switch(&env.base)?;
                                                             // make sure base up to date
                                                             repo.update_branch()?;
                                                             // merge in target's upstream
                                                             repo.upstream(&env.target).and_then(|b| repo.merge(&b))?;

                                                             // push changes
                                                             repo.push()
                                                           })
                   })

  .filter_map(|r| r.err()).collect::<Vec<_>>();

  if errs.is_empty() {
    job_q.set_state(&job.id, job::State::Done);
  } else {
    let attempts = match &job.state {
      | &job::State::Errored { attempts, .. } => attempts + 1,
      | _ => 1,
    };

    let next_attempt = Utc::now() + chrono::Duration::seconds(5);

    log::error!("job {}: executing encountered errors {:?}", job.id, errs);
    let new_state = job::State::Errored { attempts,
                                          next_attempt,
                                          errs };

    job_q.set_state(&job.id, new_state);
  }
}

/// Implementor of super::Executor
#[derive(Clone, Copy, Debug)]
pub struct Executor;

impl super::Executor for Executor {
  fn schedule_exec(&self, job: &Job) -> super::Result<()> {
    match &job.state {
      | &job::State::Approved { .. } => (),
      | s => return Err(super::Error::InvalidJobState(s.clone())),
    };

    let work = Work { job: job.clone() };
    let q = &mut *lock_discard_poison(&QUEUE);
    q.push(work);

    Ok(())
  }
}

/// Pull work out of the work queue
fn get_work() -> Option<(Work, Duration)> {
  let mut q: MutexGuard<'_, Vec<Work>> = lock_discard_poison(&QUEUE);
  let mut work = (*q).iter()
                     .enumerate()
                     .filter_map(|(ix, w)| match w.job.state {
                       | job::State::WorkQueued => Some((ix, Default::default())),
                       | job::State::Errored { next_attempt, .. } => {
                         let dur: chrono::Duration = next_attempt.signed_duration_since(Utc::now());
                         Some((ix, dur.to_std().unwrap_or_default()))
                       },
                       | _ => {
                         log::error!("job of state {:?} should not be in work queue", w.job.state);
                         None
                       },
                     })
                     .collect::<Vec<_>>();

  work.sort_by_key(|(_, dur)| *dur);

  // Yield the work to be done soonest
  work.get(0).map(|&(ix, dur)| ((*q).remove(ix), dur))
}

/// Worker thread logic
fn worker() {
  use std::thread::sleep;

  loop {
    if let Some((work, time_til)) = get_work() {
      log::info!("job {}: work picked", work.job.id);

      if !time_til.is_zero() {
        // REVISIT: if this wait is long, fresh work
        //          will be blocked until done waiting
        log::info!("job {}: waiting for {}ms to retry", work.job.id, time_til.as_millis());
        sleep(time_til);
      }

      log::info!("job {}: working", work.job.id);
      exec(&work.job);
    } else {
      log::info!("Waiting for work to be queued");
      let lock = lock_discard_poison(&WORK_QUEUED.0);

      let work_queued =
        WORK_QUEUED.1
                   .wait(lock)
                   .expect("if a thread (this or main) panicked holding a lock, this code is unreachable.");

      drop(work_queued);
    }
  }
}

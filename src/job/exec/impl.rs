use std::{sync::{Condvar, Mutex, MutexGuard},
          time::Duration, thread};

use chrono::Utc;

use crate::{git, job, job::Job, mutex_extra::lock_discard_poison};

/// Initialize executor worker thread
pub fn init(job_q: Box<dyn job::Queue>, git: Box<dyn crate::git::Client>) {
  #[allow(unsafe_code)]
  // use a mut static for one-time initialization of the worker thread
  unsafe {
    WORKER = Some(std::thread::spawn(worker));
  }
  *lock_discard_poison(&JOB_QUEUE) = Some(job_q);
  *lock_discard_poison(&GIT_CLIENT) = Some(git);
}

/// Worker thread handle
static mut WORKER: Option<thread::JoinHandle<()>> = None;

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
}

/// Work to be picked up by worker thread
enum Work {
  New(Job<job::StateApproved>),
  Retry(Job<job::StateErrored>),
}

impl Work {
  fn time_til(&self) -> Duration {
    match self {
      Self::Retry(job) => {
        job.state.next_attempt.signed_duration_since(Utc::now()).to_std().unwrap_or_default()
      },
      Self::New(job) => {
        Duration::default()
      },
    }
  }

  fn queue(self) -> () {
    let q = &mut *lock_discard_poison(&QUEUE);
    q.push(work);

    WORK_QUEUED.1.notify_all();
  }
}

fn exec<S: job::State>(job: &Job<S>) {
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
                .filter_map(|r| r.err())
                .collect::<Vec<_>>();

  if errs.is_empty() {
    jobs.state_done(&job.id);
  } else {
    jobs.state_errored(&job.id, errs);

    let work = Work::Errored(jobs.get_errored(&job.id).unwrap());
    work.queue();
  }
}

/// Implementor of super::Executor
#[derive(Clone, Copy, Debug)]
pub struct Executor;

impl super::Executor for Executor {
  fn schedule_exec(&self, job: &Job<job::StateApproved>) {
    let work = Work::New(job.clone());
    work.queue();
  }
}

/// Pull work out of the work queue
fn get_work() -> Option<(Work, Duration)> {
  let mut q: MutexGuard<'_, Vec<Work>> = lock_discard_poison(&QUEUE);
  let mut work = (*q).iter()
                     .enumerate()
                     .map(|(ix, w)| (ix, w.time_til()))
                     .collect::<Vec<_>>();

  work.sort_by_key(|(_, dur)| *dur);

  // Yield the work to be done soonest
  work.get(0).map(|&(ix, dur)| ((*q).remove(ix), dur))
}

/// Worker thread logic
fn worker() {
  use std::thread::sleep;

  std::sync::Arc::clone(&crate::APP_INIT).wait();

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

      let work_queued = WORK_QUEUED.1.wait(lock).unwrap();

      drop(work_queued);
    }
  }
}

use std::{sync::{Condvar, Mutex, MutexGuard},
          thread,
          time::Duration};

use chrono::Utc;

use crate::{git, job, job::Job, mutex_extra::lock_discard_poison};

/// Initialize executor worker thread
pub fn init(jobs: Box<dyn job::Store>, git: Box<dyn crate::git::Client>) {
  #[allow(unsafe_code)]
  // use a mut static for one-time initialization of the worker thread
  unsafe {
    WORKER = Some(std::thread::spawn(worker));
  }

  *lock_discard_poison(&JOB_STORE) = Some(jobs);
  *lock_discard_poison(&GIT_CLIENT) = Some(git);
}

/// Worker thread handle
static mut WORKER: Option<thread::JoinHandle<()>> = None;

// Dependencies
lazy_static::lazy_static! {
  pub(super) static ref JOB_STORE: Mutex<Option<Box<dyn job::Store>>> = Mutex::new(None);
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
  fn job(&self) -> Job<job::States> {
    use job::State;

    match self {
      | Self::New(j) => j.map_state(|s| s.into_states()),
      | Self::Retry(j) => j.map_state(|s| s.into_states()),
    }
  }

  fn time_til(&self) -> Duration {
    match self {
      | Self::Retry(job) => job.state
                               .next_attempt
                               .signed_duration_since(Utc::now())
                               .to_std()
                               .unwrap_or_default(),
      | Self::New(_) => Duration::default(),
    }
  }

  fn queue(self) {
    let q = &mut *lock_discard_poison(&QUEUE);
    q.push(self);

    WORK_QUEUED.1.notify_all();
  }
}

fn exec<S: job::State>(job: &Job<S>) {
  // trust someone above us to make sure these are set before a job gets here
  let jobs_lock = lock_discard_poison(&JOB_STORE);
  let git_lock = lock_discard_poison(&GIT_CLIENT);

  let jobs = jobs_lock.as_ref().unwrap();
  let git = git_lock.as_ref().unwrap();

  let errs = job.app
                .repos
                .iter()
                .map(|app_repo| {
                  let env = app_repo.environments
                                    .iter()
                                    .find(|env| env.name_eq(&job.command.env_name))
                                    .expect("Environment was already matched against command");

                  git.repo(&app_repo.url, &app_repo.name).and_then(|repo| {
                                                           repo.fetch_all()?;

                                                           repo.switch(&env.base)?;
                                                           repo.update_branch()?;

                                                           repo.switch(&env.target)?;
                                                           repo.update_branch()?;

                                                           repo.merge(&env.base)?;

                                                           repo.push()
                                                         })
                })
                .filter_map(|r| r.err().map(job::Error::Git))
                .collect::<Vec<_>>();

  if errs.is_empty() {
    jobs.state_done(&job.id);
  } else {
    jobs.state_errored(&job.id, errs);

    // The above call may poison the job
    if let Some(j) = jobs.get_errored(&job.id) {
      let work = Work::Retry(j);
      work.queue();
    }
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
      log::info!("job {:?}: work picked", work.job().id);

      if !time_til.is_zero() {
        // REVISIT: if this wait is long, fresh work
        //          will be blocked until done waiting
        log::info!("job {:?}: waiting for {}ms to retry",
                   work.job().id,
                   time_til.as_millis());
        sleep(time_til);
      }

      log::info!("job {:?}: working", work.job().id);
      exec(&work.job());
    } else {
      log::info!("worker thread up");

      let lock = lock_discard_poison(&WORK_QUEUED.0);

      let work_queued = WORK_QUEUED.1.wait(lock).unwrap();

      drop(work_queued);
    }
  }
}

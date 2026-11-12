//! Generic background-job abstraction with cooperative cancellation.
//!
//! Every long-running operation (create / verify / repair / batch) runs on its
//! own thread and reports progress through a shared [`Job`]. The UI thread
//! polls the job every frame — it never blocks. [`Job::cancel`] sets a flag
//! the job thread (and any spawned subprocess) observes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Message produced by jobs that were stopped through [`Job::cancel`].
pub const CANCELLED: &str = "Operation cancelled.";

/// Current progress state of a job.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Percent complete, clamped to `0.0..=100.0`.
    pub pct: f32,
    /// Human readable description of what the job is doing right now.
    pub message: String,
}

impl Progress {
    fn new(message: impl Into<String>) -> Self {
        Self {
            pct: 0.0,
            message: message.into(),
        }
    }
}

/// Handle to progress state, handed to the closure running on the job thread.
#[derive(Clone)]
pub struct Reporter {
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    /// When set, reports are written to stdout as worker-protocol lines
    /// (see `crate::worker`) instead of updating shared state.
    worker_mode: bool,
}

impl Reporter {
    fn new(worker_mode: bool) -> Self {
        Self {
            progress: Arc::new(Mutex::new(Progress::new(""))),
            cancel: Arc::new(AtomicBool::new(false)),
            worker_mode,
        }
    }

    /// Report the current progress.
    pub fn report(&self, pct: f32, message: impl Into<String>) {
        let message = message.into();
        if self.worker_mode {
            // Worker protocol line, parsed by the parent process.
            println!("PROGRESS\t{pct:.1}\t{message}");
            return;
        }
        let mut progress = self.progress.lock().unwrap();
        progress.pct = pct.clamp(0.0, 100.0);
        progress.message = message;
    }

    /// Whether the UI requested cancellation of this job.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Shared flag used by process-based backends to observe cancellation.
    pub(crate) fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Reporter for the headless worker subprocess (`--par2-worker`).
    pub(crate) fn new_worker() -> Self {
        Self::new(true)
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::new(false)
    }

    #[cfg(test)]
    pub(crate) fn last_report(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }
}

/// A running (or finished) background job with a result of type `T`.
pub struct Job<T: Send + 'static> {
    progress: Arc<Mutex<Progress>>,
    /// Set by the job thread *after* the result is stored (release ordering
    /// guarantees the UI thread sees the result once `finished` is true).
    finished: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<T>>>,
}

impl<T: Send + 'static> Job<T> {
    /// Spawn `f` on a background thread. The closure receives a [`Reporter`]
    /// for progress updates and returns the job result.
    pub fn spawn(message: impl Into<String>, f: impl FnOnce(&Reporter) -> T + Send + 'static) -> Self {
        let progress = Arc::new(Mutex::new(Progress::new(message)));
        let finished = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));

        let reporter = Reporter::new(false);
        let cancel = Arc::clone(&reporter.cancel);
        let finished_thread = Arc::clone(&finished);
        let result_thread = Arc::clone(&result);

        // The JoinHandle is intentionally dropped: the thread detaches and
        // completes on its own. Dropping the `Job` must never block the UI.
        std::thread::spawn(move || {
            let outcome = f(&reporter);
            *result_thread.lock().unwrap() = Some(outcome);
            finished_thread.store(true, Ordering::Release);
        });

        Self {
            progress,
            finished,
            cancel,
            result,
        }
    }

    /// Whether the job is still running.
    pub fn is_running(&self) -> bool {
        !self.finished.load(Ordering::Acquire)
    }

    /// A snapshot of the job's current progress.
    pub fn progress(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }

    /// Take the result once the job has finished. Returns `None` while the
    /// job is running or if the result was already taken.
    pub fn take_result(&self) -> Option<T> {
        if self.finished.load(Ordering::Acquire) {
            self.result.lock().unwrap().take()
        } else {
            None
        }
    }

    /// Request cancellation. The job thread observes this through
    /// `Reporter::is_cancelled` (and process-based backends kill their
    /// subprocess with it). The result, once taken, carries the outcome.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Whether [`Job::cancel`] was called for this job.
    pub fn was_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Job, CANCELLED};

    #[test]
    fn cancel_flag_reaches_reporter_and_finishes_job() {
        let job: Job<String> = Job::spawn("testing", |reporter| {
            while !reporter.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            CANCELLED.to_string()
        });
        assert!(!job.was_cancelled());

        job.cancel();
        while job.is_running() {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(job.take_result().as_deref(), Some(CANCELLED));
        assert!(job.was_cancelled());
    }

    #[test]
    fn cancel_on_finished_job_is_harmless() {
        let job: Job<u32> = Job::spawn("testing", |reporter| {
            reporter.report(100.0, "done");
            42
        });
        while job.is_running() {
            std::thread::sleep(Duration::from_millis(5));
        }
        job.cancel();
        assert_eq!(job.take_result(), Some(42));
        assert!(job.was_cancelled());
    }
}

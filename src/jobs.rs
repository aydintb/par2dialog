//! Generic background-job abstraction.
//!
//! Every long-running operation (create / verify / repair / batch) runs on its
//! own thread and reports progress through a shared [`Job`]. The UI thread
//! polls the job every frame — it never blocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
}

impl Reporter {
    /// Report the current progress.
    pub fn report(&self, pct: f32, message: impl Into<String>) {
        let mut progress = self.progress.lock().unwrap();
        progress.pct = pct.clamp(0.0, 100.0);
        progress.message = message.into();
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            progress: Arc::new(Mutex::new(Progress::new(""))),
        }
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
    result: Arc<Mutex<Option<T>>>,
}

impl<T: Send + 'static> Job<T> {
    /// Spawn `f` on a background thread. The closure receives a [`Reporter`]
    /// for progress updates and returns the job result.
    pub fn spawn(message: impl Into<String>, f: impl FnOnce(&Reporter) -> T + Send + 'static) -> Self {
        let progress = Arc::new(Mutex::new(Progress::new(message)));
        let finished = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));

        let reporter = Reporter {
            progress: Arc::clone(&progress),
        };
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
}

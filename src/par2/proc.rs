//! Subprocess helper shared by all process-based backends.
//!
//! Long-running operations are executed as child processes so they can be
//! *stopped*: `rust-par2`'s verify/repair are single blocking calls with no
//! cancellation hooks, but a killed process stops instantly. The same helper
//! runs `par2cmdline` for creation.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// Shared cancellation flag (owned by a [`crate::jobs::Reporter`]).
pub type CancelFlag = Arc<AtomicBool>;

/// Outcome of a finished child process.
pub struct ProcessResult {
    pub success: bool,
    pub code: Option<i32>,
    /// Everything the child wrote to stderr (tail-bounded).
    pub stderr: String,
}

/// Run `command`, streaming its stdout to `on_line` (lines are split on both
/// `\n` and `\r`, since par2cmdline rewrites progress lines with `\r`).
///
/// If `cancel` is set while the child is running, the child is killed and the
/// read loop ends as soon as its pipes close. The child is reaped by this
/// function; the watcher thread only signals, so there is no double-reap race.
///
/// # Errors
/// Returns an error when the process could not be spawned or waited on.
pub fn run_streaming(
    command: &mut Command,
    cancel: &CancelFlag,
    on_line: &mut dyn FnMut(&str),
) -> Result<ProcessResult, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|e| format!("failed to start process: {e}"))?;
    let child = Arc::new(Mutex::new(child));

    // Drain stderr on its own thread so the child can never deadlock on a
    // full stderr pipe while we are busy reading stdout.
    let stderr_rx = child
        .lock()
        .unwrap()
        .stderr
        .take()
        .map(spawn_stderr_drain);

    // Watcher thread: kill the child when cancellation is requested. It never
    // reaps the child — only the main thread does. It runs until the child
    // has been reaped (`done` is set below), so Stop works for the whole
    // lifetime of the process.
    let done = Arc::new(AtomicBool::new(false));
    {
        let child = Arc::clone(&child);
        let done_watcher = Arc::clone(&done);
        let cancel_watcher = Arc::clone(cancel);
        std::thread::spawn(move || {
            loop {
                if cancel_watcher.load(Ordering::Relaxed) {
                    let _ = child.lock().unwrap().kill();
                    break;
                }
                if done_watcher.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    let stdout = child.lock().unwrap().stdout.take();
    if let Some(stdout) = stdout {
        read_lines(stdout, on_line);
    }

    // Reap the child (this is the only thread that does).
    let status = loop {
        let mut guard = child.lock().unwrap();
        match guard.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                // Rare failure path: stop the watcher, best-effort kill the
                // child so it cannot linger unattended.
                drop(guard);
                let _ = child.lock().unwrap().kill();
                done.store(true, Ordering::Relaxed);
                return Err(format!("failed to wait for process: {e}"));
            }
        }
        drop(guard);
        std::thread::sleep(Duration::from_millis(25));
    };

    // Let the watcher thread exit. From here on the child is reaped and
    // cannot be (or need to be) killed any more.
    done.store(true, Ordering::Relaxed);

    let stderr = stderr_rx.and_then(|rx| rx.recv().ok()).unwrap_or_default();

    Ok(ProcessResult {
        success: status.success(),
        code: status.code(),
        stderr,
    })
}

/// Read `reader` to EOF, invoking `on_line` for every line. Lines may be
/// terminated by `\n` (unix style) or `\r` (par2cmdline progress style); a
/// trailing unterminated line is still delivered.
fn read_lines<R: Read>(mut reader: R, on_line: &mut dyn FnMut(&str)) {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        while let Some(pos) = buf.iter().position(|&b| b == b'\n' || b == b'\r') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            on_line(&String::from_utf8_lossy(&line[..line.len() - 1]));
        }
    }
    if !buf.is_empty() {
        on_line(&String::from_utf8_lossy(&buf));
    }
}

/// Read everything from `stderr` into a string, on its own thread.
fn spawn_stderr_drain(stderr: std::process::ChildStderr) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = stderr;
        let mut text = String::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let n = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            text.push_str(std::str::from_utf8(&chunk[..n]).unwrap_or_default());
            // Keep the buffer bounded; only the tail matters for errors.
            if text.len() > 64 * 1024 {
                let cut = text.len() - 32 * 1024;
                let boundary = text[cut..].find('\n').map_or(cut, |i| cut + i + 1);
                text.drain(..boundary);
            }
        }
        let _ = tx.send(text);
    });
    rx
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::{read_lines, run_streaming};

    #[test]
    fn read_lines_splits_on_newline_and_carriage_return() {
        let mut out = String::new();
        read_lines("a\rbb\nccc".as_bytes(), &mut |line| {
            out.push_str(line);
            out.push('|');
        });
        assert_eq!(out, "a|bb|ccc|");
    }

    #[test]
    fn streams_output_and_exit_code() {
        let mut command = Command::new("sh");
        command.arg("-c").arg("echo hello; echo world");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut seen = Vec::new();
        let result = run_streaming(&mut command, &cancel, &mut |line| {
            seen.push(line.to_string());
        })
        .unwrap();

        assert!(result.success);
        assert_eq!(result.code, Some(0));
        assert_eq!(seen, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn cancel_kills_the_child() {
        let mut command = Command::new("sh");
        command.arg("-c").arg("echo started; sleep 60");
        let cancel = Arc::new(AtomicBool::new(false));

        let mut seen = Vec::new();
        // Cancel as soon as the first line arrives.
        let cancel2 = Arc::clone(&cancel);
        let result = run_streaming(&mut command, &cancel, &mut |line| {
            seen.push(line.to_string());
            cancel2.store(true, Ordering::Relaxed);
        })
        .unwrap();

        assert_eq!(seen.first().map(String::as_str), Some("started"));
        assert!(!result.success, "killed child must not report success");
    }
}

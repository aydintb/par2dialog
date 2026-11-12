//! Headless worker subprocess that makes verify/repair cancellable.
//!
//! `rust_par2::verify` / `repair_from_verify` are single blocking calls with
//! no cancellation hooks, so the GUI runs them inside its *own binary* with
//! `--par2-worker`. The parent watches the cancel flag and kills the worker —
//! process death is the only reliable way to stop in-flight library work.
//!
//! Protocol (worker stdout): `PROGRESS\t<pct>\t<message>` lines while working,
//! then a single `RESULT <json>` line with the serialized outcome.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

use crate::jobs::{Reporter, CANCELLED};
use crate::par2::proc::{run_streaming, CancelFlag};
use crate::par2::repair;
use crate::par2::types::{RepairOutcome, VerifyOutcome};
use crate::par2::verify;

/// Env var set on the worker process so backend functions run in-process
/// instead of spawning yet another worker.
pub const WORKER_ENV: &str = "PAR2DIALOG_WORKER";

/// The CLI flag that puts this binary into worker mode.
const WORKER_FLAG: &str = "--par2-worker";

/// Result sent back from the worker to the parent.
#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerOutcome {
    Verify(VerifyOutcome),
    Repair {
        verify: VerifyOutcome,
        repair: RepairOutcome,
    },
}

/// True when this process was itself started as a worker.
pub fn active() -> bool {
    std::env::var(WORKER_ENV).as_deref() == Ok("1")
}

/// Handle `--par2-worker ...` on the command line.
///
/// Returns the worker's exit code when the arguments are a worker invocation,
/// or `None` when the app should start normally (GUI mode).
pub fn handle_args(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some(WORKER_FLAG) {
        return None;
    }
    Some(run_worker_command(&args[1..]))
}

fn run_worker_command(rest: &[String]) -> i32 {
    if rest.len() < 3 {
        eprintln!("usage: {WORKER_FLAG} <verify|repair> <par2file> <datadir> [extra...]");
        return 2;
    }
    let sub = rest[0].as_str();
    let par2 = Path::new(&rest[1]);
    let data_dir = Path::new(&rest[2]);
    let extras: Vec<PathBuf> = rest[3..].iter().map(PathBuf::from).collect();

    let reporter = Reporter::new_worker();
    let outcome = match sub {
        "verify" => WorkerOutcome::Verify(verify::verify_in_process(
            par2,
            data_dir,
            &extras,
            &reporter,
        )),
        "repair" => {
            let (verify, repair) =
                repair::verify_then_repair_in_process(par2, data_dir, &extras, &reporter);
            WorkerOutcome::Repair { verify, repair }
        }
        _ => {
            eprintln!("unknown worker subcommand: {sub}");
            return 2;
        }
    };

    match serde_json::to_string(&outcome) {
        Ok(json) => {
            println!("RESULT {json}");
            0
        }
        Err(e) => {
            eprintln!("failed to serialize worker result: {e}");
            2
        }
    }
}

/// Run a verify in a killable worker process.
///
/// # Errors
/// Returns [`CANCELLED`] when stopped, or an error when the worker could not
/// run / crashed without producing a result.
pub fn spawn_verify(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<VerifyOutcome, String> {
    match spawn_worker("verify", par2_path, data_dir, extra_files, reporter)? {
        WorkerOutcome::Verify(outcome) => Ok(outcome),
        WorkerOutcome::Repair { .. } => Err("unexpected worker result".to_string()),
    }
}

/// Run verify + repair in a killable worker process.
///
/// # Errors
/// Returns [`CANCELLED`] when stopped, or an error when the worker could not
/// run / crashed without producing a result.
pub fn spawn_repair(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<(VerifyOutcome, RepairOutcome), String> {
    match spawn_worker("repair", par2_path, data_dir, extra_files, reporter)? {
        WorkerOutcome::Repair { verify, repair } => Ok((verify, repair)),
        WorkerOutcome::Verify(_) => Err("unexpected worker result".to_string()),
    }
}

fn spawn_worker(
    sub: &str,
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<WorkerOutcome, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let mut command = Command::new(exe);
    command
        .arg(WORKER_FLAG)
        .arg(sub)
        .arg(par2_path)
        .arg(data_dir)
        .args(extra_files)
        .env(WORKER_ENV, "1");

    let cancel: CancelFlag = reporter.cancel_handle();
    let mut result_json: Option<String> = None;

    let process = run_streaming(&mut command, &cancel, &mut |line| {
        if let Some(json) = line.strip_prefix("RESULT ") {
            result_json = Some(json.to_string());
        } else if let Some(rest) = line.strip_prefix("PROGRESS\t") {
            let mut fields = rest.splitn(2, '\t');
            let pct = fields
                .next()
                .and_then(|p| p.parse::<f32>().ok())
                .unwrap_or(0.0);
            let message = fields.next().unwrap_or_default();
            reporter.report(pct, message);
        }
    })?;

    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.to_string());
    }

    if let Some(json) = result_json {
        return serde_json::from_str(&json)
            .map_err(|e| format!("malformed worker result: {e}"));
    }

    let code = process.code.unwrap_or(-1);
    let stderr_tail = process.stderr.trim();
    let detail = if stderr_tail.is_empty() {
        "no error output".to_string()
    } else {
        stderr_tail.lines().next_back().unwrap_or("").to_string()
    };
    Err(format!("worker process failed (exit code {code}): {detail}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::WorkerOutcome;
    use crate::par2::types::{FileEntry, FileStatus, RepairOutcome, VerifyOutcome};

    fn sample() -> WorkerOutcome {
        WorkerOutcome::Repair {
            verify: VerifyOutcome {
                files: vec![FileEntry {
                    path: PathBuf::from("/tmp/a.bin"),
                    status: FileStatus::Damaged,
                    damage_detail: Some("2/10 blocks damaged".to_string()),
                }],
                all_correct: false,
                repair_possible: true,
                blocks_available: 100,
                blocks_needed: 4,
                summary: "damaged".to_string(),
            },
            repair: RepairOutcome {
                success: true,
                blocks_repaired: 4,
                files_repaired: 1,
                summary: "repaired".to_string(),
            },
        }
    }

    #[test]
    fn worker_outcome_round_trips_through_json() {
        let outcome = sample();
        let json = serde_json::to_string(&outcome).unwrap();
        let back: WorkerOutcome = serde_json::from_str(&json).unwrap();
        match back {
            WorkerOutcome::Repair { verify, repair } => {
                assert_eq!(verify.files[0].status, FileStatus::Damaged);
                assert!(repair.success);
            }
            WorkerOutcome::Verify(_) => panic!("wrong variant"),
        }
    }
}

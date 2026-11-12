//! PAR2 verification using `rust-par2` (pure Rust, SIMD-accelerated).
//!
//! The GUI never calls these functions directly: they run inside a killable
//! worker subprocess (see `crate::worker`) so operations can be stopped.

use std::path::{Path, PathBuf};

use rust_par2::{Par2FileSet, VerifyResult as LibVerifyResult};

use crate::jobs::Reporter;
use crate::par2::types::{FileEntry, FileStatus, VerifyOutcome};
use crate::util;
use crate::worker;

/// A completed verification, keeping both the GUI-friendly outcome and the
/// library's own data so a subsequent repair does not need to verify again.
pub struct FullVerify {
    pub outcome: VerifyOutcome,
    /// Present when the PAR2 file parsed successfully.
    pub parsed: Option<(Par2FileSet, LibVerifyResult)>,
}

/// Verify the data files in `data_dir` against the PAR2 set at `par2_path`,
/// running the (expensive) verification pass exactly once.
///
/// Cancellable: when running from the GUI this is executed inside a worker
/// subprocess that is killed when the reporter's cancel flag is set.
pub fn verify(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<VerifyOutcome, String> {
    if worker::active() {
        Ok(verify_in_process(par2_path, data_dir, extra_files, reporter))
    } else {
        worker::spawn_verify(par2_path, data_dir, extra_files, reporter)
    }
}

/// The verify logic itself; called inside the worker process.
pub fn verify_in_process(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> VerifyOutcome {
    verify_full(par2_path, data_dir, extra_files, reporter).outcome
}

/// Verify and keep the library-side data for a subsequent repair.
pub(crate) fn verify_full(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> FullVerify {
    reporter.report(0.0, "Reading PAR2 index...");
    let file_set = match rust_par2::parse(par2_path) {
        Ok(file_set) => file_set,
        Err(e) => {
            let outcome = VerifyOutcome {
                files: Vec::new(),
                all_correct: false,
                repair_possible: false,
                blocks_available: 0,
                blocks_needed: 0,
                summary: format!("Failed to parse PAR2 file: {e}"),
            };
            reporter.report(100.0, &outcome.summary);
            return FullVerify {
                outcome,
                parsed: None,
            };
        }
    };

    reporter.report(0.0, "Hashing and checking files (this may take a while)...");
    let result = rust_par2::verify(&file_set, data_dir);
    let outcome = outcome_from(&result, data_dir, extra_files);
    reporter.report(100.0, &outcome.summary);

    FullVerify {
        outcome,
        parsed: Some((file_set, result)),
    }
}

/// Convert the library's verify result into our display-friendly outcome.
fn outcome_from(
    result: &LibVerifyResult,
    data_dir: &Path,
    extra_files: &[PathBuf],
) -> VerifyOutcome {
    let summary = result.to_string();

    let mut files = Vec::with_capacity(
        result.intact.len() + result.damaged.len() + result.missing.len(),
    );

    for vf in &result.intact {
        files.push(FileEntry {
            path: data_dir.join(&vf.filename),
            status: FileStatus::Ok,
            damage_detail: Some(util::human_size(vf.size)),
        });
    }

    for df in &result.damaged {
        files.push(FileEntry {
            path: data_dir.join(&df.filename),
            status: FileStatus::Damaged,
            damage_detail: Some(format!(
                "{}/{} blocks damaged",
                df.damaged_block_count, df.total_block_count
            )),
        });
    }

    for mf in &result.missing {
        files.push(FileEntry {
            path: data_dir.join(&mf.filename),
            status: FileStatus::Missing,
            damage_detail: Some(format!("expected {} bytes", mf.expected_size)),
        });
    }

    for extra in extra_files {
        if !files.iter().any(|e| e.path == *extra) {
            files.push(FileEntry {
                path: extra.clone(),
                status: FileStatus::Unknown,
                damage_detail: Some("Not in PAR2 set".to_string()),
            });
        }
    }

    VerifyOutcome {
        files,
        all_correct: result.all_correct(),
        repair_possible: result.repair_possible,
        blocks_available: result.recovery_blocks_available,
        blocks_needed: result.blocks_needed(),
        summary,
    }
}

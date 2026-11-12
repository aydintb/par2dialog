//! PAR2 repair using `rust-par2` (pure Rust, SIMD-accelerated).
//!
//! Like verify, repair runs inside a killable worker subprocess (see
//! `crate::worker`) so it can be stopped from the GUI. If the worker is
//! killed while a file is being rewritten, simply running Repair again
//! finishes the job — recovery data is never modified.

use std::path::{Path, PathBuf};

use crate::jobs::Reporter;
use crate::par2::types::{RepairOutcome, VerifyOutcome};
use crate::par2::verify::{verify, verify_full};
use crate::worker;

/// Verify, then repair only when damage is present and recovery data suffices.
///
/// The verification pass runs exactly once: its result feeds the repair via
/// the library's `repair_from_verify`, which re-verifies after fixing files.
///
/// Cancellable: when running from the GUI this is executed inside a worker
/// subprocess that is killed when the reporter's cancel flag is set.
pub fn verify_then_repair(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<(VerifyOutcome, RepairOutcome), String> {
    if worker::active() {
        Ok(verify_then_repair_in_process(
            par2_path,
            data_dir,
            extra_files,
            reporter,
        ))
    } else {
        worker::spawn_repair(par2_path, data_dir, extra_files, reporter)
    }
}

/// The verify + repair logic itself; called inside the worker process.
pub fn verify_then_repair_in_process(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> (VerifyOutcome, RepairOutcome) {
    let full = verify_full(par2_path, data_dir, extra_files, reporter);
    let verify_outcome = full.outcome;

    if verify_outcome.all_correct {
        let repair_outcome = RepairOutcome {
            success: true,
            blocks_repaired: 0,
            files_repaired: 0,
            summary: "All files are already intact. Nothing to repair.".to_string(),
        };
        reporter.report(100.0, &repair_outcome.summary);
        return (verify_outcome.clone(), repair_outcome);
    }

    if !verify_outcome.repair_possible {
        let needed = verify_outcome
            .blocks_needed
            .saturating_sub(verify_outcome.blocks_available);
        let repair_outcome = RepairOutcome {
            success: false,
            blocks_repaired: 0,
            files_repaired: 0,
            summary: format!(
                "Repair not possible. Need {needed} more recovery blocks, only {} available.",
                verify_outcome.blocks_available
            ),
        };
        reporter.report(100.0, &repair_outcome.summary);
        return (verify_outcome.clone(), repair_outcome);
    }

    reporter.report(0.0, "Repairing damaged files...");
    let repair_outcome = match full.parsed {
        Some((file_set, lib_result)) => {
            match rust_par2::repair_from_verify(&file_set, data_dir, &lib_result) {
                Ok(result) => RepairOutcome {
                    success: result.success,
                    blocks_repaired: result.blocks_repaired,
                    files_repaired: result.files_repaired,
                    summary: result.to_string(),
                },
                Err(e) => RepairOutcome {
                    success: false,
                    blocks_repaired: 0,
                    files_repaired: 0,
                    summary: format!("Repair error: {e}"),
                },
            }
        }
        None => RepairOutcome {
            success: false,
            blocks_repaired: 0,
            files_repaired: 0,
            summary: "Failed to parse PAR2 file".to_string(),
        },
    };
    reporter.report(100.0, &repair_outcome.summary);

    (verify_outcome, repair_outcome)
}

/// Check whether a repair would be possible (equivalent to a verify pass).
pub fn check_feasibility(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
    reporter: &Reporter,
) -> Result<VerifyOutcome, String> {
    verify(par2_path, data_dir, extra_files, reporter)
}

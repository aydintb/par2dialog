//! PAR2 repair using `rust-par2` (pure Rust, SIMD-accelerated).

use std::path::{Path, PathBuf};

use crate::par2::types::{RepairOutcome, VerifyOutcome};
use crate::par2::verify::{verify, verify_full};

/// Verify, then repair only when damage is present and recovery data suffices.
///
/// The verification pass runs exactly once: its result feeds the repair via
/// the library's `repair_from_verify`, which re-verifies after fixing files.
pub fn verify_then_repair(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
) -> (VerifyOutcome, RepairOutcome) {
    let full = verify_full(par2_path, data_dir, extra_files);
    let verify_outcome = full.outcome;

    if verify_outcome.all_correct {
        return (
            verify_outcome.clone(),
            RepairOutcome {
                success: true,
                blocks_repaired: 0,
                files_repaired: 0,
                summary: "All files are already intact. Nothing to repair.".to_string(),
            },
        );
    }

    if !verify_outcome.repair_possible {
        let needed = verify_outcome.blocks_needed.saturating_sub(verify_outcome.blocks_available);
        return (
            verify_outcome.clone(),
            RepairOutcome {
                success: false,
                blocks_repaired: 0,
                files_repaired: 0,
                summary: format!(
                    "Repair not possible. Need {needed} more recovery blocks, only {} available.",
                    verify_outcome.blocks_available
                ),
            },
        );
    }

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

    (verify_outcome, repair_outcome)
}

/// Check whether a repair would be possible (equivalent to a verify pass).
pub fn check_feasibility(
    par2_path: &Path,
    data_dir: &Path,
    extra_files: &[PathBuf],
) -> VerifyOutcome {
    verify(par2_path, data_dir, extra_files)
}

use std::path::PathBuf;

use crate::par2::types::{RepairResult, VerifyResult};
use crate::par2::verify::verify;

pub fn repair(par2_path: &PathBuf, data_dir: &PathBuf, _extra_files: &[PathBuf]) -> RepairResult {
    match rust_par2::parse(par2_path) {
        Ok(file_set) => {
            match rust_par2::repair(&file_set, data_dir) {
                Ok(result) => {
                    let summary = format!("{}", result);
                    let success = result.success;

                    RepairResult {
                        success,
                        files_repaired: Vec::new(),
                        summary,
                    }
                }
                Err(e) => RepairResult {
                    success: false,
                    files_repaired: Vec::new(),
                    summary: format!("Repair error: {e}"),
                },
            }
        }
        Err(e) => RepairResult {
            success: false,
            files_repaired: Vec::new(),
            summary: format!("Failed to parse PAR2 file: {e}"),
        },
    }
}

pub fn verify_then_repair(
    par2_path: &PathBuf,
    data_dir: &PathBuf,
    extra_files: &[PathBuf],
) -> (VerifyResult, RepairResult) {
    let verify_result = verify(par2_path, data_dir, extra_files);

    if verify_result.all_correct {
        return (
            verify_result,
            RepairResult {
                success: true,
                files_repaired: Vec::new(),
                summary: "All files are already intact. Nothing to repair.".to_string(),
            },
        );
    }

    if !verify_result.repair_possible {
        let needed = verify_result.blocks_needed.saturating_sub(verify_result.blocks_available);
        let available = verify_result.blocks_available;
        return (
            verify_result,
            RepairResult {
                success: false,
                files_repaired: Vec::new(),
                summary: format!(
                    "Repair not possible. Need {needed} more recovery blocks, only {available} available."
                ),
            },
        );
    }

    let repair_result = repair(par2_path, data_dir, extra_files);
    (verify_result, repair_result)
}

pub fn check_feasibility(par2_path: &PathBuf, data_dir: &PathBuf, extra_files: &[PathBuf]) -> VerifyResult {
    verify(par2_path, data_dir, extra_files)
}

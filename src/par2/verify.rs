use std::path::PathBuf;

use crate::par2::types::{FileEntry, FileStatus, VerifyResult};

fn human_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{bytes} B") }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}

pub fn verify(par2_path: &PathBuf, data_dir: &PathBuf, extra_files: &[PathBuf]) -> VerifyResult {
    let mut files = Vec::new();

    match rust_par2::parse(par2_path) {
        Ok(file_set) => {
            // rust_par2::verify returns VerifyResult directly, not Result
            let result = rust_par2::verify(&file_set, data_dir);
            let summary = format!("{}", result);

            for vf in &result.intact {
                files.push(FileEntry {
                    path: data_dir.join(&vf.filename),
                    status: FileStatus::Ok,
                    damage_detail: Some(human_size(vf.size)),
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
                let already_listed = files.iter().any(|e| e.path == *extra);
                if !already_listed {
                    files.push(FileEntry {
                        path: extra.clone(),
                        status: FileStatus::Unknown,
                        damage_detail: Some("Not in PAR2 set".to_string()),
                    });
                }
            }

            VerifyResult {
                files,
                all_correct: result.all_correct(),
                repair_possible: result.repair_possible,
                blocks_available: u64::from(result.recovery_blocks_available),
                blocks_needed: u64::from(result.blocks_needed()),
                summary,
            }
        }
        Err(e) => VerifyResult {
            files,
            all_correct: false,
            repair_possible: false,
            blocks_available: 0,
            blocks_needed: 0,
            summary: format!("Failed to parse PAR2 file: {e}"),
        },
    }
}

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use regex::Regex;

use crate::par2::types::{CreateConfig, ProgressUpdate};

/// Find par2cmdline in PATH
pub fn find_par2cmdline() -> Option<String> {
    which::which("par2")
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .or_else(|| {
            which::which("par2cmdline")
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
        })
}

/// Build the command line arguments for par2 create
fn build_args(config: &CreateConfig) -> Vec<String> {
    let mut args = Vec::new();
    args.push("create".to_string());

    // Main PAR2 archive name
    if let Some(main_name) = config.output_path.file_stem() {
        args.push("-a".to_string());
        args.push(main_name.to_string_lossy().to_string());
    }

    // Block size or block count
    if config.block_size > 0 {
        args.push(format!("-s{}", config.block_size));
    } else if config.block_count > 0 {
        args.push(format!("-b{}", config.block_count));
    }

    // Redundancy
    match &config.redundancy_mode {
        crate::par2::types::RedundancyMode::Percentage => {
            args.push(format!("-r{}", config.redundancy_value as u64));
        }
        crate::par2::types::RedundancyMode::Size { unit, value } => {
            let suffix = match unit {
                crate::par2::types::SizeUnit::KB => "k",
                crate::par2::types::SizeUnit::MB => "m",
                crate::par2::types::SizeUnit::GB => "g",
            };
            args.push(format!("-r{suffix}{value}"));
        }
        crate::par2::types::RedundancyMode::BlockCount => {
            args.push(format!("-c{}", config.redundancy_value as u64));
        }
    }

    // First recovery block number
    if let Some(frbn) = config.first_recovery_block {
        args.push(format!("-f{frbn}"));
    }

    // Recovery file mode
    match &config.recovery_file_mode {
        crate::par2::types::RecoveryFileMode::Uniform => {
            args.push("-u".to_string());
        }
        crate::par2::types::RecoveryFileMode::LimitSize => {
            args.push("-l".to_string());
        }
        crate::par2::types::RecoveryFileMode::ExactCount => {
            if config.recovery_file_count > 0 {
                args.push(format!("-n{}", config.recovery_file_count.min(31)));
            }
        }
        crate::par2::types::RecoveryFileMode::Auto => {}
    }

    // Memory limit
    if let Some(mem) = config.memory_limit {
        args.push(format!("-m{mem}"));
    }

    // Threads
    if let Some(t) = config.threads {
        args.push(format!("-t{t}"));
    }

    // Hash threads
    if let Some(ht) = config.hash_threads {
        args.push(format!("-T{ht}"));
    }

    // Recurse
    if config.recurse {
        args.push("-R".to_string());
    }

    // Purge on success
    if config.purge_on_success {
        args.push("-p".to_string());
    }

    // Base path
    if let Some(bp) = &config.base_path {
        if let Some(s) = bp.to_str() {
            args.push("-B".to_string());
            args.push(s.to_string());
        }
    }

    // Data files
    for file in &config.data_files {
        if let Some(s) = file.to_str() {
            args.push(s.to_string());
        }
    }

    args
}

/// Parse progress from par2cmdline output line
fn parse_progress(line: &str, progress_re: &Regex) -> Option<(f32, String)> {
    // par2cmdline outputs lines like "Done: 45%, ... "
    if let Some(caps) = progress_re.captures(line) {
        if let Some(pct) = caps.get(1) {
            if let Ok(val) = pct.as_str().parse::<f32>() {
                return Some((val, line.trim().to_string()));
            }
        }
    }
    None
}

/// Run par2 create asynchronously, sending progress updates through the channel
pub fn create_async(
    config: CreateConfig,
    sender: mpsc::Sender<ProgressUpdate>,
) -> std::thread::JoinHandle<Result<String, String>> {
    thread::spawn(move || {
        let cmdline = find_par2cmdline().ok_or_else(|| {
            "par2cmdline not found in PATH. Please install par2cmdline first.".to_string()
        })?;

        let args = build_args(&config);
        let _pretty_args = args.join(" ");

        tracing::info!("Running: {} {}", cmdline, args.join(" "));

        let mut child = Command::new(&cmdline)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start par2cmdline: {e}"))?;

        let progress_re = Regex::new(r"Done:\s*(\d+(?:\.\d+)?)%").unwrap();

        // Read stdout for progress
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if let Some((pct, msg)) = parse_progress(&line, &progress_re) {
                    let _ = sender.send(ProgressUpdate {
                        current: pct,
                        total: 100.0,
                        message: msg.clone(),
                        done: false,
                    });
                } else if !line.trim().is_empty() {
                    let _ = sender.send(ProgressUpdate {
                        current: 0.0,
                        total: 100.0,
                        message: line.clone(),
                        done: false,
                    });
                }
            }
        }

        // Also capture stderr for errors
        let stderr_output = if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            reader
                .lines()
                .flatten()
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for par2cmdline: {e}"))?;

        if status.success() {
            let _ = sender.send(ProgressUpdate {
                current: 100.0,
                total: 100.0,
                message: "PAR2 files created successfully!".to_string(),
                done: true,
            });
            Ok("PAR2 files created successfully!".to_string())
        } else {
            let code = status.code().unwrap_or(-1);
            let _ = sender.send(ProgressUpdate {
                current: 0.0,
                total: 100.0,
                message: format!("par2cmdline failed with exit code {code}: {stderr_output}"),
                done: true,
            });
            Err(format!(
                "par2cmdline failed (exit code {code}): {stderr_output}"
            ))
        }
    })
}

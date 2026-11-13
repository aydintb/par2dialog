//! PAR2 creation via the `par2cmdline` subprocess.
//!
//! `rust-par2` cannot create PAR2 sets yet, so creation shells out to
//! `par2 create` and streams its progress into the GUI. The subprocess is
//! killable: setting the reporter's cancel flag stops the operation.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;

use crate::jobs::{Reporter, CANCELLED};
use crate::par2::proc::run_streaming;
use crate::par2::types::{CreateConfig, RecoveryFileMode, RedundancySpec};

/// Locate the `par2` binary.
///
/// If `custom` is set and exists, it is used directly. Otherwise the PATH is
/// searched (`par2` first, `par2cmdline` as a fallback).
#[must_use]
pub fn find_par2cmdline(custom: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = custom {
        let path = Path::new(path);
        if !path.as_os_str().is_empty() && path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    which::which("par2")
        .or_else(|_| which::which("par2cmdline"))
        .ok()
}

/// Build the `par2 create` argument list.
fn build_args(config: &CreateConfig) -> Vec<String> {
    let mut args = vec!["create".to_string()];

    // Data block layout (-b and -s are mutually exclusive).
    match config.blocks {
        crate::par2::types::BlockSpec::Auto => {}
        crate::par2::types::BlockSpec::Size(n) => args.push(format!("-s{n}")),
        crate::par2::types::BlockSpec::Count(n) => args.push(format!("-b{n}")),
    }

    // Redundancy.
    match &config.redundancy {
        RedundancySpec::Percent(pct) => args.push(format!("-r{pct}")),
        RedundancySpec::Size { unit, value } => {
            args.push(format!("-r{}{value}", unit.suffix()));
        }
        RedundancySpec::Blocks(n) => args.push(format!("-c{n}")),
    }

    // First recovery block number.
    if let Some(frbn) = config.first_recovery_block {
        args.push(format!("-f{frbn}"));
    }

    // Recovery file sizing.
    match &config.recovery_file_mode {
        RecoveryFileMode::Auto => {}
        RecoveryFileMode::Uniform => args.push("-u".to_string()),
        RecoveryFileMode::LimitSize => args.push("-l".to_string()),
        RecoveryFileMode::ExactCount(n) => {
            if *n > 0 {
                args.push(format!("-n{}", (*n).min(31)));
            }
        }
    }

    // Memory / thread limits.
    if let Some(mem) = config.memory_limit {
        args.push(format!("-m{mem}"));
    }
    if let Some(t) = config.threads {
        args.push(format!("-t{t}"));
    }
    if let Some(ht) = config.hash_threads {
        args.push(format!("-T{ht}"));
    }

    if config.recurse {
        args.push("-R".to_string());
    }

    // Base path (glued form `-B<path>` as documented by par2cmdline).
    if let Some(bp) = &config.base_path {
        if let Some(s) = bp.to_str() {
            args.push(format!("-B{s}"));
        }
    }

    // PAR2 output file (positional, first unflagged argument).
    if let Some(s) = config.output_path.to_str() {
        args.push(s.to_string());
    }

    // Data files.
    for file in &config.data_files {
        if let Some(s) = file.to_str() {
            args.push(s.to_string());
        }
    }

    args
}

/// Run `par2 create` and report progress. Blocks until the subprocess exits;
/// intended to be called from a [`crate::jobs::Job`] on a background thread.
///
/// The subprocess is killed when the reporter's cancel flag is set, in which
/// case [`CANCELLED`] is returned.
pub fn create(
    config: &CreateConfig,
    par2_path: Option<&str>,
    reporter: &Reporter,
) -> Result<String, String> {
    let cmdline = find_par2cmdline(par2_path).ok_or_else(|| {
        "par2cmdline not found in PATH. Please install par2cmdline first.".to_string()
    })?;

    // par2cmdline refuses to overwrite existing PAR2 files: it exits with
    // code 3 and prints the error only to stderr, leaving stdout empty. Fail
    // fast with a clear message instead of relying on that.
    if config.output_path.exists() {
        return Err(format!(
            "Output file already exists: {}. Delete it or choose another name — \
             par2cmdline does not overwrite existing PAR2 files.",
            config.output_path.display()
        ));
    }

    let args = build_args(config);
    tracing::info!("Running: {} {}", cmdline.display(), args.join(" "));

    let mut command = Command::new(&cmdline);
    command.args(&args);

    // par2cmdline writes progress with \r line separators and never emits a
    // "Done: N%" marker; percentages appear in lines such as
    // "Constructing: 45.0%" / "Processing: 33.7%" / "Hashing: 12.3%".
    let progress_re = progress_regex();
    let cancel = reporter.cancel_handle();
    let mut last_pct = -1.0_f32;
    let mut last_label = String::new();

    // Heartbeat: par2cmdline can stay silent for several seconds (huge input
    // scans, slow custom binaries). Keep reporting liveness so the UI never
    // appears stuck on the initial "Starting par2cmdline..." message.
    let started = Instant::now();
    let last_output = Arc::new(Mutex::new(Instant::now()));
    let heartbeat_done = Arc::new(AtomicBool::new(false));
    {
        let last_output = Arc::clone(&last_output);
        let heartbeat_done = Arc::clone(&heartbeat_done);
        let cancel_watcher = Arc::clone(&cancel);
        let heartbeat_reporter = reporter.clone();
        std::thread::spawn(move || {
            while !heartbeat_done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(250));
                if heartbeat_done.load(Ordering::Relaxed)
                    || cancel_watcher.load(Ordering::Relaxed)
                {
                    break;
                }
                let silent_for = last_output.lock().unwrap().elapsed();
                if silent_for > Duration::from_secs(3) {
                    heartbeat_reporter.report_keep_progress(format!(
                        "par2cmdline still running ({}s elapsed)…",
                        started.elapsed().as_secs()
                    ));
                }
            }
        });
    }

    let result = run_streaming(&mut command, &cancel, &mut |line| {
        *last_output.lock().unwrap() = Instant::now();
        emit_line(line, progress_re, &mut last_pct, &mut last_label, reporter);
    });

    // Stop the heartbeat before reporting the final state.
    heartbeat_done.store(true, Ordering::Relaxed);
    let result = result?;

    if !result.success && cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.to_string());
    }

    if result.success {
        reporter.report(100.0, "PAR2 files created successfully!");
        Ok("PAR2 files created successfully!".to_string())
    } else {
        let code = result.code.unwrap_or(-1);
        let stderr_text = result.stderr.trim().to_string();
        let detail = if stderr_text.is_empty() {
            String::new()
        } else {
            format!(": {stderr_text}")
        };
        Err(format!("par2cmdline failed (exit code {code}){detail}"))
    }
}

/// Compile the progress-line regex once.
fn progress_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z][A-Za-z /]*:\s*(\d+(?:\.\d+)?)%").unwrap())
}

/// Parse one output line and report progress when it contains a percentage.
fn emit_line(
    line: &str,
    progress_re: &Regex,
    last_pct: &mut f32,
    last_label: &mut String,
    reporter: &Reporter,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(caps) = progress_re.captures(trimmed) {
        if let Ok(pct) = caps[1].parse::<f32>() {
            // Only re-report when something changed — par2cmdline rewrites
            // the same percentage with a rewritten progress bar many times.
            let label_end = caps.get(0).map_or(0, |m| m.end());
            let label = trimmed[..label_end.saturating_sub(1)].trim_end();
            if (pct - *last_pct).abs() > f32::EPSILON || label != *last_label {
                *last_pct = pct;
                last_label.clear();
                last_label.push_str(label);
                reporter.report(pct, trimmed.to_string());
            }
            return;
        }
    }
    reporter.report((*last_pct).max(0.0), trimmed.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::par2::types::{BlockSpec, SizeUnit};

    fn test_config() -> CreateConfig {
        CreateConfig {
            output_path: PathBuf::from("/tmp/out.par2"),
            data_files: vec![PathBuf::from("/tmp/a.bin"), PathBuf::from("/tmp/b.bin")],
            redundancy: RedundancySpec::Percent(5),
            blocks: BlockSpec::Auto,
            recovery_file_mode: RecoveryFileMode::Auto,
            memory_limit: None,
            threads: None,
            hash_threads: None,
            recurse: false,
            base_path: None,
            first_recovery_block: None,
        }
    }

    #[test]
    fn build_args_percent_redundancy() {
        let args = build_args(&test_config());
        assert_eq!(
            args,
            vec![
                "create", "-r5", "/tmp/out.par2", "/tmp/a.bin", "/tmp/b.bin"
            ]
        );
    }

    #[test]
    fn build_args_size_redundancy() {
        let mut config = test_config();
        config.redundancy = RedundancySpec::Size {
            unit: SizeUnit::MB,
            value: 100,
        };
        config.blocks = BlockSpec::Size(1024);
        config.recovery_file_mode = RecoveryFileMode::ExactCount(4);
        config.memory_limit = Some(512);
        config.threads = Some(8);
        config.hash_threads = Some(2);
        config.first_recovery_block = Some(3);
        config.recurse = true;
        config.base_path = Some(PathBuf::from("/data"));

        let args = build_args(&config);
        assert_eq!(
            args,
            vec![
                "create",
                "-s1024",
                "-rm100",
                "-f3",
                "-n4",
                "-m512",
                "-t8",
                "-T2",
                "-R",
                "-B/data",
                "/tmp/out.par2",
                "/tmp/a.bin",
                "/tmp/b.bin",
            ]
        );
    }

    #[test]
    fn build_args_block_count_redundancy() {
        let mut config = test_config();
        config.redundancy = RedundancySpec::Blocks(500);
        config.blocks = BlockSpec::Count(2000);
        let args = build_args(&config);
        assert_eq!(
            args,
            vec!["create", "-b2000", "-c500", "/tmp/out.par2", "/tmp/a.bin", "/tmp/b.bin"]
        );
    }

    #[test]
    fn custom_path_falls_back_to_path_search() {
        let fallback = which::which("par2")
            .or_else(|_| which::which("par2cmdline"))
            .ok();
        assert_eq!(find_par2cmdline(Some("/nonexistent/par2")), fallback);
    }

    #[test]
    fn progress_lines_are_parsed() {
        let re = progress_regex();
        let reporter = Reporter::for_tests();
        let mut last_pct = -1.0_f32;
        let mut last_label = String::new();

        emit_line("Constructing: 12.5%", re, &mut last_pct, &mut last_label, &reporter);
        let progress = reporter.last_report();
        assert!((progress.pct - 12.5).abs() < f32::EPSILON);
        assert_eq!(progress.message, "Constructing: 12.5%");

        emit_line("Processing: 100.0%", re, &mut last_pct, &mut last_label, &reporter);
        let progress = reporter.last_report();
        assert!((progress.pct - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn plain_lines_keep_last_percentage() {
        let re = progress_regex();
        let reporter = Reporter::for_tests();
        let mut last_pct = -1.0_f32;
        let mut last_label = String::new();

        emit_line("Processing: 40.0%", re, &mut last_pct, &mut last_label, &reporter);
        emit_line(
            "Writing recovery packets",
            re,
            &mut last_pct,
            &mut last_label,
            &reporter,
        );
        let progress = reporter.last_report();
        assert!((progress.pct - 40.0).abs() < f32::EPSILON);
        assert_eq!(progress.message, "Writing recovery packets");
    }

    #[test]
    fn non_progress_lines_are_ignored_safely() {
        let re = progress_regex();
        let reporter = Reporter::for_tests();
        let mut last_pct = -1.0_f32;
        let mut last_label = String::new();

        emit_line("Done", re, &mut last_pct, &mut last_label, &reporter);
        emit_line(
            "Input file count: 2",
            re,
            &mut last_pct,
            &mut last_label,
            &reporter,
        );
        let progress = reporter.last_report();
        assert!((progress.pct - 0.0).abs() < f32::EPSILON);
        assert_eq!(progress.message, "Input file count: 2");
    }
}

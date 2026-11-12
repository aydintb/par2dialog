//! PAR2 creation via the `par2cmdline` subprocess.
//!
//! `rust-par2` cannot create PAR2 sets yet, so creation shells out to
//! `par2 create` and streams its progress into the GUI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, OnceLock};

use regex::Regex;

use crate::jobs::Reporter;
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
pub fn create(config: &CreateConfig, par2_path: Option<&str>, reporter: &Reporter) -> Result<String, String> {
    let cmdline = find_par2cmdline(par2_path).ok_or_else(|| {
        "par2cmdline not found in PATH. Please install par2cmdline first.".to_string()
    })?;

    let args = build_args(config);
    tracing::info!("Running: {} {}", cmdline.display(), args.join(" "));

    let mut child = Command::new(&cmdline)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start par2cmdline: {e}"))?;

    // Drain stderr on its own thread so the child can never deadlock on a
    // full stderr pipe while we are busy reading stdout.
    let stderr_handle = child.stderr.take().map(spawn_stderr_drain);

    // par2cmdline writes progress with \r line separators and never emits a
    // "Done: N%" marker; percentages appear in lines such as
    // "Constructing: 45.0%" / "Processing: 33.7%" / "Hashing: 12.3%".
    let progress_re = progress_regex();

    if let Some(stdout) = child.stdout.take() {
        let mut reader = stdout;
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0_u8; 4096];
        let mut last_pct = -1.0_f32;
        let mut last_label = String::new();

        loop {
            let n = reader
                .read(&mut chunk)
                .map_err(|e| format!("Failed to read par2cmdline output: {e}"))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);

            // Split on both \n and \r; whatever remains (a partial line)
            // stays in `buf` until the next read completes it.
            while let Some(pos) = buf.iter().position(|&b| b == b'\n' || b == b'\r') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                emit_line(
                    std::str::from_utf8(&line[..line.len() - 1]).unwrap_or_default(),
                    progress_re,
                    &mut last_pct,
                    &mut last_label,
                    reporter,
                );
            }
        }
        // Flush any trailing line that was not newline-terminated.
        emit_line(
            std::str::from_utf8(&buf).unwrap_or_default(),
            progress_re,
            &mut last_pct,
            &mut last_label,
            reporter,
        );
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for par2cmdline: {e}"))?;

    let stderr_text = stderr_handle
        .and_then(|rx| rx.recv().ok())
        .unwrap_or_default();

    if status.success() {
        reporter.report(100.0, "PAR2 files created successfully!");
        Ok("PAR2 files created successfully!".to_string())
    } else {
        let code = status.code().unwrap_or(-1);
        let stderr_text = stderr_text.trim().to_string();
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

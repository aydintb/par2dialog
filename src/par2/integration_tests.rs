//! Integration test: real create → damage → verify → repair round trip.
//!
//! Requires `par2` (par2cmdline) on PATH; skipped otherwise. Backend calls
//! run in-process (worker mode) so the test binary is not spawned recursively.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::jobs::{Job, Reporter};
    use crate::par2::create;
    use crate::par2::repair;
    use crate::par2::types::{CreateConfig, RecoveryFileMode, RedundancySpec};
    use crate::par2::verify;
    use crate::worker;

    fn par2_available() -> bool {
        create::find_par2cmdline(None).is_some()
    }

    /// Run backends in-process (no worker subprocess) inside tests.
    fn in_process_mode() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| std::env::set_var(worker::WORKER_ENV, "1"));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("par2dialog-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_config(dir: &Path) -> CreateConfig {
        CreateConfig {
            output_path: dir.join("data.par2"),
            data_files: vec![dir.join("payload.bin")],
            redundancy: RedundancySpec::Percent(20),
            blocks: crate::par2::types::BlockSpec::Auto,
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
    fn create_verify_repair_round_trip() {
        in_process_mode();
        if !par2_available() {
            eprintln!("skipping: par2 not on PATH");
            return;
        }

        let dir = temp_dir("roundtrip");
        let payload_path = dir.join("payload.bin");

        // 1. Create a test payload.
        let payload: Vec<u8> = (0_u32..200_000)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        fs::write(&payload_path, &payload).unwrap();

        // 2. Create PAR2 files through our backend (background job like the GUI).
        let config = test_config(&dir);
        let job = Job::spawn("creating...", move |reporter| {
            create::create(&config, None, reporter)
        });
        while job.is_running() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let created = job.take_result().expect("create job finished");
        assert!(created.is_ok(), "create failed: {created:?}");

        // 3. Corrupt the payload.
        let mut corrupted = payload.clone();
        for chunk in corrupted.chunks_mut(1000) {
            chunk[0] ^= 0xFF;
        }
        fs::write(&payload_path, &corrupted).unwrap();

        // 4. Verify must detect damage and confirm repair is possible.
        let reporter = Reporter::for_tests();
        let outcome = repair::check_feasibility(&dir.join("data.par2"), &dir, &[], &reporter)
            .expect("verify failed");
        assert!(!outcome.all_correct, "damage should be detected");
        assert!(
            outcome.repair_possible,
            "repair should be possible with 20% redundancy"
        );

        // 5. Repair must restore the original bytes.
        let (_, repair_outcome) =
            repair::verify_then_repair(&dir.join("data.par2"), &dir, &[], &reporter)
                .expect("repair failed");
        assert!(
            repair_outcome.success,
            "repair failed: {}",
            repair_outcome.summary
        );
        let restored = fs::read(&payload_path).unwrap();
        assert_eq!(restored, payload, "repaired file must match original bytes");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_streams_live_progress_from_the_real_binary() {
        use std::io::Write as _;

        in_process_mode();
        if !par2_available() {
            eprintln!("skipping: par2 not on PATH");
            return;
        }

        let dir = temp_dir("liveprogress");
        // ~120 MB of zeros: the run lasts long enough to observe live output.
        let payload_path = dir.join("payload.bin");
        let zeros = vec![0_u8; 1_000_000];
        let mut file = fs::File::create(&payload_path).unwrap();
        for _ in 0..120 {
            file.write_all(&zeros).unwrap();
        }
        drop(file);

        // Exactly what the GUI does (create_tab::start_creation).
        let initial = "Starting par2cmdline...";
        let config = test_config(&dir);
        let job = Job::spawn(initial, move |reporter| {
            create::create(&config, None, reporter)
        });

        let mut messages: Vec<String> = Vec::new();
        let mut max_pct = 0.0_f32;
        let mut last = String::new();
        while job.is_running() {
            let progress = job.progress();
            max_pct = max_pct.max(progress.pct);
            if progress.message != last {
                messages.push(progress.message.clone());
                last = progress.message;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let result = job.take_result().expect("create job finished");

        assert!(
            result.is_ok(),
            "create should succeed: {result:?}; messages seen: {messages:?}"
        );
        assert!(
            messages.iter().any(|m| m != initial),
            "create must replace the initial message with live output; \
             only saw: {messages:?}"
        );
        assert!(
            max_pct > 0.0,
            "create must report a progress percentage, only saw: {messages:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_fails_fast_when_output_files_already_exist() {
        in_process_mode();
        if !par2_available() {
            eprintln!("skipping: par2 not on PATH");
            return;
        }

        let dir = temp_dir("overwrite");
        fs::write(dir.join("payload.bin"), b"overwrite probe").unwrap();

        // First run creates the set.
        let config = test_config(&dir);
        let job = Job::spawn("creating...", move |reporter| {
            create::create(&config, None, reporter)
        });
        while job.is_running() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(job.take_result().unwrap().is_ok());

        // Second run over the existing files must fail immediately — never
        // hang (par2cmdline exits with code 3 and only writes to stderr).
        let started = std::time::Instant::now();
        let config = test_config(&dir);
        let job = Job::spawn("Starting par2cmdline...", move |reporter| {
            create::create(&config, None, reporter)
        });
        while job.is_running() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = job.take_result().expect("create job finished");
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
        let error = result.expect_err("recreating over existing files must fail");
        assert!(
            error.contains("already exists"),
            "error should explain the existing-output problem: {error}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_detects_intact_files() {
        in_process_mode();
        if !par2_available() {
            eprintln!("skipping: par2 not on PATH");
            return;
        }

        let dir = temp_dir("intact");
        let payload_path = dir.join("payload.bin");
        fs::write(&payload_path, b"hello par2 world").unwrap();

        let config = test_config(&dir);
        let job = Job::spawn("creating...", move |reporter| {
            create::create(&config, None, reporter)
        });
        while job.is_running() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(job.take_result().unwrap().is_ok());

        let reporter = Reporter::for_tests();
        let outcome = repair::check_feasibility(&dir.join("data.par2"), &dir, &[], &reporter)
            .expect("verify failed");
        assert!(
            outcome.all_correct,
            "fresh set should verify clean: {}",
            outcome.summary
        );
        assert_eq!(outcome.files.len(), 1);
        assert_eq!(outcome.files[0].path, payload_path);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_reports_progress_via_reporter() {
        in_process_mode();
        if !par2_available() {
            eprintln!("skipping: par2 not on PATH");
            return;
        }

        let dir = temp_dir("progress");
        fs::write(dir.join("payload.bin"), b"progress probe").unwrap();

        let config = test_config(&dir);
        let job = Job::spawn("creating...", move |reporter| {
            create::create(&config, None, reporter)
        });
        while job.is_running() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(job.take_result().unwrap().is_ok());

        let reporter = Reporter::for_tests();
        verify::verify(&dir.join("data.par2"), &dir, &[], &reporter).unwrap();
        let progress = reporter.last_report();
        assert!(
            progress.pct >= 100.0,
            "verify should report completion, got: {} ({})",
            progress.pct,
            progress.message
        );
        assert!(
            !progress.message.is_empty(),
            "verify should report a final message, got: {}",
            progress.message
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

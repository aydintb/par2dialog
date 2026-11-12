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

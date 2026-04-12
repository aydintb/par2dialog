#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Configuration for the self-updater.
pub struct UpdaterConfig {
    /// GitHub owner/username
    pub owner: String,
    /// GitHub repository name
    pub repo: String,
    /// Binary name (must match the release asset name)
    pub bin_name: String,
    /// Current semver (e.g. `env!("CARGO_PKG_VERSION")`)
    pub current_version: String,
    /// Show download progress in stdout
    pub show_progress: bool,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            owner: String::new(),
            repo: String::new(),
            bin_name: String::new(),
            current_version: String::new(),
            show_progress: true,
        }
    }
}

/// Result of an update check / run.
pub enum UpdateResult {
    /// A new version was downloaded and this process was replaced.
    Updated { version: String },
    /// Already running the latest version.
    UpToDate,
}

/// Runs the self-update cycle. Returns the result so the caller can
/// continue with app logic.
pub fn run_update(config: &UpdaterConfig) -> Result<UpdateResult, Box<dyn std::error::Error>> {
    println!("Current version: {}", config.current_version);

    let exe_path = std::env::current_exe()?;

    let status = self_update::backends::github::Update::configure()
        .repo_owner(&config.owner)
        .repo_name(&config.repo)
        .bin_name(&config.bin_name)
        .target(self_update::get_target())
        .show_download_progress(config.show_progress)
        .current_version(&config.current_version)
        .build()?
        .update()?;

    if status.updated() {
        println!("Updated to version: {}", status.version());
        restart_process(&exe_path);
    } else {
        println!("Already up to date.");
    }

    // If we get here, either no update was needed or the restart failed.
    // Return a status so the caller knows.
    if status.updated() {
        Ok(UpdateResult::Updated {
            version: status.version().to_string(),
        })
    } else {
        Ok(UpdateResult::UpToDate)
    }
}

/// Replaces the current process with the (possibly new) binary.
/// On Unix this uses `exec()` so stdio is inherited cleanly.
fn restart_process(exe: &std::path::Path) {
    #[cfg(unix)]
    {
        let err = std::process::Command::new(exe).exec();
        eprintln!("Failed to exec new process: {}", err);
        std::process::exit(1);
    }
    #[cfg(windows)]
    {
        std::process::Command::new(exe).spawn().expect("failed to spawn update process");
        std::process::exit(0);
    }
}

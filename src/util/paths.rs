/// Returns the directory containing the running executable
pub fn exe_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Returns the application data directory
pub fn data_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(|p| std::path::PathBuf::from(p).join("parpar"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| std::path::PathBuf::from(h).join(".config").join("parpar"))
            })
    }
    #[cfg(target_os = "macos")]
    {
        dirs::config_dir().map(|d| d.join("Par2Dialog"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|d| d.join("Par2Dialog"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

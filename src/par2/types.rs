use std::path::PathBuf;

/// Status of a single file in a PAR2 set
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// File is intact
    Ok,
    /// File is damaged
    Damaged,
    /// File is missing
    Missing,
    /// File has not been checked
    Unknown,
}

/// Result of a verify operation
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub files: Vec<FileEntry>,
    pub all_correct: bool,
    pub repair_possible: bool,
    pub blocks_available: u64,
    pub blocks_needed: u64,
    pub summary: String,
}

/// Result of a repair operation
#[derive(Debug, Clone)]
pub struct RepairResult {
    pub success: bool,
    pub files_repaired: Vec<PathBuf>,
    pub summary: String,
}

/// A single file entry in a PAR2 set
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub status: FileStatus,
    pub damage_detail: Option<String>,
}

/// Progress update during a long operation
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub current: f32,
    pub total: f32,
    pub message: String,
    pub done: bool,
}

/// Configuration for creating PAR2 files
#[derive(Debug, Clone)]
pub struct CreateConfig {
    /// Output PAR2 file path (base name)
    pub output_path: PathBuf,
    /// Data files to protect
    pub data_files: Vec<PathBuf>,
    /// Redundancy level as percentage (e.g. 5.0 = 5%)
    pub redundancy_value: f64,
    /// Redundancy mode: percentage, size, or block count
    pub redundancy_mode: RedundancyMode,
    /// Block size in bytes (0 = auto)
    pub block_size: u64,
    /// Block count (0 = auto, mutually exclusive with block_size)
    pub block_count: u64,
    /// Number of recovery files (max 31)
    pub recovery_file_count: u64,
    /// Recovery file sizing mode
    pub recovery_file_mode: RecoveryFileMode,
    /// Memory limit in MB
    pub memory_limit: Option<u64>,
    /// Thread count for main processing
    pub threads: Option<u64>,
    /// Thread count for hashing
    pub hash_threads: Option<u64>,
    /// Recurse into subdirectories
    pub recurse: bool,
    /// Base path for data files
    pub base_path: Option<PathBuf>,
    /// Purge backup and PAR files on success
    pub purge_on_success: bool,
    /// First recovery block number
    pub first_recovery_block: Option<u64>,
}

/// Redundancy specification mode
#[derive(Debug, Clone, PartialEq)]
pub enum RedundancyMode {
    /// Percentage (default 5%)
    Percentage,
    /// Target size (KB/MB/GB)
    Size { unit: SizeUnit, value: u64 },
    /// Exact recovery block count
    BlockCount,
}

/// Size unit for redundancy target
#[derive(Debug, Clone, PartialEq)]
pub enum SizeUnit {
    KB,
    MB,
    GB,
}

/// Recovery file sizing strategy
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryFileMode {
    /// Automatic (default)
    Auto,
    /// Uniform recovery file sizes
    Uniform,
    /// Limit size of recovery files
    LimitSize,
    /// Exact number of recovery files
    ExactCount,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Damaged => write!(f, "Damaged"),
            Self::Missing => write!(f, "Missing"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl Default for CreateConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::new(),
            data_files: Vec::new(),
            redundancy_value: 5.0,
            redundancy_mode: RedundancyMode::Percentage,
            block_size: 0,
            block_count: 0,
            recovery_file_count: 0,
            recovery_file_mode: RecoveryFileMode::Auto,
            memory_limit: None,
            threads: None,
            hash_threads: None,
            recurse: false,
            base_path: None,
            purge_on_success: false,
            first_recovery_block: None,
        }
    }
}

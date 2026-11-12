//! Shared data types for the PAR2 backend layer.

use std::path::PathBuf;

/// Status of a single file in a PAR2 set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// File is intact.
    Ok,
    /// File is damaged.
    Damaged,
    /// File is missing.
    Missing,
    /// File is not part of the PAR2 set.
    Unknown,
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

/// A single file entry in a PAR2 set.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub status: FileStatus,
    /// Extra detail such as file size or damaged block counts.
    pub damage_detail: Option<String>,
}

/// Result of a verify operation.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub files: Vec<FileEntry>,
    pub all_correct: bool,
    pub repair_possible: bool,
    pub blocks_available: u32,
    pub blocks_needed: u32,
    pub summary: String,
}

/// Result of a repair operation.
#[derive(Debug, Clone)]
pub struct RepairOutcome {
    pub success: bool,
    /// Number of blocks that were reconstructed.
    pub blocks_repaired: u32,
    /// Number of files that were repaired.
    pub files_repaired: usize,
    pub summary: String,
}

/// How much recovery data to generate.
#[derive(Debug, Clone, PartialEq)]
pub enum RedundancySpec {
    /// Recovery data as a percentage of input size (par2cmdline takes integers).
    Percent(u64),
    /// Absolute recovery data target size.
    Size { unit: SizeUnit, value: u64 },
    /// Exact recovery block count.
    Blocks(u64),
}

/// Unit for [`RedundancySpec::Size`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    KB,
    MB,
    GB,
}

impl SizeUnit {
    /// Suffix used by par2cmdline's `-r<unit><value>` option.
    #[must_use]
    pub fn suffix(self) -> &'static str {
        match self {
            Self::KB => "k",
            Self::MB => "m",
            Self::GB => "g",
        }
    }
}

/// How data blocks are laid out.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockSpec {
    /// Let par2cmdline choose (default block count of 2000).
    Auto,
    /// Fixed block size in bytes (`-s<n>`).
    Size(u64),
    /// Fixed block count (`-b<n>`).
    Count(u64),
}

/// How recovery files (.vol*.par2) are sized.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryFileMode {
    /// Automatic (default, variable sizes).
    Auto,
    /// Uniform recovery file sizes (`-u`).
    Uniform,
    /// Limit size of recovery files (`-l`).
    LimitSize,
    /// Exact number of recovery files (`-n<n>`, max 31).
    ExactCount(u64),
}

/// Configuration for creating PAR2 files.
#[derive(Debug, Clone)]
pub struct CreateConfig {
    /// Output PAR2 file path (base name).
    pub output_path: PathBuf,
    /// Data files to protect.
    pub data_files: Vec<PathBuf>,
    /// How much recovery data to generate.
    pub redundancy: RedundancySpec,
    /// Data block layout.
    pub blocks: BlockSpec,
    /// Recovery file sizing strategy.
    pub recovery_file_mode: RecoveryFileMode,
    /// Memory limit in MB.
    pub memory_limit: Option<u64>,
    /// Thread count for main processing.
    pub threads: Option<u64>,
    /// Thread count for hashing.
    pub hash_threads: Option<u64>,
    /// Recurse into subdirectories.
    pub recurse: bool,
    /// Reference path for data files.
    pub base_path: Option<PathBuf>,
    /// First recovery block number (for appending recovery data).
    pub first_recovery_block: Option<u64>,
}

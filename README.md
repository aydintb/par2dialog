# Par2Dialog

A modern, cross-platform **PAR2 GUI** application built with Rust and egui.

Par2Dialog provides a user-friendly interface for creating, verifying, and repairing files using PAR2 (Parity Archive 2.0) recovery data. Whether you're archiving important data or managing Usenet downloads, Par2Dialog helps you protect against data corruption and bitrot.

![Par2Dialog](assets/screenshot.png)

## Features

### 📦 Create
- Generate PAR2 recovery files with full parameter control
- Configurable redundancy: percentage, target size (KB/MB/GB), or exact block count
- Block size/count configuration with auto-calculation
- Recovery file sizing options: auto, uniform, limit size, or exact count
- Recursive directory scanning
- Memory and thread limits for fine-tuned performance
- Real-time progress tracking

### ✅ Verify
- Check file integrity against PAR2 sets
- Per-file status display (OK / Damaged / Missing)
- Detailed damage reports with block-level information
- Support for extra files not in the original PAR2 set
- Auto-discovery of PAR2 set files

### 🔧 Repair
- Repair damaged or missing files using Reed-Solomon error correction
- Feasibility check before starting repair
- Shows available vs needed recovery blocks
- Post-repair verification

### 📋 Batch
- Queue multiple PAR2 operations
- Sequential processing
- Summary report for all tasks

### ⚙️ Settings
- Default values for common parameters
- Dark and light themes
- par2cmdline auto-detection
- About and keyboard shortcuts reference

## Installation

### Prerequisites

- **Rust** (1.75+) — install via [rustup](https://rustup.rs/)
- **par2cmdline** — required for creating PAR2 files
  - Arch Linux: `sudo pacman -S par2cmdline`
  - Debian/Ubuntu: `sudo apt install par2cmdline`
  - Fedora: `sudo dnf install par2cmdline`
  - macOS: `brew install par2cmdline`
  - Windows: download from [par2cmdline releases](https://github.com/parchive/par2cmdline/releases)

### Build from Source

```bash
git clone https://github.com/atb/par2dialog.git
cd par2dialog
cargo build --release
```

The binary will be at `target/release/par2dialog`.

### Arch Linux (AUR)

```bash
# Using yay
yay -S par2dialog-git

# Or manually
git clone https://aur.archlinux.org/par2dialog-git.git
cd par2dialog-git
makepkg -si
```

### Windows

Download the latest release from the [Releases](https://github.com/atb/parpar/releases) page.

## Usage

### Creating PAR2 Files

1. Go to the **Create** tab
2. Select an output PAR2 file location
3. Add the data files you want to protect
4. Configure redundancy (default: 5% is recommended for most use cases)
5. Adjust advanced settings if needed
6. Click **🚀 Create PAR2 Files**

### Verifying Files

1. Go to the **Verify** tab
2. Select the PAR2 file
3. The data directory is auto-detected from the PAR2 file location
4. Optionally add extra files to check
5. Click **🔍 Verify**
6. Review the per-file status

### Repairing Files

1. Go to the **Repair** tab
2. Select the PAR2 file
3. Optionally click **🔍 Check Feasibility** first to see if repair is possible
4. Click **🔧 Repair**

## All PAR2 Parameters

Par2Dialog exposes all par2cmdline parameters through its UI:

| Parameter | Description | Default |
|-----------|-------------|---------|
| Redundancy % | Recovery data as % of total size | 5% |
| Block Size | Size of each recovery block (bytes) | Auto |
| Block Count | Number of blocks (mutually exclusive with Block Size) | 2000 |
| Recovery File Count | Max number of .vol*.par2 files (max 31) | 4 |
| Memory Limit | Max memory for processing (MB) | 50% RAM |
| Threads | Processing threads | Auto-detected |
| Hash Threads | Parallel hashing threads | 2 |
| Recurse | Include subdirectories | Off |
| Purge | Delete PAR2 on success | Off |
| Base Path | Reference path for data files | — |
| First Recovery Block | Starting block for appending | 0 |

## Architecture

```
┌─────────────────────────────────────┐
│           Par2Dialog GUI            │
│  (egui + eframe — native window)   │
├─────────────────────────────────────┤
│          Application Layer          │
│  ┌────────┬────────┬──────────┐    │
│  │ Create │ Verify │  Repair  │    │
│  │  Tab   │  Tab   │   Tab    │    │
│  └────────┴────────┴──────────┘    │
├─────────────────────────────────────┤
│          Backend Layer              │
│  ┌────────────────┬──────────────┐  │
│  │   rust-par2    │ par2cmdline  │  │
│  │ (verify/repair)│  (create)    │  │
│  └────────────────┴──────────────┘  │
└─────────────────────────────────────┘
```

- **Verify/Repair**: Uses [`rust-par2`](https://crates.io/crates/rust-par2) — a pure Rust, SIMD-accelerated PAR2 library
- **Create**: Spawns `par2cmdline` as a subprocess (since rust-par2 doesn't support creation yet)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Name

**Par2Dialog** — a clear, descriptive name for a PAR2 dialog/GUI application. Easy to find, easy to type, easy to remember.

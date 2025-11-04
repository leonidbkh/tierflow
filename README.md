# tierflow

[![CI](https://github.com/leonidbkh/tierflow/workflows/CI/badge.svg)](https://github.com/leonidbkh/tierflow/actions)
[![Crates.io](https://img.shields.io/crates/v/tierflow.svg)](https://crates.io/crates/tierflow)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

**Intelligent strategy-based file balancing system for tiered storage**

A Rust-based tool that automatically moves files between different storage tiers (NVMe, SSD, HDD) based on configurable placement strategies. Perfect for media servers, download management, and automated tiered storage systems.

## Features

- 🎯 **Declarative Strategies** - Define "where files SHOULD be" rather than "when to move"
- 🔄 **Bidirectional Movement** - Files can be promoted (to faster tier) or demoted (to slower tier)
- 📊 **Two-Pass Processing** - Collects statistics first, then applies strategies for optimal decisions
- 🎛️ **Priority System** - Strategies have priorities to resolve conflicts deterministically
- 🔒 **Safe Operation** - Locking prevents concurrent runs, dry-run mode for testing
- 📏 **Rich Conditions** - Age, size, extension, path, filename patterns with blacklist/whitelist modes
- 🌐 **Global Statistics** - Future support for conditions that depend on other files (e.g., "keep last N episodes")
- ⚙️ **Tier Capacity Limits** - Set max_usage_percent per tier to prevent overfilling
- 🤖 **Daemon Mode** - Run continuously in background with configurable intervals
- 📺 **Tautulli Integration** - Smart caching for Plex viewing windows with multi-user support

## Installation

### Quick Install (Recommended)

**Linux:**
```bash
curl -sSfL https://raw.githubusercontent.com/leonidbkh/tierflow/main/install.sh | sh
```

This will download and install the latest release binary to `/usr/local/bin` (or `~/.local/bin` if no sudo).

### Using Cargo

If you have Rust installed:
```bash
cargo install tierflow
```

### Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/leonidbkh/tierflow/releases):

- **Linux x86_64** (Intel/AMD): `tierflow-x86_64-unknown-linux-gnu.tar.gz`
- **Linux ARM64** (ARM servers): `tierflow-aarch64-unknown-linux-gnu.tar.gz`

```bash
# Example for x86_64
wget https://github.com/leonidbkh/tierflow/releases/latest/download/tierflow-x86_64-unknown-linux-gnu.tar.gz
tar -xzf tierflow-x86_64-unknown-linux-gnu.tar.gz
sudo mv tierflow /usr/local/bin/
```

### Build from Source

```bash
git clone https://github.com/leonidbkh/tierflow.git
cd tierflow
cargo build --release
sudo cp target/release/tierflow /usr/local/bin/
```

## Quick Start

### Configuration

1. Copy the example configuration:
```bash
cp config.example.yaml config.yaml
```

2. Edit `config.yaml` to define your tiers and strategies (see [Configuration](#configuration) below)

3. Run in dry-run mode first:
```bash
tierflow rebalance --config config.yaml --dry-run
```

4. Execute actual file movements:
```bash
tierflow rebalance --config config.yaml
```

5. Run in daemon mode for continuous operation:
```bash
# Run every hour (3600 seconds)
tierflow daemon --config config.yaml --interval 3600

# Or use systemd for production (see tierflow.service)
sudo cp tierflow.service /etc/systemd/system/
sudo systemctl enable --now tierflow
```

## Configuration

See [`config.example.yaml`](config.example.yaml) for a comprehensive example with detailed comments.

### Basic Concepts

**Tiers** - Storage locations with priority levels:
```yaml
tiers:
  - name: cache
    path: /mnt/nvme
    priority: 1              # Lower = faster
    max_usage_percent: 85    # Don't fill above 85%

  - name: storage
    path: /mnt/hdds
    priority: 10             # Higher = slower
```

**Strategies** - Rules that define where files should be placed:
```yaml
strategies:
  - name: recent_files_on_cache
    priority: 50             # Higher priority wins
    conditions:
      - type: max_age
        max_age_hours: 168   # 7 days
    preferred_tiers:
      - cache                # Try cache first
      - storage              # Fallback if cache full
    required: false          # Warn if no tier available
```

**Conditions** - Filters that match files (all conditions must match = AND logic):
```yaml
conditions:
  - type: file_size
    min_size_mb: 100         # At least 100MB
    max_size_mb: 5000        # Up to 5GB

  - type: file_extension
    extensions: ["mkv", "mp4"]
    mode: whitelist          # Only these extensions

  - type: path_prefix
    prefix: "downloads"      # Matches /mnt/*/downloads/*

  - type: filename_contains
    patterns: ["sample", "trailer"]
    mode: blacklist          # Exclude these
    case_sensitive: false
```

## Available Conditions

| Condition | Description | Parameters |
|-----------|-------------|------------|
| `always_true` | Always matches | None |
| `max_age` | Matches files older than X hours | `max_age_hours` |
| `file_size` | Matches files by size range | `min_size_mb`, `max_size_mb` |
| `file_extension` | Matches by file extensions | `extensions`, `mode` |
| `path_prefix` | Matches by path prefix (relative to tier) | `prefix`, `mode` |
| `filename_contains` | Matches by substring in filename | `patterns`, `mode`, `case_sensitive` |
| `active_window` | Matches files in Tautulli viewing window | `days_back`, `backward_episodes`, `forward_episodes` |

### Modes (for extension, path_prefix, filename_contains)

- **`whitelist`** - Matches if file HAS the pattern/extension
- **`blacklist`** - Matches if file DOES NOT have the pattern/extension

## Architecture

### Core Components

1. **Tiers** (`src/tier.rs`) - Storage locations with metadata
2. **Conditions** (`src/conditions/`) - File matching logic
3. **Strategies** (`src/strategy.rs`) - Combinations of conditions + target tiers
4. **Balancer** (`src/balancer/`) - Two-pass decision engine
5. **Executor** (`src/executor.rs`) - Applies balancing plan with progress tracking

### Two-Pass Processing

The balancer uses a two-pass approach for optimal decisions:

**Pass 1: Statistics Collection**
- Scans all files across all tiers
- Collects global statistics (directory sizes, file counts, etc.)
- Builds `GlobalStats` structure shared via `Arc<T>` (zero-copy)

**Pass 2: Strategy Application**
- Evaluates each file against strategies (highest priority wins)
- Considers simulated free space to prevent overfilling
- Respects `max_usage_percent` per tier
- Produces `BalancingPlan` with all decisions

This architecture enables future conditions that depend on other files, such as:
- "Keep last N episodes in series"
- "Keep directories with <X files together"
- "Balance files evenly across tiers"

### Project Structure

```
src/
├── main.rs                    # CLI entry point
├── cli.rs                     # Argument parsing
├── lib.rs                     # Module exports
│
├── config/                    # Configuration parsing
│   ├── mod.rs                 # Main config (MoverConfig, tiers, strategies)
│   ├── tier.rs                # Tier config
│   ├── condition.rs           # Condition config enums
│   ├── strategy.rs            # Strategy config
│   └── error.rs               # Config errors
│
├── balancer/                  # Decision engine
│   ├── mod.rs                 # Two-pass planning logic
│   ├── decision.rs            # PlacementDecision (Stay/Promote/Demote)
│   ├── plan.rs                # BalancingPlan + warnings
│   └── state.rs               # Planning state (simulated free space)
│
├── conditions/                # Condition implementations
│   ├── mod.rs                 # Condition trait + Context
│   ├── always_true.rs
│   ├── max_age.rs
│   ├── file_size.rs
│   ├── file_extension.rs
│   ├── path_prefix.rs
│   └── filename_contains.rs
│
├── stats/                     # Global statistics
│   └── mod.rs                 # GlobalStats, FileStats
│
├── executor.rs                # Execute balancing plan
├── strategy.rs                # PlacementStrategy
├── tier.rs                    # Tier runtime
├── file.rs                    # FileInfo
├── mover.rs                   # RsyncMover wrapper
├── lock.rs                    # Lock file handling
└── error.rs                   # AppError
```

## Development

### Prerequisites

- Rust 1.75+ (edition 2024)
- `rsync` (for file movement)

### Commands

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run all tests (261 tests)
cargo test

# Run specific test
cargo test test_can_accept_file

# Check code (fast)
cargo check

# Linting
cargo clippy

# Format code
cargo fmt

# Run with logging
RUST_LOG=debug cargo run -- --config config.yaml --dry-run
```

### Running Tests

The project has comprehensive test coverage:
- **Condition tests** - Each condition type has unit tests
- **Balancer tests** - File acceptance, capacity limits, simulation
- **Stats tests** - Statistics collection
- **Integration tests** - End-to-end scenarios

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test balancer::tests
```

## Documentation

Detailed documentation is available in `.claude/docs/`:

- `CONDITIONS_IMPROVEMENTS.md` - Analysis of conditions system
- `TWO_PASS_IMPLEMENTATION.md` - Two-pass architecture details
- `CLONE_OPTIMIZATION.md` - Performance optimization notes
- `GLOBAL_STATS_ARCHITECTURE.md` - Future external data sources

## Safety Features

- **Lock file** - Prevents concurrent execution
- **Dry-run mode** - Test without moving files
- **Warnings** - Alerts for failed required strategies
- **Projected usage** - Shows tier usage before/after
- **Deterministic sorting** - Consistent decisions across runs

## Examples

### Example 1: Simple Age-Based Balancing

Move old files to storage, keep recent files on cache:

```yaml
strategies:
  - name: old_to_storage
    priority: 10
    conditions:
      - type: max_age
        max_age_hours: 168  # 7 days
    preferred_tiers:
      - storage

  - name: recent_to_cache
    priority: 50
    conditions:
      - type: always_true
    preferred_tiers:
      - cache
      - storage
```

### Example 2: Size-Based with Blacklist

Keep large files on storage, but exclude downloads folder:

```yaml
strategies:
  - name: large_files_to_storage
    priority: 40
    conditions:
      - type: file_size
        min_size_mb: 5000  # 5GB+
      - type: path_prefix
        prefix: "downloads"
        mode: blacklist    # Not in downloads
    preferred_tiers:
      - storage
```

### Example 3: Complex Multi-Condition

Move old series to storage, but only complete files:

```yaml
strategies:
  - name: old_series_to_storage
    priority: 30
    conditions:
      - type: path_prefix
        prefix: "series"
      - type: max_age
        max_age_hours: 168
      - type: file_extension
        extensions: ["!qB", "part", "tmp"]
        mode: blacklist    # Exclude incomplete
    preferred_tiers:
      - storage
```

## License

MIT

## Contributing

Contributions are welcome! Please:
1. Run tests: `cargo test`
2. Run clippy: `cargo clippy`
3. Format code: `cargo fmt`
4. Update documentation if needed

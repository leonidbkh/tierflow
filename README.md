# tierflow

[![CI](https://github.com/leonidbkh/tierflow/workflows/CI/badge.svg)](https://github.com/leonidbkh/tierflow/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-blue.svg)](https://www.rust-lang.org)

> **⚠️ ALPHA SOFTWARE**: This project is in active development and testing. While functional, it may contain bugs and the API may change. Use with caution on production data. Always test with `--dry-run` first and keep backups.

Automatically moves files between storage tiers (NVMe, SSD, HDD) based on simple rules in a config file.

**Example use case**: Keep fresh files on fast SSD, automatically move them to slow HDDs after a week. Or vice versa - automatically pull needed files to the fast tier.

## Installation

**Linux x86_64:**
```bash
curl -sSfL https://raw.githubusercontent.com/leonidbkh/tierflow/main/install.sh | sh
```

Or download binary from [GitHub Releases](https://github.com/leonidbkh/tierflow/releases).

## Updating

**Update to latest version:**
```bash
curl -sSfL https://raw.githubusercontent.com/leonidbkh/tierflow/main/update.sh | sh
```

This will update the binary without touching your config or systemd service.

## Quick Start

After installation, edit the config file at `/etc/tierflow/config.yaml`:

```yaml
tiers:
  # Fast disk
  - name: nvme
    path: /mnt/nvme
    priority: 1
    max_usage_percent: 85

  # Slow archive
  - name: archive
    path: /mnt/hdds
    priority: 10

strategies:
  # Keep fresh files on fast disk
  - name: recent_on_nvme
    priority: 100
    conditions:
      - type: max_age
        max_age_hours: 168  # 7 days
    preferred_tiers:
      - nvme
      - archive  # fallback if nvme is full

  # Everything else to archive
  - name: default_archive
    priority: 1
    conditions:
      - type: always_true
    preferred_tiers:
      - archive
```

Then test it:

```bash
# Test with dry-run (shows what would happen)
tierflow rebalance --dry-run -v

# If everything looks good, run for real
tierflow rebalance -v

# Get JSON output for scripting
tierflow rebalance --format json --quiet

# Or enable automatic daemon mode
sudo systemctl enable --now tierflow
```

## Output and Logging

### Verbosity Levels

Control logging output with `-v` flags:

```bash
# Default: only warnings and errors
tierflow rebalance

# Info level (-v): show progress
tierflow rebalance -v

# Debug level (-vv): detailed execution info
tierflow rebalance -vv

# Trace level (-vvv): everything including library calls
tierflow rebalance -vvv

# Quiet mode: only errors
tierflow rebalance --quiet
```

### Output Formats

Choose output format for machine parsing:

```bash
# Human-readable (default)
tierflow rebalance --format text

# JSON for scripts and monitoring
tierflow rebalance --format json

# YAML for configuration management
tierflow rebalance --format yaml
```

**Important**: Logs go to **stderr**, results go to **stdout**. This allows clean separation:

```bash
# Save results to file, logs to terminal
tierflow rebalance --format json > results.json

# Save logs to file, results to terminal
tierflow rebalance -v 2> debug.log

# Save both separately
tierflow rebalance -v --format json > results.json 2> logs.txt

# Silent mode - only results
tierflow rebalance --format json --quiet | jq '.files_moved'
```

### Daemon Logs

**Check daemon logs:**
```bash
# View all logs
sudo journalctl -u tierflow

# Follow logs in real-time
sudo journalctl -u tierflow -f

# Logs from last hour
sudo journalctl -u tierflow --since "1 hour ago"

# Last 100 lines
sudo journalctl -u tierflow -n 100
```

### Environment Variable (Advanced)

You can still use `RUST_LOG` environment variable for fine-grained control:

```bash
# Override with environment variable
RUST_LOG=tierflow=debug tierflow rebalance

# Module-specific logging
RUST_LOG=tierflow::balancer=trace,tierflow::executor=debug tierflow rebalance
```

## Configuration

See the [examples directory](examples/) for complete, real-world configurations:
- [simple-cache.yaml](examples/simple-cache.yaml) - Basic cache/archive setup
- [plex-tautulli.yaml](examples/plex-tautulli.yaml) - Plex with Tautulli integration
- [three-tier.yaml](examples/three-tier.yaml) - NVMe/SSD/HDD configuration
- [exclude-patterns.yaml](examples/exclude-patterns.yaml) - Using `action: stay`
- [download-automation.yaml](examples/download-automation.yaml) - Sonarr/Radarr integration

## How It Works

### Tiers (disks)

Define your disks with priorities:

```yaml
tiers:
  - name: ssd
    path: /mnt/ssd
    priority: 1                # lower number = faster tier
    max_usage_percent: 90      # don't fill above 90%
    min_usage_percent: 30      # don't demote files until 30% full
```

### Strategies (rules)

Define rules for which files should go where:

```yaml
strategies:
  - name: old_files_to_archive
    priority: 50             # higher number = higher priority
    action: move             # Optional: 'move' (default) or 'stay'
    required: false          # Optional: warn if strategy can't be satisfied
    conditions:
      - type: age
        min_hours: 720       # older than 30 days
    preferred_tiers:
      - archive              # move to archive
```

**Strategy Options:**
- `priority`: Higher number = higher priority (matched first)
- `action`:
  - `move` (default) - Move files to preferred tiers
  - `stay` - Keep files where they are (exclude from management)
- `required`: If true, warns when files can't be placed on preferred tiers
- `conditions`: List of conditions (all must match - AND logic)
- `preferred_tiers`: Ordered list of tier preferences (tries first to last)

If a file matches multiple strategies, the one with higher `priority` wins.

### Conditions

Available filters:

| Condition | Description | Parameters | Example |
|-----------|-------------|------------|---------|
| `always_true` | Matches all files | None | For default strategy |
| `age` | Files within age range | `min_hours`, `max_hours` | `max_hours: 168` (older than 7 days) |
| `file_size` | Files within size range | `min_size_mb`, `max_size_mb` | `min_size_mb: 100, max_size_mb: 5000` |
| `file_extension` | Match by extension | `extensions`, `mode` | `extensions: ["mkv", "mp4"], mode: whitelist` |
| `path_prefix` | Match by path prefix | `prefix`, `mode` | `prefix: "downloads", mode: whitelist` |
| `filename_contains` | Match by filename substring | `patterns`, `mode`, `case_sensitive` | `patterns: ["sample"], mode: blacklist` |
| `active_window` | Files in Tautulli viewing window | `days_back`, `backward_episodes`, `forward_episodes` | `backward_episodes: 2, forward_episodes: 5` |

**Notes:**
- All conditions in a strategy must match (AND logic)
- `mode` can be `whitelist` (match if present) or `blacklist` (match if NOT present)
- `age` condition: use only `min_hours` for newer files, only `max_hours` for older files, or both for a range

## Configuration Examples

### Example 1: Simple age-based archival

Keep fresh files on SSD, old files on HDD:

```yaml
tiers:
  - name: ssd
    path: /mnt/ssd
    priority: 1
    max_usage_percent: 85

  - name: hdd
    path: /mnt/hdd
    priority: 10

strategies:
  # Files less than a week old - on SSD
  - name: recent_to_ssd
    priority: 100
    conditions:
      - type: age
        max_hours: 168  # Less than 7 days old
    preferred_tiers:
      - ssd
      - hdd  # if SSD is full

  # Everything else to HDD
  - name: old_to_hdd
    priority: 10
    conditions:
      - type: always_true
    preferred_tiers:
      - hdd
```

### Example 2: Keep downloads folder on fast disk

Active downloads on SSD, then moved to HDD:

```yaml
strategies:
  # Downloads always on SSD
  - name: downloads_on_ssd
    priority: 200
    conditions:
      - type: path_prefix
        prefix: "downloads"
        mode: whitelist
    preferred_tiers:
      - ssd

  # Old completed files - to HDD
  - name: old_to_hdd
    priority: 100
    conditions:
      - type: age
        min_hours: 48
      - type: path_prefix
        prefix: "downloads"
        mode: blacklist  # NOT in downloads folder
    preferred_tiers:
      - hdd
```

### Example 3: By file size

Small files on SSD, large files on HDD:

```yaml
strategies:
  # Small files (<1GB) on SSD
  - name: small_on_ssd
    priority: 50
    conditions:
      - type: file_size
        max_size_mb: 1000
    preferred_tiers:
      - ssd

  # Large files (>5GB) directly to HDD
  - name: large_on_hdd
    priority: 60
    conditions:
      - type: file_size
        min_size_mb: 5000
    preferred_tiers:
      - hdd
```

### Example 4: Exclude incomplete downloads

Keep incomplete downloads where they are using `action: stay`:

```yaml
strategies:
  # Don't move files being downloaded
  - name: exclude_incomplete
    priority: 999       # Very high priority to match first
    action: stay        # Keep files where they are
    conditions:
      - type: file_extension
        extensions: ["part", "!qB", "tmp", "crdownload"]
        mode: whitelist # Match these extensions
    preferred_tiers: [] # Empty - files stay in place
```

## How Eviction Works

When a tier reaches its `max_usage_percent`, tierflow automatically evicts files to make space:

### Eviction Priority

Files are evicted in this order:
1. **Lowest strategy priority first** - Files with priority 0 (`no-match`) evicted first
2. **Oldest files first** - Among same priority, older files evicted first
3. **Largest files first** - Among same age, larger files evicted first

### Eviction Process

1. **Pass 1**: Scan all tiers and collect statistics
2. **Pass 2**: Apply strategies and plan file movements
3. **Pass 3a**: If high-priority files need space, evict lower-priority files
4. **Pass 3b**: If any tier exceeds `max_usage_percent`, aggressively evict files

### Special Cases

- **`no-match` files**: Files not matching any strategy get priority 0 and are evicted first
- **`action: stay` files**: Never evicted or moved, stay exactly where they are
- **`required: true` strategies**: Generate warnings if files can't be placed as desired

### Example Eviction Scenario

```yaml
# Tier at 90% full (max_usage_percent: 85)
# Files will be evicted in this order:
1. camera_recording.mp4  # no-match (priority 0)
2. old_download.zip      # priority 10, 30 days old
3. recent_movie.mkv      # priority 10, 7 days old, 5GB
4. recent_episode.mp4    # priority 10, 7 days old, 1GB
5. active_show.mkv       # priority 100 (won't be evicted)
```

## Operation Modes

### Dry-run (test run)

Shows what would be done without actually moving files:
```bash
tierflow rebalance --config /etc/tierflow/config.yaml --dry-run
```

### One-time run

Performs file movement once:
```bash
tierflow rebalance --config /etc/tierflow/config.yaml
```

### Daemon mode

The install script can set up systemd service for you. Or manually:

```bash
# Run daemon manually (every hour)
tierflow daemon --config /etc/tierflow/config.yaml --interval 3600

# Or use systemd (already installed if you chose 'y' during installation)
sudo systemctl enable --now tierflow
sudo systemctl status tierflow
```

## Integration and Automation

### Shell Scripts

```bash
#!/bin/bash
# Monitor file movements and send alerts

RESULT=$(tierflow rebalance --format json --quiet)
FILES_MOVED=$(echo "$RESULT" | jq -r '.files_moved')
ERRORS=$(echo "$RESULT" | jq -r '.errors | length')

if [ "$FILES_MOVED" -gt 100 ]; then
    echo "Warning: $FILES_MOVED files moved!" | mail -s "Tierflow Alert" admin@example.com
fi

if [ "$ERRORS" -gt 0 ]; then
    echo "Errors occurred during rebalancing" | mail -s "Tierflow Error" admin@example.com
fi
```

### Python Integration

```python
import subprocess
import json

# Run tierflow and get results
result = subprocess.run(
    ['tierflow', 'rebalance', '--format', 'json', '--quiet'],
    capture_output=True,
    text=True
)

data = json.loads(result.stdout)

print(f"Files moved: {data['files_moved']}")
print(f"Bytes moved: {data['bytes_moved']}")

# Send to monitoring system
if data['files_moved'] > 0:
    send_to_prometheus(data)
```

### Cron Jobs

```bash
# Run hourly with minimal output (cron emails only on errors)
0 * * * * /usr/local/bin/tierflow rebalance --quiet 2>&1 | grep -i error

# Run daily with full logging
0 2 * * * /usr/local/bin/tierflow rebalance -v >> /var/log/tierflow-cron.log 2>&1

# Run with JSON output for processing
0 * * * * /usr/local/bin/tierflow rebalance --format json --quiet >> /var/log/tierflow-results.jsonl
```

### Prometheus Monitoring

```bash
# Export metrics in JSON format
tierflow rebalance --format json --quiet | \
  jq '{
    files_moved: .files_moved,
    bytes_moved: .bytes_moved,
    errors: (.errors | length)
  }' | \
  curl -X POST -H "Content-Type: application/json" \
    -d @- http://prometheus-pushgateway:9091/metrics/job/tierflow
```

### CI/CD Pipelines

```yaml
# GitHub Actions / GitLab CI example
- name: Balance storage tiers
  run: |
    tierflow rebalance --format json --quiet > results.json
    FILES_MOVED=$(jq -r '.files_moved' results.json)
    echo "files_moved=$FILES_MOVED" >> $GITHUB_OUTPUT
```

## How File Movement Works

- Uses `rsync` for reliable copying
- Copies file first, then deletes original
- Locking prevents concurrent runs
- Shows progress and statistics

## Requirements

- Linux x86_64
- `rsync` for file movement

## Development

```bash
# Build
cargo build --release

# Tests
cargo test

# Code checks
cargo clippy
cargo fmt
```

## License

MIT

## Contributing

Pull requests are welcome. Before submitting, run tests and clippy.

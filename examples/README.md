# Tierflow Configuration Examples

This directory contains real-world configuration examples for different use cases.

## Available Examples

### 📁 [simple-cache.yaml](simple-cache.yaml)
Basic two-tier setup with cache and archive storage. Good starting point for most users.
- Keep recent files (< 30 days) on fast storage
- Move old large files to archive
- Simple age and size-based rules

### 🎬 [plex-tautulli.yaml](plex-tautulli.yaml)
Plex Media Server integration with Tautulli for intelligent caching.
- Keep actively watched TV episodes on fast storage
- Use viewing windows (2 episodes back, 5 forward)
- Automatically move unwatched content to archive
- Requires Tautulli API setup

### 💾 [three-tier.yaml](three-tier.yaml)
Advanced setup with NVMe, SSD, and HDD tiers.
- Small hot files on NVMe
- Medium files on SSD
- Large cold files on HDD
- Multi-tier fallback chains
- Size and age-based tiering

### 🚫 [exclude-patterns.yaml](exclude-patterns.yaml)
How to exclude files from management using `action: stay`.
- Keep incomplete downloads in place
- Exclude temporary files
- Ignore files being processed
- Prevent moving active torrents
- Essential for download clients

### 📥 [download-automation.yaml](download-automation.yaml)
Integration with Sonarr/Radarr and torrent clients.
- Handle active vs completed downloads
- Fresh downloads stay on cache for seeding
- Media organization by type and age
- 4K content management
- Metadata always on fast storage

## Quick Start

1. Choose an example that matches your use case
2. Copy it to your config location:
   ```bash
   cp examples/simple-cache.yaml /etc/tierflow/config.yaml
   ```
3. Edit the configuration:
   - Update tier paths to match your mount points
   - Adjust `max_usage_percent` based on your needs
   - Modify strategies for your workflow
4. Test with dry-run:
   ```bash
   tierflow rebalance --dry-run
   ```
5. Run for real:
   ```bash
   tierflow rebalance
   ```

## Key Concepts

### Tier Priority
- Lower number = faster/more expensive storage
- Example: NVMe=1, SSD=5, HDD=10

### Strategy Priority
- Higher number = matched first
- `action: stay` strategies should have highest priority (900+)
- Default catch-all should have lowest priority (10)

### Eviction Order
When a tier is full, files are evicted based on:
1. Strategy priority (lower evicted first)
2. File age (older evicted first)
3. File size (larger evicted first)

### Special Strategies

#### Exclusions (`action: stay`)
```yaml
- name: exclude_incomplete
  priority: 999
  action: stay  # Never move these files
  conditions:
    - type: file_extension
      extensions: ["part", "tmp"]
      mode: whitelist
  preferred_tiers: []  # Empty
```

#### Required Strategies
```yaml
- name: critical_files
  priority: 100
  required: true  # Warn if can't satisfy
  conditions:
    - type: path_prefix
      prefix: "important"
  preferred_tiers: [nvme]
```

#### Multi-Tier Fallback
```yaml
preferred_tiers:
  - nvme    # Try first
  - ssd     # If nvme full
  - hdd     # Last resort
```

## Common Patterns

### Recent vs Old
```yaml
# Recent files on fast storage
- type: age
  max_hours: 168  # < 7 days

# Old files to archive
- type: age
  min_hours: 720  # > 30 days
```

### Size-Based Tiering
```yaml
# Small files on NVMe
- type: file_size
  max_size_mb: 100

# Large files on HDD
- type: file_size
  min_size_mb: 5000
```

### Path-Based Organization
```yaml
# TV shows
- type: path_prefix
  prefix: "tv"
  mode: whitelist

# Exclude downloads
- type: path_prefix
  prefix: "downloads/incomplete"
  mode: whitelist
```

## Tips

1. **Always include a default strategy** with `always_true` condition
2. **Use high priorities (900+)** for exclusion rules
3. **Test with `--dry-run`** before running
4. **Monitor logs** with `-vv` for debugging
5. **Set `max_usage_percent`** to leave headroom (80-85% typical)
6. **Use `min_usage_percent`** to prevent thrashing
7. **Order `preferred_tiers`** from fastest to slowest

## Need Help?

- Check the [main README](../README.md) for detailed documentation
- Run `tierflow rebalance --help` for CLI options
- Enable debug logging with `-vv` flag
- File issues at [GitHub](https://github.com/yourusername/tierflow)
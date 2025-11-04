# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of Tierflow (formerly mergerfs-balancer)
- Strategy-based file placement system
- Two-pass balancing engine with global statistics
- Support for tiered storage (NVMe, SSD, HDD)
- Tautulli (Plex) integration for smart episode management
- Daemon mode for continuous background operation
- Lock file mechanism to prevent concurrent execution
- Dry-run mode for testing configurations
- 7 condition types:
  - `always_true` - Always matches
  - `max_age` - File age based on modified time
  - `file_size` - File size range filtering
  - `file_extension` - Extension whitelist/blacklist
  - `path_prefix` - Path-based filtering
  - `filename_contains` - Filename pattern matching
  - `active_window` - Tautulli viewing window integration
- Comprehensive test suite (260+ tests)
- Systemd service file for production deployment
- Configurable tier capacity limits (`max_usage_percent`)
- Progress tracking during file movements
- Detailed logging with configurable levels

### Technical
- Built with Rust 2024 edition
- Compile-time regex validation with lazy-regex
- Strict clippy linting (pedantic + all)
- Thread-safe design with Arc and OnceLock
- Zero-copy statistics sharing

## [0.1.0] - TBD

### Initial Release
- First public release
- Core balancing functionality
- Tautulli integration
- Documentation and examples

---

## Version History Notes

### Naming
- **0.1.0+**: Released as `tierflow`
- **Pre-release**: Developed as `mergerfs-balancer`

### Compatibility
- **Rust**: Requires 1.75+ (edition 2024)
- **OS**: Linux, macOS (Unix-like systems)
- **Dependencies**: rsync for file movement

[Unreleased]: https://github.com/leonidbkh/tierflow/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/leonidbkh/tierflow/releases/tag/v0.1.0

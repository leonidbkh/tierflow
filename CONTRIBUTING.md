# Contributing to Tierflow

Thanks for your interest! Bug reports, feature requests, and pull requests are welcome.

## Prerequisites

- Rust 1.91+
- `rsync` installed

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Format and lint
cargo fmt --all
cargo clippy --all-targets -- -D warnings

# Test your changes
cargo test && cargo clippy --all-targets -- -D warnings
```

## Before Submitting a PR

1. **Add tests** for new functionality
2. **Update documentation** if behavior changes (README.md, config.example.yaml)
3. **Run checks**:
   ```bash
   cargo test && cargo fmt --all && cargo clippy --all-targets -- -D warnings
   ```
4. **Write clear commit messages**:
   - `feat: add new condition type`
   - `fix: correct percentage calculation`
   - `docs: update examples`

## Bug Reports

Include:
- Tierflow version (`tierflow --version`)
- OS and Rust version
- Config file (remove sensitive data)
- Steps to reproduce
- Expected vs actual behavior
- Logs with `RUST_LOG=debug`

## Feature Requests

- Check existing issues first
- Describe the use case
- Explain why it's useful for others

## Questions

Open an issue or check closed issues for similar questions.

## License

By contributing, you agree your contributions will be licensed under MIT License.

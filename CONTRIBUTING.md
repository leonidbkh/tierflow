# Contributing to Tierflow

Thank you for your interest in contributing to Tierflow! This document provides guidelines and instructions for contributing.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/tierflow.git`
3. Create a branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Prerequisites

- Rust 1.75+ (edition 2024)
- `rsync` for file movement operations

### Building

```bash
cargo build
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all-targets

# Fix formatting issues
cargo fmt --all

# Auto-fix clippy suggestions
cargo clippy --fix
```

## Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Write clear, descriptive variable and function names
- Add comments for complex logic
- Keep functions focused and reasonably sized
- Use `Result<T>` for error handling (avoid `unwrap()` in production code)

## Testing Guidelines

- Add unit tests for new functionality
- Test edge cases and error conditions
- Ensure existing tests still pass
- Aim for comprehensive test coverage

Current test coverage: 260+ tests

## Submitting Changes

1. **Write clear commit messages**:
   ```
   feat: add new condition type for file access time
   fix: correct percentage calculation in tier usage
   docs: update configuration examples
   test: add tests for episode parsing edge cases
   ```

2. **Keep commits focused**: One logical change per commit

3. **Update documentation**: If you change behavior, update:
   - Code comments
   - README.md
   - config.example.yaml
   - .claude/docs/ (if architectural changes)

4. **Add tests**: All new features should include tests

5. **Run all checks** before pushing:
   ```bash
   cargo test && cargo fmt --all && cargo clippy --all-targets
   ```

6. **Create a Pull Request**:
   - Provide a clear description of changes
   - Reference any related issues
   - Include screenshots/examples if relevant

## Adding New Conditions

To add a new condition type:

1. Create `src/conditions/your_condition.rs`:
   ```rust
   pub struct YourCondition {
       // fields
   }

   impl Condition for YourCondition {
       fn matches(&self, file: &FileInfo, context: &Context) -> bool {
           // implementation
       }
   }
   ```

2. Add to `src/config/condition.rs`:
   ```rust
   #[derive(Debug, Deserialize)]
   #[serde(tag = "type", rename_all = "snake_case")]
   pub enum ConditionConfig {
       YourCondition { /* fields */ },
       // ...
   }
   ```

3. Export in `src/conditions/mod.rs`

4. Add tests (minimum 5-10 test cases)

5. Update `config.example.yaml` with example usage

6. Update README.md table of conditions

## Adding New Features

For larger features:

1. **Open an issue first** to discuss the design
2. Break work into smaller, reviewable commits
3. Update documentation throughout development
4. Consider backward compatibility
5. Add integration tests if needed

## Bug Reports

When filing a bug report, include:

- Tierflow version (`tierflow --version`)
- Operating system and version
- Rust version (`rustc --version`)
- Configuration file (sanitized)
- Steps to reproduce
- Expected vs actual behavior
- Relevant log output (`RUST_LOG=debug`)

## Feature Requests

Feature requests are welcome! Please:

- Check existing issues first
- Describe the use case clearly
- Explain why it would benefit other users
- Consider providing a PR if possible

## Code Review Process

- Maintainers will review PRs as time permits
- Address review feedback promptly
- Be open to suggestions and improvements
- CI must pass before merging

## Questions?

- Open an issue for questions
- Check existing documentation in `.claude/docs/`
- Review closed issues for similar questions

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help create a welcoming environment
- Assume good intentions

Thank you for contributing to Tierflow!

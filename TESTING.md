# Testing Guide for File Safety Features

This document describes how to test the file safety features that protect against data loss.

## Running Integration Tests

The integration tests verify all safety mechanisms:

```bash
# Run all integration tests (requires rsync)
cargo test --lib mover -- --ignored --test-threads=1

# Run specific test
cargo test --lib mover::tests::test_rsync_mover_actual_move -- --ignored

# Run with debug output
RUST_LOG=debug cargo test --lib mover -- --ignored --nocapture
```

## Test Coverage

The test suite covers:

### 1. Basic Functionality
- **test_rsync_mover_actual_move** - Normal file move operation
- **test_rsync_mover_source_not_found** - Error handling for missing source

### 2. Destination Handling
- **test_rsync_mover_identical_destination_exists** - Skip copy if files are identical
- **test_rsync_mover_different_destination_exists** - Create backup when destination differs

### 3. Attribute Preservation
- **test_rsync_mover_preserves_permissions** - Verify permissions are preserved
- Extended attributes and ACLs (Linux only)

### 4. Large File Handling
- **test_rsync_mover_large_file** - Test with 10MB file

## Manual Testing Scenarios

### Test 1: File In Use Detection (Linux only)

```bash
# Terminal 1: Create and open a file
echo "test content" > /tmp/source.txt
tail -f /tmp/source.txt

# Terminal 2: Try to move the file
cargo build --release
./target/release/tierflow rebalance --config config.yaml --dry-run

# Expected: "Source file is currently in use" error
```

### Test 2: Concurrent Modification Detection

```bash
# Terminal 1: Start a long copy
dd if=/dev/zero of=/tmp/large_source.bin bs=1M count=1000

# Terminal 2: Start move while file is being written
# Expected: "Source file was modified during copy" error
```

### Test 3: Destination Corruption Detection

```bash
# This is harder to test manually, but you can simulate by:
# 1. Start a file move
# 2. Pause at checksum verification (add sleep in code)
# 3. Modify destination file
# 4. Resume
# Expected: "Destination file corrupted after verification" error
```

### Test 4: Double Checksum Verification

Enable debug logging to see the double checksum in action:

```bash
RUST_LOG=debug ./target/release/tierflow rebalance --config config.yaml

# Look for logs:
# "Performing final destination integrity check before deletion"
```

### Test 5: Post-Deletion Verification

```bash
# This protects against race conditions where another process deletes destination
# Hard to test manually, but code will detect if destination disappears
```

## Safety Mechanisms Verification

The following safety checks are performed during each file move:

1. ✅ Source exists check
2. ✅ Source not in use (fuser on Linux)
3. ✅ Destination backup if exists and differs
4. ✅ rsync with checksums and archive mode
5. ✅ Destination exists after copy
6. ✅ Size verification (source == destination)
7. ✅ Initial checksum verification
8. ✅ Source size unchanged during copy
9. ✅ **Source mtime unchanged during copy** (NEW)
10. ✅ Source not in use before deletion
11. ✅ **Double checksum verification** (NEW)
12. ✅ Delete source
13. ✅ **Destination still exists** (NEW)

## Performance Testing

To measure the overhead of the safety checks:

```bash
# Without safety checks (baseline - use older commit)
time ./target/release/tierflow rebalance --config config.yaml

# With safety checks (current)
time ./target/release/tierflow rebalance --config config.yaml

# The overhead is primarily from:
# - Double checksum calculation (~2x SHA256)
# - Two fuser calls per file (Linux only, ~1ms each)
# - Two mtime comparisons (negligible)
```

For a 1GB file:
- SHA256 checksum: ~500ms (depends on disk speed)
- fuser check: ~1ms
- mtime check: <1ms

Total overhead: ~1 second for double checksum on 1GB file.

## Testing on Real Server

To test on your actual storage server:

1. **Backup first!** Create snapshots of all tiers

2. Start with dry-run mode:
   ```bash
   ./tierflow rebalance --config config.yaml --dry-run
   ```

3. Enable debug logging:
   ```bash
   RUST_LOG=debug ./tierflow rebalance --config config.yaml --dry-run > test.log 2>&1
   ```

4. Review the log for:
   - Any "in use" detections
   - Any "modified during copy" detections
   - All checksums match

5. Run actual rebalance on a small subset first:
   ```bash
   # Test with just one tier or strategy
   RUST_LOG=info ./tierflow rebalance --config config.yaml
   ```

6. Monitor logs for warnings/errors:
   ```bash
   journalctl -u tierflow -f
   ```

## Expected Behavior

### Normal Operation
```
INFO Copying file: /source/file.mkv -> /dest/file.mkv
DEBUG Performing final destination integrity check before deletion
INFO Successfully moved: /source/file.mkv -> /dest/file.mkv (checksum: abc123...)
```

### File In Use (Protected)
```
WARN Source file is currently in use: /source/file.mkv
ERROR Source file is currently in use: /source/file.mkv
```

### Modified During Copy (Protected)
```
WARN Source file changed during copy! Before: 1000 bytes @ Some(...), After: 2000 bytes @ Some(...)
INFO Cleaning up stale copy
ERROR Source file was modified during copy. Stale copy removed: /dest/file.mkv
```

### Destination Corrupted (Protected)
```
ERROR Destination checksum changed after initial verification! Initial: abc123, Final: def456
ERROR Destination file corrupted after verification. Not deleting source for safety: /source/file.mkv
```

## Troubleshooting

### Tests fail with "rsync: --acl: unknown option"
This is expected on non-Linux systems (macOS, BSD). The code now uses conditional compilation to only add --acl and --xattrs on Linux.

### Tests fail with "fuser: command not found"
On macOS and non-Linux systems, fuser checks are skipped. This is expected behavior.

### "Destination file disappeared after source deletion"
This is a critical error indicating data loss. Immediately:
1. Stop tierflow
2. Check filesystem health
3. Review system logs
4. Check for concurrent processes modifying files

## CI/CD Integration

To run tests in CI:

```yaml
# .github/workflows/test.yml
- name: Run unit tests
  run: cargo test --lib

- name: Run integration tests
  run: cargo test --lib -- --ignored --test-threads=1
```

## Fuzzing (Future)

Consider adding fuzzing tests for:
- Concurrent file modifications
- Filesystem errors during copy
- Partial writes
- Permission changes during operation

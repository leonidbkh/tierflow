#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn dry_run_skips_lsof_and_does_not_create_destination_directories()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let cache = temp.path().join("cache");
    let storage = temp.path().join("storage");
    let source_parent = cache.join("nested/show");
    fs::create_dir_all(&source_parent)?;
    fs::create_dir_all(&storage)?;
    fs::write(source_parent.join("episode.mkv"), b"media")?;

    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin)?;
    let lsof_marker = temp.path().join("lsof-called");

    let lsof = fake_bin.join("lsof");
    fs::write(&lsof, "#!/bin/sh\n: > \"$LSOF_MARKER\"\nexit 1\n")?;
    fs::set_permissions(&lsof, fs::Permissions::from_mode(0o755))?;

    let rsync = fake_bin.join("rsync");
    fs::write(
        &rsync,
        "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\nexit 99\n",
    )?;
    fs::set_permissions(&rsync, fs::Permissions::from_mode(0o755))?;

    let config = serde_json::json!({
        "tiers": [
            { "name": "cache", "path": cache, "priority": 1 },
            { "name": "storage", "path": storage, "priority": 10 }
        ],
        "strategies": [{
            "name": "move_all_to_storage",
            "priority": 10,
            "required": false,
            "conditions": [{ "type": "always_true" }],
            "preferred_tiers": ["storage"]
        }],
        "mover": { "type": "rsync" }
    });
    let config_path = temp.path().join("config.json");
    fs::write(&config_path, serde_json::to_vec(&config)?)?;

    let path = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_tierflow"))
        .args(["rebalance", "--dry-run", "--format", "json", "--config"])
        .arg(&config_path)
        .env("PATH", path)
        .env("LSOF_MARKER", &lsof_marker)
        .output()?;

    assert!(
        output.status.success(),
        "tierflow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!lsof_marker.exists(), "dry-run invoked lsof");
    assert!(
        !storage.join("nested/show").exists(),
        "dry-run created destination directories"
    );

    Ok(())
}

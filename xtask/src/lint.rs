use std::{fs, path::Path};

use crate::process::run;
use std::process::Command;

/// Longest allowed Rust source file. Longer files belong in submodules.
const MAX_FILE_LINES: usize = 600;

/// Runs every check that CI runs: formatting, clippy and the file length limit.
pub(crate) fn lint() -> Result<(), String> {
    run(Command::new("cargo").args(["fmt", "--all", "--check"]))?;
    run(Command::new("cargo").args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    check_file_lengths()
}

/// Fails with the list of Rust files over `MAX_FILE_LINES`.
fn check_file_lengths() -> Result<(), String> {
    let mut too_long = Vec::new();
    for root in ["core", "xtask"] {
        collect_too_long(Path::new(root), &mut too_long)?;
    }
    if too_long.is_empty() {
        println!("file length: ok (limit {MAX_FILE_LINES} lines)");
        return Ok(());
    }
    Err(format!(
        "split these into submodules (limit {MAX_FILE_LINES} lines):\n{}",
        too_long.join("\n")
    ))
}

fn collect_too_long(dir: &Path, found: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                collect_too_long(&path, found)?;
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            let lines = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?
                .lines()
                .count();
            if lines > MAX_FILE_LINES {
                found.push(format!("  {} ({lines} lines)", path.display()));
            }
        }
    }
    Ok(())
}

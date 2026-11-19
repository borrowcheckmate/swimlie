//! Exercise workflow entry points without Apple tools or network access.

use std::process::Command;

fn xtask() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

#[test]
fn pr_metadata_is_checked_without_interpreting_shell_syntax() {
    let accepted = xtask()
        .arg("pr-check")
        .env("PR_TITLE", "ci: align contributor checks")
        .env(
            "PR_BODY",
            "Closes #32\nChecks literal $(false) and `false` as text.",
        )
        .output()
        .unwrap();
    assert!(accepted.status.success(), "{accepted:?}");

    let rejected = xtask()
        .arg("pr-check")
        .env("PR_TITLE", "Update workflow")
        .env("PR_BODY", "")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("header must look like"));
}

#[test]
fn missing_pr_metadata_fails_clearly() {
    let output = xtask()
        .arg("pr-check")
        .env_remove("PR_TITLE")
        .env_remove("PR_BODY")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("PR_TITLE is required"));
}

#[test]
fn commit_hook_rejects_invalid_messages() {
    let path = std::env::temp_dir().join(format!("swimlie-commit-{}.txt", std::process::id()));
    std::fs::write(&path, "not a conventional header\n").unwrap();
    let rejected = xtask().arg("commit-msg").arg(&path).output().unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("header must look like"));
    std::fs::write(&path, "ci: align workflow checks\n").unwrap();
    let accepted = xtask().arg("commit-msg").arg(&path).output().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(accepted.status.success(), "{accepted:?}");
}

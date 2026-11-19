use std::path::{Path, PathBuf};
use std::process::Command;

use crate::policy::email_from_ident;

pub(crate) fn root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask must live inside the workspace".to_owned())
}

pub(crate) fn tracked_files(root: &Path) -> Result<Vec<String>, String> {
    Ok(git(root, &["ls-files", "-z"])?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

pub(crate) fn current_identity_emails(root: &Path) -> Result<Vec<String>, String> {
    ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"]
        .iter()
        .map(|variable| {
            let ident = git(root, &["var", variable])?;
            email_from_ident(&ident).ok_or_else(|| format!("{variable} has no email"))
        })
        .collect()
}

pub(crate) fn commit_emails(root: &Path, range: &str) -> Result<Vec<String>, String> {
    Ok(git(root, &["log", "--format=%ae%n%ce", range])?
        .lines()
        .map(str::to_owned)
        .collect())
}

pub(crate) fn commit_messages(root: &Path, range: &str) -> Result<Vec<String>, String> {
    Ok(git(root, &["log", "--format=%B%x00", range])?
        .split('\0')
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
        .collect())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("git output is not UTF-8: {error}"))
}

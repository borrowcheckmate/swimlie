use super::{Command, DOTENV, ErrorKind, env, fail, fs};

/// Parses `KEY=VALUE` lines: `#` comment lines, blank lines and one pair of matching quotes.
pub(crate) fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let value = match value.as_bytes() {
                [q @ (b'"' | b'\''), .., last] if last == q => {
                    value.get(1..value.len() - 1).unwrap_or(value)
                }
                _ => value,
            };
            Some((key.trim().to_string(), value.to_string()))
        })
        .collect()
}

/// The setting `key` as written: a non-empty environment variable, else a non-empty `.env` value.
pub(crate) fn raw_setting(key: &str) -> Option<String> {
    let from_env = env::var(key).ok().filter(|v| !v.is_empty());
    from_env.or_else(|| {
        DOTENV
            .get()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    })
}

/// The setting `key`; a value that is an `op://` reference is read from 1Password (exits on failure).
pub(crate) fn setting(key: &str) -> Option<String> {
    raw_setting(key).map(|v| {
        if v.starts_with("op://") {
            op_read(&v)
        } else {
            v
        }
    })
}

/// The secret at a 1Password reference, via the `op` CLI. Exits with a hint when it cannot be read.
pub(crate) fn op_read(reference: &str) -> String {
    let out = match Command::new("op").args(["read", reference]).output() {
        Ok(out) => out,
        Err(e) if e.kind() == ErrorKind::NotFound => fail(&format!(
            "1Password CLI `op` is not installed (needed for {reference}); install it from https://developer.1password.com/docs/cli and run `op signin`"
        )),
        Err(e) => fail(&format!("cannot run `op` for {reference}: {e}")),
    };
    if !out.status.success() {
        fail(&format!(
            "`op read {reference}` failed: {}; sign in with `op signin` or unlock 1Password",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        fail(&format!("`op read {reference}` returned an empty value"));
    }
    value
}

/// Writes the 1Password document at `reference` to `path` with mode 600 (removed again on failure).
pub(crate) fn op_download(reference: &str, path: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all("build").map_err(|e| format!("cannot create build: {e}"))?;
    let _ = fs::remove_file(path);
    let out = Command::new("op")
        .args(["read", reference, "--out-file", path])
        .output()
        .map_err(|e| {
            format!("cannot run `op` (install the 1Password CLI and run `op signin`): {e}")
        })?;
    if !out.status.success() {
        let _ = fs::remove_file(path);
        return Err(format!(
            "`op read {reference}` failed: {}; sign in with `op signin` or unlock 1Password",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("cannot restrict {path}: {e}"))
}

/// The iPhone app's bundle id from `SWIMLIE_BUNDLE_ID`; the watch app's adds `.watchkitapp`. Bundle ids are unique
/// across Apple, so a device or `TestFlight` build needs one registered to your team; the default only suits the simulator.
pub(crate) fn bundle_id() -> String {
    setting("SWIMLIE_BUNDLE_ID").unwrap_or_else(|| "com.swimlie.swimlie".to_string())
}

/// The Apple developer team id used for signing, from `SWIMLIE_TEAM_ID`.
pub(crate) fn team_id() -> Result<String, String> {
    setting("SWIMLIE_TEAM_ID").ok_or_else(|| {
        "SWIMLIE_TEAM_ID is not set (Apple developer team id, see .env.example)".to_string()
    })
}

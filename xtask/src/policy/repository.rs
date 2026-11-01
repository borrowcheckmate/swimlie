use std::{fs, path::Path};

use super::{Violation, leaks};

pub(crate) fn check_repository(root: &Path, paths: &[String]) -> Result<Vec<Violation>, String> {
    let mut violations = Vec::new();
    for path in paths {
        let full = root.join(path);
        if !full.exists() {
            continue;
        }
        let bytes = fs::read(&full).map_err(|error| format!("cannot read {path}: {error}"))?;
        if forbidden_path(path) {
            violations.push(Violation::new(path, "remove local settings or key file"));
        }
        if leaks::has_image_metadata(path, &bytes) {
            violations.push(Violation::new(path, "remove image metadata"));
        }
        if let Ok(text) = std::str::from_utf8(&bytes) {
            violations.extend(
                leaks::find_leaks(text)
                    .into_iter()
                    .map(|(line, message)| Violation::at_line(path, line, message)),
            );
            if leaks::declares_authors(path, text) {
                violations.push(Violation::new(path, "remove Cargo authors"));
            }
        }
    }
    Ok(violations)
}

fn forbidden_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == ".env"
        || (name.starts_with(".env.") && name != ".env.example")
        || [".pem", ".p8", ".p12", ".key"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::{check_repository, forbidden_path};

    #[test]
    fn rejects_local_settings_and_keys_but_allows_examples() {
        for path in [".env", "apple/.env.local", "cert.p8", "key.pem"] {
            assert!(forbidden_path(path), "{path}");
        }
        assert!(!forbidden_path(".env.example"));
    }

    #[test]
    fn scans_files_and_skips_deletions() {
        let root = std::env::temp_dir().join(format!("swimlie-policy-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("Cargo.toml");
        std::fs::write(&file, "[package]\nauthors = []\n").unwrap();
        let violations = check_repository(&root, &["Cargo.toml".into(), "deleted".into()]).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].message, "remove Cargo authors");
        std::fs::remove_dir_all(root).unwrap();
    }
}

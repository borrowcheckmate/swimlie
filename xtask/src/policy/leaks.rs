use super::identity::is_anonymous_email;

const PLACEHOLDER_DOMAINS: [&str; 3] = ["example.com", "example.org", "example.net"];
const PLACEHOLDER_SUFFIXES: [&str; 4] = [".example", ".test", ".invalid", ".localhost"];
const SERVICE_ADDRESSES: [&str; 1] = ["git@github.com"];
const HOME_PREFIXES: [&str; 3] = ["/Users/", "/home/", ":\\Users\\"];
const PRIVATE_KEY_MARKER: &str = concat!("PRIVATE", " KEY-----");
const TOKENS: [(&str, usize); 12] = [
    ("ghp_", 36),
    ("gho_", 36),
    ("ghu_", 36),
    ("ghs_", 36),
    ("ghr_", 36),
    ("github_pat_", 22),
    ("glpat-", 20),
    ("xoxb-", 10),
    ("xoxp-", 10),
    ("sk-ant-", 20),
    ("AKIA", 16),
    ("AIza", 35),
];
const IMAGE_EXTENSIONS: [&str; 10] = [
    "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "heic", "heif", "avif",
];
const IMAGE_METADATA_MARKERS: [&[u8]; 3] = [b"Exif", b"eXIf", b"x:xmpmeta"];

pub(crate) fn find_leaks(text: &str) -> Vec<(usize, &'static str)> {
    let mut leaks: Vec<(usize, &'static str)> = Vec::new();
    leaks.extend(personal_emails(text).map(|at| (line_of(text, at), "personal email address")));
    leaks.extend(home_paths(text).map(|at| (line_of(text, at), "path into a home directory")));
    leaks.extend(secrets(text).map(|at| (line_of(text, at), "possible secret or token")));
    leaks.sort_unstable();
    leaks
}

pub(crate) fn has_image_metadata(path: &str, bytes: &[u8]) -> bool {
    let is_image = path.rsplit_once('.').is_some_and(|(_, extension)| {
        IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    });
    is_image
        && IMAGE_METADATA_MARKERS
            .iter()
            .any(|marker| bytes.windows(marker.len()).any(|window| window == *marker))
}

pub(crate) fn declares_authors(path: &str, text: &str) -> bool {
    let is_manifest = path.rsplit('/').next() == Some("Cargo.toml");
    is_manifest
        && text.lines().any(|line| {
            line.trim_start()
                .strip_prefix("authors")
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
}

fn line_of(text: &str, offset: usize) -> usize {
    text.bytes()
        .take(offset)
        .filter(|&byte| byte == b'\n')
        .count()
        + 1
}

const fn is_local_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

const fn is_domain_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

const fn is_token_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn personal_emails(text: &str) -> impl Iterator<Item = usize> + '_ {
    let bytes = text.as_bytes();
    text.match_indices('@').filter_map(move |(at, _)| {
        let before = bytes.get(..at)?;
        let start = before
            .iter()
            .rposition(|&byte| !is_local_part(byte))
            .map_or(0, |index| index + 1);
        let after = bytes.get(at + 1..)?;
        let length = after
            .iter()
            .position(|&byte| !is_domain_part(byte))
            .unwrap_or(after.len());
        let local = text.get(start..at)?;
        let domain = text.get(at + 1..at + 1 + length)?.trim_end_matches('.');
        let email = format!("{local}@{domain}");
        is_personal_email(local, domain, &email).then_some(start)
    })
}

fn is_personal_email(local: &str, domain: &str, email: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    let has_alphabetic_tld = domain.rsplit_once('.').is_some_and(|(_, tld)| {
        tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic())
    });
    !local.is_empty()
        && has_alphabetic_tld
        && !is_anonymous_email(email)
        && !SERVICE_ADDRESSES.contains(&email.to_ascii_lowercase().as_str())
        && !PLACEHOLDER_DOMAINS.contains(&domain.as_str())
        && !PLACEHOLDER_SUFFIXES
            .iter()
            .any(|suffix| domain.ends_with(suffix))
}

fn home_paths(text: &str) -> impl Iterator<Item = usize> + '_ {
    let bytes = text.as_bytes();
    HOME_PREFIXES.iter().flat_map(move |prefix| {
        text.match_indices(prefix).filter_map(move |(at, _)| {
            let rest = bytes.get(at + prefix.len()..)?;
            let name = rest.iter().position(|&byte| {
                !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            })?;
            let separator = rest.get(name)?;
            (name > 0 && matches!(separator, b'/' | b'\\')).then_some(at)
        })
    })
}

fn secrets(text: &str) -> impl Iterator<Item = usize> + '_ {
    let bytes = text.as_bytes();
    let keys = text.match_indices(PRIVATE_KEY_MARKER).map(|(at, _)| at);
    let tokens = TOKENS.iter().flat_map(move |&(prefix, min_length)| {
        text.match_indices(prefix).filter_map(move |(at, _)| {
            let glued = at
                .checked_sub(1)
                .and_then(|index| bytes.get(index))
                .is_some_and(|&byte| is_token_part(byte));
            let rest = bytes.get(at + prefix.len()..)?;
            let length = rest.iter().take_while(|&&byte| is_token_part(byte)).count();
            (!glued && length >= min_length).then_some(at)
        })
    });
    keys.chain(tokens)
}

#[cfg(test)]
mod tests {
    use super::{declares_authors, find_leaks, has_image_metadata};

    fn email(local: &str, domain: &str) -> String {
        [local, domain].join("@")
    }

    #[test]
    fn flags_personal_emails_with_line_numbers() {
        let text = format!("first\ncontact {}.\n", email("jane.doe", "company.nl"));
        assert_eq!(find_leaks(&text), [(2, "personal email address")]);
    }

    #[test]
    fn allows_anonymous_placeholder_and_service_addresses() {
        let text = [
            email("1+octo", "users.noreply.github.com"),
            email("jane", "example.com"),
            email("jane", "corp.test"),
            email("git", "github.com:owner/repo"),
            email("actions/checkout", "v4"),
            email("serde", "1.0.200"),
            "@AGENTS.md".to_owned(),
        ]
        .join("\n");
        assert_eq!(find_leaks(&text), Vec::<(usize, &str)>::new());
    }

    #[test]
    fn flags_home_directory_paths() {
        let mac = ["", "Users", "jdoe", "project"].join("/");
        let linux = ["", "home", "jdoe", ".ssh"].join("/");
        let windows = ["C:", "Users", "jdoe", "src"].join("\\");
        for path in [mac, linux, windows] {
            assert_eq!(
                find_leaks(&path),
                [(1, "path into a home directory")],
                "{path}"
            );
        }
        let unrelated = ["", "Users", "<you>", "x"].join("/");
        assert_eq!(find_leaks(&unrelated), Vec::<(usize, &str)>::new());
    }

    #[test]
    fn flags_tokens_and_private_keys() {
        let github = format!("token = {}{}", "gh", format_args!("p_{}", "a".repeat(36)));
        let aws = format!("{}{}", "AK", format_args!("IA{}", "Z".repeat(16)));
        let key = format!("-----BEGIN RSA {}{}", "PRIVATE KEY", "-----");
        for secret in [github, aws, key] {
            assert_eq!(
                find_leaks(&secret),
                [(1, "possible secret or token")],
                "{secret}"
            );
        }
    }

    #[test]
    fn ignores_short_or_embedded_token_prefixes() {
        let short = format!("{}{}", "gh", "p_abc");
        let embedded = format!("x{}{}", "gh", format_args!("p_{}", "a".repeat(36)));
        assert_eq!(find_leaks(&short), Vec::<(usize, &str)>::new());
        assert_eq!(find_leaks(&embedded), Vec::<(usize, &str)>::new());
    }

    #[test]
    fn detects_image_metadata() {
        assert!(has_image_metadata("photo.JPG", b"\xff\xd8\xff\xe1..Exif.."));
        assert!(!has_image_metadata("photo.png", b"\x89PNG plain"));
        assert!(!has_image_metadata("notes.bin", b"Exif"));
    }

    #[test]
    fn detects_cargo_authors() {
        assert!(declares_authors(
            "crates/a/Cargo.toml",
            "[package]\nauthors = [\"x\"]\n"
        ));
        assert!(!declares_authors(
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\n"
        ));
        assert!(!declares_authors("notes.toml", "authors = []\n"));
    }
}

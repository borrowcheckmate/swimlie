use std::collections::BTreeSet;

use super::Violation;

const ANONYMOUS_DOMAINS: [&str; 1] = ["users.noreply.github.com"];
const ANONYMOUS_ADDRESSES: [&str; 1] = ["noreply@github.com"];

pub(crate) fn is_anonymous_email(email: &str) -> bool {
    let email = email.trim().to_ascii_lowercase();
    ANONYMOUS_ADDRESSES.contains(&email.as_str())
        || email
            .rsplit_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && ANONYMOUS_DOMAINS.contains(&domain))
}

pub(crate) fn email_from_ident(ident: &str) -> Option<String> {
    let (_, rest) = ident.split_once('<')?;
    let (email, _) = rest.split_once('>')?;
    Some(email.to_owned())
}

pub(crate) fn check_identities(emails: &[String]) -> Vec<Violation> {
    emails
        .iter()
        .filter(|email| !is_anonymous_email(email))
        .map(|email| redact(email))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|email| {
            Violation::new(
                "identity",
                format!(
                    "`{email}` is not anonymous; set `git config user.email` to your \
                     ID+username@users.noreply.github.com address"
                ),
            )
        })
        .collect()
}

fn redact(email: &str) -> String {
    match email.rsplit_once('@') {
        Some((local, domain)) => {
            let first: String = local.chars().take(1).collect();
            format!("{first}***@{domain}")
        }
        None => "***".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{check_identities, email_from_ident, is_anonymous_email};

    #[test]
    fn accepts_noreply_addresses() {
        assert!(is_anonymous_email("123+octo@users.noreply.github.com"));
        assert!(is_anonymous_email("Octo@Users.Noreply.GitHub.com"));
        assert!(is_anonymous_email("noreply@github.com"));
    }

    #[test]
    fn rejects_other_addresses() {
        assert!(!is_anonymous_email("jane@corp.test"));
        assert!(!is_anonymous_email(
            &["octo", "noreply.codeberg.org"].join("@")
        ));
        assert!(!is_anonymous_email(
            &["42-octo", "users.noreply.gitlab.com"].join("@")
        ));
        assert!(!is_anonymous_email("@users.noreply.github.com"));
        assert!(!is_anonymous_email(
            "jane@users.noreply.github.com.evil.test"
        ));
    }

    #[test]
    fn extracts_email_from_git_ident() {
        let ident = "Octo Cat <1+octo@users.noreply.github.com> 1700000000 +0100";
        assert_eq!(
            email_from_ident(ident).as_deref(),
            Some("1+octo@users.noreply.github.com")
        );
        assert_eq!(email_from_ident("no email"), None);
    }

    #[test]
    fn reports_each_offender_once_and_redacted() {
        let emails = [
            "jane@corp.test".to_owned(),
            "jane@corp.test".to_owned(),
            "1+octo@users.noreply.github.com".to_owned(),
            "broken".to_owned(),
        ];
        let violations = check_identities(&emails);
        assert_eq!(violations.len(), 2);
        assert!(violations[0].message.starts_with("`***`"));
        assert!(violations[1].message.starts_with("`j***@corp.test`"));
    }
}

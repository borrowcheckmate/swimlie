use std::{env, fs, process::Command};

use crate::{lint::lint, policy, process::run, repo};

const CI_COMMANDS: &[&[&str]] = &[
    &[
        "doc",
        "--workspace",
        "--no-deps",
        "--document-private-items",
        "--locked",
    ],
    &["test", "--workspace", "--locked"],
    &["deny", "--locked", "check", "advisories"],
];

pub(crate) fn ci() -> Result<(), String> {
    lint()?;
    policy()?;
    for args in CI_COMMANDS {
        let mut command = Command::new("cargo");
        command.args(*args);
        if args.first() == Some(&"doc") {
            command.env("RUSTDOCFLAGS", "-D warnings");
        }
        run(&mut command)?;
    }
    Ok(())
}

pub(crate) fn hooks() -> Result<(), String> {
    run(Command::new("git").args(["config", "--local", "core.hooksPath", ".githooks"]))
}

pub(crate) fn pre_commit() -> Result<(), String> {
    identity(None)?;
    policy()?;
    lint()
}

pub(crate) fn policy() -> Result<(), String> {
    let root = repo::root()?;
    report(&policy::check_repository(
        &root,
        &repo::tracked_files(&root)?,
    )?)
}

pub(crate) fn identity(range: Option<&str>) -> Result<(), String> {
    let root = repo::root()?;
    let emails = match range {
        Some(range) => repo::commit_emails(&root, range)?,
        None => repo::current_identity_emails(&root)?,
    };
    report(&policy::check_identities(&emails))
}

pub(crate) fn commits(range: &str) -> Result<(), String> {
    identity(Some(range))?;
    for message in repo::commit_messages(&repo::root()?, range)? {
        report(&policy::check_commit_message(&message))?;
    }
    Ok(())
}

pub(crate) fn commit_msg(path: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    report(&policy::check_commit_message(&text))
}

pub(crate) fn pr_check() -> Result<(), String> {
    let title = env::var("PR_TITLE").map_err(|_| "PR_TITLE is required")?;
    let body = env::var("PR_BODY").map_err(|_| "PR_BODY is required")?;
    report(&policy::check_pull_request(&title, &body))
}

fn report(violations: &[policy::Violation]) -> Result<(), String> {
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

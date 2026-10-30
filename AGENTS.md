# AGENTS.md

Guidance for AI coding agents working in this repository. Humans: see [CONTRIBUTING.md](CONTRIBUTING.md).

## Project

Local-first swim workouts for iPhone and Apple Watch. Rust owns all data and logic (`core/`), Swift only does UI and Apple APIs (`apple/`). How the parts fit: [ARCHITECTURE.md](ARCHITECTURE.md).

## Commands

- `cargo test`: core tests. After changing the model: `UPDATE_SCHEMA=1 cargo test`.
- `cargo xtask lint`: formatting, strict clippy and the file length limit. Run it before finishing.
- `cargo xtask`: everything else (builds, simulator, devices, screenshots). No loose scripts; add a subcommand instead.
- Apple changes: `cargo xtask test [core|unit|ui|all]`; the `ui` run saves screenshots to `build/ui-*.png`, read them to check the UI.

## Rules

- Work is issue driven: link the issue in the pull request (`Closes #n`). New issues follow `.github/ISSUE_TEMPLATE/` (what and why, how to approach it, done when) and stay short.
- Keep the core pure: no platform code, no I/O besides the SQLite path the caller passes.
- Write the smallest change that solves the task. Code should explain itself; comment the why, not the what.
- Fix clippy findings instead of silencing them. If an exception is justified, use `#[expect(..., reason = "...")]`.
- Files stay under 600 lines. Split into submodules, with tests in a sibling `tests` module.
- Commits are [Conventional Commits](https://www.conventionalcommits.org). No `Co-Authored-By` or other trailers, no "generated with" lines in commits, pull requests or files.
- No personal data anywhere: no names, emails, device or team ids, keys, or real locations. Use placeholders and `.env`.
- Do not create extra documents. The repo has README, ARCHITECTURE, CONTRIBUTING, AGENTS and the generated CHANGELOG; put information in the right one.
- Do not bump versions or edit `CHANGELOG.md`; release-plz does that from the commit messages.

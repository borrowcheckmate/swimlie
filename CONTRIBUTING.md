# Contributing

## Setup

- Rust via [rustup](https://rustup.rs); `rust-toolchain.toml` picks the toolchain.
- [pre-commit](https://pre-commit.com): `pre-commit install` once per clone. It runs formatting, strict clippy and the file length check on every commit and checks the commit message.
- For the Apple apps: full Xcode, `brew install xcodegen`. See the README.

## Workflow

1. Branch from `main` and open a pull request. Nothing is pushed to `main` directly.
2. `cargo test` and `cargo xtask lint` pass locally; CI runs the same.
3. After changing the model: `UPDATE_SCHEMA=1 cargo test` regenerates the schema in `schema/`.
4. After changing Swift: `cargo xtask test`, and read the screenshots it saves in `build/`.

## Code

- Rust owns all data and logic; Swift only does UI and Apple APIs. See [ARCHITECTURE.md](ARCHITECTURE.md).
- Code explains itself: clear names, small functions, comments only for the why.
- Clippy is strict (`pedantic`, `nursery` and a hand-picked set of `restriction` lints, all denied). Fix the code rather than silencing a lint. An exception is `#[expect(..., reason = "...")]` with a reason, never a bare `allow`.
- A file over 600 lines becomes a folder of submodules, and its tests move to a sibling `tests` module. `cargo xtask lint` fails on longer files.
- Every command that needs a script is a subcommand of `cargo xtask`; there are no loose scripts.
- No personal data in the repo: no names, emails, device ids, team ids, keys or real locations. Local settings go in `.env` (git-ignored, see `.env.example`).

## Commits and releases

- Commits follow [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`, `ci:`, `build:`. The commit-msg hook and CI enforce it.
- No trailers or attribution lines in commit messages or pull request descriptions.
- Releases are automatic: [release-plz](https://release-plz.dev) opens a release pull request from the commit messages (version bump and `CHANGELOG.md`), and merging it tags the release and creates the GitHub release. Do not bump versions or edit the changelog by hand.
- Set the `RELEASE_PLZ_TOKEN` repository secret to a token that can open pull requests, so CI runs on the release pull request; without it the workflow falls back to `GITHUB_TOKEN`.

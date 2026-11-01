# Contributing

## Setup

- Rust via [rustup](https://rustup.rs); `rust-toolchain.toml` picks the toolchain.
- Set `git config user.email "ID+username@users.noreply.github.com"` to your GitHub noreply address (GitHub → Settings → Emails).
- Install the repository hooks once per clone: `cargo xtask hooks`. If upgrading from pre-commit, run `pre-commit uninstall --hook-type pre-commit --hook-type commit-msg` first. The hooks check identity, repository hygiene, lint and commit messages; no Python hook manager is needed.
- Install the advisory checker: `cargo install --locked cargo-deny`.
- For the Apple apps: full Xcode, `brew install xcodegen`. See the README.

## Workflow

1. Work starts from an issue. Pick an open one (`good first issue` is a good start) or open one with the templates: what and why, how to approach it, done when. Keep it short, but name the files and commands so someone new can pick it up.
2. Branch from `main` and open a pull request that says `Closes #<issue>`. Nothing is pushed to `main` directly.
3. Run `cargo xtask ci` before opening the pull request; CI runs the same portable suite. Never weaken checks to make them pass.
4. After changing the model: `UPDATE_SCHEMA=1 cargo test -p swimlie-core` regenerates the schema in `schema/`.
5. After changing Swift or bindings: `cargo xtask test all`, and read the screenshots it saves in `build/ui-*.png`. Apple checks require full Xcode, xcodegen and installed iPhone/watch simulators; they run separately from the portable suite.

## Tasks

| Command | Purpose |
| --- | --- |
| `cargo xtask ci` | Formatting, strict clippy, 600-line limit, repository policy, rustdoc with warnings denied, all workspace tests and RustSec advisories |
| `cargo xtask lint` | Formatting, strict clippy and 600-line limit |
| `cargo xtask policy` | Tracked-file secrets, personal emails/home paths, key/settings files, image metadata and Cargo authors |
| `cargo xtask commits <range>` | Validate conventional commit messages and identities in a Git range |
| `cargo xtask identity [range]` | Local author/committer identity, or identities in a Git revision range |
| `cargo xtask pre-commit` | Identity, policy and lint; called by the pre-commit hook |
| `cargo xtask commit-msg <file>` | Conventional header, attribution and privacy checks; called by the commit-msg hook |
| `cargo xtask pr-check` | Validate `PR_TITLE` and `PR_BODY`; called by CI |
| `cargo xtask hooks` | Set this clone's `core.hooksPath` to `.githooks` |
| `cargo test --workspace --locked` | Core and automation tests without Apple tooling |
| `cargo xtask test [core\|unit\|ui\|all]` | Core, watch unit and watch UI tests; default `all` |
| `cargo xtask xcframework` / `cargo xtask project` | Build Rust bindings / generate the Xcode project |
| `cargo xtask sim` / `cargo xtask record` | Run or record the watch simulator; support replay, speed and start options |
| `cargo xtask devices` / `cargo xtask install` | Find devices / install on a paired phone and watch |
| `cargo xtask shots` / `cargo xtask mockups` | Capture simulator screenshots / render README images |

`cargo xtask` lists the remaining commands and arguments. Hooks check tracked working-tree files, including staged new files; stage new files before running the final suite. CI also checks PR titles/bodies, commit messages and author/committer emails in commits added by the PR. Dependency advisories need network access to refresh RustSec's database.

## Issues and pull requests

Start issues with the goal, then the ARCHITECTURE.md section and concrete modules to open. Write acceptance criteria that map to automated tests and include the verification command. Put future design and rollout plans in issues. Keep ARCHITECTURE.md accurate in the same change that alters the structure.

PR descriptions contain what, why and how tested, plus `Closes #N`. Report any checks blocked by unavailable Apple tooling; do not claim they passed.

## Dependencies

Prefer the standard library. A new dependency needs an actively maintained upstream, a compatible license, only the features actually used, a locked version in Cargo.lock and a sentence in the PR explaining why existing code or std is insufficient. Run `cargo xtask ci`, including the RustSec advisory check. The current deny configuration checks advisories.

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
- Releases are automatic: [release-plz](https://release-plz.dev) opens a release pull request from the commit messages (version bump and `CHANGELOG.md`), and merging it tags the release, creates the GitHub release and uploads the build to TestFlight (`.github/workflows/testflight.yml`, which can also be started by hand). The upload needs the repository secrets `SWIMLIE_TEAM_ID`, `SWIMLIE_ASC_KEY_ID`, `SWIMLIE_ASC_ISSUER_ID` and `SWIMLIE_ASC_KEY` (the contents of the `.p8` file). Do not bump versions or edit the changelog by hand.
- Dependabot opens weekly grouped updates (`build(deps)` for crates, `ci(deps)` for actions). The `security` workflow runs CodeQL and `cargo deny check advisories`; an advisory with no fix goes in `deny.toml` with a reason.
- Set the `RELEASE_PLZ_TOKEN` repository secret to a token that can open pull requests, so CI runs on the release pull request; without it the workflow falls back to `GITHUB_TOKEN`.

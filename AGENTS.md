# Agents

Instructions for automated contributors. Humans follow the same rules via [CONTRIBUTING.md](CONTRIBUTING.md).

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) first. Use its task map, then open only the modules your task touches.
2. Work from an issue. If a request is not one, turn it into an issue with the goal, relevant architecture section, files and testable acceptance criteria. Split large work into sub-issues.
3. Branch from `main`, keep one issue per pull request and link it with `Closes #N`. Use `gh` for GitHub issues, labels, milestones, pull requests, reviews and CI runs.
4. Run `cargo xtask ci` before declaring work done. Do not weaken checks or policy to get there. This is the portable suite; Apple changes also require the checks below.
5. Every behaviour change ships with automated tests. Keep Rust responsible for data and calculations; Swift handles UI and Apple APIs.
6. Keep Rust files under 600 lines, the existing enforced limit. Extract distinct concerns into submodules and tests into a sibling `tests` module.
7. Code explains itself. Fix lint findings; justified exceptions use `#[expect(..., reason = "...")]`.
8. No new dependencies without the checks in CONTRIBUTING.md → Dependencies.
9. No loose scripts or notes. Add automation to `xtask`; update the existing documents. ARCHITECTURE.md describes current code, never plans; plans belong in issues.
10. No personal data, home paths, device/team ids or tokens in tracked files. Local settings belong in ignored `.env`; Git identity must use a GitHub noreply address.
11. Use Conventional Commits. No attribution, advertising, co-author trailers or generated-with lines in commits, pull requests, code or docs.
12. PR descriptions say what changed, why and how it was tested. Do not bump versions or edit the generated changelog; release-plz does that.

## Task-specific checks

- Model or import format: `UPDATE_SCHEMA=1 cargo test -p swimlie-core`, review the generated `schema/` diff, then `cargo xtask ci`.
- Swift or bindings: `cargo xtask test all` (full Xcode and xcodegen). Review the UI screenshots in `build/ui-*.png`; screenshots supplement automated assertions.
- Watch timing or replay: add a Rust engine regression test; use `cargo xtask sim` or `cargo xtask record` with a replay to inspect the live UI.
- Build, simulator, device, recording and screenshot commands: run `cargo xtask` for the list. Setup and prerequisites live in CONTRIBUTING.md.

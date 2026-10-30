# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/borrowcheckmate/swimlie/compare/v0.1.0...v0.1.1) - 2026-10-30

### Added

- *(ai)* send rejected workouts back to the AI to fix

### Fixed

- *(ai)* fill in missing rest on generated sets
- *(ai)* reject generated workouts with unselected equipment
- *(engine)* start the rep clock when swimming resumes after a stop

### Other

- Merge pull request #11 from borrowcheckmate/dependabot/cargo/rusqlite-0.41.0
- Merge remote-tracking branch 'origin/main' into fix/ai-set-rest
- Merge pull request #17 from borrowcheckmate/fix/ai-equipment
- *(deps)* bump rusqlite from 0.37.0 to 0.41.0

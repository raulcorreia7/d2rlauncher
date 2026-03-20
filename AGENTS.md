# AGENTS.md

## Goal
- Keep this project small, readable, and easy to release.

## Working Rules
- Prefer simple code over clever code.
- Preserve the current structure unless there is a clear payoff.
- Ship tests with behavior changes.
- Verify with:
  - `cargo fmt`
  - `cargo test --locked`
  - `cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cargo build --locked --release`

## Project Map
- `src/app.rs`: FLTK UI, selection flow, countdown, widget state
- `src/ping.rs`: ICMP sampling and running averages
- `src/launcher.rs`: Steam, Battle.net, direct, and custom launch modes
- `src/config.rs`: persisted config next to the executable
- `src/domain.rs`: regions and region hosts

## Release
- Update the changelog before cutting a release.
- Use `scripts/release.sh <version>`.
- Push `main` and the release tag separately.

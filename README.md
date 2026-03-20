# D2R Launcher

Small cross-platform launcher for Diablo II: Resurrected.

![D2R Launcher](screenshot.png)

## Quick Start
1. Build or download the launcher.
2. If you use `direct` mode, place the launcher next to `D2R.exe` or set `d2r_path` in `config.json`.
3. Open the launcher.
4. Click a region to select it.
5. Double click a region or press `Launch` to start the game.
6. Click the star to save your favorite region.

## Local
```sh
cargo run
```

## Verify
```sh
cargo fmt
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked --release
```

## Docs
- [Development](docs/DEVELOPMENT.md)
- [Release](docs/RELEASE.md)
- [Changelog](CHANGELOG.md)

# D2R Launcher

Small cross-platform launcher for Diablo II: Resurrected.

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

# Release

## CI
- `.github/workflows/ci.yml` verifies `main` and pull requests.
- `.github/workflows/release.yml` publishes assets on tag push.

## Prepare a release
1. Make sure the worktree is clean.
2. Review `CHANGELOG.md`.
3. Run:

```sh
sh scripts/release.sh 0.1.3
```

This updates:
- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- release commit
- annotated tag

## Push
```sh
git push origin main
git push origin v0.1.3
```

## Release assets
- Windows x86
- Windows x64
- Linux x64
- macOS x64
- macOS arm64

Each archive contains only the binary.

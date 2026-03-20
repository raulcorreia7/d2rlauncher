# Development

## Requirements
- Rust `1.75+`
- Linux CI packages when building on Linux:
  `libgtk-3-dev`, `libx11-dev`, `libxext-dev`, `libxinerama-dev`, `libxcursor-dev`,
  `libxrender-dev`, `libxfixes-dev`, `libxft-dev`, `libfontconfig1-dev`,
  `libpango1.0-dev`, `libcairo2-dev`

## Local Loop
```sh
cargo run
```

```sh
cargo fmt
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo build --locked --release
```

## Structure
- `src/main.rs`: startup, logger init, top-level error path
- `src/app.rs`: window layout, UI events, countdown, region selection
- `src/ping.rs`: bounded ping sampling and running average updates
- `src/launcher.rs`: launch behavior for each mode
- `src/config.rs`: `config.json` load/save
- `src/domain.rs`: region list, labels, hosts
- `src/constants.rs`: app-wide constants

## Extend The App

### Add or change a region
- Update `src/domain.rs`.
- The UI picks up `Region::ALL`.

### Add or change a launch mode
- Update `src/config.rs` for config shape.
- Update `src/launcher.rs` for behavior.
- Supported modes are `steam` and `direct`.

### Change the launcher UI
- Work in `src/app.rs`.
- Keep the flow simple: choose region, optional countdown cancel, launch.

### Change ping behavior
- Work in `src/ping.rs`.
- Current model:
  30 attempts per region, `500 ms` delay between attempts, running average published every 3 successful samples, final average published when sampling finishes

## Config
- The app stores `config.json` next to the executable.
- Main fields: `launch_mode`, `d2r_path`, `quick_launch`, `default_region`
- `direct` first tries `d2r_path`, then `D2R.exe` next to the launcher binary.

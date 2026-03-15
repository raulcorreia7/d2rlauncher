# D2R Launcher

Cross-platform launcher for Diablo II: Resurrected with automatic region selection and ping monitoring.

## Features

- Multi-region support (Americas, Europe, Asia)
- Real-time ICMP ping monitoring with color-coded latency
- Automatic region selection with configurable default
- Quick-launch with 5-second countdown
- Right-click to set default region
- Multiple launch modes: Steam, Battle.net, Direct, Custom
- Cross-platform: Windows, macOS, Linux

## Quick Start

```bash
# Clone and build
git clone https://github.com/USER/d2rlauncher.git
cd d2rlauncher
cargo build --release
```

Run the launcher:
```bash
./target/release/d2rlauncher
```

## Usage

- **Left-click** a region button to launch immediately
- **Right-click** a region button to set it as the default
- **Any interaction** (click, key press) cancels the auto-launch countdown

### Latency Colors

| Color | Range | Status |
|-------|-------|--------|
| Green | < 100ms | Excellent |
| Yellow | 100-200ms | Good |
| Red | > 200ms | High latency |
| Gray | --ms | Timeout |

## Configuration

Configuration is stored in `config.json` alongside the executable:

```json
{
  "launch_mode": "steam",
  "quick_launch": true,
  "default_region": "Europe"
}
```

### Launch Modes

| Mode | Description |
|------|-------------|
| `steam` | Launch via Steam (default) |
| `battle_net` | Launch via Battle.net client |
| `direct` | Launch D2R.exe directly (requires `d2r_path`) |
| `custom` | Run custom shell command |

### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `launch_mode` | string | `"steam"` | Launch method |
| `quick_launch` | bool | `true` | Enable auto-launch countdown |
| `default_region` | string | `null` | Default region (Americas, Europe, Asia) |
| `d2r_path` | string | `null` | Path to D2R installation (direct mode) |
| `custom_command` | string | `null` | Custom command with `{address}` placeholder |

## Building

### Prerequisites

- Rust 1.75+
- System dependencies for fltk

### Linux

```bash
# Ubuntu/Debian
sudo apt-get install libgtk-3-dev libx11-dev libxext-dev

# Fedora
sudo dnf install gtk3-devel libX11-devel libXext-devel

# Arch
sudo pacman -S gtk3 libx11 libxext
```

### Build

```bash
cargo build --release
```

## Debug Mode

Run from command line to see debug output:

```bash
./d2rlauncher 2>&1 | less
```

Output includes:
- Configuration loading
- DNS resolution
- Ping measurements and averages
- Launch commands and URIs

## License

MIT

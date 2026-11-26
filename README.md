# QuickTunnel

A small, native cross-platform desktop app for creating instant [Cloudflare Quick Tunnels](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/) without touching a terminal.

Type a local address or port, press **Start Tunnel**, and immediately get a public HTTPS `https://<random>.trycloudflare.com` link — with all active tunnels listed below so you can copy URLs, open them in your browser, inspect logs, or stop them at any time.

## Features

- **One-button Quick Tunnels** — Enter a local port or URL and launch an instant HTTPS tunnel.
- **Multiple concurrent tunnels** — Run, monitor, and manage several local services side by side.
- **Live management** — Copy or open public URLs, view live uptime and recent `cloudflared` output logs, stop individual tunnels, or **Stop All** at once.
- **Built-in `cloudflared` setup** — Automatically detects an existing `cloudflared` binary on `PATH` or offers a **1-click downloader** right inside the app.
- **Native & lightweight** — Single small Rust desktop binary (`eframe` / `egui`) with zero Electron/browser overhead.

## Supported Platforms

- **Linux** — Native Wayland and X11 support (`x86_64` prebuilt binaries on releases).
- **Windows 10 / 11** — Native GUI binary with no console window flashes (`x86_64` prebuilt binaries on releases).

Prebuilt binaries (`quicktunnel-linux-x86_64` and `quicktunnel-windows-x86_64.exe`) are attached automatically to tagged GitHub Releases (`v*`).

## `cloudflared` Detection & 1-Click Install

QuickTunnel relies on Cloudflare's official `cloudflared` CLI under the hood:

1. **System `PATH` first** — If `cloudflared` is already installed on your system `PATH`, QuickTunnel detects it automatically and displays its version.
2. **Built-in 1-click download** — If `cloudflared` is not found, QuickTunnel shows an inline setup banner with a single button to download the official Cloudflare GitHub release binary for your OS/architecture and save it to your local app data directory:
   - Linux: `~/.local/share/quicktunnel/bin/cloudflared`
   - Windows: `%LOCALAPPDATA%\quicktunnel\bin\cloudflared.exe`

## Supported Local Address Formats

QuickTunnel normalizes common shorthand inputs automatically:

| Input | Normalized Target |
| --- | --- |
| `3000` | `http://localhost:3000` |
| `:3000` | `http://localhost:3000` |
| `localhost:8080` | `http://localhost:8080` |
| `127.0.0.1:5173` | `http://127.0.0.1:5173` |
| `[::1]:8000` | `http://[::1]:8000` |
| `https://localhost:8443` | `https://localhost:8443` |

## Building & Running from Source

Requires the [Rust toolchain](https://rustup.rs/) (stable, Rust 2024 edition).

### Linux

Install the system libraries needed for OpenGL / Wayland / X11 (Debian/Ubuntu example):

```bash
sudo apt-get install -y \
  pkg-config \
  libxkbcommon-dev \
  libwayland-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libgl1-mesa-dev
```

Run in development mode or build a small optimized release binary:

```bash
cargo run
cargo build --release
# Output: target/release/quicktunnel
```

### Windows

With the Rust MSVC toolchain installed:

```powershell
cargo run
cargo build --release
# Output: target\release\quicktunnel.exe
```

### Running Tests

```bash
cargo test
```

## Cloudflare Quick Tunnel Notes

- **No Cloudflare account required** — Quick Tunnels (`cloudflared tunnel --url ...`) work immediately with no login, DNS setup, or API tokens.
- **Ephemeral URLs** — Each tunnel receives a newly generated `*.trycloudflare.com` hostname that stops working as soon as the tunnel is stopped.
- **Automatic cleanup on exit** — Closing QuickTunnel cleanly terminates every active `cloudflared` child process so no stray background tunnels are left running.

## Security & Release Verification

- **Open-source reproducible builds** — Official release binaries are built directly from this repository via GitHub Actions with `actions/attest-build-provenance` and published alongside `SHA256SUMS.txt`.
- **Windows metadata** — Windows executables embed proper `VERSIONINFO` metadata, an application icon, and a standard `asInvoker` DPI-aware manifest.
- **Verify a release binary**:
  ```bash
  gh attestation verify quicktunnel-windows-x86_64.exe --repo DigitalniVuk/quicktunnel
  ```

## License

This project is licensed under the **GNU General Public License v3.0 or later** (`GPL-3.0-or-later`). See [LICENSE](LICENSE) for the full text.

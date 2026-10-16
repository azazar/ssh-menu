# Shui (Shell UI)

**Your remotes, one terminal menu.**

Find a machine, choose how to connect, and press Enter. Shui brings your SSH hosts, recent connections, remote file browser, and session launcher into one searchable list. Each host can use its own launch profile, and new machines can run a setup recipe before the first connection.

![Shui showing a different launch profile for each remote](screenshot.png)

**Type to fuzzy-search · ↑↓ to select · ←→ to choose a profile · Enter to connect**

[Install](docs/installation.md) · [User guide](docs/usage.md) · [Configuration](docs/configuration.md) · [Setup recipes](docs/setup.md)

## What you can do

| Choose | Shortcut | Open |
| --- | --- | --- |
| **SSH** | Alt+S | Your usual remote shell, using existing SSH settings |
| **Mosh** | Alt+M | A Mosh session for connections that need to survive network changes |
| **MC** | Alt+C | Midnight Commander browsing the remote filesystem |
| **Bash** | Alt+B | An interactive remote Bash login shell |
| **Zellij** | Alt+Z | An existing remote session, or a new session if none exists |
| **Tmux** | Alt+T | Attach to the remote `main` session, creating it if needed |
| **SFTP** | Alt+F | An interactive file-transfer session |
| **Monitor** | Alt+H | Compact health dashboard: CPU, memory, disks, network and supported GPUs |
| **Tunnel / SOCKS** | Ctrl+Alt+L / Ctrl+Alt+D | A localhost port forward or SOCKS proxy, with editable settings |
| **Your own profile** | Configurable | Any command or remote script defined in YAML |

Hosts come from SSH config, Bash history, and Midnight Commander bookmarks. Launch choices are remembered through history, so you can return to the same machine with the same tool. Press **F4** to configure the selected host or **Shift+F4** to add a new remote, choose an SSH keypair (or generate one when none exists), and optionally install dotfiles. Setup can also copy your terminal definition. Launch profiles offer to install their missing tools when you use them.

Written in Rust with [Ratatui](https://ratatui.rs/), Shui uses a compact, borderless [Catppuccin Mocha](https://catppuccin.com/palette/) interface. Linux builds are static executables. SSH, Mosh, and other connection tools run as separate programs, with your terminal restored before handoff.

## Get started

Install **v0.1.0-rc.5** with a verified download. These commands check the archive against the release's SHA-256 checksums before installing it into `~/.local/bin`, without sudo.

**Linux (including WSL) and macOS** — detects x86-64, ARM64, or Linux ARMv7:

```sh
(
  set -eu
  version=v0.1.0-rc.5
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64) target=x86_64-unknown-linux-musl ;;
    Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-musl ;;
    Linux:armv7l) target=armv7-unknown-linux-musleabihf ;;
    Darwin:arm64) target=aarch64-apple-darwin ;;
    Darwin:x86_64) target=x86_64-apple-darwin ;;
    *) echo 'Unsupported platform; see the installation guide.' >&2; exit 1 ;;
  esac
  archive="shui-$target.tar.gz"
  base="https://github.com/azazar/shui/releases/download/$version"
  temporary=$(mktemp -d)
  trap 'rm -rf "$temporary"' EXIT
  trap 'exit 1' HUP INT TERM
  cd "$temporary"
  curl --fail --location --proto '=https' --output "$archive" "$base/$archive"
  curl --fail --location --proto '=https' --output SHA256SUMS "$base/SHA256SUMS"
  awk -v name="$archive" '$2 == name { print; count++ } END { if (count != 1) exit 1 }' SHA256SUMS > checksum
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c checksum
  else
    shasum -a 256 -c checksum
  fi
  tar -xzf "$archive" shui
  mkdir -p "$HOME/.local/bin"
  install -m 755 shui "$HOME/.local/bin/shui"
)
```

**Windows x64 or ARM64** — use the [verified PowerShell installation](docs/installation.md#windows), then run Shui from Git Bash.

For manual installation, [choose your platform archive](docs/installation.md#get-a-binary) and download `SHA256SUMS` from the same release.

Add `~/.local/bin` to your `PATH` (in Bash or Zsh: `export PATH="$HOME/.local/bin:$PATH"`). On Windows, run Shui from [Git Bash](https://git-scm.com/downloads/win); PowerShell is only used for installation. macOS binaries are unsigned. See the [installation guide](docs/installation.md) for checksum verification, platform requirements, and source builds, or browse [GitHub Releases](https://github.com/azazar/shui/releases).

Help is embedded in the executable. Only `shui` (or `shui.exe`) is needed; no documentation files are required for F1 help.

Run `shui`, type to fuzzy-search your hosts and their SSH config settings/comments, and press Enter to connect, or use a profile shortcut to connect immediately. Start with a filter using `shui prod`. The single-row action bar reveals shortcuts for the exact Alt, Shift, Ctrl, or combined modifiers held. **Alt+Enter** connects and returns to the menu afterward. **Ctrl+Enter** adjusts one connection. **F4** configures the selected host. **Shift+F4** adds a new remote. **F9** edits and validates configuration. **F1** opens the built-in documentation, including navigation help and the configuration reference.

## Documentation

The [documentation index](docs/README.md) covers daily use, installation, configuration, setup scripts, and development. The user guide, profiles, and setup recipes are embedded in the executable and available offline through **F1**: Guide and Profiles from the host list, or Setup from the remote setup dialog.

![Built-in documentation viewer](screenshot-help.png)

Edit `$XDG_CONFIG_HOME/shui/config.yml` (default: `~/.config/shui/config.yml`) to customize launch profiles, setup recipes and all app keybindings. Default profiles offer installation of missing dependencies before continuing; profile shortcuts launch the selected host directly. The [commented example](shui-config.example.yml) contains the shipped defaults, editing guidance and configuration examples. Existing configs are preserved; see [upgrading](docs/installation.md#upgrading) when merging new defaults.

Use `shui --config-path` to find your configuration and `shui --check-config` to validate it without launching commands or opening the UI. Pass a filename to validate a draft: `shui --check-config ./config.yml`.

## License

[GPL-3.0-only](LICENSE).

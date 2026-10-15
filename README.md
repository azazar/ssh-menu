# SSH Menu

**Your remotes, one terminal menu.**

Find a machine, choose how to connect, and press Enter. SSH Menu brings your SSH hosts, recent connections, remote file browser, and session launcher into one searchable list. Each host can use its own launch profile, and new machines can run a setup recipe before the first connection.

![SSH Menu showing a different launch profile for each remote](screenshot.png)

**Type to filter · ↑↓ to select · ←→ to choose a profile · Enter to connect**

[Install](docs/installation.md) · [User guide](docs/usage.md) · [Configuration](docs/configuration.md) · [Setup recipes](docs/setup.md)

## What you can do

| Choose | Open |
| --- | --- |
| **SSH** | Your usual remote shell, using existing SSH settings |
| **Mosh** | A Mosh session for connections that need to survive network changes |
| **MC** | Midnight Commander browsing the remote filesystem |
| **Bash** | An interactive remote Bash login shell |
| **Zellij** | An existing remote session, or a new session if none exists |
| **Your own profile** | Any command or remote script defined in YAML |

Hosts come from SSH config, Bash history, and Midnight Commander bookmarks. Launch choices are remembered through history, so you can return to the same machine with the same tool. Press **F2** to add a remote, choose an SSH keypair, and optionally install dotfiles.

Written in Rust with [Ratatui](https://ratatui.rs/), SSH Menu uses a compact, borderless [Catppuccin Mocha](https://catppuccin.com/palette/) interface. Linux builds are static executables. SSH, Mosh, and other connection tools run as separate programs, with your terminal restored before handoff.

## Get started

Download an archive for Linux, macOS, or Windows from [GitHub Releases](https://github.com/azazar/ssh-menu/releases). Follow the [installation guide](docs/installation.md) for platform requirements, checksums, and source builds.

Run `ssh-menu`, type to filter your hosts, and press Enter to connect. **F2** adds a remote with optional setup. **F1** opens the built-in documentation, including navigation help and configuration examples.

## Documentation

The [documentation index](docs/README.md) covers daily use, installation, configuration, setup scripts, and development. These same Markdown files are embedded in the executable and available offline through **F1**.

![Built-in documentation viewer](screenshot-help.png)

Edit `~/.ssh/ssh-menu-config.yml` to customize launch profiles and setup recipes. The [commented example](ssh-menu-config.example.yml) contains the shipped defaults.

## License

[GPL-3.0-only](LICENSE).

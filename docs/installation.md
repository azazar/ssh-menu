# Installation

## Get a binary

Download **[v0.1.0-rc.5](https://github.com/azazar/shui/releases/tag/v0.1.0-rc.5)** from GitHub Releases. Release publication requires all platform builds and checks to pass. Choose the archive for your system and download `SHA256SUMS` to verify it.

F1 help is embedded in the executable and works offline. You only need to install `shui` (or `shui.exe`); the documentation files in the archive are optional.

For development builds, use the **Artifacts** section of a successful [Binaries workflow](https://github.com/azazar/shui/actions/workflows/binaries.yml) run. Downloading Actions artifacts requires signing in to GitHub. Each artifact ZIP contains a platform archive and its `.sha256` checksum file—extract that outer ZIP first.

| System | Archive to choose |
| --- | --- |
| Linux x86-64 | `shui-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `shui-aarch64-unknown-linux-musl.tar.gz` |
| Linux ARMv7, hard-float | `shui-armv7-unknown-linux-musleabihf.tar.gz` |
| macOS Intel | `shui-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `shui-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `shui-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `shui-aarch64-pc-windows-msvc.zip` |

### Linux and macOS

After downloading, choose your archive name and verify its checksum. For an Actions artifact:

```sh
archive=shui-x86_64-unknown-linux-musl.tar.gz  # Change for your platform.
sha256sum -c "$archive.sha256"                  # macOS: shasum -a 256 -c "$archive.sha256"
```

For a release download, extract the matching checksum from `SHA256SUMS` first:

```sh
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
```

Once verified, extract the executable and install it:

```sh
tar -xzf "$archive" shui
mkdir -p "$HOME/.local/bin"
install -m 755 shui "$HOME/.local/bin/shui"
export PATH="$HOME/.local/bin:$PATH"
shui
```

Add the `PATH` line to your shell's startup file to keep it across sessions. Linux builds have no dynamic libc dependency. macOS builds use system libraries and are not signed or notarized.

## Download a published version from the command line

Use an explicit version for release candidates; GitHub's `latest` download URL excludes prereleases. For example:

```sh
version=v0.1.0-rc.5
archive=shui-x86_64-unknown-linux-musl.tar.gz
base="https://github.com/azazar/shui/releases/download/$version"
curl -fLO "$base/$archive"
curl -fLO "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
sha256sum -c "$archive.sha256"  # macOS: shasum -a 256 -c "$archive.sha256"
```

Then follow the extraction and installation steps above.

### Windows

Download the matching ZIP and compare `Get-FileHash -Algorithm SHA256 .\shui-x86_64-pc-windows-msvc.zip` in PowerShell with the supplied checksum. Extract `shui.exe` into a directory on your `PATH`.

Run it from **[Git Bash](https://git-scm.com/downloads/win)**, which supplies the `sh`, `HOME`, and `USER` environment used by command templates and host discovery. PowerShell alone does not supply that environment. For WSL, install a Linux build inside WSL instead.

## Build from source

Install [Rust via rustup](https://rustup.rs/), then:

```sh
git clone https://github.com/azazar/shui.git
cd shui
cargo build --locked --release
./target/release/shui
```

The repository pins its Rust toolchain. To build the RC source specifically, run `git checkout v0.1.0-rc.5` before building. On Linux x86-64, add `--target x86_64-unknown-linux-musl` for a static build; the binary will be in `target/x86_64-unknown-linux-musl/release/`.

## Requirements

Use a UTF-8 terminal with a monospace font. The action bar needs no Powerline or Nerd Font glyphs. Install the tools for the profiles you use: `ssh` for normal connections, `mosh` locally and `mosh-server` remotely for Mosh, or local `mc` for file browsing. SFTP uses local `sftp`; Tmux needs remote `tmux`; Monitor needs remote `btop` (GPU support depends on its build and the remote drivers). The terminal-definition recipe needs local `infocmp` and remote `tic`. Key-auth setup uses `ssh-copy-id` and offers `ssh-keygen` when no keypair exists; the dotfiles sample also uses `scp`.

## Command-line options

Run `shui` to open the menu. Use `shui --dump` to list hosts without opening the UI; `--help` and `--version` are also available.

## Upgrading

Re-run the install command for your platform to replace the executable. Check `shui --version` afterward; this release is **0.1.0-rc.5**. Help remains embedded, so no separate documentation installation is needed.

When upgrading from the previous project name, copy your existing configuration to `$XDG_CONFIG_HOME/shui/config.yml` (default: `~/.config/shui/config.yml`) before the first launch to preserve custom settings. Update shell aliases and scripts to invoke `shui`.

An existing `$XDG_CONFIG_HOME/shui/config.yml` is never overwritten. To adopt new profiles or recipes, merge complete entries from the [commented config](../shui-config.example.yml), including their inputs and script fields. Preserve your custom commands and choose unused shortcuts. Saving through F9 validates and reloads the result; restart after external edits.

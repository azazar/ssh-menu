# Installation

## Get a binary

Download **[v0.1.0-rc.3](https://github.com/azazar/ssh-menu/releases/tag/v0.1.0-rc.3)** from GitHub Releases. All seven platform builds passed. Choose the archive for your system and download `SHA256SUMS` to verify it.

F1 help is embedded in the executable and works offline. You only need to install `ssh-menu` (or `ssh-menu.exe`); the documentation files in the archive are optional.

For development builds, use the **Artifacts** section of a successful [Binaries workflow](https://github.com/azazar/ssh-menu/actions/workflows/binaries.yml) run. Downloading Actions artifacts requires signing in to GitHub. Each artifact ZIP contains a platform archive and its `.sha256` checksum file—extract that outer ZIP first.

| System | Archive to choose |
| --- | --- |
| Linux x86-64 | `ssh-menu-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `ssh-menu-aarch64-unknown-linux-musl.tar.gz` |
| Linux ARMv7, hard-float | `ssh-menu-armv7-unknown-linux-musleabihf.tar.gz` |
| macOS Intel | `ssh-menu-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `ssh-menu-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `ssh-menu-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `ssh-menu-aarch64-pc-windows-msvc.zip` |

### Linux and macOS

After downloading, choose your archive name and verify its checksum. For an Actions artifact:

```sh
archive=ssh-menu-x86_64-unknown-linux-musl.tar.gz  # Change for your platform.
sha256sum -c "$archive.sha256"                  # macOS: shasum -a 256 -c "$archive.sha256"
```

For a release download, extract the matching checksum from `SHA256SUMS` first:

```sh
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
```

Once verified, extract the executable and install it:

```sh
tar -xzf "$archive" ssh-menu
mkdir -p "$HOME/.local/bin"
install -m 755 ssh-menu "$HOME/.local/bin/ssh-menu"
export PATH="$HOME/.local/bin:$PATH"
ssh-menu
```

Add the `PATH` line to your shell's startup file to keep it across sessions. Linux builds have no dynamic libc dependency. macOS builds use system libraries and are not signed or notarized.

## Download a published version from the command line

Use an explicit version for release candidates; GitHub's `latest` download URL excludes prereleases. For example:

```sh
version=v0.1.0-rc.3
archive=ssh-menu-x86_64-unknown-linux-musl.tar.gz
base="https://github.com/azazar/ssh-menu/releases/download/$version"
curl -fLO "$base/$archive"
curl -fLO "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
sha256sum -c "$archive.sha256"  # macOS: shasum -a 256 -c "$archive.sha256"
```

Then follow the extraction and installation steps above.

### Windows

Download the matching ZIP and compare `Get-FileHash -Algorithm SHA256 .\ssh-menu-x86_64-pc-windows-msvc.zip` in PowerShell with the supplied checksum. Extract `ssh-menu.exe` into a directory on your `PATH`.

Run it from **[Git Bash](https://git-scm.com/downloads/win)**, which supplies the `sh`, `HOME`, and `USER` environment used by command templates and host discovery. PowerShell alone does not supply that environment. For WSL, install a Linux build inside WSL instead.

## Build from source

Install [Rust via rustup](https://rustup.rs/), then:

```sh
git clone https://github.com/azazar/ssh-menu.git
cd ssh-menu
cargo build --locked --release
./target/release/ssh-menu
```

The repository pins its Rust toolchain. To build the RC source specifically, run `git checkout v0.1.0-rc.3` before building. On Linux x86-64, add `--target x86_64-unknown-linux-musl` for a static build; the binary will be in `target/x86_64-unknown-linux-musl/release/`.

## Requirements

Use a UTF-8 terminal with a Powerline-compatible font, such as a Nerd Font. Install the tools for the profiles you use: `ssh` for normal connections, `mosh` locally and `mosh-server` remotely for Mosh, or local `mc` for file browsing. Setup recipes use `ssh-copy-id`; the dotfiles sample also uses `scp`.

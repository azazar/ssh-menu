# SSH Menu

**Your remotes, one terminal menu.**

Find a machine, choose how to connect, and press Enter. SSH Menu brings your SSH hosts, recent connections, remote file browser, and session launcher into one searchable list. Each host can use its own launch profile, and new machines can run a setup recipe before the first connection.

![SSH Menu showing a different launch profile for each remote](screenshot.png)

**Type to filter · ↑↓ to select · ←→ to choose a profile · Enter to connect**

[Install](#install) · [Quick start](#quick-start) · [Add a remote](#add-a-remote) · [Configuration](#configuration) · [Development](#development)

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

## Install

### Get a binary

The current release candidate is **`v0.1.0-rc.1`**. Its seven platform builds have passed; download them from the **Artifacts** section of the [RC workflow run](https://github.com/azazar/ssh-menu/actions/runs/38724584773). Downloading Actions artifacts requires signing in to GitHub. Each artifact ZIP contains a platform archive and its `.sha256` checksum file—extract that outer ZIP first.

Published versions are available on [GitHub Releases](https://github.com/azazar/ssh-menu/releases), with a combined `SHA256SUMS` file. The RC workflow prepares a draft prerelease; release assets become publicly downloadable when that draft is published.

| System | Archive to choose |
| --- | --- |
| Linux x86-64 | `ssh-menu-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `ssh-menu-aarch64-unknown-linux-musl.tar.gz` |
| Linux ARMv7, hard-float | `ssh-menu-armv7-unknown-linux-musleabihf.tar.gz` |
| macOS Intel | `ssh-menu-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `ssh-menu-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `ssh-menu-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `ssh-menu-aarch64-pc-windows-msvc.zip` |

#### Linux and macOS

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

<details>
<summary>Download a published version from the command line</summary>

Use an explicit version for release candidates; GitHub's `latest` download URL excludes prereleases. These commands work after the release has been published:

```sh
version=v0.1.0-rc.1
archive=ssh-menu-x86_64-unknown-linux-musl.tar.gz
base="https://github.com/azazar/ssh-menu/releases/download/$version"
curl -fLO "$base/$archive"
curl -fLO "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
sha256sum -c "$archive.sha256"  # macOS: shasum -a 256 -c "$archive.sha256"
```

Then follow the extraction and installation steps above.

</details>

#### Windows

Download the matching ZIP and compare `Get-FileHash -Algorithm SHA256 .\ssh-menu-x86_64-pc-windows-msvc.zip` in PowerShell with the supplied checksum. Extract `ssh-menu.exe` into a directory on your `PATH`.

Run it from **[Git Bash](https://git-scm.com/downloads/win)**, which supplies the `sh`, `HOME`, and `USER` environment used by command templates and host discovery. PowerShell alone does not supply that environment. For WSL, install a Linux build inside WSL instead.

### Build from source

Install [Rust via rustup](https://rustup.rs/), then:

```sh
git clone https://github.com/azazar/ssh-menu.git
cd ssh-menu
cargo build --locked --release
./target/release/ssh-menu
```

The repository pins its Rust toolchain. To build the RC source specifically, run `git checkout v0.1.0-rc.1` before building. On Linux x86-64, add `--target x86_64-unknown-linux-musl` for a static build; the binary will be in `target/x86_64-unknown-linux-musl/release/`.

### Requirements

Use a UTF-8 terminal with a Powerline-compatible font, such as a Nerd Font. Install the tools for the profiles you use: `ssh` for normal connections, `mosh` locally and `mosh-server` remotely for Mosh, or local `mc` for file browsing. Setup recipes use `ssh-copy-id`; the dotfiles sample also uses `scp`.

## Quick start

1. Run `ssh-menu`. Your existing hosts appear automatically.
2. Type part of a hostname and select a matching row with **↑ / ↓**.
3. Use **← / →** to choose how that host opens, then press **Enter**.

Changing a profile affects only the selected host. Moving away or changing the filter preserves the choice while the menu is open.

| Key | Action |
| --- | --- |
| Type | Filter hosts, case-insensitively |
| ↑ / ↓ · PgUp / PgDn | Move through hosts |
| ← / → | Change the selected host's profile |
| Enter | Connect |
| F2 | Add a remote with optional setup |
| F8 | Toggle plain SSH / Mosh for the selected host |
| Backspace / Delete | Remove the last character |
| Ctrl-Backspace / Ctrl-Delete | Remove the last word |
| Esc / Ctrl-Q / Ctrl-C | Quit |

Editing happens at the end of the text. Ctrl-Backspace also accepts the legacy Ctrl-H encoding. Use `ssh-menu --dump` to list hosts without opening the UI; `--help` and `--version` are also available.

<details>
<summary>Where hosts and remembered profiles come from</summary>

SSH Menu reads `~/.ssh/config`, `~/.bash_history`, and `~/.config/mc/hotlist`. Duplicate targets are merged; recently encountered connections appear first. The SSH config reader handles explicit `Host` and `User` sections and skips wildcard aliases. It does not expand `Include` or `Match`; SSH itself resolves connection settings at launch.

On startup, history entries are matched against configured profiles. Host, username, and explicit port distinguish entries. Unknown or ambiguous commands fall back to the default profile. History is parsed as data, never executed.

Launching appends its command to an existing `.bash_history`; adding a remote creates that file if needed. Switching profiles and quitting writes nothing. Generated commands include a target comment so custom commands can be rediscovered. Run `history -a` in another Bash session to make its recent commands visible.

</details>

<details>
<summary>How the Zellij profile works</summary>

The recipe attaches to an existing remote session, or creates one if none exists. It reuses an installed Zellij; otherwise it installs pinned **0.45.1** into remote `~/.local/bin` and verifies the executable's SHA-256 against the published checksum. It uses no sudo and leaves existing installations in place.

Automatic installation supports Linux x86-64 and ARM64 and requires `tar`, `curl` or `wget`, and `sha256sum` or `shasum`. See the [bootstrap script](https://github.com/azazar/ssh-menu/blob/master/scripts/zellij-remote.sh) for the complete recipe.

</details>

## Add a remote

Press **F2**, enter `user@host[:port]` (or `user@[IPv6]:port`), and choose an optional setup recipe. Tab/Shift-Tab or Up/Down moves between fields; Left/Right changes a recipe or dropdown choice.

![New remote dialog with SSH keypair and dotfiles inputs](screenshot-new-remote.png)

The samples included in the config are:

| Recipe | What it does |
| --- | --- |
| **ssh-copy-id** | Lets you pick a local SSH keypair and copies its public key to the remote |
| **ssh-copy-id + dotfiles** | Copies the key, uploads a dotfiles directory, and runs its installer |

The dotfiles sample defaults to local `~/src/dotfiles`, installer `install.sh`, and shell `sh`. Change the dialog values or edit the recipe for your own repository. Both samples are hidden when no matching `~/.ssh/id_*` private/public keypairs exist. The key selector reads filenames, not key contents.

Setup defaults to **None**. **Enter** runs the selected setup, saves the connection to history, and connects using the default launch profile. **Esc** cancels. Authentication prompts run with the terminal restored. Failed setup stops the connection and does not save the entry. Setup is not repeated on later connections, and SSH config is not modified.

## Configuration

Edit **`~/.ssh/ssh-menu-config.yml`** and restart SSH Menu to reload it. The file is created from the [commented example](ssh-menu-config.example.yml) on the first interactive launch if `~/.ssh` already exists. Existing configs are preserved. With no `~/.ssh` directory, built-in defaults are used.

Launch profiles are shell command templates. A minimal config is:

```yaml
default: SSH
profiles:
  - name: SSH
    command: ssh %h
  - name: Mosh
    command: mosh %h
  - name: MC
    command: mc /sh://%t/
```

Each profile needs a unique `name` and a `command`; `default` names one of them. Commands run through local `sh -c`, and additional string fields can hold reusable commands or scripts. **Placeholders are shell-escaped automatically—leave them unquoted.**

The example above lets SSH config manage ports. The shipped defaults also handle explicitly entered ports. See the reference below for that template and remote-script quoting.

<details>
<summary>Launch profile reference: placeholders, ports, and remote scripts</summary>

### Placeholders and ports

| Placeholder | Value |
| --- | --- |
| `%h` | Host or alias, including username when present |
| `%p` | Explicit port, or an empty string |
| `%t` | Full target, including an explicit `:port` when present |
| `%{field}` | Another string field in the same profile or recipe |
| `%{input:id}` | A setup recipe's dialog input |
| `%%` | A literal percent in a command template |

**Leave placeholders unquoted.** SSH Menu shell-escapes their values. Prefixes such as `/sh://%h` work, and input values remain literal even when they contain shell syntax or placeholder text.

The minimal `ssh %h` example lets SSH config manage ports. To also support ports entered in the menu, use the shipped template:

```yaml
  - name: SSH
    command: port=%p; exec ssh ${port:+-p "$port"} -- %h
```

An absent explicit port passes no `-p` option, preserving SSH alias settings. The [full example](ssh-menu-config.example.yml) includes the equivalent Mosh template.

### Remote scripts

Use a reusable `remote` field to preserve quoting through both the local and remote shell:

```yaml
  - name: Work
    command: port=%p; exec ssh ${port:+-p "$port"} -tt -- %h %{remote}
    remote: bash -c %{script}
    script: |
      ~/bin/work-session.sh
      exec bash -l
```

Each named-field substitution adds a shell-quoting layer. Fields without `%{...}` references remain literal, including shell expressions in a script. Missing references, reference cycles, and quoted placeholders are rejected when configuration is loaded.

</details>

<details>
<summary>Setup recipe reference: custom inputs, generated choices, and availability checks</summary>

### Custom fields

Add recipes under top-level `configuration_scripts`. They use the same command templates as launch profiles, plus optional `inputs`:

```yaml
configuration_scripts:
  - name: Install tools
    inputs:
      - id: email
        type: promptString
        description: Git email
        required: true
      - id: mode
        type: pickString
        description: Install mode
        options: [minimal, full]
        default: minimal
    command: port=%p; exec ssh ${port:+-p "$port"} -tt -- %h %{remote}
    remote: sh -c %{script} -- %{input:email} %{input:mode}
    script: |
      exec ~/bin/install-tools.sh --email "$1" --mode "$2"
```

`promptString` creates a text field; `pickString` creates a dropdown. IDs are unique within a recipe and contain letters, digits, `_`, or `-`. `description` supplies the label, otherwise the ID is shown. Text fields default to empty; dropdowns default to their first option. `required: true` rejects blank values.

Pass remote-script inputs as positional arguments, as above. Values survive switching recipes within the dialog but are not saved to YAML or history. Fields display plain text and are intended for installation parameters. Input declarations are supported only in setup recipes.

### Generate field defaults or choices

An input's `command` can produce a YAML or JSON object containing `default`, `options`, and/or `description`. Returned fields override static values; omitted fields remain unchanged. For example, place this under a recipe:

```yaml
inputs:
  - id: email
    type: promptString
    command: |
      email=$(git config --global user.email) || exit 1
      escaped=$(printf '%s' "$email" | sed "s/'/''/g")
      printf "default: '%s'\n" "$escaped"
  - id: mode
    type: pickString
    command: printf '%s' '{"options":["minimal","full"],"default":"full"}'
```

Generators run when the recipe is first selected. Successful results are cached for that dialog. A failure blocks submission and displays an error; switch away and back to retry. Output must be a valid mapping of at most 64 KiB; dropdown choices must be nonempty and contain the default if one is specified. The shipped SSH key selector uses this mechanism.

### Show a recipe only when available

A recipe's `available_command` controls whether it appears in the selector. For example:

```yaml
configuration_scripts:
  - name: Copy SSH key
    available_command: |
      for public in "$HOME"/.ssh/id_*.pub; do
        [ -f "$public" ] && [ -f "${public%.pub}" ] && exit 0
      done
      exit 1
    command: port=%p; exec ssh-copy-id ${port:+-p "$port"} %h
```

Exit **0** shows the recipe; another status or failure to start hides it. Without a check, the recipe is always available. Checks run once each time the dialog opens, and **None** is always an option. Hidden recipes' input generators do not run.

Availability checks and input generators are literal local `sh -c` commands with stdin closed. Use quick, noninteractive commands; menu placeholders are not expanded inside them. Availability output is discarded; generator stdout is parsed as configuration. Neither runs during config loading or ordinary connections.

</details>

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/zellij.py
cargo build --locked --release --target x86_64-unknown-linux-musl
python3 tests/terminal.py target/x86_64-unknown-linux-musl/release/ssh-menu
```

The Python tests use temporary fixtures and fake connection tools; they do not connect to remote hosts. To refresh screenshots, install Pillow and pyte, build the static binary, then run `python3 scripts/screenshot.py` and `python3 scripts/screenshot.py --dialog`. These use fontconfig and DejaVu Sans Mono for Powerline; `SSH_MENU_SCREENSHOT_FONT` can point to another compatible TTF.

The [Binaries workflow](https://github.com/azazar/ssh-menu/blob/master/.github/workflows/binaries.yml) runs on pull requests, branch pushes, version tags, and manual dispatch. It runs the [Linux checks](https://github.com/azazar/ssh-menu/blob/master/.github/workflows/ci.yml), builds the platform matrix, smoke-tests native executables, verifies static Linux linkage, and uploads packaged artifacts. Linux ARM builds are cross-compiled; the workflow does not run their executables.

To prepare a release, push a `v*` tag. After all builds and checks pass, the workflow creates or updates a **draft release** with archives and `SHA256SUMS`. Review its notes and publish it on GitHub. Windows builds statically link the MSVC runtime; macOS archives are not signed or notarized.

## License

[GPL-3.0-only](LICENSE).

# SSH Menu

**Find a remote, choose how to connect, and get to work.**

SSH Menu is a keyboard-driven terminal launcher for your remote machines. It finds hosts in your SSH config and shell history, lets you choose a different launch command for each host, and can run a setup recipe when you add a new remote.

Type to filter. Use **← / →** to choose SSH, Mosh, Midnight Commander, Bash, or Zellij. Press **Enter** to connect.

![SSH Menu with per-host launch profiles](screenshot.png)

- **Your existing hosts, ready to use.** Reads SSH config, Bash history, and Midnight Commander bookmarks.
- **One host, several ways in.** Open a shell, browse remote files, attach to a Zellij session, or run your own command.
- **Setup from the same menu.** Add a remote, pick an SSH keypair, and optionally install dotfiles with editable script inputs.
- **A compact terminal interface.** Built with [Ratatui](https://ratatui.rs/), with a borderless [Catppuccin Mocha](https://catppuccin.com/palette/) theme and Powerline shortcuts.
- **A portable executable.** Written in Rust, with static musl builds for Linux. Connection tools remain separate programs, using your existing credentials and SSH configuration.

[Install](#install) · [Use](#use) · [Configure](#configure) · [Setup recipes](#setup-recipes) · [Development](#development)

## Install

### Download a binary

Choose an archive from [GitHub Releases](https://github.com/azazar/ssh-menu/releases). Each contains the executable, example config, README, license, and screenshots. `SHA256SUMS` contains the archive checksums.

| Platform | Archive |
| --- | --- |
| Linux x86-64 | `ssh-menu-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `ssh-menu-aarch64-unknown-linux-musl.tar.gz` |
| Linux ARMv7, hard-float | `ssh-menu-armv7-unknown-linux-musleabihf.tar.gz` |
| macOS Intel | `ssh-menu-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `ssh-menu-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `ssh-menu-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `ssh-menu-aarch64-pc-windows-msvc.zip` |

These are the build workflow's targets. Linux binaries have no dynamic libc dependency; macOS and Windows builds use their OS libraries. Rust's embedded and WebAssembly targets cannot run this interactive terminal application.

For example, to install a published Linux x86-64 release:

```sh
archive=ssh-menu-x86_64-unknown-linux-musl.tar.gz
base=https://github.com/azazar/ssh-menu/releases/latest/download
curl -fLO "$base/$archive"
curl -fLO "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS > "$archive.sha256"
sha256sum -c "$archive.sha256"
tar -xzf "$archive"
mkdir -p "$HOME/.local/bin"
install -m 755 ssh-menu "$HOME/.local/bin/ssh-menu"
```

On macOS, choose the matching archive and use `shasum -a 256 -c "$archive.sha256"` for verification. Add `~/.local/bin` to your `PATH` if it is not already there, then run `ssh-menu`.

On Windows, download and extract the matching ZIP. Compare its `Get-FileHash -Algorithm SHA256` output with `SHA256SUMS`, then put `ssh-menu.exe` in a directory on your `PATH`. Run it from **[Git Bash](https://git-scm.com/downloads/win)**: command templates require `sh`, and discovery uses `HOME` and `USER`. PowerShell alone does not provide that environment. WSL users should use a Linux build inside WSL.

For builds that have not been published, download an artifact from a successful [Binaries workflow](https://github.com/azazar/ssh-menu/actions/workflows/binaries.yml) run. GitHub wraps the platform archive and its checksum in an artifact ZIP. Tag builds prepare **draft** releases; the release download links work after a maintainer publishes a release.

### Build from source

Install [Rust via rustup](https://rustup.rs/), then:

```sh
git clone https://github.com/azazar/ssh-menu.git
cd ssh-menu
cargo build --locked --release
./target/release/ssh-menu
```

The repository pins its Rust toolchain. On Linux x86-64, build a static executable with:

```sh
cargo build --locked --release --target x86_64-unknown-linux-musl
./target/x86_64-unknown-linux-musl/release/ssh-menu
```

Use a UTF-8 terminal and a Powerline-compatible font, such as a Nerd Font. Install `ssh` locally; install `mosh` or `mc` if you use those profiles. Mosh also needs `mosh-server` on the remote. The setup samples use `ssh-copy-id`, and the dotfiles sample additionally uses `scp`.

## Use

Launch `ssh-menu`, type part of a host name, select a row, and press Enter. Left/Right changes the profile **only for that row**. Moving away or changing the filter preserves your choice while the menu is open.

| Key | Action |
| --- | --- |
| Type | Filter hosts, case-insensitively |
| ↑ / ↓ | Select a host |
| PgUp / PgDn | Move by a page |
| ← / → | Change the selected host's launch profile |
| Enter | Connect |
| F2 | Add a remote, with optional setup |
| F8 | Toggle plain SSH / Mosh for the selected host |
| Backspace / Delete | Remove the last character |
| Ctrl-Backspace / Ctrl-Delete | Remove the last word |
| Esc / Ctrl-Q / Ctrl-C | Quit |

Editing happens at the end of the filter. Ctrl-Backspace also accepts the legacy Ctrl-H encoding. The terminal's cursor, styles, and input mode are restored before handing control to your command.

### Included launch profiles

| Profile | What it does |
| --- | --- |
| SSH | Connects with your normal remote shell |
| Mosh | Opens a Mosh session |
| MC | Opens Midnight Commander at the remote `/sh://` path |
| Bash | Starts an interactive remote Bash login shell |
| Zellij | Attaches to an existing remote session, or creates one |

The Zellij recipe reuses a remote installation if available. Otherwise it installs pinned Zellij **0.45.1** into remote `~/.local/bin`, verifying the executable's SHA-256 against the published checksum. Automatic installation supports Linux x86-64 and ARM64, needs `tar`, `curl` or `wget`, and `sha256sum` or `shasum`, and uses no sudo. Existing installations are left in place. See the [bootstrap script](https://github.com/azazar/ssh-menu/blob/master/scripts/zellij-remote.sh) for the full recipe.

### Hosts and remembered choices

Hosts are discovered from `~/.ssh/config`, `~/.bash_history`, and `~/.config/mc/hotlist`. Duplicate targets are merged; recently encountered connections appear first. The config reader handles explicit `Host` and `User` sections and skips wildcard aliases. It does not expand `Include` or `Match`; SSH itself still resolves connection settings when launched.

On startup, SSH Menu matches history entries against your configured profiles to restore each host's last choice. Host, username, and explicit port distinguish entries. Unknown or ambiguous commands fall back to the configured default. History is parsed as data, never executed.

Launching appends a command to an existing `.bash_history`; adding a remote creates that file if needed. Switching profiles and quitting writes nothing. Generated history lines include a target comment so custom commands can be rediscovered. Only history already written to disk is visible—run `history -a` in another Bash session to expose its recent commands.

Use `ssh-menu --dump` to list discovered targets without opening the UI. `--help` and `--version` are also available.

## Configure

Configuration lives in **`~/.ssh/ssh-menu-config.yml`**. On the first interactive launch, SSH Menu creates it from the [example configuration](ssh-menu-config.example.yml) if `~/.ssh` already exists. It uses owner-only permissions on Unix, preserves existing configs, and uses built-in defaults when that directory is absent. Noninteractive commands do not create it.

Edit the file and restart to reload it. A minimal configuration is:

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

Each profile has a unique `name` and a `command`; `default` names one of them. Commands run through local `sh -c`. Additional string fields can hold reusable commands or scripts. The built-in recipes are ordinary YAML templates, so you can add any application or shell command.

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

## Setup recipes

Press **F2**, enter `user@host[:port]` (or `user@[IPv6]:port`), and choose an optional setup recipe. Tab/Shift-Tab or Up/Down moves between fields; Left/Right changes a recipe or dropdown choice. **Enter adds and connects** with the default launch profile. Esc cancels the dialog.

![New remote dialog with SSH keypair and dotfiles inputs](screenshot-new-remote.png)

The included recipes offer:

- **ssh-copy-id:** choose a local SSH keypair and copy its public key to the remote.
- **ssh-copy-id + dotfiles:** copy the key, upload a local dotfiles directory, and run its installer. Inputs default to local `~/src/dotfiles`, `install.sh`, and `sh`; edit them to suit your repository.

Both recipes are hidden when no matching `~/.ssh/id_*` private/public keypairs exist. The key selector reads filenames, not key contents, and passes the chosen public key to `ssh-copy-id -i`.

Setup defaults to **None**. The terminal is restored before setup so authentication prompts work normally. Failed setup stops the connection and does not save the new entry. Successful setup is followed by saving the launch command to history and connecting; SSH config is not modified. Setup is not replayed on subsequent connections.

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

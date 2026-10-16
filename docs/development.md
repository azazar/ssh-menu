# Development

```sh
python3 -m pip install pyte==0.8.2
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/zellij.py
cargo build --locked --release --target x86_64-unknown-linux-musl
python3 tests/cli.py target/x86_64-unknown-linux-musl/release/shui
python3 tests/terminal.py target/x86_64-unknown-linux-musl/release/shui
```

The terminal suite checks rendered screens after real keyboard, modifier, focus and resize events. It compares dialog anchors across recipes and profiles, scroll positions, visible selections and shortcut groups at several terminal sizes. Use `python3 tests/terminal.py PATH_TO_SHUI --interaction-only` for this focused pass. When auditing UI changes, check the entire screen before and after an interaction; checking that individual labels occur in terminal output misses stale hints, overlapping content and layout movement.

The Zellij test reads its bootstrap directly from the shipped config; there is no separate script copy to synchronize. The fast Python tests above use temporary fixtures and fake connection tools; they do not connect to remote hosts. To refresh screenshots, install Pillow and pyte, build the static binary, then run `python3 scripts/screenshot.py`, `python3 scripts/screenshot.py --dialog`, `python3 scripts/screenshot.py --configure`, `python3 scripts/screenshot.py --connection`, `python3 scripts/screenshot.py --shortcuts`, `python3 scripts/screenshot.py --shift-shortcuts`, and `python3 scripts/screenshot.py --help-viewer`. Use `python3 scripts/screenshot.py --install-prompt` for the dependency prompt (isolated PATH, no installs) and `python3 scripts/screenshot.py --search` to capture hint-text search. To capture the health dashboard, put `btop` on PATH and run `python3 scripts/screenshot.py --monitor`; the real Monitor script runs locally through a fake SSH command. These use fontconfig and DejaVu Sans Mono for Powerline; `SHUI_SCREENSHOT_FONT` can point to another compatible TTF.

The [Docker integration suite](../tests/docker/README.md) runs shipped SSH-executable profiles and every setup recipe against Debian, Ubuntu, Fedora, Rocky Linux, Alpine, Arch and openSUSE. Run `python3 tests/docker/run.py` locally, or select distributions with `--distro`. It exercises real SSH, dependency installation prompts, package managers, multiplexers, monitoring, key setup, dotfiles and terminfo inside disposable containers. Mosh, MC, SFTP and client forwarding are explicitly excluded. Logs are retained under `target/docker-tests/`; the CI matrix gates releases.

The [Binaries workflow](https://github.com/azazar/shui/blob/master/.github/workflows/binaries.yml) runs on pull requests, branch pushes, version tags, and manual dispatch. It runs the [Linux checks](https://github.com/azazar/shui/blob/master/.github/workflows/ci.yml), builds the platform matrix, runs native CLI and unit tests, verifies static Linux linkage, and uploads packaged artifacts. Both Intel and Apple Silicon macOS runners also run Clippy, the Zellij tests, and the full PTY suite using the same test sources as Linux. Linux-only desktop-terminal checks remain Linux-only. macOS testing uses GitHub-hosted hardware; no local Darling setup is required. Linux ARM builds are cross-compiled; the workflow does not run their executables.

Before publishing, update the version in `Cargo.toml` and `Cargo.lock`, update the README and installation guide download URLs, and refresh screenshots if the UI changed. Commit those changes and push a matching `v*` tag to publish. After all builds and checks pass, the workflow uploads the archives and `SHA256SUMS` to a draft, then publishes it. Uploading before publication supports immutable releases. Tags containing a hyphen, such as `v0.1.0-rc.1`, are marked as prereleases. To publish an existing draft, run the **Publish existing release** workflow with its tag; it verifies all seven archives and their checksums first. Windows builds statically link the MSVC runtime; macOS archives are not signed or notarized.

## Documentation and help

The user guide, launch profiles, and setup recipes in `docs/` are also the built-in help source, selected by the active screen: Guide, Profiles and Bindings for the host list, Setup for the remote setup dialog, Connection for one-off settings, and Profiles/Bindings/Setup for the configuration editor. Each context also adds a generated Keys topic describing the currently configured bindings. `src/help.rs` embeds them with `include_str!`; rebuilding updates F1 help. Installation, development, and the documentation index remain repository-only pages. Add new pages to the documentation index, and include them in help only when useful while running the app. Release archives include the Markdown files as well as the compiled help.

Configuration examples belong in comments in `shui-config.example.yml`. Keep Markdown help focused on usage and field semantics so examples have one maintained home. The packager includes all root-level screenshots referenced by the docs.

## Windows tests under Wine

```sh
docker build -t shui-wine-tests -f tests/wine.Dockerfile .
docker run --rm --mount "type=bind,src=$PWD,dst=/src,readonly" shui-wine-tests
```

The Wine job also gates CI. It cross-compiles the Windows GNU binary, runs its Rust tests with a checksum-pinned BusyBox Windows shell, then runs the shared CLI suite against Windows paths, home lookup, SSH includes, configuration validation and redirected streams. It exercises Windows pipe capture and job-object cleanup, including timeouts and surviving descendants. The disposable container owns the Wine prefix and build output; the checkout is read-only. Native Windows CI separately tests the MSVC builds with Git Bash. Wine does not verify Windows Terminal rendering, native keyboard input, or macOS behavior.

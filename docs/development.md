# Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/zellij.py
cargo build --locked --release --target x86_64-unknown-linux-musl
python3 tests/terminal.py target/x86_64-unknown-linux-musl/release/ssh-menu
```

The Python tests use temporary fixtures and fake connection tools; they do not connect to remote hosts. To refresh screenshots, install Pillow and pyte, build the static binary, then run `python3 scripts/screenshot.py`, `python3 scripts/screenshot.py --dialog`, `python3 scripts/screenshot.py --connection`, `python3 scripts/screenshot.py --shortcuts`, and `python3 scripts/screenshot.py --help-viewer`. Use `python3 scripts/screenshot.py --search` to capture hint-text search. To capture the health dashboard, put `btop` on PATH and run `python3 scripts/screenshot.py --monitor`; the real Monitor script runs locally through a fake SSH command. These use fontconfig and DejaVu Sans Mono for Powerline; `SSH_MENU_SCREENSHOT_FONT` can point to another compatible TTF.

The [Binaries workflow](https://github.com/azazar/ssh-menu/blob/master/.github/workflows/binaries.yml) runs on pull requests, branch pushes, version tags, and manual dispatch. It runs the [Linux checks](https://github.com/azazar/ssh-menu/blob/master/.github/workflows/ci.yml), builds the platform matrix, smoke-tests native executables, verifies static Linux linkage, and uploads packaged artifacts. Linux ARM builds are cross-compiled; the workflow does not run their executables.

Before publishing, update the version in `Cargo.toml` and `Cargo.lock`, update the README and installation guide download URLs, and refresh screenshots if the UI changed. Commit those changes and push a matching `v*` tag to publish. After all builds and checks pass, the workflow uploads the archives and `SHA256SUMS` to a draft, then publishes it. Uploading before publication supports immutable releases. Tags containing a hyphen, such as `v0.1.0-rc.1`, are marked as prereleases. To publish an existing draft, run the **Publish existing release** workflow with its tag; it verifies all seven archives and their checksums first. Windows builds statically link the MSVC runtime; macOS archives are not signed or notarized.

## Documentation and help

The user guide, launch profiles, and setup recipes in `docs/` are also the built-in help source, selected by the active screen: Guide, Profiles and Bindings for the host list, Setup for the new-remote dialog, Connection for one-off settings, and Profiles/Bindings/Setup for the configuration editor. Each context also adds a generated Keys topic describing the currently configured bindings. `src/help.rs` embeds them with `include_str!`; rebuilding updates F1 help. Installation, development, and the documentation index remain repository-only pages. Add new pages to the documentation index, and include them in help only when useful while running the app. Release archives include the Markdown files as well as the compiled help.

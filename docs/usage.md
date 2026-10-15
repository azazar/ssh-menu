# Using SSH Menu

Your existing hosts appear automatically.

1. Type part of a hostname and select a matching row with **↑ / ↓**.
2. Use **← / →** to choose how that host opens, then press **Enter**.

Changing a profile affects only the selected host. Moving away or changing the filter preserves the choice while the menu is open.

| Key | Action |
| --- | --- |
| Type | Filter hosts, case-insensitively |
| ↑ / ↓ · PgUp / PgDn | Move through hosts |
| ← / → | Change the selected host's profile |
| Enter | Connect |
| Configured Ctrl/Alt shortcut | Connect using that profile immediately |
| F1 | Open built-in help |
| F2 | Add a remote with optional setup |
| F8 | Toggle plain SSH / Mosh for the selected host |
| Backspace / Delete | Remove the last character |
| Ctrl-Backspace / Ctrl-Delete | Remove the last word |
| Esc / Ctrl-C | Quit |

Editing happens at the end of the text. Ctrl-Backspace also accepts the legacy Ctrl-H encoding.

## Where hosts and remembered profiles come from

SSH Menu reads `~/.ssh/config`, `~/.bash_history`, and `~/.config/mc/hotlist`. Duplicate targets are merged; recently encountered connections appear first. The SSH config reader handles explicit `Host` and `User` sections and skips wildcard aliases. It does not expand `Include` or `Match`; SSH itself resolves connection settings at launch.

On startup, history entries are matched against configured profiles. Host, username, and explicit port distinguish entries. Unknown or ambiguous commands fall back to the default profile. History is parsed as data, never executed.

Launching appends its command to an existing `.bash_history`; adding a remote creates that file if needed. Switching profiles and quitting writes nothing. Generated commands include a target comment so custom commands can be rediscovered. Run `history -a` in another Bash session to make its recent commands visible.

## How the Zellij profile works

The recipe attaches to an existing remote session, or creates one if none exists. It reuses an installed Zellij; otherwise it installs pinned **0.45.1** into remote `~/.local/bin` and verifies the executable's SHA-256 against the published checksum. It uses no sudo and leaves existing installations in place.

Automatic installation supports Linux x86-64 and ARM64 and requires `tar`, `curl` or `wget`, and `sha256sum` or `shasum`. See the [bootstrap script](https://github.com/azazar/ssh-menu/blob/master/scripts/zellij-remote.sh) for the complete recipe.

## Built-in help

Press **F1** from the host list or new-remote dialog to read these docs without leaving SSH Menu. The host list shows Guide and Profiles; the new-remote dialog shows only Setup.

- **Left/Right or Tab/Shift-Tab:** switch topics.
- **Up/Down:** scroll one line.
- **PgUp/PgDn or Space:** scroll a page.
- **Home/End:** jump to the start or end.
- **Esc, F1, or q:** close help and return to where you were.

The help viewer wraps text to the terminal width and shows your position in each topic. Resizing adjusts the layout. It preserves your host filter, selected profile, and unfinished dialog inputs. Links are displayed as text; use the topic keys to navigate. The documentation is embedded in the binary, so help works offline without installed documentation files.

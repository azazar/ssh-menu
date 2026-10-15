# Launch profiles

Edit **`~/.ssh/ssh-menu-config.yml`** and restart SSH Menu to reload it. The file is created from the [commented example](../ssh-menu-config.example.yml) on the first interactive launch if `~/.ssh` already exists. Existing configs are preserved. With no `~/.ssh` directory, built-in defaults are used.

Launch profiles are shell command templates. A minimal config is:

```yaml
default: SSH
profiles:
  - name: SSH
    command: ssh %h
  - name: Mosh
    available_command: command -v mosh
    command: mosh %h
  - name: MC
    command: mc /sh://%t/
```

Each profile needs a unique `name` and a `command`; `default` names one of them. Commands run through local `sh -c`, and additional string fields can hold reusable commands or scripts. **Placeholders are shell-escaped automatically—leave them unquoted.**

The example above lets SSH config manage ports. The shipped defaults also handle explicitly entered ports. See the reference below for that template and remote-script quoting.

## Profile availability

An optional `available_command` checks whether a launch profile can be used. It runs once per profile when the configuration is loaded for an interactive session, before the TUI opens. Exit **0** keeps the profile; a nonzero status or failure to start hides it. Profiles without a check remain available.

Checks are literal local `sh -c` commands with stdin closed and output discarded. Use quick, noninteractive checks such as `command -v mosh`; placeholders are not expanded and no remote connection is made automatically. Checks do not run for `--dump`, `--help`, or `--version`.

If the configured default is unavailable, the first available profile becomes the session default. History restoration and F8 only select available profiles. If none remain, SSH Menu reports an error before opening the TUI. Filtering does not rewrite your config; restart after installing a missing tool.

The shipped config checks for local `ssh`, `mosh`, and `mc`. Bash and Zellij check for `ssh`, since they run remotely; this does not verify remote tools or connectivity. Existing config files are preserved—add `available_command` to their profiles to enable checks.

## Placeholders and ports

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

An absent explicit port passes no `-p` option, preserving SSH alias settings. The [full example](../ssh-menu-config.example.yml) includes the equivalent Mosh template.

## Remote scripts

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

"""Exercise the real TUI and command handoff without making network connections."""
import fcntl
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time

source_binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="ssh-menu-pty-") as directory:
    root = Path(directory)
    # Exercise a single-file installation, with no docs beside it or in the cwd.
    binary = str(root / "ssh-menu")
    shutil.copy2(source_binary, binary)
    (root / ".ssh").mkdir()
    (root / ".ssh/id_ed25519").write_text("fake private key")
    (root / ".ssh/id_ed25519.pub").write_text("fake public key")
    (root / ".ssh/id_orphan.pub").write_text("not a pair")
    (root / ".ssh/config").write_text("Host alpha beta\n User tester\n")
    (root / ".bash_history").write_text("ssh alpha\nssh -p 2222 beta\n")
    (root / "bin").mkdir()
    for name in ("ssh", "mosh", "mc", "custom-connect"):
        stub = root / "bin" / name
        stub.write_text('#!/bin/sh\nprintf "%s\\n" "$0" "$@" > "$SSH_MENU_TEST_LOG"\n')
        stub.write_text(stub.read_text() + 'stty -g > "$SSH_MENU_TEST_LOG.tty"\nprintf "SSH_MENU_HANDOFF\\n"\n')
        stub.chmod(0o755)
    child_env = dict(os.environ, HOME=str(root), USER="tester", TERM="xterm",
                     PATH=str(root / "bin") + os.pathsep + os.environ["PATH"],
                     SSH_MENU_TEST_LOG=str(root / "command"))
    dump = subprocess.check_output([binary, "--dump"], env=child_env, text=True)
    assert dump.splitlines()[:2] == ["beta:2222", "alpha"], dump

    def session(keys, rows=24, cols=100, expected_status=0):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        before = termios.tcgetattr(slave)
        tty_before = subprocess.check_output(["stty", "-g"], stdin=slave).strip()
        (root / "command.tty").unlink(missing_ok=True)
        process = subprocess.Popen([binary], stdin=slave, stdout=slave, stderr=slave, env=child_env, cwd=root)
        output = bytearray()
        def read_for(seconds):
            end = time.monotonic() + seconds
            while time.monotonic() < end:
                if select.select([master], [], [], 0.05)[0]:
                    output.extend(os.read(master, 65536))
        try:
            read_for(0.3)
            assert b"SSH MENU" in output, output
            for key in keys:
                os.write(master, key)
                read_for(0.1)
            process.wait(timeout=3)
            assert process.returncode == expected_status, output
            assert termios.tcgetattr(slave) == before, "terminal settings were not restored"
            handoff = output.find(b"SSH_MENU_HANDOFF")
            cleanup = output[:handoff] if handoff >= 0 else output
            assert cleanup.rfind(b"\x1b[?25h") > cleanup.rfind(b"\x1b[?25l"), "cursor hidden at handoff/exit"
            assert cleanup.rfind(b"\x1b[?1049l") > cleanup.rfind(b"\x1b[?1049h"), "alternate screen active"
            assert b"\x1b[0m" in cleanup[cleanup.rfind(b"\x1b[?1049l"):], "styles not reset"
            if handoff >= 0:
                assert (root / "command.tty").read_bytes().strip() == tty_before, "child inherited modified tty settings"
            return output
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)

    session([b"\x1b[3~", b"ALPHA", "я".encode(), b"\x1b[3~", b"\r"])  # Delete on empty filter and a Unicode character
    assert (root / "command").read_text().splitlines()[1:] == ["--", "alpha"]
    session([b"beta:2222", b"\x1b[19~", b"\r"])  # F8: SSH to Mosh
    command = (root / "command").read_text().splitlines()
    assert command[0].endswith("/mosh"), command
    assert command[1:] == ["--ssh=ssh -p 2222", "--", "beta"], command
    session([b"beta:2222", b"\r"])  # Restore Mosh from the previous launch.
    assert (root / "command").read_text().splitlines()[0].endswith("/mosh")
    session([b"beta:2222", b"\x1b[D", b"\r"])  # Left from remembered Mosh to SSH
    command = (root / "command").read_text().splitlines()
    assert command[0].endswith("/ssh"), command
    assert command[1:] == ["-p", "2222", "--", "beta"], command
    session([b"wrong", b"\x08", b"alphax", b"\x7f", b"\x1b[19~", b"\x1b[19~", b"\r"])  # erase word, backspace, toggle twice back to SSH
    command = (root / "command").read_text().splitlines()
    assert command[0].endswith("/ssh"), command
    assert command[1:] == ["--", "alpha"], command
    for erase in (b"\x08", b"\x1b[127;5u", b"\x1b[3;5~"):
        session([erase, b"alpha extra", erase, b"\x7f", b"\x15", b"\r"])
        assert (root / "command").read_text().splitlines()[1:] == ["--", "alpha"]
    config_file = root / ".ssh/ssh-menu-config.yml"
    original_config = config_file.read_text()
    session([b"beta:2222", b"\x1b[C", b"\x1b[C", b"\r"])
    command = (root / "command").read_text().splitlines()
    assert command[0].endswith("/mc"), command
    assert command[1:] == ["/sh://beta:2222/"], command
    session([b"beta:2222", b"\x1b[C", b"\r"])  # Right from remembered MC to Bash
    assert (root / "command").read_text().splitlines()[1:] == ["-p", "2222", "-tt", "--", "beta", "exec bash -l"]
    assert config_file.read_text() == original_config, "existing config was overwritten"
    session([b"beta:2222", b"\x1b[C", b"\r"])  # Bash -> Zellij
    command = (root / "command").read_text().splitlines()
    assert command[1:6] == ["-p", "2222", "-tt", "--", "beta"], command
    # SSH receives one fully quoted remote command, including the multiline script.
    remote_command = "\n".join(command[6:])
    assert "attach --create" in remote_command
    # Execute the actual encoded remote command with an existing mock Zellij.
    zellij = root / "bin/zellij"
    zellij.write_text('#!/bin/sh\ncase "$1" in\nlist-sessions) printf "existing session\\n" ;;\nattach) printf "%s\\n" "$@" > "$SSH_MENU_TEST_LOG.zellij"; read -r line; [ "$line" = terminal-input ] ;;\nesac\n')
    zellij.chmod(0o755)
    remote = subprocess.run(["sh", "-c", remote_command], env=child_env, input="terminal-input\n", text=True, capture_output=True)
    assert remote.returncode == 0, remote.stderr
    assert (root / "command.zellij").read_text().splitlines() == ["attach", "--create", "existing session"]
    session([b"beta:2222", b"\r"])  # Zellij is restored from the single-line history entry.
    assert "\n".join((root / "command").read_text().splitlines()[6:]) == remote_command
    config_file.write_text("""default: Script
profiles:
  - name: Script
    command: port=%p; exec ssh ${port:+-p "$port"} -tt -- %h %{remote}
    remote: exec bash -lc '~/bin/work.sh; exec bash -l'
  - name: Custom
    command: custom-connect %h %p %{value}
    value: literal ; $HOME
""")
    session([b"beta:2222", b"\r"])
    assert (root / "command").read_text().splitlines()[1:] == ["-p", "2222", "-tt", "--", "beta", "exec bash -lc '~/bin/work.sh; exec bash -l'"]
    session([b"beta:2222", b"\x1b[C", b"\r"])
    command = (root / "command").read_text().splitlines()
    assert command[0].endswith("/custom-connect"), command
    assert command[1:] == ["beta", "2222", "literal ; $HOME"], command
    session([b"beta:2222", b"\r"])  # Custom argv is restored too.
    assert (root / "command").read_text().splitlines()[0].endswith("/custom-connect")
    config_file.write_text("profiles: [")
    invalid = subprocess.run([binary], env=child_env, capture_output=True)
    assert invalid.returncode != 0
    assert b"ssh-menu-config.yml" in invalid.stderr
    assert b"\x1b[?1049h" not in invalid.stdout, "invalid config opened the TUI"
    config_file.write_text(original_config)
    (root / "command").unlink()
    session([b"no-such-host", b"\r", b"\x11"])  # empty result, then Ctrl-Q
    assert not (root / "command").exists()
    session([b"\x11"], rows=10, cols=40)
    # Profile changes belong to target identity, not the current row or filter.
    baseline = "ssh alpha\nssh -p 2222 beta\n"
    (root / ".bash_history").write_text(baseline)
    session([b"\x1b[C", b"\x1b[B", b"\r"])  # Change beta, launch unchanged alpha.
    assert (root / "command").read_text().splitlines()[1:] == ["--", "alpha"]
    assert (root / "command").read_text().splitlines()[0].endswith("/ssh")
    (root / ".bash_history").write_text(baseline)
    session([b"\x1b[C", b"\x1b[B", b"\x1b[A", b"\r"])  # beta remembers its edit.
    assert (root / "command").read_text().splitlines()[0].endswith("/mosh")
    (root / ".bash_history").write_text(baseline)
    session([b"\x1b[C", b"alpha", b"\x08", b"\r"])  # Filtering preserves beta's edit.
    assert (root / "command").read_text().splitlines()[0].endswith("/mosh")
    (root / ".bash_history").write_text(baseline)
    session([b"\x1b[C", b"\x11"])  # Merely cycling does not save settings.
    assert (root / ".bash_history").read_text() == baseline
    assert config_file.read_text() == original_config
    session([b"beta:2222", b"\r"])
    assert (root / "command").read_text().splitlines()[0].endswith("/ssh")
    # Exercise both shipped recipes with fake tools; no network or real dotfiles.
    for name in ("ssh-copy-id", "scp"):
        stub = root / "bin" / name
        stub.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$SSH_MENU_TEST_LOG.' + name + '"\n')
        stub.chmod(0o755)
    session([b"\x1bOQ", b"setup@example.org:2200", b"\x1b[C", b"\r"])
    assert (root / "command.ssh-copy-id").read_text().splitlines() == ["-p", "2200", "-i", str(root / ".ssh/id_ed25519.pub"), "setup@example.org"]
    (root / "src/dotfiles").mkdir(parents=True)
    (root / "src/dotfiles/install.sh").write_text("exit 0\n")
    ssh = root / "bin/ssh"
    original_ssh = ssh.read_text()
    ssh.write_text(original_ssh + 'printf "%s\\n" "$@" >> "$SSH_MENU_TEST_LOG.calls"\n')
    session([b"\x1bOQ", b"dotfiles@example.org:2201", b"\x1b[C", b"\x1b[C", b"\r"])
    assert (root / "command.ssh-copy-id").read_text().splitlines() == ["-p", "2201", "-i", str(root / ".ssh/id_ed25519.pub"), "dotfiles@example.org"]
    assert (root / "command.scp").read_text().splitlines() == ["-P", "2201", "-r", str(root / "src/dotfiles") + "/.", "dotfiles@example.org:src/dotfiles/"]
    assert 'exec "$2" "./$1"' in (root / "command.calls").read_text()
    ssh.write_text(original_ssh)
    # Adding a remote persists through ordinary history, even if no history existed.
    (root / ".bash_history").unlink()
    session([b"\x1bOQ", b"new@example.org:2200", b"\r"])
    assert (root / "command").read_text().splitlines()[1:] == ["-p", "2200", "--", "new@example.org"]
    assert "new@example.org:2200" in subprocess.check_output([binary, "--dump"], env=child_env, text=True)
    before = (root / ".bash_history").read_text()
    session([b"\x1bOQ", b"invalid:0", b"\r", b"\x1b", b"\x11"])
    assert (root / ".bash_history").read_text() == before
    # Configured setup receives escaped target/port and a restored terminal before connect.
    stub = root / "bin/setup-test"
    stub.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$SSH_MENU_TEST_LOG.setup"\nstty -g > "$SSH_MENU_TEST_LOG.setup-tty"\nexit "${SETUP_EXIT:-0}"\n')
    stub.chmod(0o755)
    config_file.write_text("""default: SSH
profiles: [{name: SSH, command: 'ssh -- %h'}]
configuration_scripts:
  - name: Test setup
    command: setup-test %h %p %{value}
    value: literal ; $HOME
""")
    session([b"\x1bOQ", b"added:2222", b"\t", b"\x1b[C", b"\r"])
    assert (root / "command.setup").read_text().splitlines() == ["added", "2222", "literal ; $HOME"]
    assert (root / "command.setup-tty").read_bytes() == (root / "command.tty").read_bytes()
    before = (root / ".bash_history").read_text()
    (root / "command").unlink()
    child_env["SETUP_EXIT"] = "7"
    session([b"\x1bOQ", b"failed", b"\x1b[C", b"\r"], expected_status=1)
    assert not (root / "command").exists()
    assert (root / ".bash_history").read_text() == before
    del child_env["SETUP_EXIT"]
    # Script-specific fields, required validation, choices, cancellation and literal input.
    config_file.write_text("""default: SSH
profiles: [{name: SSH, command: 'ssh -- %h'}]
configuration_scripts:
  - name: Inputs
    command: setup-test %h %{input:value} %{input:mode}
    inputs:
      - id: value
        type: promptString
        description: Install argument
        required: true
      - id: mode
        type: pickString
        options: [minimal, full]
  - name: Other
    command: setup-test %h
""")
    literal = "a ' ; $(touch SHOULD_NOT_EXIST) %{input:mode}"
    output = session([b"\x1bOQ", b"inputs-host", b"\t", b"\x1b[C", b"\r", literal.encode(), b"\x1bOP", b"\x1b[6~", b"\x1bOP", b"\t", b"\x1b[C",
                       b"\x1b[Z", b"\x1b[Z", b"\x1b[C", b"\x1b[D", b"\r"], rows=9, cols=90)
    assert b"is required" in output
    assert (root / "command.setup").read_text().splitlines() == ["inputs-host", literal, "full"]
    assert literal not in (root / ".bash_history").read_text(), "setup values must not enter history"
    before = (root / ".bash_history").read_text()
    session([b"\x1bOQ", b"cancelled", b"\x1b[C", b"\t", b"\t", b"discard me", b"\x1b", b"\x11"])
    assert (root / ".bash_history").read_text() == before
    # Generators run on selection, cache results in this dialog, and pass resolved choices.
    config_file.write_text("""default: SSH
profiles: [{name: SSH, command: 'ssh -- %h'}]
configuration_scripts:
  - name: Generated
    command: setup-test %{input:text} %{input:choice}
    inputs:
      - id: text
        type: promptString
        command: |
          printf 'default: generated text'
      - id: choice
        type: pickString
        command: |
          printf 'run\\n' >> "$HOME/generator-runs"
          printf '%s' '{"options":["first","second"],"default":"second"}'
  - name: Broken
    command: setup-test should-not-run
    inputs:
      - id: choice
        type: pickString
        command: echo 'generator test failure' >&2; exit 5
""")
    session([b"\x11"])
    assert not (root / "generator-runs").exists(), "generators ran during config load"
    session([b"\x1bOQ", b"generated", b"\t", b"\x1b[C", b"\x1b[D", b"\x1b[C", b"\r"])
    assert (root / "command.setup").read_text().splitlines() == ["generated text", "second"]
    assert (root / "generator-runs").read_text().splitlines() == ["run"], "generator was rerun instead of cached"
    (root / "command.setup").unlink()
    output = session([b"\x1bOQ", b"broken", b"\x1b[D", b"\r", b"\t", b"\t", b"\x1b[C", b"\x1b", b"\x11"])
    assert b"generator test failure" in output
    assert not (root / "command.setup").exists(), "failed generator allowed setup to run"
    # Availability filters choices, preserves original script identity, and skips generators.
    config_file.write_text("""default: SSH
profiles: [{name: SSH, command: 'ssh -- %h'}]
configuration_scripts:
  - name: Hidden
    available_command: |
      echo checked >> "$HOME/availability-checks"
      exit 1
    command: setup-test hidden
    inputs:
      - id: unused
        type: promptString
        command: touch "$HOME/hidden-generator"; printf '{}'
  - name: Visible
    available_command: exit 0
    command: setup-test visible
""")
    session([b"\x11"])
    assert not (root / "availability-checks").exists()
    output = session([b"\x1bOQ", b"available", b"\x1b[C", b"\r"])
    assert b"Running Visible for available" in output
    assert (root / "command.setup").read_text().splitlines() == ["visible"]
    assert (root / "availability-checks").read_text().splitlines() == ["checked"]
    assert not (root / "hidden-generator").exists()
    config_file.write_text(original_config)
    (root / ".ssh/id_ed25519").unlink()  # Public files alone do not make a keypair.
    output = session([b"\x1bOQ", b"no-keys", b"\x1b[C", b"\x1b[D", b"\r"])
    assert b"ssh-copy-id" not in output, "unavailable setup offered without keypairs"
    assert (root / "command").read_text().splitlines()[1:] == ["--", "no-keys"]
    (root / ".ssh/id_ed25519").write_text("fake private key")
    output = session([b"\x1bOQ", b"\x1b[C", b"\x1b", b"\x11"])
    assert b"ssh-copy-id" in output, "availability was not refreshed on reopening"
    config_file.write_text(original_config)
    # Help is embedded and returns to the same filter/profile without launching anything.
    before = (root / ".bash_history").read_text()
    output = session([b"\x1bOP", b"\x1b[F", b"\x1b[H", b"\t", b"\x1b[6~", b"\x1b[5~", b"\x1b[Z", b"\x1bOP", b"\x11"], rows=10, cols=40)
    assert b"HELP" in output and b"Profiles" in output
    assert b"Setup" not in output, "host help offered dialog-only topics"
    assert (root / ".bash_history").read_text() == before
    (root / ".bash_history").write_text("ssh alpha\n")
    session([b"alpha", b"\x1b[C", b"\x1bOP", b"\x1b[C", b"\x1bOP", b"\r"])
    assert (root / "command").read_text().splitlines()[0].endswith("/mosh")
    assert (root / "command").read_text().splitlines()[1:] == ["--", "alpha"]
    output = session([b"\x1bOQ", b"help-host:2222", b"\x1bOP", b"\t", b"\x1b[D", b"\x1bOP", b"\r"])
    assert b"Setting up a remote" in output
    assert b"Profiles" not in output and b"TOPIC" not in output, "dialog help offered irrelevant topics"
    assert (root / "command").read_text().splitlines()[1:] == ["-p", "2222", "--", "help-host"]
    # Launch availability is evaluated once at load, before history and navigation.
    config_file.write_text("""default: Missing
profiles:
  - name: Missing
    available_command: echo missing >> "$HOME/profile-checks"; exit 1
    command: custom-connect %h
  - name: SSH
    available_command: echo ssh >> "$HOME/profile-checks"; command -v ssh
    command: ssh -- %h
  - name: Mosh
    available_command: echo mosh >> "$HOME/profile-checks"; command-that-does-not-exist
    command: mosh -- %h
configuration_scripts:
  - name: Setup
    available_command: touch "$HOME/unexpected-setup-check"
    command: true
""")
    for flag in ('--dump', '--help', '--version'):
        subprocess.run([binary, flag], env=child_env, capture_output=True, check=True)
    assert not (root / "profile-checks").exists(), "noninteractive mode ran availability checks"
    (root / ".bash_history").write_text("mosh alpha\n")
    output = session([b"alpha", b"\x1b[C", b"\x1b[D", b"\x1b[19~", b"\x1bOP", b"\x1bOP", b"\r"])
    assert (root / "profile-checks").read_text().splitlines() == ["missing", "ssh", "mosh"]
    assert not (root / "unexpected-setup-check").exists()
    assert b"Missing" not in output and b"command-that-does-not-exist" not in output
    assert (root / "command").read_text().splitlines()[0].endswith('/ssh')
    assert (root / "command").read_text().splitlines()[1:] == ['--', 'alpha']
    # All unavailable is an actionable error, without entering the alternate screen.
    config_file.write_text("default: Missing\nprofiles: [{name: Missing, available_command: 'exit 1', command: 'ssh %h'}]\n")
    result = subprocess.run([binary], env=child_env, capture_output=True)
    assert result.returncode != 0 and b"no launch profiles are available" in result.stderr
    assert b"\x1b[?1049h" not in result.stdout
    config_file.write_text(original_config)
    print("PASS: discovery, editing, per-entry profiles and history restore, SSH/Mosh/MC/script/custom handoff, config validation, new remotes and setup recipes, embedded help, terminal restoration")

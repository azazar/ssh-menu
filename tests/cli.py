"""Check noninteractive CLI behavior without a terminal or real SSH connections."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="shui-cli-") as directory:
    root = Path(directory)
    env = {key: value for key, value in os.environ.items()
           if key not in ("HOME", "USER", "XDG_CONFIG_HOME")}

    def run(*args):
        return subprocess.run([binary, *args], env=env, capture_output=True,
                              text=True, timeout=5)

    for option in ("--help", "--version"):
        result = run(option)
        assert result.returncode == 0, result.stderr
        assert "shui" in result.stdout.lower()
    assert run("--unknown").returncode != 0

    marker = root / "executed"
    config = root / "custom.yml"
    config.write_text(f"""default: SSH
profiles:
  - name: SSH
    available_command: touch '{marker}'
    prepare_command: touch '{marker}'
    command: ssh %h
""")
    result = run("--check-config", str(config))
    assert result.returncode == 0, result.stderr
    assert "configuration is valid" in result.stdout
    assert not marker.exists(), "validation executed trusted shell code"
    valid_source = config.read_text()
    config.write_text("profiles: [")
    assert run("--check-config", str(config)).returncode != 0
    assert run("--check-config", str(root / "missing.yml")).returncode != 0

    env["HOME"] = str(root)
    env["XDG_CONFIG_HOME"] = str(root / "settings")
    assert run("--config-path").stdout.strip() == str(root / "settings/shui/config.yml")
    assert not (root / "settings").exists(), "read-only CLI created config"
    assert run("--check-config").returncode != 0
    assert not (root / "settings").exists(), "validation created a missing config"
    result = run()
    assert result.returncode != 0 and "requires a terminal" in result.stderr
    assert "\x1b" not in result.stdout
    assert not (root / "settings").exists(), "failed TUI startup created config"

    (root / ".ssh/conf.d").mkdir(parents=True)
    (root / ".ssh/config").write_text('Include conf.d/*\nHost="local"\n User="deploy"\n')
    (root / ".ssh/conf.d/servers").write_text("Host included\n User other\n")
    result = run("--dump")
    assert result.returncode == 0, result.stderr
    assert set(result.stdout.splitlines()) == {"local", "included"}, result.stdout
    assert not marker.exists()
    assert not (root / "settings").exists()
    default_config = root / "settings/shui/config.yml"
    default_config.parent.mkdir(parents=True)
    default_config.write_text(valid_source)
    assert run("--check-config").returncode == 0
    assert not marker.exists(), "default config validation executed a command"
    assert default_config.read_text() == valid_source
print("PASS: CLI validation, discovery, environment independence, and terminal preflight")

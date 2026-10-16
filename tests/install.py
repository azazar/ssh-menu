"""Run the real installers against local archives; never download or install globally."""
import hashlib
import io
import os
import re
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zipfile

repo = Path(__file__).resolve().parents[1]
# A release bump must update both installers as well as README download links.
version = re.search(r'^version = "([^"]+)"', (repo / "Cargo.toml").read_text(), re.M)[1]
for script in ("install.sh", "install.ps1"):
    assert f"v{version}" in (repo / "scripts" / script).read_text(), script
windows = os.name == "nt"
payload = b"verified Shui fixture\n"
platforms = (
    [("Windows", "AMD64", "x86_64-pc-windows-msvc"),
     ("Windows", "ARM64", "aarch64-pc-windows-msvc")]
    if windows else
    [("Linux", "x86_64", "x86_64-unknown-linux-musl"),
     ("Linux", "aarch64", "aarch64-unknown-linux-musl"),
     ("Linux", "arm64", "aarch64-unknown-linux-musl"),
     ("Linux", "armv7l", "armv7-unknown-linux-musleabihf"),
     ("Darwin", "x86_64", "x86_64-apple-darwin"),
     ("Darwin", "arm64", "aarch64-apple-darwin")]
)

with tempfile.TemporaryDirectory(prefix="shui installer ") as directory:
    root = Path(directory)
    fixtures = root / "downloads"
    fixtures.mkdir()
    destination = root / "local bin"
    destination.mkdir()
    executable = destination / ("shui.exe" if windows else "shui")
    scratch = root / "scratch"
    scratch.mkdir()
    env = dict(os.environ, SHUI_INSTALL_DIR=str(destination),
               SHUI_FIXTURES=str(fixtures), TMPDIR=str(scratch),
               TMP=str(scratch), TEMP=str(scratch))

    if windows:
        # Only replace transport; use real PowerShell hashing, extraction and copying.
        runner = root / "run.ps1"
        runner.write_text('''$ErrorActionPreference = "Stop"
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile)
    if (!$Uri.StartsWith("https://github.com/azazar/shui/releases/download/")) {
        throw "Unexpected download URL: $Uri"
    }
    Copy-Item (Join-Path $env:SHUI_FIXTURES ($Uri.Split('/')[-1])) $OutFile
}
# Windows can reset PROCESSOR_ARCHITECTURE when starting a native process.
$env:PROCESSOR_ARCHITECTURE = $env:SHUI_TEST_ARCH
Get-Content -Raw $env:SHUI_INSTALLER | Invoke-Expression
''', encoding="utf-8")
        env["SHUI_INSTALLER"] = str(repo / "scripts/install.ps1")
        command = ["pwsh", "-NoProfile", "-NonInteractive", "-File", str(runner)]
        source = None
    else:
        tools = root / "bin"
        tools.mkdir()
        transport = tools / "curl"
        transport.write_text(f"#!{sys.executable}\n" + '''import os, pathlib, shutil, sys
args = sys.argv[1:]
assert args[-1].startswith('https://github.com/azazar/shui/releases/download/')
assert '--proto' in args and args[args.index('--proto') + 1] == '=https'
shutil.copyfile(pathlib.Path(os.environ['SHUI_FIXTURES']) / args[-1].split('/')[-1], args[args.index('--output') + 1])
''')
        transport.chmod(0o755)
        uname = tools / "uname"
        uname.write_text('#!/bin/sh\ncase "$1" in -s) echo "$SHUI_TEST_OS";; -m) echo "$SHUI_TEST_ARCH";; *) exit 1;; esac\n')
        uname.chmod(0o755)
        env["PATH"] = str(tools) + os.pathsep + os.environ["PATH"]
        command = ["sh"]
        source = (repo / "scripts/install.sh").read_text()

    def run(system, architecture, succeeds):
        env.update(SHUI_TEST_OS=system, SHUI_TEST_ARCH=architecture,
                   PROCESSOR_ARCHITECTURE=architecture)
        executable.write_bytes(b"previous installation")
        result = subprocess.run(command, input=source, text=True, env=env,
                                capture_output=True, timeout=30)
        assert (result.returncode == 0) == succeeds, (result.stdout, result.stderr)
        assert executable.read_bytes() == (payload if succeeds else b"previous installation")
        assert not list(scratch.iterdir()), "installer leaked temporary downloads"
        assert set(destination.iterdir()) == {executable}, "installer leaked staged files"
        if succeeds and not windows:
            assert os.access(executable, os.X_OK)

    def archive(target, member=True):
        name = f"shui-{target}.{'zip' if windows else 'tar.gz'}"
        path = fixtures / name
        entry = executable.name if member else "wrong-name"
        if windows:
            with zipfile.ZipFile(path, "w") as bundle:
                bundle.writestr(entry, payload)
        else:
            with tarfile.open(path, "w:gz") as bundle:
                info = tarfile.TarInfo(entry)
                info.size = len(payload)
                bundle.addfile(info, io.BytesIO(payload))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        return path, f"{digest}  {name}\n"

    for system, architecture, target in platforms:
        path, checksum = archive(target)
        sums = fixtures / "SHA256SUMS"
        sums.write_text(checksum)
        run(system, architecture, True)
        for broken in ("0" * 64 + checksum[64:], checksum * 2, ""):
            sums.write_text(broken)
            run(system, architecture, False)
        sums.write_text(checksum)
        path.unlink()
        run(system, architecture, False)
        path, checksum = archive(target, member=False)
        sums.write_text(checksum)
        run(system, architecture, False)
        print(f"PASS: {system}/{architecture}: install, update, bad/missing/duplicate checksum, download and extraction failures")
    run("Unsupported", "unsupported", False)
    print("PASS: unsupported platforms rejected; existing installs and temporary cleanup preserved")

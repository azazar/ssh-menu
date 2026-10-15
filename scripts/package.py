"""Package one built target for GitHub artifacts/releases (Python standard library only)."""
import hashlib
from pathlib import Path
import re
import sys
import tarfile
import zipfile


def package(target):
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", target):
        raise ValueError("expected a Rust target triple")
    root = Path(__file__).resolve().parents[1]
    windows = "windows" in target
    executable = "ssh-menu.exe" if windows else "ssh-menu"
    files = {
        executable: root / "target" / target / "release" / executable,
        "README.md": root / "README.md",
        "LICENSE": root / "LICENSE",
        "ssh-menu-config.example.yml": root / "ssh-menu-config.example.yml",
        "screenshot-help.png": root / "screenshot-help.png",
        "screenshot.png": root / "screenshot.png",
        "screenshot-new-remote.png": root / "screenshot-new-remote.png",
    }
    files.update({path.relative_to(root).as_posix(): path for path in sorted((root / "docs").glob("*.md"))})
    for source in files.values():
        if not source.is_file():
            raise FileNotFoundError(source)
    output = root / "dist"
    output.mkdir(exist_ok=True)
    archive = output / f"ssh-menu-{target}.{'zip' if windows else 'tar.gz'}"
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
            for name, source in files.items():
                bundle.write(source, name)
    else:
        with tarfile.open(archive, "w:gz") as bundle:
            for name, source in files.items():
                bundle.add(source, arcname=name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(
        f"{checksum}  {archive.name}\n", encoding="utf-8"
    )
    print(archive)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: python scripts/package.py TARGET")
    package(sys.argv[1])

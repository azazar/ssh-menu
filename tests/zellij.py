"""Offline tests of the remote bootstrap, with controlled download/hash fixtures."""
import io
import itertools
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import textwrap

repo = Path(__file__).resolve().parents[1]
# Read the literal script field from the shipped recipe (no extra YAML dependency).
config = (repo / 'docs/config.example.yml').read_text()
block = config.split('  - name: Zellij\n', 1)[1].split('    script: |\n', 1)[1]
source = textwrap.dedent(''.join(itertools.takewhile(
    lambda line: line.startswith('      ') or not line.strip(), block.splitlines(keepends=True)
))).rstrip() + '\n'
assert source.startswith('#!/bin/sh\n'), 'Zellij recipe must contain a literal shell script'
expected = re.search(r'expected=([0-9a-f]{64})', source).group(1)

with tempfile.TemporaryDirectory(prefix='shui-zellij-test-') as directory:
    root = Path(directory)
    script = root / 'zellij-remote.sh'
    script.write_text(source)
    bins = root / 'bin'
    bins.mkdir()
    fake_zellij = '''#!/bin/sh
printf '%s\\n' "$*" >> "$TEST_LOG"
case "$1" in
  list-sessions)
    [ -n "${TEST_SESSIONS:-}" ] || exit 1
    printf '%s\\n' "$TEST_SESSIONS" ;;
  attach)
    read -r line
    [ "$line" = terminal-input ] || exit 9 ;;
  *) exit 8 ;;
esac
'''
    archive = root / 'fixture.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        info = tarfile.TarInfo('zellij')
        info.mode = 0o755
        data = fake_zellij.encode()
        info.size = len(data)
        tar.addfile(info, io.BytesIO(data))

    def executable(name, content):
        path = bins / name
        path.write_text(content)
        path.chmod(0o755)
        return path

    executable('uname', '#!/bin/sh\n[ "$1" = -s ] && echo Linux || echo "${TEST_ARCH:-x86_64}"\n')
    executable('curl', '''#!/bin/sh
printf '%s\\n' "$*" >> "$TEST_DOWNLOAD_LOG"
[ "${TEST_DOWNLOAD_FAIL:-0}" = 0 ] || exit 22
while [ "$#" -gt 0 ]; do
  if [ "$1" = --output ]; then shift; cp "$TEST_ARCHIVE" "$1"; exit; fi
  shift
done
exit 1
''')
    # The fixture executable is not an official release: mock only the checksum
    # process for successful orchestration; production hashes remain untouched.
    executable('sha256sum', '#!/bin/sh\nprintf "%s  %s\\n" "$TEST_DIGEST" "$1"\n')

    def run(name, sessions='', consent=None, **changes):
        home = root / name
        home.mkdir(exist_ok=True)
        log = root / 'calls'
        downloads = root / 'downloads'
        log.unlink(missing_ok=True)
        downloads.unlink(missing_ok=True)
        child_env = dict(os.environ, HOME=str(home), PATH=f'{bins}:/usr/bin:/bin',
                         TEST_LOG=str(log), TEST_DOWNLOAD_LOG=str(downloads),
                         TEST_ARCHIVE=str(archive), TEST_DIGEST=expected,
                         TEST_SESSIONS=sessions)
        child_env.update(changes)
        result = subprocess.run(['sh', str(script)], env=child_env, input=('y\n' if consent is None and not (bins / 'zellij').exists() and not (home / '.local/bin/zellij').exists() else (consent or '')) + 'terminal-input\n',
                                text=True, capture_output=True, timeout=10)
        return home, result, log.read_text().splitlines() if log.exists() else [], downloads

    installed = executable('zellij', fake_zellij)
    home, result, calls, downloads = run('existing', 'old session\nnew-session')
    assert result.returncode == 0, result.stderr
    assert calls == ['list-sessions --short --no-formatting', 'attach --create old session'], calls
    assert not downloads.exists() and not (home / '.local').exists()
    installed.unlink()

    for answer in ['n\n', '\n']:
        home, result, calls, downloads = run('declined', consent=answer)
        assert result.returncode != 0 and not calls and not downloads.exists()
        assert not (home / '.local').exists()

    home, result, calls, downloads = run('install')
    assert result.returncode == 0, result.stderr
    assert calls[-1] == 'attach --create', calls
    assert (home / '.local/bin/zellij').is_file()
    assert '--proto =https' in downloads.read_text()
    assert 'v0.45.1/zellij-x86_64-unknown-linux-musl.tar.gz' in downloads.read_text()
    assert not list((home / '.local/bin').glob('.shui-zellij.*'))
    _, result, calls, downloads = run('install', 'restored-session')
    assert result.returncode == 0 and calls[-1] == 'attach --create restored-session'
    assert not downloads.exists(), 'local installation should be reused without PATH changes'

    for name, changes, message in [
        ('download-fail', {'TEST_DOWNLOAD_FAIL': '1'}, 'download failed'),
        ('checksum-fail', {'TEST_DIGEST': '0' * 64}, 'checksum mismatch'),
        ('unsupported', {'TEST_ARCH': 'riscv64'}, 'supports Linux'),
    ]:
        home, result, calls, _ = run(name, **changes)
        assert result.returncode != 0 and message in result.stderr, result.stderr
        assert not calls and not (home / '.local/bin/zellij').exists()
        assert not list((home / '.local/bin').glob('.shui-zellij.*'))

print('PASS: Zellij reuse, attach/create, verified-install flow, input preservation, failures and cleanup')

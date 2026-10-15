#!/usr/bin/env python3
"""Run shipped profile/recipe integration tests in disposable Linux containers."""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import os
from pathlib import Path
import shutil
import socket
from urllib.parse import urlsplit
import subprocess
import tempfile
import time
import uuid

DISTROS = {
    'debian': 'debian:13',
    'ubuntu': 'ubuntu:24.04',
    'fedora': 'fedora:43',
    'rocky': 'rockylinux:9',
    'alpine': 'alpine:3.22',
    'arch': 'archlinux:base',
    'opensuse': 'opensuse/tumbleweed:latest',
}
REPO = Path(__file__).resolve().parents[2]
PROXIES = [name for name in ('http_proxy', 'https_proxy', 'no_proxy', 'HTTP_PROXY', 'HTTPS_PROXY', 'NO_PROXY') if name in os.environ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--distro', action='append', choices=DISTROS, help='repeat to select distros; default: all')
    parser.add_argument('--fixtures', type=Path, help='use fixtures already exported by the Rust test')
    parser.add_argument('--output', type=Path, default=REPO / 'target/docker-tests')
    parser.add_argument('--jobs', type=int, default=2)
    parser.add_argument('--timeout', type=int, default=1200, help='seconds per build and per test container')
    parser.add_argument('--ca-cert', action='append', type=Path, default=[], help='additional trusted PEM CA certificate (repeatable, for HTTPS proxies)')
    parser.add_argument('--keep', action='store_true', help='retain this run’s containers/images for debugging')
    args = parser.parse_args()
    if args.jobs < 1 or args.timeout < 1:
        parser.error('--jobs and --timeout must be positive')
    subprocess.run(['docker', 'info'], check=True, stdout=subprocess.DEVNULL)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    run_id = uuid.uuid4().hex[:10]
    # Some cloud proxies are host-only DNS aliases; make them resolvable in Docker.
    proxy_hosts = {urlsplit(os.environ[name]).hostname for name in PROXIES if name.lower() != 'no_proxy'} - {None}
    extra_hosts = [arg for host in sorted(proxy_hosts) for arg in ('--add-host', f'{host}:{socket.gethostbyname(host)}')]
    with tempfile.TemporaryDirectory(prefix='ssh-menu-docker-') as temporary:
        context = Path(temporary)
        certificates = context / 'ca'
        certificates.mkdir()
        (certificates / '.keep').touch()
        for index, certificate in enumerate(args.ca_cert):
            shutil.copy2(certificate, certificates / f'{index}.crt')
        fixtures = context / 'fixtures'
        if args.fixtures:
            shutil.copytree(args.fixtures.resolve(), fixtures)
        else:
            fixtures.mkdir()
            subprocess.run(['cargo', 'test', '--locked', 'export_docker_fixtures', '--', '--ignored'],
                           cwd=REPO, env=dict(os.environ, SSH_MENU_DOCKER_FIXTURES=str(fixtures)), check=True)
        if not (fixtures / 'coverage.txt').is_file():
            parser.error('missing exported fixtures')
        print((fixtures / 'coverage.txt').read_text(), flush=True)
        for name in ['Dockerfile', 'bootstrap.sh', 'container.py']:
            shutil.copy2(Path(__file__).with_name(name), context / name)

        def run(distro):
            name = f'ssh-menu-test-{distro}-{run_id}'
            image = f'{name}:latest'
            logs = output / run_id / distro
            logs.mkdir(parents=True, exist_ok=True)
            started = time.monotonic()
            print(f'[{distro}] building {DISTROS[distro]} (logs: {logs})', flush=True)
            try:
                with (logs / 'build.log').open('w') as log:
                    subprocess.run(['docker', 'build', *extra_hosts, '--build-arg', f'BASE_IMAGE={DISTROS[distro]}',
                                    '--build-arg', f'DISTRO={distro}', *[arg for name in PROXIES for arg in ('--build-arg', name)], '-t', image, str(context)],
                                   stdout=log, stderr=subprocess.STDOUT, check=True, timeout=args.timeout)
                print(f'[{distro}] testing profiles and recipes', flush=True)
                with (logs / 'container.log').open('w') as log:
                    subprocess.run(['docker', 'run', *extra_hosts, '--name', name, '--hostname', 'ssh-menu-docker', *[arg for name in PROXIES for arg in ('--env', name)], image],
                                   stdout=log, stderr=subprocess.STDOUT, check=True, timeout=args.timeout)
                print(f'[{distro}] PASS ({time.monotonic() - started:.0f}s)', flush=True)
                return True
            except (subprocess.SubprocessError, OSError) as error:
                print(f'[{distro}] FAIL: {error}; see {logs}', flush=True)
                for file in ['build.log', 'container.log']:
                    path = logs / file
                    if path.exists():
                        print(f'[{distro}] {file} (last lines):\n' + '\n'.join(path.read_text(errors='replace').splitlines()[-15:]), flush=True)
                return False
            finally:
                subprocess.run(['docker', 'cp', f'{name}:/results/.', str(logs)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if args.keep:
                    print(f'[{distro}] retained {name} / {image}', flush=True)
                else:
                    subprocess.run(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    subprocess.run(['docker', 'image', 'rm', image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            futures = [pool.submit(run, distro) for distro in dict.fromkeys(args.distro or DISTROS)]
            results = [future.result() for future in as_completed(futures)]
        return 0 if all(results) else 1


if __name__ == '__main__':
    raise SystemExit(main())

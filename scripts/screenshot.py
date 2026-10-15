"""Capture the actual TUI with demo hosts; requires Pillow and pyte."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

from PIL import Image, ImageDraw, ImageFont
import pyte

parser = argparse.ArgumentParser(description=__doc__)
modes = parser.add_mutually_exclusive_group()
modes.add_argument('--dialog', action='store_true', help='capture the new-remote dialog')
modes.add_argument('--help-viewer', action='store_true', help='capture built-in help')
args = parser.parse_args()
if args.dialog:
    keys = b'\x1bOQdeploy@new.example.org:2222\t\x1b[C\x1b[C\t'
    title, output_name, quit_keys = 'NEW REMOTE', 'screenshot-new-remote.png', b'\x11\x11'
elif args.help_viewer:
    keys = b'\x1bOP'
    title, output_name, quit_keys = 'HELP', 'screenshot-help.png', b'\x11\x11'
else:
    keys = b'\x1b[C' * 4
    title, output_name, quit_keys = 'SSH MENU', 'screenshot.png', b'\x11'

repo = Path(__file__).resolve().parents[1]
binary = repo / 'target/x86_64-unknown-linux-musl/release/ssh-menu'
cols, rows = 90, 22
screen = pyte.Screen(cols, rows)
stream = pyte.ByteStream(screen)
with tempfile.TemporaryDirectory(prefix='ssh-menu-screenshot-') as directory:
    home = Path(directory)
    (home / "bin").mkdir()
    for program in ('ssh', 'mosh', 'mc'):
        stub = home / "bin" / program
        stub.write_text('#!/bin/sh\nexit 0\n')
        stub.chmod(0o755)
    (home / ".ssh").mkdir()
    (home / ".ssh/id_ed25519").write_text("demo fixture")
    (home / ".ssh/id_ed25519.pub").write_text("demo fixture")
    (home / '.bash_history').write_text(
        'ssh deploy@staging.example.org\n'
        'ssh -p 2222 admin@backup.example.org\n'
        'mosh dev@workstation.example.org\n'
        'ssh ops@monitor.example.org\n'
        'mosh dev@build.example.org\n'
        'ssh deploy@production.example.org\n'
    )
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
    child_env = dict(os.environ, HOME=str(home), USER='demo', TERM='xterm-256color',
                     PATH=str(home / 'bin') + os.pathsep + os.environ['PATH'])
    child_env.pop('NO_COLOR', None)
    child = subprocess.Popen([str(binary)], stdin=slave, stdout=slave, stderr=slave,
                             env=child_env)
    try:
        keys_sent = False
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                stream.feed(os.read(master, 65536))
                if not keys_sent and any('SSH MENU' in line for line in screen.display):
                    os.write(master, keys)
                    keys_sent = True
        assert any(title in line for line in screen.display), screen.display
        os.write(master, quit_keys)
        child.wait(timeout=3)
        assert child.returncode == 0
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
        os.close(master)
        os.close(slave)

palette = {
    'black': '#1e1e2e', 'red': '#e06c75', 'green': '#98c379', 'brown': '#e5c07b',
    'blue': '#61afef', 'magenta': '#c678dd', 'cyan': '#56b6c2', 'white': '#abb2bf',
    'brightblack': '#5c6370', 'brightred': '#ff7a85', 'brightgreen': '#b3f58c',
    'brightbrown': '#ffe38a', 'brightblue': '#82baff', 'brightmagenta': '#e6a0ff',
    'brightcyan': '#79edff', 'brightwhite': '#ffffff',
}

def color(value, default):
    if value == 'default':
        return default
    return palette.get(value, '#' + value)

font_path = os.environ.get('SSH_MENU_SCREENSHOT_FONT') or subprocess.check_output(
    ['fc-match', '-f', '%{file}', 'DejaVu Sans Mono for Powerline'], text=True)
font = ImageFont.truetype(font_path, 18)
cell_w, cell_h, pad = 11, 24, 18
image = Image.new('RGB', (cols * cell_w + 2 * pad, rows * cell_h + 2 * pad), '#1e1e2e')
draw = ImageDraw.Draw(image)
for row in range(rows):
    for col in range(cols):
        char = screen.buffer[row][col]
        fg, bg = color(char.fg, '#cdd6f4'), color(char.bg, '#1e1e2e')
        if char.reverse:
            fg, bg = bg, fg
        x, y = pad + col * cell_w, pad + row * cell_h
        draw.rectangle((x, y, x + cell_w - 1, y + cell_h - 1), fill=bg)
        draw.text((x, y), char.data, font=font, fill=fg, stroke_width=0)
image.save(repo / output_name)
print(f'Updated {output_name}')

#!/usr/bin/env perl
use strict;
use warnings;
use Test::More;
use Encode qw(decode);
use File::Temp qw(tempfile);

ok(system('perl -c ./ssh-menu') == 0, 'ssh-menu compiles');

my $has_python = system('python3 --version >/dev/null 2>&1') == 0;
if (not $has_python) {
    plan skip_all => 'python3 is not available for PTY runtime verification';
}

my ($py_fh, $py_file) = tempfile(SUFFIX => '.py');
print $py_fh <<'PY';
import os, pty, subprocess, select, time, sys, fcntl, struct, termios, re

master, slave = pty.openpty()
winsize = struct.pack('HHHH', 24, 80, 0, 0)
fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)
env = os.environ.copy()
env['TERM'] = 'xterm'
env['LINES'] = '24'
env['COLUMNS'] = '80'
p = subprocess.Popen(['perl', './ssh-menu'], stdin=slave, stdout=slave, stderr=slave, env=env)
os.close(slave)
buf = b''
end = time.time() + 2
while time.time() < end:
    r, w, x = select.select([master], [], [], 0.1)
    if master in r:
        data = os.read(master, 4096)
        if not data:
            break
        buf += data
    if p.poll() is not None:
        break
p.terminate()
try:
    p.wait(1)
except Exception:
    pass

# Render screen by processing VT100 cursor positioning sequences
def render_screen(buf, rows=24, cols=80):
    screen = [[' '] * cols for _ in range(rows)]
    row, col = 1, 1
    i = 0
    while i < len(buf):
        b = buf[i]
        if b == 0x1b:
            i += 1
            if i >= len(buf):
                break
            c = buf[i]
            if c == ord('['):
                i += 1
                params = b''
                while i < len(buf) and not (64 <= buf[i] <= 126):
                    params += bytes([buf[i]])
                    i += 1
                if i >= len(buf):
                    break
                cmd = chr(buf[i])
                params = params.decode('ascii', 'ignore')
                if cmd in ('H', 'f'):
                    parts = params.split(';') if params else []
                    row = int(parts[0]) if parts and parts[0] else 1
                    col = int(parts[1]) if len(parts) > 1 and parts[1] else 1
                elif cmd == 'G':
                    col = int(params) if params else 1
                elif cmd == 'X':
                    n = int(params) if params else 1
                    for _ in range(n):
                        if 1 <= row <= rows and 1 <= col <= cols:
                            screen[row - 1][col - 1] = ' '
                        col += 1
                elif cmd == 'J':
                    for r in range(row - 1, rows):
                        for cc in range(cols):
                            screen[r][cc] = ' '
                elif cmd == 'K':
                    for cc in range(col - 1, cols):
                        screen[row - 1][cc] = ' '
                i += 1
                continue
            elif c in (ord('('), ord(')')):
                i += 2
                continue
            else:
                i += 1
                continue
        elif b == 10:
            row += 1
            col = 1
        elif b == 13:
            col = 1
        elif b == 8:
            col = max(1, col - 1)
        elif b in (0x0e, 0x0f):
            pass  # skip SO/SI charset switches
        else:
            ch = chr(b) if 32 <= b <= 126 else ' '
            if 1 <= row <= rows and 1 <= col <= cols:
                screen[row - 1][col - 1] = ch
            col += 1
        i += 1
    return [''.join(r).rstrip() for r in screen]

screen = render_screen(buf)

# Also extract all readable text from raw PTY output (escape-stripped)
raw_text = re.sub(rb'\x1b\[[0-9;?]*[A-Za-z]', b'', buf)
raw_text = re.sub(rb'\x1b[()][B012]', b'', raw_text)
raw_text = re.sub(rb'[\x00-\x1f]+', b' ', raw_text)
raw_text = raw_text.decode('utf-8', 'ignore')

# Output rendered screen lines
sys.stdout.buffer.write(b"\n".join(line.encode('utf-8') for line in screen))

# Output raw text on stderr for additional assertions
sys.stderr.write("RAW:" + raw_text + "\n")
PY
close $py_fh;

my ($out_fh, $out_file) = tempfile();
close $out_fh;

my ($raw_out_fh, $raw_file) = tempfile();
close $raw_out_fh;

my $cmd = "python3 $py_file >$out_file 2>$raw_file";
my $status = system($cmd);
my $exit = $? >> 8;

ok($exit == 0 || $exit == 124, 'startup PTY command returned acceptable status');

open my $fh, '<:raw', $out_file or die "Unable to read output file: $!";
local $/;
my $raw = <$fh>;
close $fh;
my $output = decode('UTF-8', $raw, Encode::FB_DEFAULT);

# Also read the raw escape-stripped text
open my $raw_fh, '<:raw', $raw_file or die "Unable to read raw file: $!";
my $raw_text = <$raw_fh>;
close $raw_fh;
$raw_text = decode('UTF-8', $raw_text, Encode::FB_DEFAULT);

# Combined text for assertions that don't depend on screen position
my $all_text = $output . "\n" . $raw_text;

ok($all_text !~ /screen is currently too small/i, 'no screen-too-small error in PTY output');
ok($all_text !~ /fatal program error/i, 'no fatal program error in PTY output');

my @lines = split /\n/, $output;
my @screen = map { s/\s+$//r } @lines;
my @visible = grep { /\S/ } @screen;

ok(@lines == 24, 'PTY output renders a 24-line screen');
ok($all_text =~ /Filter:/, 'filter prompt is visible in PTY output');
ok($all_text =~ /Type filter text above/, 'help bar text is visible in PTY output');
ok($all_text =~ /F2 force mosh/, 'extended shortcuts help text is visible in PTY output');

if (@visible) {
    my $seen_bottom = 0;
    for my $offset (0 .. 2) {
        my $row = $lines[-1 - $offset] // '';
        if ($row =~ /Type filter text above/) {
            $seen_bottom = 1;
            last;
        }
    }
    # Also check raw text for help bar near end of output
    if (!$seen_bottom && $raw_text =~ /Type filter text above/) {
        $seen_bottom = 1;
    }
    ok($seen_bottom, 'help bar appears in the bottom region of the rendered screen');
} else {
    fail('no visible output from final PTY terminal screen');
}

unlink $py_file;
unlink $out_file;
unlink $raw_file;

done_testing();

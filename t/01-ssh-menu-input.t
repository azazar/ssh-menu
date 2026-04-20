#!/usr/bin/env perl
use strict;
use warnings;
use Test::More;

ok(system('perl -c ./ssh-menu') == 0, 'ssh-menu compiles');

open my $fh, '<', 'ssh-menu' or die "Unable to open ssh-menu: $!";
local $/;
my $src = <$fh>;
close $fh;

ok($src =~ /\@printable_keys/, 'printable key list exists');
ok($src =~ /-width\s*=>\s*-1/, 'search field width is set to -1');
ok($src =~ /help_bar/, 'help bar label exists');
ok($src =~ /Ctrl-U clear/, 'help bar documents Ctrl-U clear');
ok($src =~ /F2 force mosh/, 'help bar documents F2 force mosh');
ok($src =~ /F3 force ssh/, 'help bar documents F3 force ssh');
ok($src =~ /Esc\/Ctrl-Q quit/, 'help bar documents quit shortcuts');
ok($src =~ /\&remove_filter_char,.*Curses::KEY_BACKSPACE\(\)/s, 'backspace binding exists');
ok($src =~ /\&select_current,.*Curses::KEY_ENTER\(\)/s, 'enter binding exists');

my @expected_keys = (
    ('a'..'z'),
    ('A'..'Z'),
    ('0'..'9'),
    ' ', '_', '-', '.', '@', ':', '/', '\\', '"', "'", ',', ';', '!', '?',
    '[', ']', '{', '}', '(', ')', '<', '>', '+', '=', '*', '&', '^', '%', '#', '$'
);

for my $key (@expected_keys) {
    my $escaped = quotemeta($key);
    ok($src =~ /\Q$key\E/, "printable key '$key' is present");
}

done_testing();

#!/bin/sh
# Runs on the SSH server with its terminal still attached to stdin.
set -eu
fail() { printf 'ssh-menu: %s\n' "$*" >&2; exit 1; }

if command -v zellij >/dev/null 2>&1; then
    zellij_bin=$(command -v zellij)
elif [ -x "$HOME/.local/bin/zellij" ]; then
    zellij_bin=$HOME/.local/bin/zellij
else
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64)
            platform=x86_64-unknown-linux-musl
            expected=d006c521dcb475a6005d741e9dd7c5758e5a23b28dd60a5c10cebfa4876319dd
            ;;
        Linux:aarch64|Linux:arm64)
            platform=aarch64-unknown-linux-musl
            expected=2a2c0621e6f3b11ecbb05d66939a48000a2508604620aa20727a5fd61c49f451
            ;;
        *) fail 'automatic Zellij installation supports Linux x86-64/ARM64; install zellij manually on this platform' ;;
    esac
    command -v tar >/dev/null 2>&1 || fail 'tar is required to install Zellij'
    if command -v sha256sum >/dev/null 2>&1; then
        checksum=sha256sum
    elif command -v shasum >/dev/null 2>&1; then
        checksum=shasum
    else
        fail 'sha256sum or shasum is required to verify Zellij'
    fi
    umask 077
    mkdir -p "$HOME/.local/bin"
    install_dir=$(mktemp -d "$HOME/.local/bin/.ssh-menu-zellij.XXXXXX")
    trap 'rm -rf "$install_dir"' 0
    trap 'exit 1' HUP INT TERM
    archive=$install_dir/zellij.tar.gz
    url=https://github.com/zellij-org/zellij/releases/download/v0.45.1/zellij-$platform.tar.gz
    printf 'Installing Zellij 0.45.1 into %s/.local/bin ...\n' "$HOME" >&2
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --location \
            --retry 2 --connect-timeout 15 --max-time 180 --silent --show-error \
            --output "$archive" "$url" || fail 'Zellij download failed'
    elif command -v wget >/dev/null 2>&1; then
        wget --https-only --timeout=30 --tries=2 -q -O "$archive" "$url" || fail 'Zellij download failed'
    else
        fail 'curl or wget is required to download Zellij'
    fi
    # Official .sha256sum assets hash the executable, not the tarball.
    tar -xzf "$archive" -C "$install_dir" zellij || fail 'cannot extract Zellij'
    [ -f "$install_dir/zellij" ] && [ ! -L "$install_dir/zellij" ] || fail 'invalid Zellij archive member'
    if [ "$checksum" = sha256sum ]; then
        digest=$(sha256sum "$install_dir/zellij")
    else
        digest=$(shasum -a 256 "$install_dir/zellij")
    fi
    [ "${digest%% *}" = "$expected" ] || fail 'Zellij checksum mismatch; refusing to install or execute it'
    chmod 755 "$install_dir/zellij"
    mv -f "$install_dir/zellij" "$HOME/.local/bin/zellij"
    rm -rf "$install_dir"
    trap - 0 HUP INT TERM
    zellij_bin=$HOME/.local/bin/zellij
fi

# Names are returned without formatting, oldest session first. Keep spaces intact.
# When no sessions exist, list-sessions returns nonzero; attach --create handles it.
if sessions=$("$zellij_bin" list-sessions --short --no-formatting 2>/dev/null); then
    session=$(printf '%s\n' "$sessions" | sed -n '1p')
    if [ -n "$session" ]; then
        exec "$zellij_bin" attach --create "$session"
    fi
fi
exec "$zellij_bin" attach --create

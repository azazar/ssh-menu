#!/bin/sh
# Test prerequisites only. tmux, btop and Zellij must be installed by the profiles.
set -eu
# Optional trusted proxy CAs. Never disable TLS or repository signature checks.
have_proxy_ca=0
for cert in /tests/ca/*.crt; do
  [ -f "$cert" ] || continue
  have_proxy_ca=1
  case "$1" in
    fedora|rocky) anchors=/etc/pki/ca-trust/source/anchors; bundle=/etc/pki/tls/certs/ca-bundle.crt ;;
    opensuse) anchors=/etc/pki/trust/anchors; bundle=/etc/ssl/ca-bundle.pem ;;
    *) anchors=/usr/local/share/ca-certificates; bundle=/etc/ssl/certs/ca-certificates.crt ;;
  esac
  mkdir -p "$anchors" "${bundle%/*}"
  cp "$cert" "$anchors/ssh-menu-${cert##*/}"
  printf "\n" >> "$bundle"
  cat "$cert" >> "$bundle"
  printf "\n" >> "$bundle"
done
case "$1" in
  debian|ubuntu)
    export DEBIAN_FRONTEND=noninteractive
    # Preserve upstream signed-repository defaults on ordinary Docker hosts.
    # HTTPS-only proxies need trusted CAs before ca-certificates can be installed.
    if [ "$have_proxy_ca" = 1 ]; then
      for file in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources; do
        [ ! -f "$file" ] || sed -i 's|http://|https://|g' "$file"
      done
      printf 'Acquire::https::CaInfo "/etc/ssl/certs/ca-certificates.crt";\n' > /etc/apt/apt.conf.d/99ssh-menu-test-ca
    fi
    apt-get update
    apt-get install -y --no-install-recommends python3 openssh-client openssh-server sudo ca-certificates curl tar gzip ncurses-bin ncurses-term procps passwd
    rm -f /etc/apt/apt.conf.d/99ssh-menu-test-ca
    ;;
  fedora)
    dnf install -y --allowerasing python3 openssh-clients openssh-server sudo ca-certificates curl tar gzip ncurses procps-ng shadow-utils passwd
    ;;
  rocky)
    # btop is supplied by EPEL on Enterprise Linux, not the base repositories.
    dnf install -y epel-release
    dnf install -y --allowerasing python3 openssh-clients openssh-server sudo ca-certificates curl tar gzip ncurses procps-ng shadow-utils passwd
    ;;
  alpine)
    apk add --no-cache python3 openssh sudo ca-certificates curl tar gzip ncurses ncurses-terminfo-base procps shadow
    ;;
  arch)
    pacman -Syu --noconfirm --needed python openssh sudo ca-certificates curl tar gzip ncurses procps-ng shadow
    ;;
  opensuse)
    zypper --non-interactive refresh
    zypper --non-interactive install python3 openssh sudo ca-certificates curl tar gzip ncurses-utils ncurses procps shadow
    ;;
  *) echo "Unknown distro: $1" >&2; exit 1 ;;
esac
mkdir -p /run/sshd /results /root/.ssh
chmod 700 /root/.ssh
useradd -m -U -s /bin/sh remote
printf 'remote:ssh-menu-test-password\n' | chpasswd
printf 'Defaults env_keep += "http_proxy https_proxy no_proxy HTTP_PROXY HTTPS_PROXY NO_PROXY"\nremote ALL=(ALL) NOPASSWD: ALL\n' > /etc/sudoers.d/ssh-menu-test
chmod 440 /etc/sudoers.d/ssh-menu-test

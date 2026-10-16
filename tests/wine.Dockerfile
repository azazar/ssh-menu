ARG RUST_VERSION=1.99
FROM rust:${RUST_VERSION}-slim-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
      wine gcc-mingw-w64-x86-64-posix python3 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-pc-windows-gnu
ADD --checksum=sha256:8263abc2aee6b6f3b3c41aa704bee98987bf7b1ea9bce833f75c7bce89ec1602 https://github.com/moby/busybox/releases/download/FRP-5007-g82accfc19/busybox-w64-FRP-5007-g82accfc19.exe /tools/sh.exe
ENV CARGO_TARGET_DIR=/build WINEPREFIX=/wine WINEDEBUG=-all WINEPATH=Z:\\tools
WORKDIR /src
CMD ["python3", "tests/wine.py"]

ARG BASE_IMAGE=debian:13
FROM ${BASE_IMAGE}
ARG DISTRO
COPY ca /tests/ca
COPY bootstrap.sh /tests/bootstrap.sh
RUN sh /tests/bootstrap.sh "$DISTRO"
COPY container.py /tests/container.py
COPY fixtures /tests/fixtures
ENV TERM=xterm-256color
ENV LANG=C.UTF-8
CMD ["python3", "-u", "/tests/container.py"]

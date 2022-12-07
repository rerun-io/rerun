FROM rust:1.65
LABEL maintainer="opensource@rerun.io"
LABEL version="0.1"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# packaging dependencies
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    libatk-bridge2.0 \
    libfontconfig1-dev \
    libfreetype6-dev \
    libglib2.0-dev \
    libgtk-3-dev \
    libspeechd-dev \
    libssl-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    python3-pip && \
    rm -rf /var/lib/apt/lists/*

ADD requirements-build.txt requirements-build.txt

RUN pip install -r requirements-build.txt

# Install some tools we need
RUN cargo install cargo-deny
RUN cargo install cargo-cranky

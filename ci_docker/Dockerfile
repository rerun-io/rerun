FROM ubuntu:20.04
LABEL maintainer="opensource@rerun.io"
# Remember to update the version in publish.sh
# TODO(jleibs) use this version in the publish.sh script and below in the CACHE_KEY
LABEL version="0.15.0"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# Install the ubuntu package dependencies
ENV DEBIAN_FRONTEND=noninteractive
ARG TARGETARCH

RUN set -eux; \
    apt-get update; \
    apt-get install -y ca-certificates lsb-release wget; \
    wget https://apache.jfrog.io/artifactory/arrow/ubuntu/apache-arrow-apt-source-latest-focal.deb; \
    apt-get install -y ./apache-arrow-apt-source-latest-focal.deb; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
    libarrow-dev \
    build-essential \
    cmake \
    curl \
    git \
    git-lfs \
    libatk-bridge2.0 \
    libfontconfig1-dev \
    libfreetype6-dev \
    libglib2.0-dev \
    libgtk-3-dev \
    libssl-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    lsb-release \
    python3-pip \
    software-properties-common \
    sudo; \
    rm -rf /var/lib/apt/lists/*;

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Install Rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Increment this to invalidate cache
ENV CACHE_KEY=rerun_docker_v0.15.0

# See: https://github.com/actions/runner-images/issues/6775#issuecomment-1410270956
RUN git config --system --add safe.directory '*'


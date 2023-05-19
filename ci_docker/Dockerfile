FROM ubuntu:20.04
LABEL maintainer="opensource@rerun.io"
# Remember to update the version in publish.sh
# TODO(jleibs) use this version in the publish.sh script and below in the CACHE_KEY
LABEL version="0.7"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# Install the ubuntu package dependencies
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    git \
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
    sudo \
    wget && \
    rm -rf /var/lib/apt/lists/*

# Need a more recent pip for manylinux packages to work properly
RUN python3 -m pip install -U pip

# We need a more modern patchelf than ships on ubuntu-20.04
RUN curl -L https://github.com/NixOS/patchelf/releases/download/0.17.2/patchelf-0.17.2-x86_64.tar.gz | tar -xz ./bin/patchelf

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.69.0 \
    RUSTUP_VERSION=1.25.2

# Install Rust
# Borrowed from: https://github.com/rust-lang/docker-rust/blob/master/1.67.1/bookworm/Dockerfile
RUN set -eux; \
    rustArch='x86_64-unknown-linux-gnu'; \
    rustupSha256='bb31eaf643926b2ee9f4d8d6fc0e2835e03c0a60f34d324048aa194f0b29a71c'; \
    url="https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${rustArch}/rustup-init"; \
    wget "$url"; \
    echo "${rustupSha256} *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile default --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# Install some cargo tools we know we'll always need
# We can't do this until after we've installed rust / cargo above
RUN cargo install cargo-deny && \
    cargo install cargo-cranky && \
    cargo install cargo-benchcmp

# Install the python build dependencies
ADD rerun_py/requirements-build.txt requirements-build.txt
RUN pip install -r requirements-build.txt

# Install tools from setup_web.sh
ADD scripts/setup_web.sh setup_web.sh
# Installing binaryen is going to cause this script to fail, but that's ok -- we install it manually
RUN ./setup_web.sh || true
# Note: We need a more modern binaryen than ships on ubuntu-20.4, so we pull it from github
RUN curl -L https://github.com/WebAssembly/binaryen/releases/download/version_112/binaryen-version_112-x86_64-linux.tar.gz | tar xzk --strip-components 1

# Increment this to invalidate cache
ENV CACHE_KEY=rerun_docker_v0.7

# See: https://github.com/actions/runner-images/issues/6775#issuecomment-1410270956
RUN git config --system --add safe.directory '*'


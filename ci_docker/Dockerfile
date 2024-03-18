FROM ubuntu:20.04
LABEL maintainer="opensource@rerun.io"
# Remember to update the version in publish.sh
# TODO(jleibs) use this version in the publish.sh script and below in the CACHE_KEY
LABEL version="0.11.0"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# Install the ubuntu package dependencies
ENV DEBIAN_FRONTEND=noninteractive
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

# Need a more recent pip for manylinux packages to work properly
RUN python3 -m pip install -U pip

# Need a more recent clang-format for codegen
RUN add-apt-repository ppa:ubuntu-toolchain-r/test && \
    apt update -y && \
    apt upgrade -y && \
    wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh 15 && \
    apt-get install clang-format-15 && \
    ln -sf clang-format-15 /usr/bin/clang-format && \
    rm -rf /var/lib/apt/lists/*;

# Need a more recent flatbuffers for codegen
RUN git clone --depth 1 --branch v23.5.9 https://github.com/google/flatbuffers.git && \
    cd flatbuffers && \
    cmake -G "Unix Makefiles" -DCMAKE_BUILD_TYPE=Release && \
    make -j4 && \
    make install && \
    cd .. && rm -rf flatbuffers

# We need a more modern patchelf than ships on ubuntu-20.04
RUN curl --fail -L https://github.com/NixOS/patchelf/releases/download/0.17.2/patchelf-0.17.2-x86_64.tar.gz | tar -xz ./bin/patchelf

# TODO(andreas): Update cargo-deny below when updating rust version to 1.70.0 or above!
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.74.0 \
    RUSTUP_VERSION=1.26.0

# Install Rust
# Borrowed from: https://github.com/rust-lang/docker-rust/blob/a8a2a9d/1.71.0/bookworm/Dockerfile
RUN set -eux; \
    rustArch='x86_64-unknown-linux-gnu'; \
    rustupSha256='0b2f6c8f85a3d02fde2efc0ced4657869d73fccfce59defb4e8d29233116e6db'; \
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

# Install stuff from `rust-toolchain`
ADD rust-toolchain rust-toolchain
# `show` is currently the only way to install whatever is in `rust-toolchain`
# This is subject to change, but there is no alternative: https://github.com/rust-lang/rustup/issues/1397
RUN rustup show

# Install some cargo tools we know we'll always need
# We can't do this until after we've installed rust / cargo above
# Latest cargo-deny requires a rust update!
RUN cargo install cargo-deny@0.13.9
RUN cargo install cargo-cranky
RUN cargo install cargo-benchcmp

# Install the python build dependencies
ADD rerun_py/requirements-build.txt requirements-build.txt
RUN pip install -r requirements-build.txt
ADD rerun_py/requirements-lint.txt requirements-lint.txt
RUN pip install -r requirements-lint.txt

# Note: We need a more modern binaryen than ships on ubuntu-20.4, so we pull it from github
RUN curl --fail -L https://github.com/WebAssembly/binaryen/releases/download/version_112/binaryen-version_112-x86_64-linux.tar.gz | tar xzk --strip-components 1

# Increment this to invalidate cache
ENV CACHE_KEY=rerun_docker_v0.10.0

# See: https://github.com/actions/runner-images/issues/6775#issuecomment-1410270956
RUN git config --system --add safe.directory '*'


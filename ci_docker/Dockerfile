FROM rust:1.67
LABEL maintainer="opensource@rerun.io"
LABEL version="0.1"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# Install some cargo tools we know we'll always need
# We do this first in its own layer because the layer is quite large (500MB+)
# And updating the crates.io index is one of the slower steps
RUN cargo install cargo-deny && \
    cargo install cargo-cranky

# Install the ubuntu package dependencies
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
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
    patchelf \
    python3-pip && \
    rm -rf /var/lib/apt/lists/*

# Install the python build dependencies
ADD rerun_py/requirements-build.txt requirements-build.txt
RUN pip install -r requirements-build.txt

# Install some additional versioned cargo tools
RUN cargo install wasm-bindgen-cli@0.2.84

# Increment this to invalidate cache
ENV CACHE_KEY=rerun_docker_v0.1

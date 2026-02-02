FROM quay.io/pypa/manylinux_2_28
LABEL maintainer="opensource@rerun.io"
# Remember to update the version in publish.sh
# TODO(jleibs) use this version in the publish.sh script and below in the CACHE_KEY
LABEL version="0.17.0-x86-64"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

RUN set -eux; \
  dnf update -y; \
  dnf install -y ca-certificates wget; \
  dnf install -y https://packages.apache.org/artifactory/arrow/almalinux/$(cut -d: -f5 /etc/system-release-cpe | cut -d. -f1)/apache-arrow-release-latest.rpm; \
  dnf config-manager --set-enabled epel; \
  dnf config-manager --set-enabled powertools; \
  dnf update -y; \
  dnf install -y \
    arrow-devel \
    make automake gcc gcc-c++ kernel-devel \
    cmake \
    curl \
    git \
    git-lfs \
    atk at-spi2-atk \
    fontconfig-devel \
    freetype-devel \
    glib2-devel \
    gtk3-devel \
    openssl-devel \
    xcb-util-renderutil-devel \
    xcb-util-devel \
    xcb-util-wm-devel \
    libxkbcommon-devel \
    python3-pip \
    tar \
    jq \
    sudo; \
  dnf clean all;

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Install Rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Increment this to invalidate cache
ENV CACHE_KEY=rerun_docker_v0.17.0

# See: https://github.com/actions/runner-images/issues/6775#issuecomment-1410270956
RUN git config --system --add safe.directory '*'



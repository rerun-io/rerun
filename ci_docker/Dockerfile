# FROM ubuntu:20.04
FROM rust:1.65
LABEL maintainer="opensource@rerun.io"
LABEL version="0.1"
LABEL description="Docker image used for the CI of https://github.com/rerun-io/rerun"

# Install dependnecies needed by eframe:
RUN apt-get update
RUN apt-get install -y \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libssl-dev

# RUN apt-get install -y libgtk-3-dev libspeechd-dev  # This pauses and asks questions, so… fuck it?

# Install some tools we need
RUN cargo install cargo-deny
RUN cargo install cargo-cranky

# RUN bash

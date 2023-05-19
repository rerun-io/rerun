#!/usr/bin/env bash
set -eux

VERSION=0.7 # Bump on each new version. Remember to update the version in the Dockerfile too.

# The build needs to run from top of repo to access the requirements.txt
cd `git rev-parse --show-toplevel`

# Pull :latest so we have the correct cache
docker pull rerunio/ci_docker

# Build the image
docker build -t ci_docker -f ci_docker/Dockerfile .
# This is necessary to build on mac, but is doing something weird with the Cache
# TODO(jleibs): Make this all work portably with caching
# docker buildx build --platform=linux/amd64 -t ci_docker -f ci_docker/Dockerfile .

# Tag latest and version
docker tag ci_docker rerunio/ci_docker
docker tag ci_docker rerunio/ci_docker:$VERSION

# Push the images back up
docker push rerunio/ci_docker
docker push rerunio/ci_docker:$VERSION

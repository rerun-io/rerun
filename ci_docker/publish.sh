#!/usr/bin/env bash
set -eux

VERSION=0.15.1-manylinux # Bump on each new version. Remember to update the version in the Dockerfile too.

# The build needs to run from top of repo to access the requirements.txt
cd `git rev-parse --show-toplevel`

# Build the image
# buildx wants to do all of this in one step
docker buildx build --pull --platform linux/arm64,linux/amd64 -t rerunio/ci_docker -t rerunio/ci_docker:$VERSION --push -f ci_docker/Dockerfile-manylinux .

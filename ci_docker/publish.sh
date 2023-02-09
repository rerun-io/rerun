VERSION=0.4 # Bump on each new version. Maybe it should also match the version in ci_docker/Dockerfile ?

# The build needs to run from top of repo to access the requirments.txt
cd `git rev-parse --show-toplevel`

# Pull :latest so we have the correct cache
docker pull rerunio/ci_docker

# Build the image
# docker build -t ci_docker -f ci_docker/Dockerfile .
docker buildx build --platform=linux/amd64 -t ci_docker -f ci_docker/Dockerfile .

# Tag latest and version
docker tag ci_docker rerunio/ci_docker
docker tag ci_docker rerunio/ci_docker:$VERSION

# Push the images back up
docker push rerunio/ci_docker
docker push rerunio/ci_docker:$VERSION

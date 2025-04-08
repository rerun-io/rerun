# Start the rerun app in Docker

It is possible to run various versions of `rerun` in a Docker container. 

Currently supported versions:
- `0.22.1`
- `0.17.0`

## (First time only) Docker installation
To install `docker`, refer to the [Install Docker Engine](https://docs.docker.com/engine/install/) page on the `Docker` website.

Next, if docker is being set up on an internal drive (not on an SSD or external drive), ensure that `/etc/docker/daemon.json` contains:
```
{
  "runtimes": {
    "nvidia": {
      "args": [],
      "path": "nvidia-container-runtime"
    }
  }
}
```

## Building the rerun container

Build `rerun:VERSION`:
`sudo docker build -f docker/Dockerfile.VERSION -t rerun:VERSION .`

Replace `VERSION` with one of the supported versions, such as 0.22.1 or 0.17.0.

## Run the application
Run the application: `cargo run --package re_docker VERSION` where `VERSION` is one of the currently supported versions.

## Adding a new Dockerfile
When a new version of rerun is released, a new Dockerfile should be created. There may be changes to make in order for that version of rerun to run in Docker. 
1. Create a new `Dockerfile.VERSION` with the version number for VERSION in `./docker/`.
2. Use an existing Dockerfile as a template (e.g. `Dockerfile.0.22.1`)
3. Update the `rerun-sdk` version in the file.
4. Build and test the new image using the steps above.

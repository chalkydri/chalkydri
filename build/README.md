
# Chalkydri build infra

These scripts are used to build the dev container and release images.

The build process is meant to run inside a container on Linux.

Building the dev container will take a very long time, because we first need to build TensorFlow Lite.

The build process is set up to run in Github Actions automatically, but can be run manually:
```shell
# Make sure you have Docker
docker --version

# Build the dev container
make dev

# Build release images
make build
```

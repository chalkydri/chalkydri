
Because we use a lot of platform-specific libraries, building the codebase is a little complicated.

Required libraries:
 - TensorFlow Lite
 - libedgetpu
 - FlatBuffers

~~Only Alpine Linux is officially supported, being our distro of choice for pre-built images.
Alpine is a very solid option for Chalkydri: lightweight (no hard dependency on GNU and systemd stuff), stable, and fast.~~

Ubuntu for now :(

We're using a Docker container to simplify the build process.

```shell
# (In the Chalkydri repo)

cd build/
docker build -t ghcr.io/chalkydri/chalkydri:dev
docker run --rm ghcr.io/chalkydri/chalkydri:dev
```

Then wait...

It might take a while

Go get some water

Maybe a snack

...

Ok, it's done!


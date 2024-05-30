
Images are based on Alpine Linux and built using a little custom tooling.

Alpine is a very solid option, as they don't pull in GNU and systemd stuff by default.

An Alpine host is required to build images.
We're using a Docker container to simplify the build process.

```shell
# (In the Chalkydri repo)

cd build/
docker build -t chalkydri-builder:latest
docker run --rm chalkydri-builder:latest
```

Then wait...

It might take a while

Go get some water

Maybe a snack

...

Ok, it's done!


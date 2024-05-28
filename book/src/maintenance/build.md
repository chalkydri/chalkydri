
Images are based on Alpine Linux and built using [their tooling](https://wiki.alpinelinux.org/wiki/How_to_make_a_custom_ISO_image_with_mkimage).
Alpine's image creation tooling is implemented as a set of shell scripts.

We're using a Docker container to simplify the build process.

```shell
# (In the Chalkydri repo)

cd build/

./build.sh
```


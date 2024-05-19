
Chalkydri was built on and for Linux, so using other operating systems may be a little more difficult.

## Install the Rust toolchain

It's highly recommended you use [rustup](https://rustup.rs) to install the toolchain.

## Build requirements

 - A reasonably recent and powerful processor
 - A reasonable amount of RAM
 - Docker (if cross-compiling)
 - A complete build requires a considerable amount of disk space:
   - 2GB (rec. 3GB): Chalkydri itself
   - 4GB (rec. 6GB): Bazel / libedgetpu
   - 2GB (rec. 8GB): TFLite
   - That totals up to a recommended 17GB chunk of your drive (or a minimum 8GB).
     The hope is most of this can happen in CI, so it's not a massive pain for everybody trying to work on Chalkydri.


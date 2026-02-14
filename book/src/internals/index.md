
# Chalkydri internals

Chalkydri is built on top of [Copper](), a robotics SDK and runtime for Rust.
We're mainly using it for its runtime, but it's meant to be used as a ROS alternative.

Chalkydri must be compiled *with* the Copper configuration currently.
The configurator generates Copper configs, which can then be used to build Chalkydri.

We use GStreamer for pulling frames from the camera and most video processing tasks.
For MJPEG sources, we use [`turbojpeg`](https://crates.io/crates/turbojpeg) (Rust bindings for `libjpeg-turbo`).
For gaussian blurring, we use [`libblur`](https://crates.io/crates/libblur).


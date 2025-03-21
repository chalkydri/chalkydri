
# Chalkydri internals

There's two major parts:
 - the backend, written in Rust, which does all the work
 - the frontend, written in Svelte/TypeScript, which provides users with quick configuration and status monitoring

## Backend

We use GStreamer for pulling frames from the camera and most video processing tasks.
For MJPEG sources, we use [`turbojpeg`](https://crates.io/crates/turbojpeg) (Rust bindings for `libjpeg-turbo`).
For gaussian blurring, we use [`libblur`](https://crates.io/crates/libblur).



# TFLedge

These are bindings to TensorFlow Lite (TFLite), a popular machine learning library, and libedgetpu, used to interact with Coral acceleration devices.

Only Coral devices are supported currently.

---

Raw bindings to the C code are generated with [bindgen](https://github.com/rust-lang/rust-bindgen).
The public interface is written by hand.

The raw bindings must be handled with care, as misuse can lead to memory bugs.
Luckily, you probably don't need to work with those.
And hopefully I will have taught that part by then.


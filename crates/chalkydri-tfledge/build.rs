extern crate bindgen;

#[cfg(all(feature = "direct", feature = "throttled"))]
compile_error!("Can't select 'direct' and 'throttled' features");

#[cfg(not(any(feature = "direct", feature = "throttled")))]
compile_error!("Must select 'direct' or 'throttled' feature");

fn main() {
    #[cfg(feature = "__bindgen")]
    {
        let mut b = bindgen::builder()
            .clang_arg("-I../../third_party/libedgetpu/tflite/public")
            .clang_arg("-I../../third_party/tensorflow")
            .rustified_enum(".*")
            .use_core();
        for h in [
            "edgetpu_c.h",
            "tensorflow/lite/core/c/c_api.h",
            "tensorflow/lite/core/c/common.h",
            "/usr/include/stdio.h",
        ] {
            b = b.header(h);
        }
        b.generate().unwrap().write_to_file("src/gen.rs").unwrap();
    }

    #[cfg(feature = "direct")]
    println!(
        "cargo:rustc-link-search={}/../../third_party/libedgetpu/out/direct",
        env!("CARGO_MANIFEST_DIR")
    );

    #[cfg(feature = "throttled")]
    println!(
        "cargo:rustc-link-search={}/../../third_party/libedgetpu/out/throttled",
        env!("CARGO_MANIFEST_DIR")
    );

    println!("cargo:rustc-link-lib=edgetpu");

    println!(
        "cargo:rustc-link-search={}/../../third_party/tensorflow/build",
        env!("CARGO_MANIFEST_DIR")
    );
    println!("cargo:rustc-link-lib=tensorflowlite_c");
}

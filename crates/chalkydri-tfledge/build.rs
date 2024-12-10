extern crate bindgen;

#[cfg(all(feature = "direct", feature = "throttled"))]
compile_error!("Can't select 'direct' and 'throttled' features");

#[cfg(not(any(feature = "direct", feature = "throttled")))]
compile_error!("Must select 'direct' or 'throttled' feature");

fn main() {
    #[cfg(feature = "__bindgen")]
    {
        let mut b = bindgen::builder()
            .clang_arg("-I/usr/local/include")
            .rustified_enum(".*")
            .use_core();
        for h in [
            "edgetpu_c.h",
            "tensorflow/lite/core/c/c_api.h",
            "tensorflow/lite/core/c/common.h",
            "tensorflow/lite/core/async/c/async_kernel.h",
            "tensorflow/lite/core/async/c/task.h",
            "tensorflow/lite/core/async/c/async_signature_runner.h",
            "/usr/include/stdio.h",
        ] {
            b = b.header(h);
        }
        b.generate().unwrap().write_to_file("src/gen.rs").unwrap();
    }

    println!("cargo:rustc-link-search=/usr/local/lib");
    println!("cargo:rustc-link-lib=edgetpu");
    println!("cargo:rustc-link-lib=tensorflowlite_c");
}

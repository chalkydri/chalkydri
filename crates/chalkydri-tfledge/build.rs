extern crate bindgen;

fn main() {
    #[cfg(feature = "__bindgen")]
    {
        let mut b = bindgen::builder()
            .clang_arg("-I/home/lincoln/chalkydri/chalkydri-tfledge")
            .rustified_enum(".*")
            .use_core();
        for h in [
            "edgetpu_runtime/libedgetpu/edgetpu_c.h",
            "tensorflow/lite/core/c/c_api.h",
            "tensorflow/lite/core/c/common.h",
            "/usr/include/stdio.h",
        ] {
            b = b.header(h);
        }
        b.generate().unwrap().write_to_file("src/gen.rs").unwrap();
    }

    println!("cargo:rustc-link-search=/home/lincoln/chalkydri/chalkydri-tfledge");
    println!("cargo:rustc-link-lib=edgetpu");
    println!("cargo:rustc-link-lib=tensorflowlite_c");
}

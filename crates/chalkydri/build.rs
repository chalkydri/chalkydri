fn main() {
    println!("cargo::rerun-if-changed=copperconfig.ron");
    println!(concat!(
        "cargo::rustc-env=LOG_INDEX_DIR=",
        env!("CARGO_MANIFEST_DIR"),
        "/target"
    ));
}

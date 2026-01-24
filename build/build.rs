use std::{
    fs::{self, File},
    path::Path,
};

fn main() {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();

    assert!(
        workspace_dir.join("ui/openapi.json").exists(),
        "openapi spec must be present"
    );
}

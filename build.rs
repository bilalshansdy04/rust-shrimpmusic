use std::{collections::HashMap, path::PathBuf};

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let library = HashMap::from([
        ("lucide".to_string(), PathBuf::from(lucide_slint::lib())),
        ("material".to_string(), PathBuf::from(manifest_dir).join("material-1.1.0/material.slint")),
    ]);

    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(library);

    slint_build::compile_with_config("ui/appwindow.slint", config)
        .expect("Slint build failed");
}

use std::{collections::HashMap, path::PathBuf};

fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        res.set("FileDescription", "ShrimpMusic Player");
        res.set("ProductName", "ShrimpMusic Player");
        res.set("OriginalFilename", "shrimp-music-player.exe");
        res.set("CompanyName", "Bilal Shandyarta Syamsudin");
        let _ = res.compile();
    }

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

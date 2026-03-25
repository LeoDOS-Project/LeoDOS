use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let apps_dir = PathBuf::from("../..");
    let mut builder = bindgen::Builder::default()
        .clang_arg(format!(
            "-I{}/inc",
            env::var("BUILD_DIR").unwrap_or_default()
        ))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for app in ["spacecomp_wildfire", "router"] {
        let config_dir = apps_dir.join(app).join("config");
        if let Ok(headers) = std::fs::read_dir(&config_dir) {
            for header in headers.flatten() {
                let path = header.path();
                if path.extension().is_some_and(|e| e == "h") {
                    println!("cargo:rerun-if-changed={}", path.display());
                    builder = builder.header(path.to_string_lossy().to_string());
                }
            }
        }
    }

    let bindings = builder
        .allowlist_var("[A-Z_]+")
        .generate()
        .expect("Unable to generate bindings for app config");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("config.rs"))
        .expect("Couldn't write config bindings!");
}

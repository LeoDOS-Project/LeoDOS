use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Link against novatel GPS driver (provides NOVATEL_OEM615_* symbols)
    if let Ok(build_dir) = std::env::var("CARGO_TARGET_DIR") {
        let lib_dir = format!("{build_dir}/../../../exe/cpu1/cf");
        println!("cargo:rustc-link-search=native={lib_dir}");
        println!("cargo:rustc-link-lib=dylib=novatel_oem615");
    }

    let apps_dir = PathBuf::from("../..");
    let build_dir = env::var("BUILD_DIR").unwrap_or_default();
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut builder = bindgen::Builder::default()
        .clang_arg(format!("-I{build_dir}/inc"))
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

    bindings
        .write_to_file(format!("{out_dir}/config.rs"))
        .expect("Couldn't write config bindings!");
}

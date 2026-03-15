use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../config/default_sample_rust_app_msgids.h");
    println!("cargo:rerun-if-changed=../config/default_sample_rust_app_perfids.h");

    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}/inc", std::env::var("BUILD_DIR").unwrap_or_default()))
        .header("../config/default_sample_rust_app_msgids.h")
        .header("../config/default_sample_rust_app_perfids.h")
        .allowlist_var("SAMPLE_RUST_APP_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings for app config");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("config.rs"))
        .expect("Couldn't write config bindings!");
}

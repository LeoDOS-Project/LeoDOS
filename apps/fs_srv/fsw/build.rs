use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../config/default_fs_srv_msgids.h");
    println!("cargo:rerun-if-changed=../config/default_fs_srv_perfids.h");

    let bindings = bindgen::Builder::default()
        .header("../config/default_fs_srv_msgids.h")
        .header("../config/default_fs_srv_perfids.h")
        .allowlist_var("FS_SRV_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings for app config");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("config.rs"))
        .expect("Couldn't write config bindings!");
}

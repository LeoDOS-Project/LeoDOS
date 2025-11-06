use cmake::Config;
use std::{env, path::PathBuf};

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wamr_dir = crate_dir.join("wasm-micro-runtime");

    assert!(
        wamr_dir.exists() && wamr_dir.join("CMakeLists.txt").exists(),
        "WAMR submodule not found. Run `git submodule update --init --recursive`"
    );

    let dst = Config::new(&wamr_dir)
        .define("WAMR_BUILD_FAST_INTERP", "1")
        .define("WAMR_BUILD_INTERP", "0")
        .define("WAMR_BUILD_JIT", "0")
        .define("WAMR_BUILD_AOT", "1")
        .define("WAMR_BUILD_LIBC_WASI", "0")
        .define("WAMR_BUILD_LIBC_BUILTIN", "1")
        .define("WAMR_BUILD_OPCODE_COUNTER", "1")
        .define("WAMR_BUILD_GLOBAL_HEAP_POOL", "1")
        .define("WAMR_BUILD_THREAD_MGR", "0")
        .build_target("vmlib")
        .build();

    println!("cargo:rustc-link-search=native={}/build", dst.display());
    println!("cargo:rustc-link-lib=static=iwasm");

    let header_path = wamr_dir.join("core/iwasm/include/wasm_export.h");
    bindgen::Builder::default()
        .header(header_path.to_str().unwrap())
        .ctypes_prefix("::core::ffi")
        .use_core()
        .derive_default(true)
        .allowlist_function("wasm_.*|WASM_.*")
        .allowlist_type("wasm_.*|WASM_.*")
        .allowlist_var("wasm_.*|WASM_.*")
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings");

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=System");
    }
    println!("cargo:rerun-if-changed={}", wamr_dir.display());
}

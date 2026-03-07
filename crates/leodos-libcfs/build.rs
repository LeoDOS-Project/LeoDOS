use bindgen::callbacks::{IntKind, MacroParsingBehavior, ParseCallbacks};
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

// Filter out public macros from the CFE/OSAL APIs.
fn is_api_macro(name: &str) -> bool {
    let prefixes = ["CFE_", "OSAL_", "OS_", "CF_"];
    #[cfg(feature = "nos3")]
    let prefixes = [
        "CFE_", "OSAL_", "OS_", "CF_", "UART_", "I2C_", "SPI_",
        "CAN_", "GPIO_", "SOCKET_", "MEM_", "TRQ_", "HWLIB_",
    ];
    prefixes.iter().any(|p| name.starts_with(p))
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !name.ends_with("_H")
        && !name.ends_with("_H_")
}

fn get_path(env_var: &str) -> PathBuf {
    env::var(env_var)
        .map(PathBuf::from)
        .expect(&format!("Environment variable {} not set", env_var))
}

fn header(base: &PathBuf, path: &str) -> String {
    base.join(path).display().to_string()
}

#[derive(Debug, Default, Clone)]
struct MacroDetector {
    all_potential_macros: Rc<RefCell<HashSet<String>>>,
    converted_macros: Rc<RefCell<HashSet<String>>>,
}

impl ParseCallbacks for MacroDetector {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        if is_api_macro(name) {
            self.all_potential_macros
                .borrow_mut()
                .insert(name.to_string());
        }
        MacroParsingBehavior::Default
    }
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        self.converted_macros.borrow_mut().insert(name.to_string());
        None
    }

    fn str_macro(&self, name: &str, _value: &[u8]) {
        self.converted_macros.borrow_mut().insert(name.to_string());
    }
}

#[derive(Debug)]
struct MacroDefinition {
    name: String,
    value: String,
    doc_comment: Option<String>,
}

fn format_doc_comment(raw_comment: &str) -> String {
    let lines = raw_comment
        .lines()
        .map(|line| {
            let mut cleaned = line.trim();
            if let Some(stripped) = cleaned.strip_prefix("/**") {
                cleaned = stripped.trim_start();
            }
            if let Some(stripped) = cleaned.strip_suffix("*/") {
                cleaned = stripped.trim_end();
            }
            if let Some(stripped) = cleaned.strip_prefix('*') {
                cleaned = stripped.trim_start();
            }
            cleaned
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("\n");

    lines.trim().escape_default().to_string()
}

fn find_macro_definitions(
    macros_to_find: &HashSet<&str>,
    include_paths: &[PathBuf],
) -> HashMap<PathBuf, Vec<MacroDefinition>> {
    let mut results: HashMap<PathBuf, Vec<MacroDefinition>> = HashMap::new();
    let define_re = Regex::new(r"^\s*#define\s+([A-Z0-9_]+)[ \t]+(.+)").unwrap();
    let trailing_comment_re = Regex::new(r"/\*\*<\s*((?:@brief\s*)?.*?)\s*\*/").unwrap();

    for path in include_paths {
        if !path.is_dir() {
            continue;
        }

        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
        {
            let entry_path = entry.path();
            if !entry_path.is_file() {
                continue;
            }
            let ext = entry_path.extension().and_then(|s| s.to_str());
            if !matches!(ext, Some("h") | Some("c")) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(entry_path) {
                let mut last_doc_comment: Option<String> = None;
                let mut doc_comment_buffer = String::new();
                let mut in_doc_comment = false;

                for line in content.lines() {
                    let trimmed_line = line.trim();

                    if trimmed_line.starts_with("/**") && !trimmed_line.contains("/**<") {
                        if trimmed_line.contains(r"\name")
                            || trimmed_line.contains(r"\{")
                            || trimmed_line.contains(r"\}")
                        {
                            last_doc_comment = None;
                            continue;
                        }
                        in_doc_comment = true;
                        doc_comment_buffer.clear();
                    }

                    if in_doc_comment {
                        doc_comment_buffer.push_str(line);
                        doc_comment_buffer.push('\n');
                        if trimmed_line.ends_with("*/") {
                            in_doc_comment = false;
                            last_doc_comment = Some(doc_comment_buffer.clone());
                        }
                        continue;
                    }

                    if let Some(caps) = define_re.captures(line) {
                        let name = caps.get(1).unwrap().as_str();
                        if macros_to_find.contains(name) {
                            let value = caps.get(2).unwrap().as_str().trim();
                            let mut doc = None;

                            if let Some(trailing_caps) = trailing_comment_re.captures(value) {
                                doc =
                                    Some(trailing_caps.get(1).unwrap().as_str().trim().to_string());
                            } else if let Some(comment) = last_doc_comment.take() {
                                doc = Some(comment);
                            }

                            let definition = MacroDefinition {
                                name: name.to_string(),
                                doc_comment: doc,
                                value: trailing_comment_re.replace(value, "").trim().to_string(),
                            };
                            results
                                .entry(entry_path.to_path_buf())
                                .or_default()
                                .push(definition);
                        }
                        last_doc_comment = None;
                    } else if !trimmed_line.is_empty() {
                        last_doc_comment = None;
                    }
                }
            }
        }
    }
    results
}

fn parse_simple_cast_macro(value: &str) -> Option<(String, String)> {
    let re =
        Regex::new(r"^\s*\(\s*\(\s*([a-zA-Z0-9_]+)\s*\)\s*(-?(?:0x[0-9a-fA-F]+|[0-9]+))\s*\)\s*$")
            .unwrap();

    if let Some(caps) = re.captures(value) {
        let type_str = caps.get(1).unwrap().as_str().to_string();
        let value_str = caps.get(2).unwrap().as_str().trim().to_string();
        Some((type_str, value_str))
    } else {
        None
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=nos3_hwlib_wrapper.h");

    let cfe_dir = get_path("CFE_DIR");
    let osal_dir = get_path("OSAL_DIR");
    let psp_dir = get_path("PSP_DIR");
    let build_dir = get_path("BUILD_DIR");
    let cf_dir = env::var("CF_DIR").ok().map(PathBuf::from);
    #[cfg(feature = "nos3")]
    let hwlib_dir = env::var("HWLIB_DIR").ok().map(PathBuf::from);
    let debug = env::var("DEBUG").as_deref() == Ok("1");

    if debug {
        println!("cargo::warning=CFE_DIR={}", cfe_dir.display());
        println!("cargo::warning=OSAL_DIR={}", osal_dir.display());
        println!("cargo::warning=PSP_DIR={}", psp_dir.display());
        println!("cargo::warning=BUILD_DIR={}", build_dir.display());
        if let Some(ref cf) = cf_dir {
            println!("cargo::warning=CF_DIR={}", cf.display());
        }
        #[cfg(feature = "nos3")]
        if let Some(ref hw) = hwlib_dir {
            println!("cargo::warning=HWLIB_DIR={}", hw.display());
        }
    }

    let macro_detector = MacroDetector::default();

    let mut include_paths = vec![
        build_dir.join("inc"),
        build_dir.join("native/default_cpu1/osal/inc"),
        build_dir.join("native/default_cpu1/psp/inc"),
        build_dir.join("native/default_cpu1/inc"),
        cfe_dir.join("modules/core_api/fsw/inc"),
        cfe_dir.join("modules/core_api/config"),
        cfe_dir.join("modules/core_private/config"),
        cfe_dir.join("modules/es/fsw/inc"),
        cfe_dir.join("modules/es/config"),
        cfe_dir.join("modules/evs/fsw/inc"),
        cfe_dir.join("modules/evs/config"),
        cfe_dir.join("modules/sb/fsw/inc"),
        cfe_dir.join("modules/sb/config"),
        cfe_dir.join("modules/tbl/fsw/inc"),
        cfe_dir.join("modules/tbl/config"),
        cfe_dir.join("modules/time/fsw/inc"),
        cfe_dir.join("modules/time/config"),
        cfe_dir.join("modules/fs/config"),
        cfe_dir.join("modules/msg/fsw/inc"),
        cfe_dir.join("modules/msg/option_inc"),
        cfe_dir.join("modules/config/fsw/inc"),
        cfe_dir.join("modules/resourceid/fsw/inc"),
        cfe_dir.join("modules/resourceid/option_inc"),
        osal_dir.join("src/os/inc"),
        osal_dir.join("src/os/posix"),
        osal_dir.join("src/bsp/generic-linux/config"),
        psp_dir.join("fsw/inc"),
        psp_dir.join("fsw/pc-linux/inc"),
    ];

    if let Some(ref cf) = cf_dir {
        include_paths.push(cf.join("fsw/src"));
        include_paths.push(cf.join("fsw/inc"));
        include_paths.push(cf.join("config"));
    }

    #[cfg(feature = "nos3")]
    if let Some(ref hw) = hwlib_dir {
        include_paths.push(hw.join("fsw/public_inc"));
    }

    let mut builder = bindgen::Builder::default()
        .header(header(&cfe_dir, "modules/core_api/fsw/inc/cfe.h"))
        .header(header(&cfe_dir, "modules/core_api/fsw/inc/cfe_error.h"))
        .header(header(&osal_dir, "src/os/inc/common_types.h"))
        .header(header(&osal_dir, "src/os/inc/osapi.h"))
        .header(header(&psp_dir, "fsw/inc/cfe_psp.h"))
        .default_visibility(bindgen::FieldVisibilityKind::PublicCrate)
        .use_core()
        .ctypes_prefix("libc")
        .clang_arg("-D_LINUX_OS_")
        .clang_arg("-D_POSIX_OS_")
        .allowlist_function("CFE_.*|OSAL_.*|OS_.*|CF_.*")
        .allowlist_type("CFE_.*|OSAL_.*|OS_.*|CF_.*")
        .allowlist_var("CFE_.*|OSAL_.*|OS_.*|CF_.*")
        .layout_tests(false)
        .derive_default(true)
        .parse_callbacks(Box::new(macro_detector.clone()));

    if let Ok(sysroot) = env::var("SYSROOT") {
        builder = builder.clang_arg(format!("--sysroot={}", sysroot));
    }

    for path in &include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    if let Some(ref cf) = cf_dir {
        builder = builder
            .header(header(cf, "fsw/src/cf_cfdp_pdu.h"))
            .header(header(cf, "fsw/src/cf_logical_pdu.h"))
            .header(header(cf, "fsw/src/cf_codec.h"))
            .header(header(cf, "fsw/src/cf_cfdp_types.h"))
            .header(header(cf, "fsw/src/cf_cfdp.h"))
            .header(header(cf, "fsw/src/cf_app.h"));
    }

    #[cfg(feature = "nos3")]
    if let Some(ref hw) = hwlib_dir {
        // Use wrapper header that provides stub types for non-Linux hosts
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        builder = builder
            .header(manifest_dir.join("nos3_hwlib_wrapper.h").display().to_string())
            .allowlist_function("uart_.*|i2c_.*|spi_.*|can_.*|gpio_.*|socket_.*|devmem_.*|trq_.*|HostToIp")
            .allowlist_type("uart_.*|i2c_.*|spi_.*|can_.*|gpio_.*|socket_.*|trq_.*|canid_t|addr_fam_e|type_e|category_e")
            .allowlist_var("UART_.*|I2C_.*|SPI_.*|CAN_.*|GPIO_.*|SOCKET_.*|MEM_.*|TRQ_.*|PORT_.*|NUM_.*|HWLIB_.*");
        let _ = hw; // suppress unused warning
    }

    let bindings = builder.generate().expect("Unable to generate bindings!");
    // Use a mutable string so we can inject comments later
    let mut bindings_str = bindings.to_string();

    let all = macro_detector.all_potential_macros.borrow();
    let converted = macro_detector.converted_macros.borrow();
    let skipped_macros: Vec<_> = all.difference(&converted).cloned().collect();

    // --- PART 1: Process Skipped Macros (Prepend to file) ---
    let mut comment = String::new();
    comment.push_str("// The following constants were skipped by bindgen.\n");
    comment.push_str("// The `build.rs` script was able to process some of them.\n");
    comment.push_str("// It is recommended to define the rest manually in a separate Rust file.\n");
    comment.push_str(&format!("// Total skipped: {}\n", skipped_macros.len()));

    if !skipped_macros.is_empty() {
        let skipped_set: HashSet<&str> = skipped_macros.iter().map(String::as_str).collect();
        let locations = find_macro_definitions(&skipped_set, &include_paths);
        let mut sorted_files: Vec<_> = locations.keys().collect();
        sorted_files.sort();

        for file_path in sorted_files {
            if let Some(definitions) = locations.get(file_path) {
                comment.push_str("//\n");
                comment.push_str(&format!("// File: {}\n", file_path.display()));
                comment.push_str("// ----------------------------------------\n");
                for def in definitions {
                    if let Some((ty, val)) = parse_simple_cast_macro(&def.value) {
                        if let Some(raw_comment) = &def.doc_comment {
                            let formatted_doc = format_doc_comment(raw_comment);
                            comment.push_str(&format!("#[doc = \"{}\"]\n", formatted_doc));
                        }
                        comment.push_str(&format!("pub const {}: {} = {};\n", def.name, ty, val));
                    } else {
                        if let Some(raw_comment) = &def.doc_comment {
                            let formatted_doc = format_doc_comment(raw_comment);
                            comment.push_str(&format!("// #[doc = \"{}\"]\n", formatted_doc));
                        }
                        comment.push_str(&format!(
                            "// pub const {}: /* ? */ = /* {} */;\n",
                            def.name, def.value
                        ));
                    }
                }
            }
        }
    }

    // Process Converted Macros
    if !converted.is_empty() {
        let converted_set: HashSet<&str> = converted.iter().map(String::as_str).collect();
        // Re-use our robust parser to find docs for macros bindgen ALREADY converted
        let converted_docs = find_macro_definitions(&converted_set, &include_paths);

        for (_path, defs) in converted_docs {
            for def in defs {
                if let Some(raw_comment) = def.doc_comment {
                    let formatted_doc = format_doc_comment(&raw_comment);
                    // Create a regex to find the exact `pub const NAME:` line.
                    // We use `(?m)^` to match the start of a line in multiline mode.
                    // We escape the macro name just in case, though C identifiers are usually safe.
                    let re = Regex::new(&format!(r"(?m)^pub const {}:", regex::escape(&def.name)))
                        .unwrap();

                    // Construct the replacement: the doc comment, a newline, then the original match.
                    let replacement =
                        format!("#[doc = \"{}\"]\npub const {}:", formatted_doc, def.name);

                    // Perform the replacement in the bindings string.
                    bindings_str = re.replace(&bindings_str, replacement).to_string();
                }
            }
        }
    }

    let final_content = format!("{}\n{}", comment, bindings_str);
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_file = out_dir.join("bindings.rs");
    fs::write(&out_file, final_content).expect("Couldn't write bindings");

    let content = fs::read_to_string(&out_file).unwrap();
    let content = content.replace("pub fn", "pub(crate) fn");
    let content = content.replace("pub type", "pub(crate) type");
    let content = content.replace("pub const", "pub(crate) const");
    let content = content.replace("pub use", "pub(crate) use");
    let content = content.replace("pub struct", "pub(crate) struct");
    let content = content.replace("pub union", "pub(crate) union");
    let content = content.replace("pub enum", "pub(crate) enum");
    fs::write(&out_file, content).unwrap();
}

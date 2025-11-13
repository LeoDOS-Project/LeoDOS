use bindgen::callbacks::{IntKind, MacroParsingBehavior, ParseCallbacks};
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

fn is_api_macro(name: &str) -> bool {
    name.starts_with("CSP_")
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !name.ends_with("_H")
        && !name.ends_with("_H_")
}

fn get_path(env_var: &str) -> PathBuf {
    env::var(env_var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| panic!("Environment variable {} not set", env_var))
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

fn generate_autoconfig(out_dir: &PathBuf) -> PathBuf {
    let use_rdp = env::var("CARGO_FEATURE_RDP").is_ok();
    let use_hmac = env::var("CARGO_FEATURE_HMAC").is_ok();
    let use_promisc = env::var("CARGO_FEATURE_PROMISC").is_ok();

    let config = format!(
        r#"#pragma once

#define CSP_POSIX 1
#define CSP_ZEPHYR 0
#define CSP_FREERTOS 0

#define CSP_HAVE_STDIO 1
#define CSP_ENABLE_CSP_PRINT 1
#define CSP_PRINT_STDIO 1

#define CSP_REPRODUCIBLE_BUILDS 0

#define CSP_QFIFO_LEN 16
#define CSP_PORT_MAX_BIND 16
#define CSP_CONN_RXQUEUE_LEN 16
#define CSP_CONN_MAX 8
#define CSP_BUFFER_SIZE 256
#define CSP_BUFFER_COUNT 16
#define CSP_RDP_MAX_WINDOW 5
#define CSP_RTABLE_SIZE 10

#define CSP_USE_RDP {}
#define CSP_USE_HMAC {}
#define CSP_USE_PROMISC {}
#define CSP_USE_RTABLE 1
#define CSP_BUFFER_ZERO_CLEAR 0

#define CSP_HAVE_LIBSOCKETCAN 0
#define CSP_HAVE_LIBZMQ 0

#define CSP_FIXUP_V1_ZMQ_LITTLE_ENDIAN 0
#define CSP_ENABLE_KISS_CRC 0
"#,
        if use_rdp { 1 } else { 0 },
        if use_hmac { 1 } else { 0 },
        if use_promisc { 1 } else { 0 },
    );

    let autoconfig_dir = out_dir.join("csp");
    fs::create_dir_all(&autoconfig_dir).expect("Failed to create autoconfig dir");

    let autoconfig_path = autoconfig_dir.join("autoconfig.h");
    fs::write(&autoconfig_path, config).expect("Failed to write autoconfig.h");

    out_dir.clone()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let csp_dir = get_path("CSP_DIR");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let autoconfig_dir = generate_autoconfig(&out_dir);

    let macro_detector = MacroDetector::default();

    let include_paths = vec![
        autoconfig_dir.clone(),
        csp_dir.join("include"),
        csp_dir.join("include/csp"),
        csp_dir.join("include/csp/arch"),
        csp_dir.join("include/csp/crypto"),
        csp_dir.join("include/csp/drivers"),
        csp_dir.join("include/csp/interfaces"),
        csp_dir.join("src"),
        csp_dir.join("src/arch/posix"),
    ];

    let mut builder = bindgen::Builder::default()
        .header(header(&csp_dir, "include/csp/csp.h"))
        .header(header(&csp_dir, "include/csp/csp_types.h"))
        .header(header(&csp_dir, "include/csp/csp_buffer.h"))
        .header(header(&csp_dir, "include/csp/csp_cmp.h"))
        .header(header(&csp_dir, "include/csp/csp_crc32.h"))
        .header(header(&csp_dir, "include/csp/csp_debug.h"))
        .header(header(&csp_dir, "include/csp/csp_error.h"))
        .header(header(&csp_dir, "include/csp/csp_hooks.h"))
        .header(header(&csp_dir, "include/csp/csp_id.h"))
        .header(header(&csp_dir, "include/csp/csp_iflist.h"))
        .header(header(&csp_dir, "include/csp/csp_interface.h"))
        .header(header(&csp_dir, "include/csp/csp_promisc.h"))
        .header(header(&csp_dir, "include/csp/csp_rtable.h"))
        .header(header(&csp_dir, "include/csp/csp_sfp.h"))
        .header(header(&csp_dir, "include/csp/arch/csp_queue.h"))
        .header(header(&csp_dir, "include/csp/arch/csp_time.h"))
        .header(header(&csp_dir, "include/csp/interfaces/csp_if_lo.h"))
        .header(header(&csp_dir, "include/csp/interfaces/csp_if_udp.h"))
        .default_visibility(bindgen::FieldVisibilityKind::PublicCrate)
        .use_core()
        .ctypes_prefix("libc")
        .clang_arg("-DCSP_POSIX=1")
        .allowlist_function("csp_.*")
        .allowlist_type("csp_.*")
        .allowlist_var("CSP_.*")
        .blocklist_type("csp_queue_handle_t")
        .blocklist_type("csp_static_queue_t")
        .layout_tests(false)
        .derive_default(true)
        .parse_callbacks(Box::new(macro_detector.clone()));

    if let Ok(sysroot) = env::var("SYSROOT") {
        builder = builder.clang_arg(format!("--sysroot={}", sysroot));
    }

    for path in &include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder.generate().expect("Unable to generate bindings!");
    let mut bindings_str = bindings.to_string();

    let all = macro_detector.all_potential_macros.borrow();
    let converted = macro_detector.converted_macros.borrow();
    let skipped_macros: Vec<_> = all.difference(&converted).cloned().collect();

    let mut comment = String::new();
    comment.push_str("// CSP bindings generated by leodos-libcsp build.rs\n");
    comment.push_str("// The following constants were skipped by bindgen.\n");
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

    if !converted.is_empty() {
        let converted_set: HashSet<&str> = converted.iter().map(String::as_str).collect();
        let converted_docs = find_macro_definitions(&converted_set, &include_paths);

        for (_path, defs) in converted_docs {
            for def in defs {
                if let Some(raw_comment) = def.doc_comment {
                    let formatted_doc = format_doc_comment(&raw_comment);
                    let re = Regex::new(&format!(r"(?m)^pub const {}:", regex::escape(&def.name)))
                        .unwrap();
                    let replacement =
                        format!("#[doc = \"{}\"]\npub const {}:", formatted_doc, def.name);
                    bindings_str = re.replace(&bindings_str, replacement).to_string();
                }
            }
        }
    }

    let final_content = format!("{}\n{}", comment, bindings_str);
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

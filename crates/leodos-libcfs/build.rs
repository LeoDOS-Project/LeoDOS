use bindgen::callbacks::IntKind;
use bindgen::callbacks::MacroParsingBehavior;
use bindgen::callbacks::ParseCallbacks;
use bindgen::Builder;
use bindgen::FieldVisibilityKind;
use indoc::indoc;
use regex::escape;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(feature = "nos3")]
const NOS3_COMPAT: &str = indoc! {"
    #include <stdint.h>
    #if !defined(__linux__) && !defined(__rtems__)
    typedef uint32_t canid_t;
    struct can_frame {
        uint32_t can_id; uint8_t can_dlc;
        uint8_t __pad; uint8_t __res0; uint8_t __res1;
        uint8_t data[8];
    };
    struct sockaddr_can { int can_family; int can_ifindex; };
    #ifndef __APPLE__
    struct ifreq { char ifr_name[16]; int ifr_ifindex; };
    #endif
    struct spi_ioc_transfer {
        uint64_t tx_buf; uint64_t rx_buf;
        uint32_t len; uint32_t speed_hz; uint16_t delay_usecs;
        uint8_t bits_per_word; uint8_t cs_change; uint32_t pad;
    };
    struct i2c_msg { uint16_t addr; uint16_t flags; uint16_t len; uint8_t *buf; };
    struct i2c_rdwr_ioctl_data { struct i2c_msg *msgs; uint32_t nmsgs; };
    #endif
"};

#[cfg(not(feature = "nos3"))]
const PREFIXES: &[&str] = &["CFE_", "OSAL_", "OS_", "CF_"];

#[cfg(feature = "nos3")]
#[rustfmt::skip]
const PREFIXES: &[&str] = &[
    "CFE_", "OSAL_", "OS_", "CF_",
    "UART_", "I2C_", "SPI_", "CAN_", "GPIO_", "SOCKET_", "MEM_", "TRQ_", "HWLIB_",
    "GENERIC_RADIO_", "GENERIC_EPS_", "GENERIC_ADCS_", "GENERIC_CSS_", "GENERIC_FSS_",
    "GENERIC_IMU_", "GENERIC_MAG_", "GENERIC_STAR_TRACKER_", "GENERIC_RW_",
    "GENERIC_REACTION_WHEEL_", "GENERIC_TORQUER_", "GENERIC_THRUSTER_",
    "NOVATEL_OEM615_", "CAM_", "PASSIVE_", "BDOT_", "SUNSAFE_", "INERTIAL_",
];

#[cfg(feature = "nos3")]
#[rustfmt::skip]
const NOS3_COMPONENTS: &[(&str, &str, &str)] = &[
    ("generic_radio", "generic_radio", "GENERIC_RADIO"),
    ("generic_eps", "generic_eps", "GENERIC_EPS"),
    ("generic_adcs", "generic_adcs", "Generic_ADCS|GENERIC_ADCS"),
    ("generic_css", "generic_css", "GENERIC_CSS"),
    ("generic_fss", "generic_fss", "GENERIC_FSS"),
    ("generic_imu", "generic_imu", "GENERIC_IMU"),
    ("generic_mag", "generic_mag", "GENERIC_MAG"),
    ("generic_star_tracker", "generic_star_tracker", "GENERIC_STAR_TRACKER"),
    ("generic_reaction_wheel", "generic_reaction_wheel", "GENERIC_RW|GENERIC_REACTION_WHEEL"),
    ("generic_torquer", "generic_torquer", "GENERIC_TORQUER"),
    ("generic_thruster", "generic_thruster", "GENERIC_THRUSTER"),
    ("novatel_oem615", "novatel_oem615", "NOVATEL_OEM615|GPGGA"),
    ("arducam", "cam", "CAM"),
];

/// True if `name` is an ALL_CAPS C macro from a known API prefix.
fn is_api_macro(name: &str) -> bool {
    PREFIXES.iter().any(|p| name.starts_with(p))
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

/// Bindgen callback that records which C macros were seen vs successfully converted.
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

/// Strips C doc-comment markers (`/** */`, leading `*`) and escapes for Rust `#[doc]`.
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

/// Tracks `/** ... */` doc comments across lines in a C header.
struct DocCommentTracker {
    buffer: String,
    in_comment: bool,
    last_doc: Option<String>,
}

impl DocCommentTracker {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            in_comment: false,
            last_doc: None,
        }
    }

    fn feed_line(&mut self, line: &str) -> bool {
        let trimmed = line.trim();

        if trimmed.starts_with("/**") && !trimmed.contains("/**<") {
            if trimmed.contains(r"\name") || trimmed.contains(r"\{") || trimmed.contains(r"\}") {
                self.last_doc = None;
                return true;
            }
            self.in_comment = true;
            self.buffer.clear();
        }

        if self.in_comment {
            self.buffer.push_str(line);
            self.buffer.push('\n');
            if trimmed.ends_with("*/") {
                self.in_comment = false;
                self.last_doc = Some(self.buffer.clone());
            }
            return true;
        }

        false
    }

    fn take_doc(&mut self) -> Option<String> {
        self.last_doc.take()
    }
}

/// Scans C headers for `#define` macros, extracting values and preceding doc comments.
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

        for entry in walkdir::WalkDir::new(path) {
            let Ok(entry) = entry else {
                continue;
            };
            let entry_path = entry.path();
            if !entry_path.is_file() {
                continue;
            }
            let ext = entry_path.extension().and_then(|s| s.to_str());
            if !matches!(ext, Some("h") | Some("c")) {
                continue;
            }

            let Ok(content) = fs::read_to_string(entry_path) else {
                continue;
            };
            let mut tracker = DocCommentTracker::new();

            for line in content.lines() {
                if tracker.feed_line(line) {
                    continue;
                }

                if let Some(caps) = define_re.captures(line) {
                    let name = caps.get(1).unwrap().as_str();
                    if macros_to_find.contains(name) {
                        let value = caps.get(2).unwrap().as_str().trim();
                        let doc = if let Some(trailing_caps) = trailing_comment_re.captures(value) {
                            Some(trailing_caps.get(1).unwrap().as_str().trim().to_string())
                        } else {
                            tracker.take_doc()
                        };

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
                    tracker.last_doc = None;
                } else if !line.trim().is_empty() {
                    tracker.last_doc = None;
                }
            }
        }
    }
    results
}

/// Parses `((type)value)` C cast macros into (type, value) pairs.
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

/// Accumulates headers, include paths, patterns, and clang args for bindgen.
struct BindgenConfig {
    headers: Vec<String>,
    include_paths: Vec<PathBuf>,
    clang_args: Vec<String>,
    raw_lines: Vec<String>,
    fn_patterns: Vec<String>,
    type_patterns: Vec<String>,
    var_patterns: Vec<String>,
    blocklist_types: Vec<String>,
}

#[allow(dead_code)]
impl BindgenConfig {
    fn new() -> Self {
        Self {
            headers: Vec::new(),
            include_paths: Vec::new(),
            clang_args: Vec::new(),
            raw_lines: Vec::new(),
            fn_patterns: Vec::new(),
            type_patterns: Vec::new(),
            var_patterns: Vec::new(),
            blocklist_types: Vec::new(),
        }
    }

    fn add_header(&mut self, h: String) {
        self.headers.push(h);
    }

    fn add_include(&mut self, path: PathBuf) {
        self.include_paths.push(path);
    }

    fn add_clang_arg(&mut self, arg: String) {
        self.clang_args.push(arg);
    }

    fn add_raw_line(&mut self, line: &str) {
        self.raw_lines.push(line.to_string());
    }

    fn add_pattern(&mut self, pat: String) {
        self.fn_patterns.push(pat.clone());
        self.type_patterns.push(pat.clone());
        self.var_patterns.push(pat);
    }

    fn add_fn_pattern(&mut self, pat: String) {
        self.fn_patterns.push(pat);
    }

    fn add_type_pattern(&mut self, pat: String) {
        self.type_patterns.push(pat);
    }

    fn add_var_pattern(&mut self, pat: String) {
        self.var_patterns.push(pat);
    }

    fn blocklist_type(&mut self, ty: &str) {
        self.blocklist_types.push(ty.to_string());
    }

    fn add_cfe(
        &mut self,
        cfe_dir: &PathBuf,
        osal_dir: &PathBuf,
        psp_dir: &PathBuf,
        build_dir: &PathBuf,
    ) {
        self.add_header(header(cfe_dir, "modules/core_api/fsw/inc/cfe.h"));
        self.add_header(header(cfe_dir, "modules/core_api/fsw/inc/cfe_error.h"));
        self.add_header(header(osal_dir, "src/os/inc/common_types.h"));
        self.add_header(header(osal_dir, "src/os/inc/osapi.h"));
        self.add_header(header(psp_dir, "fsw/inc/cfe_psp.h"));

        for pat in [
            "CFE_.*", "OSAL_.*", "OS_.*", "CF_.*", "BPLib_.*", "BPLIB_.*",
        ] {
            self.add_pattern(pat.into());
        }

        self.collect_include_paths(cfe_dir, osal_dir, psp_dir, build_dir);
    }

    fn collect_include_paths(
        &mut self,
        cfe_dir: &PathBuf,
        osal_dir: &PathBuf,
        psp_dir: &PathBuf,
        build_dir: &PathBuf,
    ) {
        let paths = [
            build_dir.join("inc"),
            build_dir.join("native/default_cpu1/osal/inc"),
            build_dir.join("native/default_cpu1/psp/inc"),
            build_dir.join("native/default_cpu1/inc"),
            build_dir.join("i686-linux-gnu/default_cpu1/osal/inc"),
            build_dir.join("i686-linux-gnu/default_cpu1/psp/inc"),
            build_dir.join("i686-linux-gnu/default_cpu1/inc"),
            build_dir.join("amd64-nos3/default_cpu1/osal/inc"),
            build_dir.join("amd64-nos3/default_cpu1/psp/inc"),
            build_dir.join("amd64-nos3/default_cpu1/inc"),
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
            psp_dir.join("fsw/nos-linux/inc"),
        ];
        for path in paths {
            self.add_include(path);
        }
    }

    fn add_cf(&mut self, cf_dir: &PathBuf) {
        self.add_include(cf_dir.join("fsw/src"));
        self.add_include(cf_dir.join("fsw/inc"));
        self.add_include(cf_dir.join("config"));
        self.add_header(header(cf_dir, "fsw/src/cf_cfdp_pdu.h"));
        self.add_header(header(cf_dir, "fsw/src/cf_logical_pdu.h"));
        self.add_header(header(cf_dir, "fsw/src/cf_codec.h"));
        self.add_header(header(cf_dir, "fsw/src/cf_cfdp_types.h"));
        self.add_header(header(cf_dir, "fsw/src/cf_cfdp.h"));
        self.add_header(header(cf_dir, "fsw/src/cf_app.h"));
    }

    fn add_bplib(&mut self, bplib_dir: &PathBuf) {
        self.add_include(bplib_dir.join("inc"));
        for entry in walkdir::WalkDir::new(bplib_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() && entry.file_name() == "inc" {
                self.add_include(entry.path().to_path_buf());
            }
        }
        self.add_header(header(bplib_dir, "inc/bplib.h"));
    }

    fn add_bp(&mut self, bp_dir: &PathBuf) {
        self.add_include(bp_dir.join("fsw/inc"));
        self.add_include(bp_dir.join("config"));
    }

    #[cfg(feature = "nos3")]
    fn add_nos3(&mut self, hwlib_dir: &Option<PathBuf>, nos3_comp_dir: &Option<PathBuf>) {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let compat_h = out_dir.join("nos3_compat.h");
        fs::write(&compat_h, NOS3_COMPAT).unwrap();
        self.add_clang_arg("-include".into());
        self.add_clang_arg(compat_h.display().to_string());

        if let Some(ref hw) = hwlib_dir {
            self.add_include(hw.join("fsw/public_inc"));
            self.add_header(header(hw, "fsw/public_inc/hwlib.h"));
            for pat in [
                "uart_.*",
                "i2c_.*",
                "spi_.*",
                "can_.*",
                "gpio_.*",
                "socket_.*",
                "devmem_.*",
                "trq_.*",
                "HostToIp",
            ] {
                self.add_fn_pattern(pat.into());
            }
            for pat in [
                "uart_.*",
                "i2c_.*",
                "spi_.*",
                "can_.*",
                "gpio_.*",
                "socket_.*",
                "trq_.*",
                "canid_t",
                "addr_fam_e",
                "type_e",
                "category_e",
            ] {
                self.add_type_pattern(pat.into());
            }
            for pat in [
                "UART_.*",
                "I2C_.*",
                "SPI_.*",
                "CAN_.*",
                "GPIO_.*",
                "SOCKET_.*",
                "MEM_.*",
                "TRQ_.*",
                "PORT_.*",
                "NUM_.*",
                "HWLIB_.*",
            ] {
                self.add_var_pattern(pat.into());
            }
            self.blocklist_type("can_frame");
            self.add_raw_line("#[repr(C)]");
            self.add_raw_line("#[derive(Debug, Default, Copy, Clone)]");
            self.add_raw_line("pub struct can_frame {");
            self.add_raw_line("    pub can_id: u32,");
            self.add_raw_line("    pub can_dlc: u8,");
            self.add_raw_line("    pub __pad: u8,");
            self.add_raw_line("    pub __res0: u8,");
            self.add_raw_line("    pub __res1: u8,");
            self.add_raw_line("    pub data: [u8; 8usize],");
            self.add_raw_line("}");
        }

        if let Some(ref comp) = nos3_comp_dir {
            self.add_nos3_components(comp);
        }
    }

    #[cfg(feature = "nos3")]
    fn add_nos3_components(&mut self, comp_dir: &PathBuf) {
        for (dir, _, _) in NOS3_COMPONENTS {
            let base = comp_dir.join(dir);
            let standalone = base.join("fsw/standalone");
            let platform = base.join("fsw/cfs/platform_inc");
            let shared = base.join("fsw/shared");
            if standalone.exists() {
                self.add_clang_arg(format!("-I{}", standalone.display()));
            }
            if platform.exists() {
                self.add_clang_arg(format!("-I{}", platform.display()));
            }
            if shared.exists() {
                self.add_clang_arg(format!("-I{}", shared.display()));
            }
        }

        for (dir, prefix, _) in NOS3_COMPONENTS {
            let base = comp_dir.join(dir);
            let device_h = base.join(format!("fsw/shared/{}_device.h", prefix));
            let msg_h = base.join(format!("fsw/cfs/src/{}_msg.h", prefix));
            let utils_h = base.join(format!("fsw/shared/{}_utilities.h", prefix));
            let adac_h = base.join(format!("fsw/shared/{}_adac.h", prefix));
            if device_h.exists() {
                self.add_header(device_h.display().to_string());
            }
            if msg_h.exists() {
                self.add_header(msg_h.display().to_string());
            }
            if utils_h.exists() {
                self.add_header(utils_h.display().to_string());
            }
            if adac_h.exists() {
                self.add_header(adac_h.display().to_string());
            }
        }

        for (_, _, p) in NOS3_COMPONENTS {
            for alt in p.split('|') {
                let pat = format!("{}.*", alt);
                self.add_pattern(pat);
            }
        }
        for pat in [
            "GetCurrentMomentum",
            "SetRWTorque",
            "take_picture",
            "VoV",
            "VxV",
            "SxV",
            "MAGV",
            "UNITV",
            "CopyUnitV",
            "QxQ",
            "QxQT",
            "QxV",
            "QTxV",
            "UNITQ",
            "RECTIFYQ",
            "arccos",
            "Limit",
        ] {
            self.add_fn_pattern(pat.into());
        }
    }

    fn generate(&self, macro_detector: &MacroDetector) -> String {
        let mut builder = Builder::default()
            .default_visibility(FieldVisibilityKind::PublicCrate)
            .use_core()
            .ctypes_prefix("libc")
            .clang_arg("-D_LINUX_OS_")
            .clang_arg("-D_POSIX_OS_")
            .layout_tests(false)
            .derive_default(true)
            .derive_debug(true)
            .parse_callbacks(Box::new(macro_detector.clone()));

        for h in &self.headers {
            builder = builder.header(h);
        }

        for path in &self.include_paths {
            builder = builder.clang_arg(format!("-I{}", path.display()));
        }

        for arg in &self.clang_args {
            builder = builder.clang_arg(arg);
        }

        for line in &self.raw_lines {
            builder = builder.raw_line(line);
        }

        for ty in &self.blocklist_types {
            builder = builder.blocklist_type(ty);
        }

        builder = builder
            .allowlist_function(&self.fn_patterns.join("|"))
            .allowlist_type(&self.type_patterns.join("|"))
            .allowlist_var(&self.var_patterns.join("|"));

        let bindings = builder.generate().expect("Unable to generate bindings!");
        bindings.to_string()
    }
}

const NOS3_FALLBACKS: &[(&str, &str)] = &[
    ("CFE_ES_CrcType_Enum_CFE_ES_CrcType_16_ARC",
     "pub(crate) const CFE_ES_CrcType_Enum_CFE_ES_CrcType_16_ARC: u32 = 2;"),
    ("CFE_ES_CrcType_Enum_t",
     "pub(crate) type CFE_ES_CrcType_Enum_t = u32;"),
    ("CFE_MISSION_TBL_MAX_FULL_NAME_LEN",
     "pub(crate) const CFE_MISSION_TBL_MAX_FULL_NAME_LEN: u32 = OS_MAX_API_NAME + CFE_MISSION_MAX_API_LEN + 4;"),
    ("CFE_PLATFORM_CMD_MID_BASE",
     "pub(crate) const CFE_PLATFORM_CMD_MID_BASE: CFE_SB_MsgId_Atom_t = 0x1800;"),
    ("CFE_PLATFORM_TLM_MID_BASE",
     "pub(crate) const CFE_PLATFORM_TLM_MID_BASE: CFE_SB_MsgId_Atom_t = 0x0800;"),
    ("CFE_PLATFORM_CMD_MID_BASE_GLOB",
     "pub(crate) const CFE_PLATFORM_CMD_MID_BASE_GLOB: CFE_SB_MsgId_Atom_t = 0x1860;"),
    ("CFE_PLATFORM_TLM_MID_BASE_GLOB",
     "pub(crate) const CFE_PLATFORM_TLM_MID_BASE_GLOB: CFE_SB_MsgId_Atom_t = 0x0880;"),
    ("CFE_SB_CmdTopicIdToMsgId", indoc! {"
        #[inline]
        pub(crate) unsafe fn CFE_SB_CmdTopicIdToMsgId(topic_id: u16, _instance: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_PLATFORM_CMD_MID_BASE + topic_id as CFE_SB_MsgId_Atom_t
        }
        #[inline]
        pub(crate) unsafe fn CFE_SB_TlmTopicIdToMsgId(topic_id: u16, _instance: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_PLATFORM_TLM_MID_BASE + topic_id as CFE_SB_MsgId_Atom_t
        }
        #[inline]
        pub(crate) unsafe fn CFE_SB_GlobalCmdTopicIdToMsgId(topic_id: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_PLATFORM_CMD_MID_BASE_GLOB + topic_id as CFE_SB_MsgId_Atom_t
        }
        #[inline]
        pub(crate) unsafe fn CFE_SB_GlobalTlmTopicIdToMsgId(topic_id: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_PLATFORM_TLM_MID_BASE_GLOB + topic_id as CFE_SB_MsgId_Atom_t
        }
        #[inline]
        pub(crate) unsafe fn CFE_SB_LocalCmdTopicIdToMsgId(topic_id: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_SB_CmdTopicIdToMsgId(topic_id, 0)
        }
        #[inline]
        pub(crate) unsafe fn CFE_SB_LocalTlmTopicIdToMsgId(topic_id: u16) -> CFE_SB_MsgId_Atom_t {
            CFE_SB_TlmTopicIdToMsgId(topic_id, 0)
        }
    "}),
];

/// Appends Rust definitions for symbols missing in the NOS3 cFE fork.
fn inject_fallbacks(output: &mut String) {
    for (symbol, code) in NOS3_FALLBACKS {
        if !output.contains(symbol) {
            output.push('\n');
            output.push_str(code);
        }
    }
}

/// Converts `#define` macros that bindgen couldn't parse into Rust constants.
fn recover_skipped_macros(skipped: &[String], include_paths: &[PathBuf]) -> String {
    let mut result = String::new();
    result.push_str(&format!("// Skipped by bindgen: {}\n", skipped.len()));

    if skipped.is_empty() {
        return result;
    }

    let skipped_set: HashSet<&str> = skipped.iter().map(String::as_str).collect();
    let locations = find_macro_definitions(&skipped_set, include_paths);
    let mut sorted_files: Vec<_> = locations.keys().collect();
    sorted_files.sort();

    for file_path in sorted_files {
        let Some(definitions) = locations.get(file_path) else {
            continue;
        };
        result.push_str(&format!("// File: {}\n", file_path.display()));
        for def in definitions {
            if let Some(raw_comment) = &def.doc_comment {
                let formatted_doc = format_doc_comment(raw_comment);
                let prefix = if parse_simple_cast_macro(&def.value).is_some() {
                    ""
                } else {
                    "// "
                };
                result.push_str(&format!("{}#[doc = \"{}\"]\n", prefix, formatted_doc));
            }
            if let Some((ty, val)) = parse_simple_cast_macro(&def.value) {
                result.push_str(&format!("pub const {}: {} = {};\n", def.name, ty, val));
            } else {
                result.push_str(&format!(
                    "// pub const {}: /* ? */ = /* {} */;\n",
                    def.name, def.value
                ));
            }
        }
    }
    result
}

/// Attaches C doc comments to constants that bindgen successfully converted.
fn inject_doc_comments(
    output: &mut String,
    converted: &HashSet<String>,
    include_paths: &[PathBuf],
) {
    if converted.is_empty() {
        return;
    }
    let converted_set: HashSet<&str> = converted.iter().map(String::as_str).collect();
    let docs = find_macro_definitions(&converted_set, include_paths);

    for (_path, defs) in docs {
        for def in defs {
            let Some(raw_comment) = def.doc_comment else {
                continue;
            };
            let formatted_doc = format_doc_comment(&raw_comment);
            let re = Regex::new(&format!(r"(?m)^pub const {}:", escape(&def.name))).unwrap();
            let replacement = format!("#[doc = \"{}\"]\npub const {}:", formatted_doc, def.name);
            *output = re.replace(output, replacement).to_string();
        }
    }
}

/// Rewrites `pub` to `pub(crate)` so bindings don't leak from the crate.
fn restrict_visibility(content: &str) -> String {
    content
        .replace("pub fn", "pub(crate) fn")
        .replace("pub type", "pub(crate) type")
        .replace("pub const", "pub(crate) const")
        .replace("pub use", "pub(crate) use")
        .replace("pub struct", "pub(crate) struct")
        .replace("pub union", "pub(crate) union")
        .replace("pub enum", "pub(crate) enum")
}

/// Applies fallbacks, doc comments, skipped-macro recovery, and visibility restriction.
fn postprocess_bindings(
    bindings_str: &str,
    macro_detector: &MacroDetector,
    include_paths: &[PathBuf],
) -> String {
    let mut output = bindings_str.to_string();

    let all = macro_detector.all_potential_macros.borrow();
    let converted = macro_detector.converted_macros.borrow();
    let skipped: Vec<_> = all.difference(&converted).cloned().collect();

    inject_fallbacks(&mut output);
    inject_doc_comments(&mut output, &converted, include_paths);

    let skipped_header = recover_skipped_macros(&skipped, include_paths);
    restrict_visibility(&format!("{}\n{}", skipped_header, output))
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let cfe_dir = get_path("CFE_DIR");
    let osal_dir = get_path("OSAL_DIR");
    let psp_dir = get_path("PSP_DIR");
    let build_dir = get_path("BUILD_DIR");
    let cf_dir = env::var("CF_DIR").ok().map(PathBuf::from);
    let bp_dir = env::var("BP_DIR").ok().map(PathBuf::from);
    let bplib_dir = env::var("BPLIB_DIR").ok().map(PathBuf::from);
    #[cfg(feature = "nos3")]
    let hwlib_dir = env::var("HWLIB_DIR").ok().map(PathBuf::from);
    #[cfg(feature = "nos3")]
    let nos3_comp_dir = env::var("NOS3_COMPONENTS_DIR").ok().map(PathBuf::from);
    let debug = env::var("DEBUG").as_deref() == Ok("1");

    if debug {
        println!("cargo::warning=CFE_DIR={}", cfe_dir.display());
        println!("cargo::warning=OSAL_DIR={}", osal_dir.display());
        println!("cargo::warning=PSP_DIR={}", psp_dir.display());
        println!("cargo::warning=BUILD_DIR={}", build_dir.display());
        if let Some(ref cf) = cf_dir {
            println!("cargo::warning=CF_DIR={}", cf.display());
        }
        if let Some(ref bp) = bp_dir {
            println!("cargo::warning=BP_DIR={}", bp.display());
        }
        if let Some(ref bplib) = bplib_dir {
            println!("cargo::warning=BPLIB_DIR={}", bplib.display());
        }
        #[cfg(feature = "nos3")]
        if let Some(ref hw) = hwlib_dir {
            println!("cargo::warning=HWLIB_DIR={}", hw.display());
        }
    }

    let macro_detector = MacroDetector::default();

    let mut config = BindgenConfig::new();
    config.add_cfe(&cfe_dir, &osal_dir, &psp_dir, &build_dir);

    if let Some(ref cf) = cf_dir {
        config.add_cf(cf);
    }

    if let Some(ref bplib) = bplib_dir {
        config.add_bplib(bplib);
    }

    if let Some(ref bp) = bp_dir {
        config.add_bp(bp);
    }

    if let Ok(sysroot) = env::var("SYSROOT") {
        config.add_clang_arg(format!("--sysroot={}", sysroot));
    }

    #[cfg(feature = "nos3")]
    config.add_nos3(&hwlib_dir, &nos3_comp_dir);

    let bindings_str = config.generate(&macro_detector);

    // Detect NOS3 cFE fork (missing upstream OSAL/cFE symbols)
    if !bindings_str.contains("OS_TimeToRelativeMilliseconds") {
        println!("cargo:rustc-cfg=nos3_cfe");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_file = out_dir.join("bindings.rs");
    let final_content = postprocess_bindings(&bindings_str, &macro_detector, &config.include_paths);
    fs::write(&out_file, final_content).expect("Couldn't write bindings");
}

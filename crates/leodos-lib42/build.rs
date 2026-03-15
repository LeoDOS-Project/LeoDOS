use bindgen::callbacks::{IntKind, MacroParsingBehavior, ParseCallbacks};
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

/// 42 uses simple uppercase #defines for constants (planet IDs, mode
/// enums, etc.). We want to capture all of them from 42defines.h.
fn is_api_macro(name: &str) -> bool {
    // Skip header guards
    if name.starts_with("__") && name.ends_with("__") {
        return false;
    }
    if name.ends_with("_H") || name.ends_with("_H_") {
        return false;
    }
    // Skip standard C macros
    if matches!(
        name,
        "TRUE" | "FALSE" | "ON" | "OFF" | "POSITIVE" | "NEGATIVE"
    ) {
        return false;
    }
    // Must be uppercase/digits/underscores
    name.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && name.len() > 1
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
            if let Some(stripped) = cleaned.strip_prefix("/*") {
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
    let trailing_comment_re = Regex::new(r"/\*\*?<?\s*(.*?)\s*\*/").unwrap();

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

                    if trimmed_line.starts_with("/*") && !trimmed_line.contains("/*<") {
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
                                doc = Some(
                                    trailing_caps.get(1).unwrap().as_str().trim().to_string(),
                                );
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
    let re = Regex::new(
        r"^\s*\(\s*\(\s*([a-zA-Z0-9_]+)\s*\)\s*(-?(?:0x[0-9a-fA-F]+|[0-9]+))\s*\)\s*$",
    )
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
    println!("cargo:rerun-if-env-changed=FORTYTWO_DIR");

    let forty_two_dir = PathBuf::from(
        env::var("FORTYTWO_DIR").expect("FORTYTWO_DIR not set — configure it in .cargo/config.toml"),
    )
    .canonicalize()
    .expect("FORTYTWO_DIR does not point to a valid directory");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!(
        "cargo:rerun-if-changed={}",
        forty_two_dir.join("Include").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        forty_two_dir.join("Kit/Include").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        forty_two_dir.join("Source").display()
    );

    // --- Compile 42 as a static library ---

    let kit_sources: Vec<&str> = vec![
        "Kit/Source/dcmkit.c",
        "Kit/Source/envkit.c",
        "Kit/Source/fswkit.c",
        "Kit/Source/iokit.c",
        "Kit/Source/mathkit.c",
        "Kit/Source/meshkit.c",
        "Kit/Source/msis86kit.c",
        "Kit/Source/nrlmsise00kit.c",
        "Kit/Source/orbkit.c",
        "Kit/Source/radbeltkit.c",
        "Kit/Source/sigkit.c",
        "Kit/Source/sphkit.c",
        "Kit/Source/timekit.c",
    ];

    let sim_sources: Vec<&str> = vec![
        "Source/42exec.c",
        "Source/42actuators.c",
        "Source/42cmd.c",
        "Source/42dynamics.c",
        "Source/42environs.c",
        "Source/42ephem.c",
        "Source/42fsw.c",
        "Source/42init.c",
        "Source/42ipc.c",
        "Source/42jitter.c",
        "Source/42joints.c",
        "Source/42optics.c",
        "Source/42perturb.c",
        "Source/42report.c",
        "Source/42sensors.c",
        "Source/42nos3.c",
        "Source/AcApp.c",
        "Source/AutoCode/WriteAcToCsv.c",
        "Source/AutoCode/WriteScToCsv.c",
        "Source/AutoCode/TxRxIPC.c",
    ];

    let mut cc_build = cc::Build::new();
    cc_build
        .std("c11")
        .warnings(false)
        .include(forty_two_dir.join("Include"))
        .include(forty_two_dir.join("Kit/Include"))
        .include(forty_two_dir.join("Kit/Source"));

    for src in &kit_sources {
        cc_build.file(forty_two_dir.join(src));
    }
    for src in &sim_sources {
        cc_build.file(forty_two_dir.join(src));
    }

    cc_build.compile("fortytwo");

    // --- Generate bindings ---

    let macro_detector = MacroDetector::default();

    let include_paths = vec![
        forty_two_dir.join("Include"),
        forty_two_dir.join("Kit/Include"),
        forty_two_dir.join("Kit/Source"),
    ];

    // We generate a wrapper header to control exactly what gets
    // included (42.h pulls in everything via EXTERN globals).
    let wrapper = out_dir.join("wrapper.h");
    fs::write(
        &wrapper,
        r#"
/* Kit headers (standalone utilities) */
#include "dcmkit.h"
#include "envkit.h"
#include "fswkit.h"
#include "iokit.h"
#include "mathkit.h"
#include "meshkit.h"
#include "orbkit.h"
#include "sigkit.h"
#include "sphkit.h"
#include "timekit.h"

/* 42 type definitions */
#include "42defines.h"
#include "42types.h"
#include "AcTypes.h"

/* 42 simulation API — declare globals as extern */
#include "42.h"
"#,
    )
    .unwrap();

    // Build the allowlist of 42-specific functions.
    // Kit functions (mathkit, dcmkit, orbkit, timekit, envkit, sigkit, etc.)
    let kit_functions = [
        // dcmkit
        "C2Q", "Q2C", "A2C", "C2A", "SimpRot", "Q2AngleVec", "QW2QDOT",
        "PARAXIS", "PrincipalMOI", "Q2W", "JointPartials", "ADOT2W",
        "W2ADOT", "W2CDOT", "CDOT2W",
        // mathkit
        "signum", "sinc", "MxM", "MxMT", "MTxM", "MTxMT", "VxM", "MxV",
        "VxMT", "MTxV", "SxV", "SxM", "MINV4", "MINV3", "MINV2", "PINV4x3",
        "MT", "VoV", "VxV", "vxMov", "MAGV", "UNITV", "CopyUnitV",
        "V2CrossM", "V2DoubleCrossM", "VcrossM", "VcrossMT",
        "QxQ", "QTxQ", "QxQT", "VxQ", "QxV", "QTxV", "UNITQ", "RECTIFYQ",
        "PerpBasis", "fact", "oddfact", "Legendre", "SphericalHarmonics",
        "MxMG", "MxMTG", "MTxMG", "MxVG", "SxMG", "MINVG", "FastMINV6",
        "PINVG", "CreateMatrix", "DestroyMatrix", "LINSOLVE",
        "CholeskySolve", "ConjGradSolve", "Bairstow", "Amoeba",
        "FindNormal", "LinInterp", "SphereInterp",
        "CubicInterp1D", "CubicInterp2D", "CubicInterp3D",
        "DistanceToLine", "ProjectPointOntoPoly", "ProjectPointOntoTriangle",
        "CubicSpline", "ChebyPolys", "ChebyInterp", "FindChebyCoefs",
        "VecToLngLat",
        // timekit
        "TimeToJD", "JDToTime", "DateToTime", "DateToJD", "JDToDate",
        "TimeToDate", "MD2DOY", "DOY2MD", "JD2GMST", "GpsTimeToGpsDate",
        "GpsDateToGpsTime", "usec", "RealSystemTime", "RealRunTime",
        // orbkit
        "Eph2RV", "RV2Eph", "MeanEph2RV", "MeanAnomToTrueAnom",
        "TrueAnomaly", "TimeSincePeriapsis", "FindCLN", "FindCEN",
        "FindENU", "TLE2MeanEph", "LoadTleFromFile", "PlanetEphemerides",
        "LunaPosition", "LunaInertialFrame", "LunaPriMerAng",
        "TETE2J2000", "SimpleEarthPrecNute", "HiFiEarthPrecNute",
        "FindLagPtParms", "FindLagPtPosVel", "LagModes2RV", "RV2LagModes",
        "AmpPhase2LagModes", "XYZ2LagModes", "R2StableLagMode",
        "EHRV2EHModes", "EHModes2EHRV", "EHRV2RelRV", "RelRV2EHRV",
        "RV02RV", "RV2RVp", "LambertProblem", "LambertTOF",
        "PlanTwoImpulseRendezvous", "RendezvousCostFunction",
        "RadiusOfInfluence", "CloneOrbit", "TDRSPosVel",
        "MeanEphToOscEph", "OscEphToMeanEph", "FindLightLagOffsets",
        // envkit
        "EGM96", "GMM2B", "GLGM2", "IGRFMagField", "DipoleMagField",
        "KpToAp", "NRLMSISE00", "SimpleMSIS", "JacchiaRoberts",
        "MarsAtmosphereModel", "WGS84ToECEF", "ECEFToWGS84",
        "PolyhedronGravAcc", "PolyhedronGravGrad", "GravGradTimesInertia",
        // sigkit
        "CreateRandomProcess", "DestroyRandomProcess",
        "UniformRandom", "GaussianRandom", "PRN2D", "PRN3D",
        "CreateFirstOrderLowpassFilter", "CreateSecondOrderLowpassFilter",
        "CreateFirstOrderHighpassFilter", "CreateSecondOrderHighpassFilter",
        "CreateGeneralFilter", "DestroyFilter",
        "FirstOrderLowpassFilter", "SecondOrderLowpassFilter",
        "FirstOrderHighpassFilter", "SecondOrderHighpassFilter",
        "GeneralFilter", "CreateDelay", "ResizeDelay", "Delay",
        "Step", "Clamp", "RampStep", "CubicStep",
        // fswkit
        "AcFsw",
        // iokit
        "FileOpen", "FileToString", "InitSocketServer", "InitSocketClient",
    ];

    // 42 simulation functions
    let sim_functions = [
        "SimStep", "Ephemerides", "OrbitMotion", "Environment",
        "Perturbations", "Sensors", "SensorDriver", "FlightSoftWare",
        "ActuatorDriver", "Actuators", "CmdInterpreter", "Report",
        "Dynamics", "Cleanup", "ThreeBodyOrbitRK4", "MotionConstraints",
        "SCMassProps", "MapJointStatesToStateVector",
        "MapStateVectorToBodyStates", "BodyStatesToNodeStates",
        "PartitionForces", "FindInterBodyDCMs", "FindPathVectors",
        "FindTotalAngMom", "FindTotalKineticEnergy", "UpdateScBoundingBox",
        "FindUnshadedAreas", "RadBelt", "InitAlbedo", "FindCssAlbedo",
        "FindFssAlbedo", "JointFrcTrq", "InitActuatedJoint",
        "WheelJitter", "ShakerJitter", "OpticalFieldPoint", "OpticalTrain",
        "InitSim", "InitOrbits", "InitSpacecraft", "LoadPlanets",
        "LoadJplEphems", "DecodeString", "InitFSW", "InitAC",
        "InitLagrangePoints", "LoadTRVfromFile", "SplineToPosVel",
        "CfdSlosh", "FakeCfdSlosh", "NOS3Time",
        "InterProcessComm", "InitInterProcessComm",
    ];

    let fn_pattern = kit_functions.iter()
        .chain(sim_functions.iter())
        .map(|s| format!("^{}$", s))
        .collect::<Vec<_>>()
        .join("|");

    // 42 simulation globals from 42.h and msis86kit.h.
    // Listed explicitly to avoid pulling in thousands of
    // system constants from macOS SDK headers.
    let sim_globals = [
        // 42.h — counts and paths
        "Norb", "Nsc", "Nmatl", "Nmesh", "InOutPath", "ModelPath",
        "CmdFileName",
        // 42.h — math constants
        "Pi", "TwoPi", "HalfPi", "SqrtTwo", "SqrtHalf",
        "D2R", "R2D", "A2R", "R2A", "GoldenRatio",
        // 42.h — time
        "TimeMode", "SimTime", "STOPTIME", "DTSIM", "DTOUT", "DTOUTGL",
        "OutFlag", "GLOutFlag", "GLEnable", "CleanUpFlag",
        "DynTime0", "DynTime", "AtomicTime", "LeapSec", "CivilTime",
        "GpsTime", "TT", "UTC", "GpsRollover", "GpsWeek", "GpsSecond",
        // 42.h — environment models
        "MagModel", "SurfaceModel", "EarthGravModel", "MarsGravModel",
        "LunaGravModel", "AtmoOption", "Flux10p7", "GeomagIndex",
        "SchattenTable",
        // 42.h — perturbation flags
        "AeroActive", "AeroShadowsActive", "GGActive",
        "SolPressActive", "SolPressShadowsActive", "GravPertActive",
        "ThrusterPlumesActive", "ResidualDipoleActive", "ContactActive",
        "SloshActive", "AlbedoActive", "ComputeEnvTrq", "EphemOption",
        // 42.h — world, orbit, spacecraft
        "World", "LagSys", "CGH", "qjh", "Orb", "SC", "Frm", "Mesh",
        "POV", "Tdrs", "GroundStation", "Ngnd", "Nfov", "FOV", "Matl",
        "ShadowMap", "AlbedoFBO",
        // 42.h — misc
        "Nmb", "Nrgn", "Rgn", "ExecuteCFDStep", "EndCFD",
        "Nipc", "IPC", "RNG", "RngSeed", "Constell",
        // 42.h — profiling timers
        "MapTime", "JointTime", "PathTime", "PVelTime", "FrcTrqTime",
        "AssembleTime", "LockTime", "TriangleTime", "SubstTime",
        "SolveTime",
        // msis86kit.h
        "gts3c_86", "csw_86", "fit_86", "lsqv_86", "lpoly_86",
        "parmb_86", "lower5_", "parm5_",
    ];

    let var_pattern = sim_globals.iter()
        .map(|s| format!("^{}$", s))
        .collect::<Vec<_>>()
        .join("|");

    let mut builder = bindgen::Builder::default()
        .header(wrapper.to_str().unwrap())
        .default_visibility(bindgen::FieldVisibilityKind::Public)
        .use_core()
        .ctypes_prefix("libc")
        .allowlist_function(&fn_pattern)
        .allowlist_type(".*Type")
        .allowlist_var(&var_pattern)
        .layout_tests(false)
        .derive_default(true)
        .parse_callbacks(Box::new(macro_detector.clone()));

    for path in &include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder.generate().expect("Unable to generate 42 bindings");
    let mut bindings_str = bindings.to_string();

    // --- Handle skipped macros ---

    let all = macro_detector.all_potential_macros.borrow();
    let converted = macro_detector.converted_macros.borrow();
    let skipped_macros: Vec<_> = all.difference(&converted).cloned().collect();

    let mut preamble = String::new();
    preamble.push_str("// 42 bindings generated by leodos-42 build.rs\n");
    preamble.push_str(&format!(
        "// Skipped macros: {}\n",
        skipped_macros.len()
    ));

    if !skipped_macros.is_empty() {
        let skipped_set: HashSet<&str> = skipped_macros.iter().map(String::as_str).collect();
        let locations = find_macro_definitions(&skipped_set, &include_paths);
        let mut sorted_files: Vec<_> = locations.keys().collect();
        sorted_files.sort();

        for file_path in sorted_files {
            if let Some(definitions) = locations.get(file_path) {
                preamble.push_str("//\n");
                preamble.push_str(&format!("// File: {}\n", file_path.display()));
                preamble.push_str("// ----------------------------------------\n");
                for def in definitions {
                    if let Some((ty, val)) = parse_simple_cast_macro(&def.value) {
                        if let Some(raw_comment) = &def.doc_comment {
                            let formatted_doc = format_doc_comment(raw_comment);
                            preamble.push_str(&format!("#[doc = \"{}\"]\n", formatted_doc));
                        }
                        preamble
                            .push_str(&format!("pub const {}: {} = {};\n", def.name, ty, val));
                    } else {
                        if let Some(raw_comment) = &def.doc_comment {
                            let formatted_doc = format_doc_comment(raw_comment);
                            preamble
                                .push_str(&format!("// #[doc = \"{}\"]\n", formatted_doc));
                        }
                        preamble.push_str(&format!(
                            "// pub const {}: /* ? */ = /* {} */;\n",
                            def.name, def.value
                        ));
                    }
                }
            }
        }
    }

    // Inject doc comments for converted macros
    if !converted.is_empty() {
        let converted_set: HashSet<&str> = converted.iter().map(String::as_str).collect();
        let converted_docs = find_macro_definitions(&converted_set, &include_paths);

        for (_path, defs) in converted_docs {
            for def in defs {
                if let Some(raw_comment) = def.doc_comment {
                    let formatted_doc = format_doc_comment(&raw_comment);
                    let re =
                        Regex::new(&format!(r"(?m)^pub const {}:", regex::escape(&def.name)))
                            .unwrap();
                    let replacement =
                        format!("#[doc = \"{}\"]\npub const {}:", formatted_doc, def.name);
                    bindings_str = re.replace(&bindings_str, replacement).to_string();
                }
            }
        }
    }

    let final_content = format!("{}\n{}", preamble, bindings_str);
    let out_file = out_dir.join("bindings.rs");
    fs::write(&out_file, final_content).expect("Couldn't write bindings");

    // Raw FFI is exposed via `pub mod ffi` — no visibility
    // reduction needed. Safe wrappers in sibling modules are
    // the preferred API.
}

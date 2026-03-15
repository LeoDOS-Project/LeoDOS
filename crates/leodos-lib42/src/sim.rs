extern crate alloc;

use alloc::vec::Vec;
use crate::ffi;
use core::ffi::CStr;

/// Spacecraft state (configuration, bodies, joints, sensors, actuators).
pub use crate::ffi::SCType;
/// Orbital elements and ephemeris state.
pub use crate::ffi::OrbitType;
/// Celestial body / world parameters.
pub use crate::ffi::WorldType;
/// Rigid body within a spacecraft.
pub use crate::ffi::BodyType;
/// Joint connecting two rigid bodies.
pub use crate::ffi::JointType;
/// Attitude control system state.
pub use crate::ffi::AcType;
/// Reaction wheel parameters and state.
pub use crate::ffi::WhlType;
/// Magnetic torque bar parameters and state.
pub use crate::ffi::MTBType;
/// Thruster parameters and state.
pub use crate::ffi::ThrType;
/// Gyroscope sensor model.
pub use crate::ffi::GyroType;
/// Magnetometer sensor model.
pub use crate::ffi::MagnetometerType;
/// Coarse sun sensor model.
pub use crate::ffi::CssType;
/// Fine sun sensor model.
pub use crate::ffi::FssType;
/// Star tracker sensor model.
pub use crate::ffi::StarTrackerType;
/// GPS receiver model.
pub use crate::ffi::GpsType;
/// Accelerometer sensor model.
pub use crate::ffi::AccelType;
/// Inter-process communication handle.
pub use crate::ffi::IpcType;
/// Dynamics configuration and state.
pub use crate::ffi::DynType;
/// Multi-spacecraft formation parameters.
pub use crate::ffi::FormationType;
/// Lagrange-point system definition.
pub use crate::ffi::LagrangeSystemType;
/// Spherical harmonic gravity model.
pub use crate::ffi::SphereHarmType;
/// Calendar / Julian date representation.
pub use crate::ffi::DateType;
/// Surface mesh for rendering or analysis.
pub use crate::ffi::MeshType;
/// Finite-element node on a flexible body.
pub use crate::ffi::NodeType;
/// Jitter source (vibration shaker) model.
pub use crate::ffi::ShakerType;
/// Optical element (lens, mirror, detector).
pub use crate::ffi::OpticsType;
/// Field-of-view cone / polygon definition.
pub use crate::ffi::FovType;
/// Surface material optical properties.
pub use crate::ffi::MatlType;
/// Stochastic random process model.
pub use crate::ffi::RandomProcessType;
/// Discrete-time filter state.
pub use crate::ffi::FilterType;
/// Transport delay buffer.
pub use crate::ffi::DelayType;
/// Command / script interpreter state.
pub use crate::ffi::CmdType;
/// Point-of-view (camera) parameters.
pub use crate::ffi::POVType;
/// Environmental torque breakdown.
pub use crate::ffi::EnvTrqType;
/// Kalman filter state and covariance.
pub use crate::ffi::KalmanFilterType;

// --- Simulation lifecycle ---

/// Initialize the 42 simulator with command-line arguments.
pub fn init_sim(args: &[&CStr]) {
    let mut ptrs: Vec<*mut libc::c_char> = args
        .iter()
        .map(|s| s.as_ptr().cast_mut())
        .collect();
    unsafe { ffi::InitSim(ptrs.len() as libc::c_int, ptrs.as_mut_ptr()) }
}

/// Advance the simulation by one time step. Returns `false` when done.
pub fn sim_step() -> bool {
    unsafe { ffi::SimStep() == 0 }
}

/// Release all simulator resources.
pub fn cleanup() {
    unsafe { ffi::Cleanup() }
}

// --- Initialization ---

/// Initialize all orbit states from input files.
pub fn init_orbits() {
    unsafe { ffi::InitOrbits() }
}

/// Load planetary ephemeris and physical parameters.
pub fn load_planets() {
    unsafe { ffi::LoadPlanets() }
}

/// Initialize Lagrange-point orbit systems.
pub fn init_lagrange_points() {
    unsafe { ffi::InitLagrangePoints() }
}

/// Initialize Earth albedo model tables.
pub fn init_albedo() {
    unsafe { ffi::InitAlbedo() }
}

/// Initialize a spacecraft's bodies, joints, and sensors.
pub fn init_spacecraft(sc: &mut SCType) {
    unsafe { ffi::InitSpacecraft(sc) }
}

/// Initialize the flight software model for a spacecraft.
pub fn init_fsw(sc: &mut SCType) {
    unsafe { ffi::InitFSW(sc) }
}

/// Initialize the attitude control system for a spacecraft.
pub fn init_ac(sc: &mut SCType) {
    unsafe { ffi::InitAC(sc) }
}

/// Load JPL planetary ephemerides from `path` at Julian date `jd`.
pub fn load_jpl_ephems(path: &CStr, jd: f64) -> i64 {
    unsafe { ffi::LoadJplEphems(path.as_ptr().cast_mut(), jd) }
}

/// Decode a configuration string into a numeric identifier.
pub fn decode_string(s: &CStr) -> i64 {
    unsafe { ffi::DecodeString(s.as_ptr().cast_mut()) }
}

// --- Simulation step functions ---

/// Update planetary and lunar ephemerides for the current time.
pub fn ephemerides() {
    unsafe { ffi::Ephemerides() }
}

/// Propagate all orbits forward to the given simulation `time`.
pub fn orbit_motion(time: f64) {
    unsafe { ffi::OrbitMotion(time) }
}

/// Compute environmental forces and torques on a spacecraft.
pub fn environment(sc: &mut SCType) {
    unsafe { ffi::Environment(sc) }
}

/// Compute orbital and attitude perturbations on a spacecraft.
pub fn perturbations(sc: &mut SCType) {
    unsafe { ffi::Perturbations(sc) }
}

/// Simulate all sensor measurements for a spacecraft.
pub fn sensors(sc: &mut SCType) {
    unsafe { ffi::Sensors(sc) }
}

/// Run the sensor hardware driver models for a spacecraft.
pub fn sensor_driver(sc: &mut SCType) {
    unsafe { ffi::SensorDriver(sc) }
}

/// Execute the flight software control loop for a spacecraft.
pub fn flight_software(sc: &mut SCType) {
    unsafe { ffi::FlightSoftWare(sc) }
}

/// Run the actuator hardware driver models for a spacecraft.
pub fn actuator_driver(sc: &mut SCType) {
    unsafe { ffi::ActuatorDriver(sc) }
}

/// Apply actuator commands (wheels, thrusters, torquers).
pub fn actuators(sc: &mut SCType) {
    unsafe { ffi::Actuators(sc) }
}

/// Integrate equations of motion for a spacecraft.
pub fn dynamics(sc: &mut SCType) {
    unsafe { ffi::Dynamics(sc) }
}

/// Process any pending script / command interpreter inputs.
pub fn cmd_interpreter() {
    unsafe { ffi::CmdInterpreter() }
}

/// Write telemetry and state output for the current time step.
pub fn report() {
    unsafe { ffi::Report() }
}

// --- Spacecraft utilities ---

/// Recompute composite mass, center of mass, and inertia tensor.
pub fn sc_mass_props(sc: &mut SCType) {
    unsafe { ffi::SCMassProps(sc) }
}

/// Pack joint angles and rates into the dynamics state vector.
pub fn map_joint_states_to_state_vector(sc: &mut SCType) {
    unsafe { ffi::MapJointStatesToStateVector(sc) }
}

/// Propagate rigid-body states to finite-element node states.
pub fn body_states_to_node_states(sc: &mut SCType) {
    unsafe { ffi::BodyStatesToNodeStates(sc) }
}

/// Partition applied forces into translational and rotational parts.
pub fn partition_forces(sc: &mut SCType) {
    unsafe { ffi::PartitionForces(sc) }
}

/// Compute direction cosine matrices between all body pairs.
pub fn find_inter_body_dcms(sc: &mut SCType) {
    unsafe { ffi::FindInterBodyDCMs(sc) }
}

/// Compute position vectors along the kinematic tree.
pub fn find_path_vectors(sc: &mut SCType) {
    unsafe { ffi::FindPathVectors(sc) }
}

/// Compute total angular momentum of the spacecraft system.
pub fn find_total_ang_mom(sc: &mut SCType) {
    unsafe { ffi::FindTotalAngMom(sc) }
}

/// Compute total kinetic energy of the spacecraft system.
pub fn find_total_kinetic_energy(sc: &mut SCType) -> f64 {
    unsafe { ffi::FindTotalKineticEnergy(sc) }
}

/// Update the axis-aligned bounding box for the spacecraft.
pub fn update_sc_bounding_box(sc: &mut SCType) {
    unsafe { ffi::UpdateScBoundingBox(sc) }
}

/// Find unshaded surface areas along the given direction vector.
pub fn find_unshaded_areas(sc: &mut SCType, dir_vec_n: &mut [f64; 3]) {
    unsafe { ffi::FindUnshadedAreas(sc, dir_vec_n.as_mut_ptr()) }
}

/// Apply joint motion constraints (locked, prescribed, etc.).
pub fn motion_constraints(sc: &mut SCType) {
    unsafe { ffi::MotionConstraints(sc) }
}

/// Propagate a three-body orbit one step using RK4 integration.
pub fn three_body_orbit_rk4(orb: &mut OrbitType) {
    unsafe { ffi::ThreeBodyOrbitRK4(orb) }
}

/// Interpolate spline ephemeris to position and velocity.
pub fn spline_to_pos_vel(orb: &mut OrbitType) {
    unsafe { ffi::SplineToPosVel(orb) }
}

// --- Sensor/actuator helpers ---

/// Compute Earth albedo contribution for a coarse sun sensor.
pub fn find_css_albedo(sc: &mut SCType, css: &mut CssType) {
    unsafe { ffi::FindCssAlbedo(sc, css) }
}

/// Compute Earth albedo contribution for a fine sun sensor.
pub fn find_fss_albedo(sc: &mut SCType, fss: &mut FssType) {
    unsafe { ffi::FindFssAlbedo(sc, fss) }
}

/// Compute joint reaction forces and torques.
pub fn joint_frc_trq(joint: &mut JointType, sc: &mut SCType) {
    unsafe { ffi::JointFrcTrq(joint, sc) }
}

/// Initialize an actuated joint's controller state.
pub fn init_actuated_joint(joint: &mut JointType, sc: &mut SCType) {
    unsafe { ffi::InitActuatedJoint(joint, sc) }
}

/// Simulate reaction wheel jitter disturbance.
pub fn wheel_jitter(whl: &mut WhlType, sc: &mut SCType) {
    unsafe { ffi::WheelJitter(whl, sc) }
}

/// Simulate mechanical shaker jitter disturbance.
pub fn shaker_jitter(sh: &mut ShakerType, sc: &mut SCType) {
    unsafe { ffi::ShakerJitter(sh, sc) }
}

/// Simulate fuel slosh using CFD-derived model.
pub fn cfd_slosh(sc: &mut SCType) {
    unsafe { ffi::CfdSlosh(sc) }
}

/// Simulate fuel slosh using a simplified pendulum model.
pub fn fake_cfd_slosh(sc: &mut SCType) {
    unsafe { ffi::FakeCfdSlosh(sc) }
}

// --- IPC ---

/// Initialize inter-process communication sockets.
pub fn init_inter_process_comm() {
    unsafe { ffi::InitInterProcessComm() }
}

/// Exchange data with external processes (MATLAB, COSMOS, etc.).
pub fn inter_process_comm() {
    unsafe { ffi::InterProcessComm() }
}

// --- NOS3 ---

/// Query the NOS3 simulation clock and return `(year, doy, month, day, hour, minute, second)`.
pub fn nos3_time() -> (i64, i64, i64, i64, i64, i64, f64) {
    let mut year = 0i64;
    let mut doy = 0i64;
    let mut month = 0i64;
    let mut day = 0i64;
    let mut hour = 0i64;
    let mut minute = 0i64;
    let mut second = 0.0f64;
    unsafe {
        ffi::NOS3Time(
            &mut year, &mut doy, &mut month, &mut day,
            &mut hour, &mut minute, &mut second,
        )
    };
    (year, doy, month, day, hour, minute, second)
}

// --- Orbit file loading ---

/// Load orbit from a TRV (Table of R and V) file.
pub fn load_trv_from_file(
    path: &CStr, trv_file: &CStr, elem_label: &CStr,
    dyn_time: f64, orb: &mut OrbitType,
) -> i64 {
    unsafe {
        ffi::LoadTRVfromFile(
            path.as_ptr(), trv_file.as_ptr(),
            elem_label.as_ptr(), dyn_time, orb,
        )
    }
}

// --- Optics ---

/// Find the field point and direction in body frame for
/// a given star vector through an optical element.
pub fn optical_field_point(
    star_vec_b: &[f64; 3], optics: &mut OpticsType,
) -> Option<([f64; 3], [f64; 3])> {
    let mut fld_pnt = [0.0; 3];
    let mut fld_dir = [0.0; 3];
    let ok = unsafe {
        ffi::OpticalFieldPoint(
            star_vec_b.as_ptr().cast_mut(),
            optics,
            fld_pnt.as_mut_ptr(),
            fld_dir.as_mut_ptr(),
        )
    };
    if ok != 0 { Some((fld_pnt, fld_dir)) } else { None }
}

/// Trace a ray through an optical train.
pub fn optical_train(
    fld_sc: i64, fld_body: i64,
    fld_pnt_b: &[f64; 3], fld_dir_b: &[f64; 3],
    opt: &mut [OpticsType],
) -> Option<(i64, i64, [f64; 3], [f64; 3])> {
    let mut out_sc = 0i64;
    let mut out_body = 0i64;
    let mut out_pnt = [0.0; 3];
    let mut out_dir = [0.0; 3];
    let nopt = opt.len() as i64;
    let ok = unsafe {
        ffi::OpticalTrain(
            fld_sc, fld_body,
            fld_pnt_b.as_ptr().cast_mut(),
            fld_dir_b.as_ptr().cast_mut(),
            nopt, opt.as_mut_ptr(),
            &mut out_sc, &mut out_body,
            out_pnt.as_mut_ptr(), out_dir.as_mut_ptr(),
        )
    };
    if ok != 0 {
        Some((out_sc, out_body, out_pnt, out_dir))
    } else {
        None
    }
}


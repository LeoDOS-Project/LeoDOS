//! SpaceCoMP job request.
//!
//! A job specifies a geographic area of interest and the parameters for each
//! of the three processing phases (Collect, Map, Reduce). Ground stations
//! submit jobs to the nearest visible (LOS) satellite for orchestration.

use bon::Builder;

use crate::network::isl::geo::GeoAoi;

/// Which assignment algorithm to use for collector-to-mapper matching.
#[derive(Debug, Clone, Copy, Default)]
pub enum AssignmentSolver {
    /// Kuhn-Munkres (Hungarian) algorithm, O(n^3).
    #[default]
    Hungarian,
    /// Jonker-Volgenant (LAPJV) algorithm, O(n^3) but typically faster.
    JonkerVolgenant,
}

/// A complete SpaceCoMP job request (Section II-A of the paper).
#[derive(Debug, Clone, Copy, Builder)]
pub struct Job {
    /// Geographic bounding box defining the region to observe.
    pub geo_aoi: GeoAoi,

    /// Data volume per collect task (bytes).
    pub data_volume_bytes: u64,

    /// Map processing time factor (m_p in Eq. 5).
    #[builder(default = 1.0)]
    pub map_processing_factor: f32,

    /// Reduce processing time factor (r_p).
    #[builder(default = 1.0)]
    pub reduce_processing_factor: f32,

    /// Map output compression ratio (F_M). 1.0 = no compression.
    #[builder(default = 1.0)]
    pub map_reduction_factor: f32,

    /// Reduce output compression ratio (F_R). Higher = more compression.
    #[builder(default = 5.0)]
    pub reduce_reduction_factor: f32,

    /// Only use ascending satellites (Section III-B.2).
    #[builder(default = false)]
    pub ascending_only: bool,

    /// Assignment algorithm to use.
    #[builder(default)]
    pub solver: AssignmentSolver,
}

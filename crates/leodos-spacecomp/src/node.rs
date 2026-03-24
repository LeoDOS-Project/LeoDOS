//! [`SpaceCompNode`] — the main entry point for running
//! a SpaceCoMP computation on a cFS satellite.

use crate::SpaceCompConfig;
use crate::SpaceCompError;
use crate::SpaceCompJob;

/// A SpaceCoMP node that participates in distributed
/// computation across the constellation.
///
/// Handles SRSPP transport setup, message dispatch, and
/// coordinator orchestration. The user provides a
/// [`SpaceCompJob`] implementation with their compute logic.
pub struct SpaceCompNode<J> {
    job: J,
    config: SpaceCompConfig,
}

#[bon::bon]
impl<J: SpaceCompJob> SpaceCompNode<J> {
    #[builder]
    pub fn new(job: J, config: SpaceCompConfig) -> Self {
        Self { job, config }
    }
}

impl<J: SpaceCompJob> SpaceCompNode<J> {
    /// Runs the SpaceCoMP node.
    ///
    /// Sets up SRSPP transport, enters the message dispatch
    /// loop, and handles coordinator/worker roles as assigned.
    pub async fn run(&mut self) -> Result<(), SpaceCompError> {
        // TODO: SRSPP setup + dispatch loop
        // This will be wired during the migration step.
        Ok(())
    }
}

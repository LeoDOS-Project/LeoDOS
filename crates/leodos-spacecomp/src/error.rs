//! Error types for the SpaceCoMP library.

use leodos_libcfs::error::CfsError;
use leodos_protocols::application::spacecomp::packet::BuildError;
use leodos_protocols::application::spacecomp::packet::ParseError;
use leodos_protocols::transport::srspp::api::cfs::TransportError;

/// Errors from SpaceCoMP operations.
#[derive(Debug, thiserror::Error)]
pub enum SpaceCompError {
    #[error(transparent)]
    Cfs(#[from] CfsError),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
    #[error("build: {0}")]
    Build(#[from] BuildError),
    #[error("transport: {0}")]
    Transport(#[from] TransportError<CfsError>),
    #[error("plan: {0}")]
    Plan(&'static str),
}

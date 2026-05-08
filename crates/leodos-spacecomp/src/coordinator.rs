//! Coordinator role — plans jobs and assigns roles.
//!
//! Moved from `apps/spacecomp/fsw/src/roles/coordinator.rs`.
//! Fully generic — only uses Job, Plan, and assignment payloads.

use core::mem::size_of;

use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::error::CfsError;
use leodos_protocols::transport::srspp::api::cfs::SrsppEndpoint;
use leodos_protocols::transport::srspp::dtn::MessageStore;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverBackend;
use crate::job::Job;
use crate::packet::AssignCollectorPayload;
use crate::packet::AssignMapperPayload;
use crate::packet::AssignReducerPayload;
use crate::packet::OpCode;
use crate::packet::SpaceCompMessage;
use crate::plan::Plan;
use crate::plan::ReducerPlacement;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::Point;
use zerocopy::IntoBytes;

use crate::SpaceCompError;

const MAX_SATELLITES: usize = 64;

/// Runs the coordinator role for a submitted job.
pub async fn run<
    S: MessageStore,
    R: Reachable,
    Rb: ReceiverBackend,
    const WIN: usize,
    const MTU: usize,
    const MAX_TX: usize,
    const MAX_STREAMS: usize,
>(
    endpoint: &SrsppEndpoint<'_, CfsError, MemPool, S, R, Rb, WIN, MTU, MAX_TX, MAX_STREAMS>,
    buf: &mut [u8],
    shell: Shell,
    local_point: Point,
    job_id: u16,
    job: Job,
) -> Result<(), SpaceCompError> {
    let plan: Plan<MAX_SATELLITES> = job
        .plan(shell, ReducerPlacement::CenterOfAoi, local_point)
        .map_err(SpaceCompError::Plan)?;

    for (i, pt) in plan.collectors.iter().enumerate() {
        let payload = AssignCollectorPayload::builder()
            .mapper_addr(plan.mappers[plan.assignment[i]])
            .partition_id(i as u8)
            .build();
        let m = SpaceCompMessage::builder()
            .buffer(buf)
            .op_code(OpCode::AssignCollector)
            .job_id(job_id)
            .payload_len(size_of::<AssignCollectorPayload>())
            .build()?;
        m.payload_mut().copy_from_slice(payload.as_bytes());
        let mut tx = endpoint
            .sender(Address::Satellite(*pt))
            .map_err(|_| SpaceCompError::Plan("endpoint sender allocation failed"))?;
        tx.send(m.as_bytes()).await?;
        tx.flush().await?;
    }

    for (j, pt) in plan.mappers.iter().enumerate() {
        let count = plan.assignment.iter().filter(|&&a| a == j).count() as u8;
        let payload = AssignMapperPayload::builder()
            .reducer_addr(plan.reducer)
            .collector_count(count)
            .build();
        let m = SpaceCompMessage::builder()
            .buffer(buf)
            .op_code(OpCode::AssignMapper)
            .job_id(job_id)
            .payload_len(size_of::<AssignMapperPayload>())
            .build()?;
        m.payload_mut().copy_from_slice(payload.as_bytes());
        let mut tx = endpoint
            .sender(Address::Satellite(*pt))
            .map_err(|_| SpaceCompError::Plan("endpoint sender allocation failed"))?;
        tx.send(m.as_bytes()).await?;
        tx.flush().await?;
    }

    let payload = AssignReducerPayload::builder()
        .los_addr(local_point)
        .mapper_count(plan.mappers.len() as u8)
        .build();
    let m = SpaceCompMessage::builder()
        .buffer(buf)
        .op_code(OpCode::AssignReducer)
        .job_id(job_id)
        .payload_len(size_of::<AssignReducerPayload>())
        .build()?;
    m.payload_mut().copy_from_slice(payload.as_bytes());
    let mut tx = endpoint
        .sender(Address::Satellite(plan.reducer))
        .map_err(|_| SpaceCompError::Plan("endpoint sender allocation failed"))?;
    tx.send(m.as_bytes()).await?;
    tx.flush().await?;

    Ok(())
}

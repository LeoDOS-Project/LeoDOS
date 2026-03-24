//! Coordinator role — plans jobs and assigns roles.
//!
//! Moved from `apps/spacecomp/fsw/src/roles/coordinator.rs`.
//! Fully generic — only uses Job, Plan, and assignment payloads.

use core::mem::size_of;

use leodos_protocols::application::spacecomp::io::writer::MessageSender;
use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::application::spacecomp::plan::Plan;
use leodos_protocols::application::spacecomp::plan::ReducerPlacement;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::Point;
use zerocopy::IntoBytes;

use crate::SpaceCompError;

const MAX_SATELLITES: usize = 64;

/// Runs the coordinator role for a submitted job.
pub async fn run<Tx: MessageSender<Error = SpaceCompError>>(
    tx: &mut Tx,
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
        tx.send_message(Address::Satellite(*pt), m.as_bytes()).await?;
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
        tx.send_message(Address::Satellite(*pt), m.as_bytes()).await?;
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
    tx.send_message(Address::Satellite(plan.reducer), m.as_bytes()).await?;

    Ok(())
}

use core::mem::size_of;

use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::application::spacecomp::plan::Plan;
use leodos_protocols::application::spacecomp::plan::ReducerPlacement;
use leodos_protocols::network::isl::torus::Point;
use zerocopy::IntoBytes;

use crate::Buffers;
use crate::RxHandle;
use crate::SpaceCompError;
use crate::TxHandle;
use crate::MAX_SATELLITES;
use crate::SHELL;

pub async fn run(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    local_point: Point,
    job_id: u16,
    job: Job,
) -> Result<(), SpaceCompError> {
    let plan: Plan<MAX_SATELLITES> = job
        .plan(SHELL, ReducerPlacement::CenterOfAoi, local_point)
        .map_err(SpaceCompError::Plan)?;

    for (i, pt) in plan.collectors.iter().enumerate() {
        let payload = AssignCollectorPayload::builder()
            .mapper_addr(plan.mappers[plan.assignment[i]])
            .partition_id(i as u8)
            .build();
        let m = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::AssignCollector)
            .job_id(job_id)
            .payload_len(size_of::<AssignCollectorPayload>())
            .build()?;
        m.payload_mut().copy_from_slice(payload.as_bytes());
        tx.send(*pt, m).await.ok();
    }
    for (j, pt) in plan.mappers.iter().enumerate() {
        let count = plan.assignment.iter().filter(|&&a| a == j).count() as u8;
        let payload = AssignMapperPayload::builder()
            .reducer_addr(plan.reducer)
            .collector_count(count)
            .build();
        let m = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::AssignMapper)
            .job_id(job_id)
            .payload_len(size_of::<AssignMapperPayload>())
            .build()?;
        m.payload_mut().copy_from_slice(payload.as_bytes());
        tx.send(*pt, m).await.ok();
    }
    {
        let payload = AssignReducerPayload::builder()
            .los_addr(local_point)
            .mapper_count(plan.mappers.len() as u8)
            .build();
        let m = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::AssignReducer)
            .job_id(job_id)
            .payload_len(size_of::<AssignReducerPayload>())
            .build()?;
        m.payload_mut().copy_from_slice(payload.as_bytes());
        tx.send(plan.reducer, m).await.ok();
    }

    loop {
        let Ok(token) = rx.wait_for_message().await else {
            return Ok(());
        };
        let is_result = token.consume(|data| {
            SpaceCompMessage::parse(data)
                .map(|msg| msg.op_code() == Ok(OpCode::JobResult) && msg.job_id() == job_id)
                .unwrap_or(false)
        });
        if is_result {
            return Ok(());
        }
    }
}

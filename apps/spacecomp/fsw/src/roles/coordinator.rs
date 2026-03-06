use core::mem::size_of;

use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::ParseError;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::application::spacecomp::plan::Plan;
use leodos_protocols::application::spacecomp::plan::ReducerPlacement;
use leodos_protocols::network::isl::address::Address;
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
    reply_to: Address,
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

    loop {
        let Ok(len) = rx
            .recv_with(|data| -> Result<usize, ParseError> {
                let msg = SpaceCompMessage::parse(data)?;
                if msg.op_code()? != OpCode::JobResult || msg.job_id() != job_id {
                    return Err(ParseError::UnexpectedMessage);
                }
                let n = data.len().min(bufs.msg.len());
                bufs.msg[..n].copy_from_slice(&data[..n]);
                Ok(n)
            })
            .await?
        else {
            continue;
        };
        tx.send(reply_to, &bufs.msg[..len]).await?;
        return Ok(());
    }
}

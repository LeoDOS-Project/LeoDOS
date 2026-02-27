use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorMessage;
use leodos_protocols::application::spacecomp::packet::AssignMapperMessage;
use leodos_protocols::application::spacecomp::packet::AssignReducerMessage;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::application::spacecomp::plan::Plan;
use leodos_protocols::application::spacecomp::plan::ReducerPlacement;
use leodos_protocols::network::isl::geo::GeoAoi;
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::torus::Point;

use crate::Buffers;
use crate::NodeHandle;
use crate::SpaceCompError;
use crate::MAX_SATELLITES;
use crate::SHELL;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    local_point: Point,
    job_id: u16,
) -> Result<(), SpaceCompError> {
    let plan: Plan<MAX_SATELLITES> = Job::builder()
        .geo_aoi(GeoAoi::new(
            LatLon::new(55.0, 10.0),
            LatLon::new(50.0, 20.0),
        ))
        .data_volume_bytes(1024)
        .build()
        .plan(SHELL, ReducerPlacement::CenterOfAoi, local_point)
        .map_err(SpaceCompError::Plan)?;

    for (i, pt) in plan.collectors.iter().enumerate() {
        let msg = AssignCollectorMessage::builder()
            .buffer(&mut bufs.msg)
            .job_id(job_id)
            .mapper_addr(plan.mappers[plan.assignment[i]])
            .partition_id(i)
            .build()?;
        handle.send(*pt, msg).await.ok();
    }
    for (j, pt) in plan.mappers.iter().enumerate() {
        let msg = AssignMapperMessage::builder()
            .buffer(&mut bufs.msg)
            .job_id(job_id)
            .reducer_addr(plan.reducer)
            .collector_count(plan.assignment.iter().filter(|&&a| a == j).count())
            .build()?;
        handle.send(*pt, msg).await.ok();
    }
    let msg = AssignReducerMessage::builder()
        .buffer(&mut bufs.msg)
        .job_id(job_id)
        .los_addr(local_point)
        .mapper_count(plan.mappers.len())
        .build()?;
    handle.send(plan.reducer, msg).await.ok();

    loop {
        let Ok((_, len)) = handle.recv(&mut bufs.recv).await else {
            return Ok(());
        };
        let Ok(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
            continue;
        };
        if msg.op_code() == Ok(OpCode::JobResult) && msg.job_id() == job_id {
            return Ok(());
        }
    }
}

use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::mapreduce::proxy::Coordinator;
use leodos_protocols::application::spacecomp::mapreduce::proxy::JobPlan;
use leodos_protocols::application::spacecomp::mapreduce::proxy::ReducerPlacement;
use leodos_protocols::application::spacecomp::packet::AssignCollectorMessage;
use leodos_protocols::application::spacecomp::packet::AssignMapperMessage;
use leodos_protocols::application::spacecomp::packet::AssignReducerMessage;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::geo::GeoAoi;
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::isl::torus::Torus;

use crate::Buffers;
use crate::NodeHandle;
use crate::SpaceCompError;
use crate::{NUM_ORBITS, NUM_SATS};

const MAX_SATELLITES: usize = 64;
const ALTITUDE_M: f32 = 550_000.0;
const INCLINATION_DEG: f32 = 87.0;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    local_node: Point,
    job_id: u16,
) -> Result<(), SpaceCompError> {
    let plan = plan(local_node).map_err(SpaceCompError::Plan)?;

    let local_address = Address::from(local_node);
    send_assignments(handle, bufs, &plan, local_address, job_id).await?;

    loop {
        let Ok((_, len)) = handle.recv(&mut bufs.recv).await else {
            return Ok(());
        };
        let Some(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
            continue;
        };
        if msg.op_code() == Ok(OpCode::JobResult) && msg.job_id() == job_id {
            return Ok(());
        }
    }
}

fn plan(local_node: Point) -> Result<JobPlan<MAX_SATELLITES>, &'static str> {
    let torus = Torus::new(NUM_ORBITS, NUM_SATS);
    let shell = Shell::new(torus, ALTITUDE_M, INCLINATION_DEG);

    let job = Job::builder()
        .geo_aoi(GeoAoi::new(
            LatLon::new(55.0, 10.0),
            LatLon::new(50.0, 20.0),
        ))
        .data_volume_bytes(1024)
        .build();

    let coordinator = Coordinator::new(shell, ReducerPlacement::CenterOfAoi);
    coordinator.plan(&job, local_node)
}

async fn send_assignments(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    plan: &JobPlan<MAX_SATELLITES>,
    local_address: Address,
    job_id: u16,
) -> Result<(), SpaceCompError> {
    for (i, collector_pos) in plan.collectors.iter().enumerate() {
        let mapper_idx = plan.assignment[i];
        let mapper_pos = plan.mappers[mapper_idx];

        let target = Address::from(*collector_pos);
        let msg = AssignCollectorMessage::builder()
            .buffer(&mut bufs.msg)
            .job_id(job_id)
            .mapper_addr(Address::from(mapper_pos))
            .partition_id(i)
            .build()?;
        handle.send(target, msg.as_bytes()).await.ok();
    }

    for (j, mapper_pos) in plan.mappers.iter().enumerate() {
        let collector_count = plan.assignment.iter().filter(|&&a| a == j).count();

        let target = Address::from(*mapper_pos);
        let msg = AssignMapperMessage::builder()
            .buffer(&mut bufs.msg)
            .job_id(job_id)
            .reducer_addr(Address::from(plan.reducer))
            .collector_count(collector_count)
            .build()?;
        handle.send(target, msg.as_bytes()).await.ok();
    }

    let target = Address::from(plan.reducer);
    let msg = AssignReducerMessage::builder()
        .buffer(&mut bufs.msg)
        .job_id(job_id)
        .los_addr(local_address)
        .mapper_count(plan.mappers.len())
        .build()?;
    handle.send(target, msg.as_bytes()).await.ok();

    Ok(())
}

use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::mapreduce::proxy::{
    Coordinator, JobPlan, ReducerPlacement,
};
use leodos_protocols::application::spacecomp::packet::{
    AssignCollectorPayload, AssignMapperPayload, AssignReducerPayload, OpCode, SpaceCompMessage,
};
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::geo::{GeoAoi, LatLon};
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::{Point, Torus};
use zerocopy::IntoBytes;

use crate::NodeHandle;
use crate::{NUM_ORBITS, NUM_SATS};

const MAX_SATELLITES: usize = 64;
const ALTITUDE_M: f32 = 550_000.0;
const INCLINATION_DEG: f32 = 87.0;
const MSG_BUF_SIZE: usize = 512;

pub async fn run(handle: &mut NodeHandle<'_>, local_node: Point, job_id: u16) {
    let plan = match plan(local_node) {
        Ok(p) => p,
        Err(_) => return,
    };

    let local_address = Address::from(local_node);
    send_assignments(handle, &plan, local_address, job_id).await;

    let mut recv_buf = [0u8; 8192];
    loop {
        let Ok((_, len)) = handle.recv(&mut recv_buf).await else {
            return;
        };
        let Some(msg) = SpaceCompMessage::parse(&recv_buf[..len]) else {
            continue;
        };
        if msg.op_code() == Ok(OpCode::JobResult) && msg.job_id() == job_id {
            return;
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
    plan: &JobPlan<MAX_SATELLITES>,
    local_address: Address,
    job_id: u16,
) {
    let mut msg_buf = [0u8; MSG_BUF_SIZE];

    for (i, collector_pos) in plan.collectors.iter().enumerate() {
        let mapper_idx = plan.assignment[i];
        let mapper_pos = plan.mappers[mapper_idx];

        let payload = AssignCollectorPayload::builder()
            .mapper_addr(Address::from(mapper_pos))
            .partition_id(i as u8)
            .build();
        let target = Address::from(*collector_pos);
        if let Some(msg) = SpaceCompMessage::builder()
            .buffer(&mut msg_buf)
            .op_code(OpCode::AssignCollector)
            .job_id(job_id)
            .payload(payload.as_bytes())
            .build()
        {
            handle.send(target, msg.as_bytes()).await.ok();
        }
    }

    for (j, mapper_pos) in plan.mappers.iter().enumerate() {
        let collector_count = plan.assignment.iter().filter(|&&a| a == j).count();

        let payload = AssignMapperPayload::builder()
            .reducer_addr(Address::from(plan.reducer))
            .collector_count(collector_count as u8)
            .build();
        let target = Address::from(*mapper_pos);
        if let Some(msg) = SpaceCompMessage::builder()
            .buffer(&mut msg_buf)
            .op_code(OpCode::AssignMapper)
            .job_id(job_id)
            .payload(payload.as_bytes())
            .build()
        {
            handle.send(target, msg.as_bytes()).await.ok();
        }
    }

    let payload = AssignReducerPayload::builder()
        .los_addr(local_address)
        .mapper_count(plan.mappers.len() as u8)
        .build();
    let target = Address::from(plan.reducer);
    if let Some(msg) = SpaceCompMessage::builder()
        .buffer(&mut msg_buf)
        .op_code(OpCode::AssignReducer)
        .job_id(job_id)
        .payload(payload.as_bytes())
        .build()
    {
        handle.send(target, msg.as_bytes()).await.ok();
    }
}

use leodos_protocols::mission::compute::job::Job;
use leodos_protocols::mission::compute::mapreduce::proxy::{
    Coordinator, JobPlan, ReducerPlacement,
};
use leodos_protocols::mission::compute::packet::{
    AssignCollectorPayload, AssignMapperPayload, AssignReducerPayload, OpCode,
};
use leodos_protocols::network::isl::address::{Address, RawAddress};
use leodos_protocols::network::isl::geo::{GeoAoi, LatLon};
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::{Point, Torus};
use zerocopy::IntoBytes;

use crate::isl::{self, NodeHandle};
use crate::{NUM_ORBITS, NUM_SATS};

const MAX_SATELLITES: usize = 64;
const ALTITUDE_M: f32 = 550_000.0;
const INCLINATION_DEG: f32 = 87.0;

pub async fn run(handle: &mut NodeHandle<'_>, local_node: Point, job_id: u16) {
    let plan = match plan(local_node) {
        Ok(p) => p,
        Err(_) => return,
    };

    let local_address = Address::from(local_node);
    send_assignments(handle, &plan, local_address, job_id).await;

    let mut buf = [0u8; 512];
    loop {
        let Ok(msg) = handle.recv().await else { return };
        let Some(cmd) = isl::parse(&msg.data, &mut buf) else {
            continue;
        };
        if cmd.op_code == OpCode::JobResult && cmd.job_id == job_id {
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
    for (i, collector_pos) in plan.collectors.iter().enumerate() {
        let mapper_idx = plan.assignment[i];
        let mapper_pos = plan.mappers[mapper_idx];

        let payload = AssignCollectorPayload {
            mapper_addr: RawAddress::from(Address::from(mapper_pos)),
            partition_id: i as u8,
        };
        let target = Address::from(*collector_pos);
        isl::send(handle, target, OpCode::AssignCollector, job_id, payload.as_bytes())
            .await
            .ok();
    }

    for (j, mapper_pos) in plan.mappers.iter().enumerate() {
        let collector_count = plan.assignment.iter().filter(|&&a| a == j).count();

        let payload = AssignMapperPayload {
            reducer_addr: RawAddress::from(Address::from(plan.reducer)),
            collector_count: collector_count as u8,
        };
        let target = Address::from(*mapper_pos);
        isl::send(handle, target, OpCode::AssignMapper, job_id, payload.as_bytes())
            .await
            .ok();
    }

    let payload = AssignReducerPayload {
        los_addr: RawAddress::from(local_address),
        mapper_count: plan.mappers.len() as u8,
    };
    let target = Address::from(plan.reducer);
    isl::send(handle, target, OpCode::AssignReducer, job_id, payload.as_bytes())
        .await
        .ok();
}

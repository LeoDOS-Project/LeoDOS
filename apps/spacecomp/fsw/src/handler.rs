use core::mem::size_of;

use leodos_libcfs::cfe::evs::event;
use leodos_protocols::mission::compute::packet::{
    AssignCollectorPayload, AssignMapperPayload, AssignReducerPayload, OpCode,
};
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::NetworkLayer;
use zerocopy::FromBytes;

use crate::isl;
use crate::roles;

pub enum State {
    Idle,
    Coordinating { job_id: u16 },
    Mapping {
        reducer_addr: Address,
        job_id: u16,
        expected: u8,
        received: u8,
        map_state: roles::mapper::MapState,
    },
    Reducing {
        los_addr: Address,
        job_id: u16,
        expected: u8,
        received: u8,
        reduce_state: roles::reducer::ReduceState,
    },
}

impl State {
    pub fn new() -> Self {
        Self::Idle
    }

    pub async fn handle<L: NetworkLayer>(
        &mut self,
        link: &mut L,
        ctx: &isl::Context,
        local_node: Point,
        op: OpCode,
        job_id: u16,
        payload: &[u8],
    ) {
        match self {
            State::Idle => self.handle_idle(link, ctx, local_node, op, job_id, payload).await,
            State::Coordinating { job_id: jid } => {
                if op == OpCode::JobResult && job_id == *jid {
                    event::info(0, "Job complete").ok();
                    *self = State::Idle;
                }
            }
            State::Mapping {
                reducer_addr,
                job_id: jid,
                expected,
                received,
                map_state,
            } => {
                if op == OpCode::DataChunk {
                    map_state.ingest_chunk(payload);
                    *received += 1;
                    if *received >= *expected {
                        let ra = *reducer_addr;
                        let j = *jid;
                        map_state.emit_results(link, ctx, ra, j).await;
                        event::info(0, "Map phase complete").ok();
                        *self = State::Idle;
                    }
                }
            }
            State::Reducing {
                los_addr,
                job_id: jid,
                expected,
                received,
                reduce_state,
            } => {
                if op == OpCode::DataChunk {
                    reduce_state.ingest_chunk(payload);
                    *received += 1;
                    if *received >= *expected {
                        let la = *los_addr;
                        let j = *jid;
                        reduce_state.emit_results(link, ctx, la, j).await;
                        event::info(0, "Reduce phase complete").ok();
                        *self = State::Idle;
                    }
                }
            }
        }
    }

    async fn handle_idle<L: NetworkLayer>(
        &mut self,
        link: &mut L,
        ctx: &isl::Context,
        local_node: Point,
        op: OpCode,
        job_id: u16,
        payload: &[u8],
    ) {
        match op {
            OpCode::SubmitJob => {
                roles::coordinator::plan_and_assign(link, ctx, local_node, job_id)
                    .await;
                *self = State::Coordinating { job_id };
            }
            OpCode::AssignCollector => {
                if payload.len() >= size_of::<AssignCollectorPayload>() {
                    if let Ok(p) = AssignCollectorPayload::read_from_bytes(
                        &payload[..size_of::<AssignCollectorPayload>()],
                    ) {
                        roles::collector::send_data(link, ctx, &p, job_id).await;
                    }
                }
            }
            OpCode::AssignMapper => {
                if payload.len() >= size_of::<AssignMapperPayload>() {
                    if let Ok(p) = AssignMapperPayload::read_from_bytes(
                        &payload[..size_of::<AssignMapperPayload>()],
                    ) {
                        event::info(0, "Assigned as mapper").ok();
                        *self = State::Mapping {
                            reducer_addr: p.reducer_addr.parse(),
                            job_id,
                            expected: p.collector_count,
                            received: 0,
                            map_state: roles::mapper::MapState::new(),
                        };
                    }
                }
            }
            OpCode::AssignReducer => {
                if payload.len() >= size_of::<AssignReducerPayload>() {
                    if let Ok(p) = AssignReducerPayload::read_from_bytes(
                        &payload[..size_of::<AssignReducerPayload>()],
                    ) {
                        event::info(0, "Assigned as reducer").ok();
                        *self = State::Reducing {
                            los_addr: p.los_addr.parse(),
                            job_id,
                            expected: p.mapper_count,
                            received: 0,
                            reduce_state: roles::reducer::ReduceState::new(),
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

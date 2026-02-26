use core::mem;
use core::mem::size_of;

use leodos_protocols::mission::compute::mapreduce::proxy::JobPlan;
use leodos_protocols::mission::compute::packet::{
    AssignCollectorPayload, AssignMapperPayload, AssignReducerPayload, OpCode,
};
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::torus::Point;
use zerocopy::FromBytes;

use crate::roles;

pub const MAX_SATELLITES: usize = 64;

pub struct Event<'a> {
    pub op: OpCode,
    pub job_id: u16,
    pub payload: &'a [u8],
}

pub enum Action {
    SendAssignments {
        plan: JobPlan<MAX_SATELLITES>,
        local_address: Address,
        job_id: u16,
    },
    CollectAndSend {
        mapper_addr: Address,
        partition_id: u8,
        job_id: u16,
    },
    EmitMapResults {
        map_state: roles::mapper::MapState,
        reducer_addr: Address,
        job_id: u16,
    },
    EmitReduceResults {
        reduce_state: roles::reducer::ReduceState,
        los_addr: Address,
        job_id: u16,
    },
}

pub enum Machine {
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

impl Machine {
    pub fn new() -> Self {
        Self::Idle
    }

    pub fn on_event(&mut self, event: Event<'_>, local_node: Point) -> Option<Action> {
        match self {
            Machine::Idle => match event.op {
                OpCode::SubmitJob => {
                    let plan = roles::coordinator::plan(local_node).ok()?;
                    let local_address = Address::from(local_node);
                    *self = Machine::Coordinating { job_id: event.job_id };
                    Some(Action::SendAssignments {
                        plan,
                        local_address,
                        job_id: event.job_id,
                    })
                }
                OpCode::AssignCollector => {
                    let p = AssignCollectorPayload::read_from_bytes(
                        event.payload.get(..size_of::<AssignCollectorPayload>())?,
                    )
                    .ok()?;
                    Some(Action::CollectAndSend {
                        mapper_addr: p.mapper_addr.parse(),
                        partition_id: p.partition_id,
                        job_id: event.job_id,
                    })
                }
                OpCode::AssignMapper => {
                    let p = AssignMapperPayload::read_from_bytes(
                        event.payload.get(..size_of::<AssignMapperPayload>())?,
                    )
                    .ok()?;
                    *self = Machine::Mapping {
                        reducer_addr: p.reducer_addr.parse(),
                        job_id: event.job_id,
                        expected: p.collector_count,
                        received: 0,
                        map_state: roles::mapper::MapState::new(),
                    };
                    None
                }
                OpCode::AssignReducer => {
                    let p = AssignReducerPayload::read_from_bytes(
                        event.payload.get(..size_of::<AssignReducerPayload>())?,
                    )
                    .ok()?;
                    *self = Machine::Reducing {
                        los_addr: p.los_addr.parse(),
                        job_id: event.job_id,
                        expected: p.mapper_count,
                        received: 0,
                        reduce_state: roles::reducer::ReduceState::new(),
                    };
                    None
                }
                _ => None,
            },
            Machine::Coordinating { job_id } => {
                if event.op == OpCode::JobResult && event.job_id == *job_id {
                    *self = Machine::Idle;
                }
                None
            }
            Machine::Mapping { .. } => {
                if event.op != OpCode::DataChunk {
                    return None;
                }
                let Machine::Mapping {
                    reducer_addr,
                    job_id,
                    expected,
                    mut received,
                    mut map_state,
                } = mem::replace(self, Machine::Idle)
                else {
                    unreachable!()
                };
                map_state.ingest_chunk(event.payload);
                received += 1;
                if received >= expected {
                    Some(Action::EmitMapResults {
                        map_state,
                        reducer_addr,
                        job_id,
                    })
                } else {
                    *self = Machine::Mapping {
                        reducer_addr,
                        job_id,
                        expected,
                        received,
                        map_state,
                    };
                    None
                }
            }
            Machine::Reducing { .. } => {
                if event.op != OpCode::DataChunk {
                    return None;
                }
                let Machine::Reducing {
                    los_addr,
                    job_id,
                    expected,
                    mut received,
                    mut reduce_state,
                } = mem::replace(self, Machine::Idle)
                else {
                    unreachable!()
                };
                reduce_state.ingest_chunk(event.payload);
                received += 1;
                if received >= expected {
                    Some(Action::EmitReduceResults {
                        reduce_state,
                        los_addr,
                        job_id,
                    })
                } else {
                    *self = Machine::Reducing {
                        los_addr,
                        job_id,
                        expected,
                        received,
                        reduce_state,
                    };
                    None
                }
            }
        }
    }
}

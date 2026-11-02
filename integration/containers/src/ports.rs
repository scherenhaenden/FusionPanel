#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

const RESERVED_PORTS: &[u16] = &[22, 80, 443, 3306, 5432];

#[derive(Debug)]
pub enum AllocationError {
    NoPortsAvailable,
    PortAlreadyAllocated(u16),
    PortReserved(u16),
}

impl fmt::Display for AllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocationError::NoPortsAvailable => write!(f, "No ports available in the given range"),
            AllocationError::PortAlreadyAllocated(p) => {
                write!(f, "Port {} is already allocated", p)
            }
            AllocationError::PortReserved(p) => {
                write!(f, "Port {} is a reserved system service port", p)
            }
        }
    }
}

impl std::error::Error for AllocationError {}

pub struct PortAllocator {
    state: Arc<Mutex<AllocatorState>>,
}

struct AllocatorState {
    allocated: HashSet<u16>,
    range_start: u16,
    range_end: u16,
    reserved: HashSet<u16>,
}

impl PortAllocator {
    pub fn new(range_start: u16, range_end: u16) -> Self {
        let mut reserved = HashSet::new();
        for &port in RESERVED_PORTS {
            reserved.insert(port);
        }

        Self {
            state: Arc::new(Mutex::new(AllocatorState {
                allocated: HashSet::new(),
                range_start,
                range_end,
                reserved,
            })),
        }
    }

    pub fn allocate(&self) -> Result<u16, AllocationError> {
        let mut state = self.state.lock().unwrap();
        for port in state.range_start..=state.range_end {
            if !state.reserved.contains(&port) && !state.allocated.contains(&port) {
                state.allocated.insert(port);
                return Ok(port);
            }
        }
        Err(AllocationError::NoPortsAvailable)
    }

    pub fn allocate_specific(&self, port: u16) -> Result<(), AllocationError> {
        let mut state = self.state.lock().unwrap();

        if state.reserved.contains(&port) {
            return Err(AllocationError::PortReserved(port));
        }

        if state.allocated.contains(&port) {
            return Err(AllocationError::PortAlreadyAllocated(port));
        }

        state.allocated.insert(port);
        Ok(())
    }

    pub fn release(&self, port: u16) {
        let mut state = self.state.lock().unwrap();
        state.allocated.remove(&port);
    }
}

impl Clone for PortAllocator {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

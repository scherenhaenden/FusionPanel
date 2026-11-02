#[path = "../src/ports.rs"]
mod ports;

use ports::{AllocationError, PortAllocator};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn test_reserved_ports() {
    let allocator = PortAllocator::new(1000, 2000);

    let reserved = vec![22, 80, 443, 3306, 5432];
    for port in reserved {
        let result = allocator.allocate_specific(port);
        assert!(matches!(result, Err(AllocationError::PortReserved(_))));
    }
}

#[test]
fn test_allocation_and_release() {
    let allocator = PortAllocator::new(8000, 8005);

    let p1 = allocator.allocate().expect("Should allocate");
    assert_eq!(p1, 8000);

    let p2 = allocator.allocate().expect("Should allocate");
    assert_eq!(p2, 8001);

    allocator.release(8000);

    let p3 = allocator.allocate().expect("Should allocate");
    assert_eq!(p3, 8000); // Should re-allocate the freed port
}

#[test]
fn test_specific_allocation() {
    let allocator = PortAllocator::new(8000, 8005);

    allocator
        .allocate_specific(8002)
        .expect("Should allocate 8002");

    let p1 = allocator.allocate().expect("Should allocate");
    assert_eq!(p1, 8000);

    let p2 = allocator.allocate().expect("Should allocate");
    assert_eq!(p2, 8001);

    let p3 = allocator.allocate().expect("Should allocate");
    assert_eq!(p3, 8003); // 8002 was skipped because it was already allocated

    let result = allocator.allocate_specific(8001);
    assert!(matches!(
        result,
        Err(AllocationError::PortAlreadyAllocated(8001))
    ));
}

#[test]
fn test_exhaustion() {
    let allocator = PortAllocator::new(8000, 8001);

    allocator.allocate().expect("Should allocate 8000");
    allocator.allocate().expect("Should allocate 8001");

    let result = allocator.allocate();
    assert!(matches!(result, Err(AllocationError::NoPortsAvailable)));
}

#[test]
fn test_concurrency() {
    let allocator = PortAllocator::new(10000, 11000);
    let mut handles = vec![];
    let allocated_ports = Arc::new(Mutex::new(HashSet::new()));

    for _ in 0..10 {
        let alloc_clone = allocator.clone();
        let set_clone = Arc::clone(&allocated_ports);

        handles.push(thread::spawn(move || {
            for _ in 0..50 {
                if let Ok(port) = alloc_clone.allocate() {
                    let mut set = set_clone.lock().unwrap();
                    assert!(!set.contains(&port), "Port was allocated twice: {}", port);
                    set.insert(port);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let set = allocated_ports.lock().unwrap();
    assert_eq!(set.len(), 500);
}

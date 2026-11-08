#![forbid(unsafe_code)]

use containers::ipam_allocator::{IpamAllocator, IpamError};

#[test]
fn test_sequential_allocation() {
    let mut allocator = IpamAllocator::new("172.20.0.0/16", 24).expect("Should create allocator");
    let s1 = allocator.allocate().expect("Should allocate s1");
    assert_eq!(s1, "172.20.0.0/24");
    
    let s2 = allocator.allocate().expect("Should allocate s2");
    assert_eq!(s2, "172.20.1.0/24");
}

#[test]
fn test_collision_detection() {
    let mut allocator = IpamAllocator::new("172.20.0.0/16", 24).expect("Should create allocator");
    allocator.reserve("172.20.0.0/24").expect("Should reserve");
    
    let s1 = allocator.allocate().expect("Should allocate");
    assert_eq!(s1, "172.20.1.0/24");
}

#[test]
fn test_release_and_reuse() {
    let mut allocator = IpamAllocator::new("172.20.0.0/16", 24).expect("Should create allocator");
    let s1 = allocator.allocate().expect("Should allocate");
    assert_eq!(s1, "172.20.0.0/24");
    
    allocator.free(&s1).expect("Should free");
    
    let s2 = allocator.allocate().expect("Should allocate");
    assert_eq!(s2, "172.20.0.0/24");
}

#[test]
fn test_pool_exhaustion() {
    let mut allocator = IpamAllocator::new("172.20.0.0/30", 31).expect("Should create allocator");
    // Pool: 172.20.0.0/30 gives us two /31s
    let _s1 = allocator.allocate().expect("Should allocate 1");
    let _s2 = allocator.allocate().expect("Should allocate 2");
    
    let err = allocator.allocate().expect_err("Should fail to allocate 3");
    match err {
        IpamError::PoolExhausted => {}
        _ => panic!("Expected PoolExhausted error"),
    }
}

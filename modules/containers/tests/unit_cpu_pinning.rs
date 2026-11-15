#![forbid(unsafe_code)]

use containers::cpu_pinning::{
    format_cpuset, parse_cpuset, CpuAllocator, HostTopology, LogicalCpu,
};

fn create_topology() -> HostTopology {
    // 2 NUMA nodes, 1 Socket each, 2 Cores each, 2 Threads per Core
    // Node 0: CPUs 0,1 (Core 0), 2,3 (Core 1)
    // Node 1: CPUs 4,5 (Core 0), 6,7 (Core 1)
    HostTopology {
        cpus: vec![
            LogicalCpu {
                id: 0,
                numa_node: 0,
                socket: 0,
                core: 0,
            },
            LogicalCpu {
                id: 1,
                numa_node: 0,
                socket: 0,
                core: 0,
            },
            LogicalCpu {
                id: 2,
                numa_node: 0,
                socket: 0,
                core: 1,
            },
            LogicalCpu {
                id: 3,
                numa_node: 0,
                socket: 0,
                core: 1,
            },
            LogicalCpu {
                id: 4,
                numa_node: 1,
                socket: 1,
                core: 0,
            },
            LogicalCpu {
                id: 5,
                numa_node: 1,
                socket: 1,
                core: 0,
            },
            LogicalCpu {
                id: 6,
                numa_node: 1,
                socket: 1,
                core: 1,
            },
            LogicalCpu {
                id: 7,
                numa_node: 1,
                socket: 1,
                core: 1,
            },
        ],
    }
}

#[test]
fn test_parse_cpuset() {
    assert_eq!(parse_cpuset("").unwrap(), Vec::<u32>::new());
    assert_eq!(parse_cpuset("0").unwrap(), vec![0]);
    assert_eq!(parse_cpuset("0,2,4").unwrap(), vec![0, 2, 4]);
    assert_eq!(parse_cpuset("0-2").unwrap(), vec![0, 1, 2]);
    assert_eq!(parse_cpuset("0-2,4,6-7").unwrap(), vec![0, 1, 2, 4, 6, 7]);

    assert!(parse_cpuset("invalid").is_err());
    assert!(parse_cpuset("0-2-4").is_err());
    assert!(parse_cpuset("2-0").is_err());
}

#[test]
fn test_format_cpuset() {
    assert_eq!(format_cpuset(&[]), "");
    assert_eq!(format_cpuset(&[0]), "0");
    assert_eq!(format_cpuset(&[0, 2, 4]), "0,2,4");
    assert_eq!(format_cpuset(&[0, 1, 2]), "0-2");
    assert_eq!(format_cpuset(&[0, 1, 2, 4, 6, 7]), "0-2,4,6-7");
    assert_eq!(format_cpuset(&[7, 6, 4, 2, 1, 0]), "0-2,4,6-7"); // Unsorted input
}

#[test]
fn test_allocate_single_node() {
    let mut allocator = CpuAllocator::new(create_topology());

    // Allocate 3 CPUs, should come from a single node (Node 0 usually, but we check logically)
    let res = allocator.allocate(3).unwrap();

    // Check that memory node is 1 (either 0 or 1, but length is 1)
    let mems = parse_cpuset(&res.cpuset_mems).unwrap();
    assert_eq!(mems.len(), 1);

    let cpus = parse_cpuset(&res.cpuset_cpus).unwrap();
    assert_eq!(cpus.len(), 3);

    // Since node 0 and 1 have 4 CPUs, it should pick one node entirely and take 3 CPUs
    // We don't hardcode 0-2 because Hash iteration order or sorting might pick node 1
    // but they should be logically sequential from the same node.

    assert!(cpus == vec![0, 1, 2] || cpus == vec![4, 5, 6]);
}

#[test]
fn test_allocate_cross_node() {
    let mut allocator = CpuAllocator::new(create_topology());

    // Allocate 6 CPUs, which forces it to span both nodes (4 in one, 2 in another)
    let res = allocator.allocate(6).unwrap();

    let mems = parse_cpuset(&res.cpuset_mems).unwrap();
    assert_eq!(mems.len(), 2);
    assert_eq!(mems, vec![0, 1]);

    let cpus = parse_cpuset(&res.cpuset_cpus).unwrap();
    assert_eq!(cpus.len(), 6);

    // It should take all 4 from one node, and 2 from the other
    assert!(cpus == vec![0, 1, 2, 3, 4, 5] || cpus == vec![0, 1, 4, 5, 6, 7]);
}

#[test]
fn test_allocation_exhaustion() {
    let mut allocator = CpuAllocator::new(create_topology());

    // Total CPUs = 8
    assert!(allocator.allocate(9).is_none());

    let res1 = allocator.allocate(5).unwrap();
    assert_eq!(parse_cpuset(&res1.cpuset_cpus).unwrap().len(), 5);

    let res2 = allocator.allocate(3).unwrap();
    assert_eq!(parse_cpuset(&res2.cpuset_cpus).unwrap().len(), 3);

    assert!(allocator.allocate(1).is_none());
}

#[test]
fn test_free_cpus() {
    let mut allocator = CpuAllocator::new(create_topology());

    let res1 = allocator.allocate(4).unwrap();
    let _cpus1 = parse_cpuset(&res1.cpuset_cpus).unwrap();

    assert!(allocator.allocate(5).is_none()); // Only 4 left

    allocator.free(&res1.cpuset_cpus).unwrap();

    // Now we should be able to allocate 8 again
    let res2 = allocator.allocate(8).unwrap();
    assert_eq!(parse_cpuset(&res2.cpuset_cpus).unwrap().len(), 8);
}

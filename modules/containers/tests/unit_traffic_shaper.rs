#![forbid(unsafe_code)]

use containers::traffic_shaper::{BandwidthCap, TrafficShapingPolicy, TrafficShaper};

#[test]
fn test_no_shaping() {
    let policy = TrafficShapingPolicy {
        interface: "veth0".to_string(),
        ingress: None,
        egress: None,
    };

    let cmds = TrafficShaper::generate_tc_args(&policy);

    // We should always clear existing rules
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].args, vec!["qdisc", "del", "dev", "veth0", "root"]);
    assert_eq!(cmds[1].args, vec!["qdisc", "del", "dev", "veth0", "ingress"]);
}

#[test]
fn test_egress_shaping() {
    let policy = TrafficShapingPolicy {
        interface: "veth1".to_string(),
        ingress: None,
        egress: Some(BandwidthCap {
            limit_mbit: 50,
            burst_mbit: 100,
        }),
    };

    let cmds = TrafficShaper::generate_tc_args(&policy);

    // 2 for clear, 3 for egress
    assert_eq!(cmds.len(), 5);

    assert_eq!(cmds[0].args, vec!["qdisc", "del", "dev", "veth1", "root"]);
    assert_eq!(cmds[1].args, vec!["qdisc", "del", "dev", "veth1", "ingress"]);

    assert_eq!(cmds[2].args, vec!["qdisc", "add", "dev", "veth1", "root", "handle", "1:", "htb", "default", "11"]);
    assert_eq!(cmds[3].args, vec!["class", "add", "dev", "veth1", "parent", "1:", "classid", "1:11", "htb", "rate", "50mbit", "ceil", "100mbit"]);
    assert_eq!(cmds[4].args, vec!["qdisc", "add", "dev", "veth1", "parent", "1:11", "handle", "10:", "fq_codel"]);
}

#[test]
fn test_ingress_shaping() {
    let policy = TrafficShapingPolicy {
        interface: "veth2".to_string(),
        ingress: Some(BandwidthCap {
            limit_mbit: 25,
            burst_mbit: 50,
        }),
        egress: None,
    };

    let cmds = TrafficShaper::generate_tc_args(&policy);

    // 2 for clear, 6 for ingress (using ifb)
    assert_eq!(cmds.len(), 8);

    assert_eq!(cmds[0].args, vec!["qdisc", "del", "dev", "veth2", "root"]);
    assert_eq!(cmds[1].args, vec!["qdisc", "del", "dev", "veth2", "ingress"]);

    assert_eq!(cmds[2].args, vec!["qdisc", "add", "dev", "veth2", "handle", "ffff:", "ingress"]);
    assert_eq!(cmds[3].args, vec!["filter", "add", "dev", "veth2", "parent", "ffff:", "protocol", "all", "u32", "match", "u32", "0", "0", "action", "mirred", "egress", "redirect", "dev", "ifb-veth2"]);

    assert_eq!(cmds[4].args, vec!["qdisc", "del", "dev", "ifb-veth2", "root"]);
    assert_eq!(cmds[5].args, vec!["qdisc", "add", "dev", "ifb-veth2", "root", "handle", "1:", "htb", "default", "11"]);
    assert_eq!(cmds[6].args, vec!["class", "add", "dev", "ifb-veth2", "parent", "1:", "classid", "1:11", "htb", "rate", "25mbit", "ceil", "50mbit"]);
    assert_eq!(cmds[7].args, vec!["qdisc", "add", "dev", "ifb-veth2", "parent", "1:11", "handle", "10:", "fq_codel"]);
}

#[test]
fn test_ingress_and_egress_shaping() {
    let policy = TrafficShapingPolicy {
        interface: "veth3".to_string(),
        ingress: Some(BandwidthCap {
            limit_mbit: 10,
            burst_mbit: 20,
        }),
        egress: Some(BandwidthCap {
            limit_mbit: 30,
            burst_mbit: 60,
        }),
    };

    let cmds = TrafficShaper::generate_tc_args(&policy);

    // 2 for clear, 3 for egress, 6 for ingress
    assert_eq!(cmds.len(), 11);

    // Clear
    assert_eq!(cmds[0].args, vec!["qdisc", "del", "dev", "veth3", "root"]);
    assert_eq!(cmds[1].args, vec!["qdisc", "del", "dev", "veth3", "ingress"]);

    // Egress
    assert_eq!(cmds[2].args, vec!["qdisc", "add", "dev", "veth3", "root", "handle", "1:", "htb", "default", "11"]);
    assert_eq!(cmds[3].args, vec!["class", "add", "dev", "veth3", "parent", "1:", "classid", "1:11", "htb", "rate", "30mbit", "ceil", "60mbit"]);
    assert_eq!(cmds[4].args, vec!["qdisc", "add", "dev", "veth3", "parent", "1:11", "handle", "10:", "fq_codel"]);

    // Ingress
    assert_eq!(cmds[5].args, vec!["qdisc", "add", "dev", "veth3", "handle", "ffff:", "ingress"]);
    assert_eq!(cmds[6].args, vec!["filter", "add", "dev", "veth3", "parent", "ffff:", "protocol", "all", "u32", "match", "u32", "0", "0", "action", "mirred", "egress", "redirect", "dev", "ifb-veth3"]);

    assert_eq!(cmds[7].args, vec!["qdisc", "del", "dev", "ifb-veth3", "root"]);
    assert_eq!(cmds[8].args, vec!["qdisc", "add", "dev", "ifb-veth3", "root", "handle", "1:", "htb", "default", "11"]);
    assert_eq!(cmds[9].args, vec!["class", "add", "dev", "ifb-veth3", "parent", "1:", "classid", "1:11", "htb", "rate", "10mbit", "ceil", "20mbit"]);
    assert_eq!(cmds[10].args, vec!["qdisc", "add", "dev", "ifb-veth3", "parent", "1:11", "handle", "10:", "fq_codel"]);
}

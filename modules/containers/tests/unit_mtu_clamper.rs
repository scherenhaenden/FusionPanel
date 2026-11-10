#![forbid(unsafe_code)]

use containers::mtu_clamper::{InterfaceType, MtuClamper, NetworkInterface};

#[test]
fn test_no_mismatches_empty() {
    let interfaces = vec![];
    let mismatches = MtuClamper::detect_mismatches(&interfaces);
    assert!(mismatches.is_empty());
}

#[test]
fn test_no_mismatches_same_type_different_mtu() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "eth1".to_string(),
            mtu: 9000,
            interface_type: InterfaceType::Physical,
        },
    ];
    let mismatches = MtuClamper::detect_mismatches(&interfaces);
    // Mismatches are only between different interface types.
    assert!(mismatches.is_empty());
}

#[test]
fn test_no_mismatches_different_type_same_mtu() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "docker0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::DockerBridge,
        },
    ];
    let mismatches = MtuClamper::detect_mismatches(&interfaces);
    assert!(mismatches.is_empty());
}

#[test]
fn test_mismatches_different_type_different_mtu() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 9000,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "docker0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::DockerBridge,
        },
    ];
    let mismatches = MtuClamper::detect_mismatches(&interfaces);
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].interface_a.name, "eth0");
    assert_eq!(mismatches[0].interface_b.name, "docker0");
}

#[test]
fn test_mismatches_multiple() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 9000,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "docker0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::DockerBridge,
        },
        NetworkInterface {
            name: "flannel.1".to_string(),
            mtu: 1450,
            interface_type: InterfaceType::Overlay,
        },
    ];
    let mismatches = MtuClamper::detect_mismatches(&interfaces);
    assert_eq!(mismatches.len(), 3);
}

#[test]
fn test_calculate_tcp_mss_clamping_rules_no_mismatch() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "docker0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::DockerBridge,
        },
    ];
    let rules = MtuClamper::calculate_tcp_mss_clamping_rules(&interfaces);
    assert!(rules.is_empty());
}

#[test]
fn test_calculate_tcp_mss_clamping_rules_with_mismatch() {
    let interfaces = vec![
        NetworkInterface {
            name: "eth0".to_string(),
            mtu: 9000,
            interface_type: InterfaceType::Physical,
        },
        NetworkInterface {
            name: "docker0".to_string(),
            mtu: 1500,
            interface_type: InterfaceType::DockerBridge,
        },
    ];
    let rules = MtuClamper::calculate_tcp_mss_clamping_rules(&interfaces);
    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0],
        "iptables -t mangle -A POSTROUTING -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu"
    );
}

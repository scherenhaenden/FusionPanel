#![forbid(unsafe_code)]

use nodes::wireguard_mesh::{WireGuardNode, WireGuardMeshConfigurator, WireGuardError};

#[test]
fn test_add_remove_node() {
    let mut configurator = WireGuardMeshConfigurator::new();

    let node1 = WireGuardNode {
        id: "node1".into(),
        private_key: "priv1".into(),
        public_key: "pub1".into(),
        listen_port: Some(51820),
        address: Some("10.0.0.1/32".into()),
        endpoint: Some("192.168.1.1:51820".into()),
        allowed_ips: vec!["10.0.0.1/32".into()],
        persistent_keepalive: Some(25),
    };

    assert!(configurator.add_node(node1.clone()).is_ok());
    assert_eq!(configurator.get_node("node1"), Some(&node1));

    configurator.remove_node("node1");
    assert_eq!(configurator.get_node("node1"), None);
}

#[test]
fn test_generate_config_single_node() {
    let mut configurator = WireGuardMeshConfigurator::new();

    let node1 = WireGuardNode {
        id: "node1".into(),
        private_key: "priv1".into(),
        public_key: "pub1".into(),
        listen_port: Some(51820),
        address: Some("10.0.0.1/32".into()),
        endpoint: Some("192.168.1.1:51820".into()),
        allowed_ips: vec!["10.0.0.1/32".into()],
        persistent_keepalive: None,
    };

    configurator.add_node(node1).unwrap();
    let config = configurator.generate_config("node1").unwrap();

    assert!(config.contains("[Interface]"));
    assert!(config.contains("PrivateKey = priv1"));
    assert!(config.contains("Address = 10.0.0.1/32"));
    assert!(config.contains("ListenPort = 51820"));
    assert!(!config.contains("[Peer]"));
}

#[test]
fn test_generate_config_mesh() {
    let mut configurator = WireGuardMeshConfigurator::new();
    configurator.set_default_keepalive(25);

    let node1 = WireGuardNode {
        id: "node1".into(),
        private_key: "priv1".into(),
        public_key: "pub1".into(),
        listen_port: Some(51820),
        address: Some("10.0.0.1/32".into()),
        endpoint: Some("192.168.1.1:51820".into()),
        allowed_ips: vec!["10.0.0.1/32".into()],
        persistent_keepalive: None,
    };

    let node2 = WireGuardNode {
        id: "node2".into(),
        private_key: "priv2".into(),
        public_key: "pub2".into(),
        listen_port: Some(51821),
        address: Some("10.0.0.2/32".into()),
        endpoint: Some("192.168.1.2:51821".into()),
        allowed_ips: vec!["10.0.0.2/32".into()],
        persistent_keepalive: Some(15),
    };

    configurator.add_node(node1).unwrap();
    configurator.add_node(node2).unwrap();

    configurator.set_psk("node1", "node2", "psk123".into()).unwrap();

    let config1 = configurator.generate_config("node1").unwrap();

    // Check interface of node1
    assert!(config1.contains("[Interface]"));
    assert!(config1.contains("PrivateKey = priv1"));
    assert!(config1.contains("Address = 10.0.0.1/32"));

    // Check peer config for node2 in node1's config
    assert!(config1.contains("[Peer]"));
    assert!(config1.contains("PublicKey = pub2"));
    assert!(config1.contains("PresharedKey = psk123"));
    assert!(config1.contains("AllowedIPs = 10.0.0.2/32"));
    assert!(config1.contains("Endpoint = 192.168.1.2:51821"));
    assert!(config1.contains("PersistentKeepalive = 15"));

    let config2 = configurator.generate_config("node2").unwrap();

    // Check interface of node2
    assert!(config2.contains("[Interface]"));
    assert!(config2.contains("PrivateKey = priv2"));

    // Check peer config for node1 in node2's config
    assert!(config2.contains("[Peer]"));
    assert!(config2.contains("PublicKey = pub1"));
    assert!(config2.contains("PresharedKey = psk123"));
    assert!(config2.contains("AllowedIPs = 10.0.0.1/32"));
    assert!(config2.contains("Endpoint = 192.168.1.1:51820"));
    assert!(config2.contains("PersistentKeepalive = 25")); // Should use default
}

#[test]
fn test_invalid_node_addition() {
    let mut configurator = WireGuardMeshConfigurator::new();

    let invalid_node1 = WireGuardNode {
        id: "".into(), // Empty ID
        private_key: "priv".into(),
        public_key: "pub".into(),
        listen_port: None,
        address: None,
        endpoint: None,
        allowed_ips: vec![],
        persistent_keepalive: None,
    };

    assert!(matches!(configurator.add_node(invalid_node1), Err(WireGuardError::InvalidConfig(_))));

    let invalid_node2 = WireGuardNode {
        id: "node".into(),
        private_key: "".into(), // Empty key
        public_key: "pub".into(),
        listen_port: None,
        address: None,
        endpoint: None,
        allowed_ips: vec![],
        persistent_keepalive: None,
    };

    assert!(matches!(configurator.add_node(invalid_node2), Err(WireGuardError::InvalidConfig(_))));
}

#[test]
fn test_generate_config_missing_node() {
    let configurator = WireGuardMeshConfigurator::new();
    assert!(matches!(configurator.generate_config("unknown"), Err(WireGuardError::NodeNotFound(_))));
}

#[test]
fn test_psk_validation() {
    let mut configurator = WireGuardMeshConfigurator::new();

    let node1 = WireGuardNode {
        id: "node1".into(),
        private_key: "priv1".into(),
        public_key: "pub1".into(),
        listen_port: None,
        address: None,
        endpoint: None,
        allowed_ips: vec![],
        persistent_keepalive: None,
    };

    configurator.add_node(node1).unwrap();

    assert!(matches!(
        configurator.set_psk("node1", "unknown", "psk".into()),
        Err(WireGuardError::NodeNotFound(_))
    ));

    let node2 = WireGuardNode {
        id: "node2".into(),
        private_key: "priv2".into(),
        public_key: "pub2".into(),
        listen_port: None,
        address: None,
        endpoint: None,
        allowed_ips: vec![],
        persistent_keepalive: None,
    };
    configurator.add_node(node2).unwrap();

    assert!(matches!(
        configurator.set_psk("node1", "node2", "".into()),
        Err(WireGuardError::InvalidConfig(_))
    ));

    assert!(configurator.set_psk("node1", "node2", "valid_psk".into()).is_ok());
}

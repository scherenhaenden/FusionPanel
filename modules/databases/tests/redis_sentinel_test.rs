#![forbid(unsafe_code)]

use databases::redis_sentinel::{
    MasterConfig, SentinelClusterManager, SentinelError, SentinelInstance,
};

fn default_master() -> MasterConfig {
    MasterConfig {
        name: "mymaster".to_string(),
        ip: "127.0.0.1".to_string(),
        port: 6379,
        auth_pass: Some("super_secret".to_string()),
        down_after_milliseconds: 5000,
        parallel_syncs: 1,
        failover_timeout: 10000,
        configured_quorum: None,
    }
}

#[test]
fn test_quorum_calculation() {
    let mut manager = SentinelClusterManager::new(default_master());

    // 3 sentinels -> quorum 2
    for i in 1..=3 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    assert_eq!(manager.calculate_optimal_quorum(), 2);

    // 5 sentinels -> quorum 3
    for i in 4..=5 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    assert_eq!(manager.calculate_optimal_quorum(), 3);

    // 7 sentinels -> quorum 4
    for i in 6..=7 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    assert_eq!(manager.calculate_optimal_quorum(), 4);
}

#[test]
fn test_topology_validation_success() {
    let mut manager = SentinelClusterManager::new(default_master());
    for i in 1..=3 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    assert!(manager.validate_topology().is_ok());
}

#[test]
fn test_topology_validation_split_brain_less_than_3() {
    let mut manager = SentinelClusterManager::new(default_master());
    for i in 1..=2 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    let res = manager.validate_topology();
    assert!(matches!(res, Err(SentinelError::SplitBrainRisk(_))));
}

#[test]
fn test_topology_validation_split_brain_even_nodes() {
    let mut manager = SentinelClusterManager::new(default_master());
    for i in 1..=4 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    let res = manager.validate_topology();
    assert!(matches!(res, Err(SentinelError::SplitBrainRisk(_))));
}

#[test]
fn test_topology_validation_missing_auth() {
    let mut master = default_master();
    master.auth_pass = None;
    let mut manager = SentinelClusterManager::new(master);
    for i in 1..=3 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    let res = manager.validate_topology();
    assert!(matches!(res, Err(SentinelError::MissingAuth(_))));
}

#[test]
fn test_topology_validation_invalid_quorum() {
    let mut master = default_master();
    master.configured_quorum = Some(3); // optimal for 3 nodes is 2
    let mut manager = SentinelClusterManager::new(master);
    for i in 1..=3 {
        manager.add_sentinel(SentinelInstance {
            id: format!("s{}", i),
            ip: format!("10.0.0.{}", i),
            port: 26379,
        });
    }
    let res = manager.validate_topology();
    assert!(matches!(res, Err(SentinelError::InvalidQuorum(_))));
}

#[test]
fn test_generate_sentinel_conf() {
    let mut master = default_master();
    master.configured_quorum = Some(2);
    let manager = SentinelClusterManager::new(master);

    let conf = manager.generate_sentinel_conf();

    assert!(conf.contains("sentinel monitor mymaster 127.0.0.1 6379 2"));
    assert!(conf.contains("sentinel auth-pass mymaster super_secret"));
    assert!(conf.contains("sentinel down-after-milliseconds mymaster 5000"));
    assert!(conf.contains("sentinel parallel-syncs mymaster 1"));
    assert!(conf.contains("sentinel failover-timeout mymaster 10000"));
}

#[test]
fn test_parse_sentinel_conf() {
    let conf = "
sentinel monitor mymaster 192.168.1.100 6380 2
sentinel auth-pass mymaster my_password
sentinel down-after-milliseconds mymaster 3000
sentinel parallel-syncs mymaster 2
sentinel failover-timeout mymaster 15000
";

    let master = SentinelClusterManager::parse_sentinel_conf(conf).unwrap();

    assert_eq!(master.name, "mymaster");
    assert_eq!(master.ip, "192.168.1.100");
    assert_eq!(master.port, 6380);
    assert_eq!(master.configured_quorum, Some(2));
    assert_eq!(master.auth_pass, Some("my_password".to_string()));
    assert_eq!(master.down_after_milliseconds, 3000);
    assert_eq!(master.parallel_syncs, 2);
    assert_eq!(master.failover_timeout, 15000);
}

#[test]
fn test_parse_sentinel_conf_missing_monitor() {
    let conf = "sentinel auth-pass mymaster super_secret";
    let res = SentinelClusterManager::parse_sentinel_conf(conf);
    assert!(matches!(res, Err(SentinelError::InvalidConfig(_))));
}

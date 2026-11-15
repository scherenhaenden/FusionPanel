#![forbid(unsafe_code)]

use std::fs;
use tempfile::tempdir;

use administration::{PerformanceTuner, SysctlConfig, TunerError, UlimitConfig};

#[test]
fn test_valid_tuner_creation() {
    let ulimits = UlimitConfig {
        nofile: 1048576,
        nproc: 65535,
        memlock: 1024,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 2097152,
        net_core_somaxconn: 65535,
        net_ipv4_tcp_tw_reuse: 1,
        vm_swappiness: 10,
    };
    let tuner = PerformanceTuner::new(ulimits, sysctls).unwrap();
    assert_eq!(tuner.ulimits.nofile, 1048576);
}

#[test]
fn test_invalid_ulimit_nofile() {
    let ulimits = UlimitConfig {
        nofile: 0,
        nproc: 65535,
        memlock: 1024,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 2097152,
        net_core_somaxconn: 65535,
        net_ipv4_tcp_tw_reuse: 1,
        vm_swappiness: 10,
    };
    match PerformanceTuner::new(ulimits, sysctls) {
        Err(TunerError::InvalidUlimit(msg)) => assert_eq!(msg, "nofile cannot be zero"),
        _ => panic!("Expected InvalidUlimit error"),
    }
}

#[test]
fn test_invalid_sysctl_tw_reuse() {
    let ulimits = UlimitConfig {
        nofile: 1048576,
        nproc: 65535,
        memlock: 1024,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 2097152,
        net_core_somaxconn: 65535,
        net_ipv4_tcp_tw_reuse: 3,
        vm_swappiness: 10,
    };
    match PerformanceTuner::new(ulimits, sysctls) {
        Err(TunerError::InvalidSysctl(msg)) => {
            assert_eq!(msg, "net.ipv4.tcp_tw_reuse must be 0, 1, or 2")
        }
        _ => panic!("Expected InvalidSysctl error"),
    }
}

#[test]
fn test_invalid_sysctl_swappiness() {
    let ulimits = UlimitConfig {
        nofile: 1048576,
        nproc: 65535,
        memlock: 1024,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 2097152,
        net_core_somaxconn: 65535,
        net_ipv4_tcp_tw_reuse: 1,
        vm_swappiness: 101,
    };
    match PerformanceTuner::new(ulimits, sysctls) {
        Err(TunerError::InvalidSysctl(msg)) => {
            assert_eq!(msg, "vm.swappiness must be between 0 and 100")
        }
        _ => panic!("Expected InvalidSysctl error"),
    }
}

#[test]
fn test_generate_configs() {
    let ulimits = UlimitConfig {
        nofile: 1000,
        nproc: 2000,
        memlock: 3000,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 4000,
        net_core_somaxconn: 5000,
        net_ipv4_tcp_tw_reuse: 2,
        vm_swappiness: 20,
    };
    let tuner = PerformanceTuner::new(ulimits, sysctls).unwrap();

    let ulimits_conf = tuner.generate_ulimits_conf();
    assert!(ulimits_conf.contains("* soft nofile 1000"));
    assert!(ulimits_conf.contains("* hard nofile 1000"));
    assert!(ulimits_conf.contains("* soft nproc 2000"));
    assert!(ulimits_conf.contains("* hard nproc 2000"));
    assert!(ulimits_conf.contains("* soft memlock 3000"));
    assert!(ulimits_conf.contains("* hard memlock 3000"));

    let sysctl_conf = tuner.generate_sysctl_conf();
    assert!(sysctl_conf.contains("fs.file-max = 4000"));
    assert!(sysctl_conf.contains("net.core.somaxconn = 5000"));
    assert!(sysctl_conf.contains("net.ipv4.tcp_tw_reuse = 2"));
    assert!(sysctl_conf.contains("vm.swappiness = 20"));
}

#[test]
fn test_apply_to_paths() {
    let ulimits = UlimitConfig {
        nofile: 1000,
        nproc: 2000,
        memlock: 3000,
    };
    let sysctls = SysctlConfig {
        fs_file_max: 4000,
        net_core_somaxconn: 5000,
        net_ipv4_tcp_tw_reuse: 2,
        vm_swappiness: 20,
    };
    let tuner = PerformanceTuner::new(ulimits, sysctls).unwrap();

    let dir = tempdir().unwrap();
    let limits_path = dir.path().join("limits.d/fusionpanel.conf");
    let sysctl_path = dir.path().join("sysctl.d/99-fusionpanel.conf");

    tuner.apply_to_paths(&limits_path, &sysctl_path).unwrap();

    let limits_content = fs::read_to_string(&limits_path).unwrap();
    assert!(limits_content.contains("* soft nofile 1000"));

    let sysctl_content = fs::read_to_string(&sysctl_path).unwrap();
    assert!(sysctl_content.contains("fs.file-max = 4000"));
}

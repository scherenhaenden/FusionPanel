#![forbid(unsafe_code)]

use containers::seccomp_profile::{
    CapabilityBuilder, SeccompAction, SeccompArchitecture, SeccompProfile, SyscallArg,
    SyscallArgOp, SyscallRule,
};
use serde_json::json;

#[test]
fn test_default_seccomp_profile() {
    let profile = SeccompProfile::default();
    assert_eq!(profile.default_action, SeccompAction::SCMP_ACT_ERRNO);
    assert!(profile.architectures.is_none());
    assert!(profile.syscalls.is_none());

    let json = profile.to_json().unwrap();
    let expected = json!({
        "defaultAction": "SCMP_ACT_ERRNO"
    });
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, expected);
}

#[test]
fn test_custom_seccomp_profile() {
    let profile = SeccompProfile::new()
        .with_default_action(SeccompAction::SCMP_ACT_LOG)
        .add_architecture(SeccompArchitecture::SCMP_ARCH_X86_64)
        .add_architecture(SeccompArchitecture::SCMP_ARCH_AARCH64)
        .add_syscall_rule(SyscallRule {
            names: vec!["chmod".to_string(), "chown".to_string()],
            action: SeccompAction::SCMP_ACT_ERRNO,
            args: None,
        })
        .add_syscall_rule(SyscallRule {
            names: vec!["clone".to_string()],
            action: SeccompAction::SCMP_ACT_ALLOW,
            args: Some(vec![SyscallArg {
                index: 0,
                value: 2080505856,
                value_two: None,
                op: SyscallArgOp::SCMP_CMP_MASKED_EQ,
            }]),
        });

    let json = profile.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    let expected = json!({
        "defaultAction": "SCMP_ACT_LOG",
        "architectures": [
            "SCMP_ARCH_X86_64",
            "SCMP_ARCH_AARCH64"
        ],
        "syscalls": [
            {
                "names": ["chmod", "chown"],
                "action": "SCMP_ACT_ERRNO"
            },
            {
                "names": ["clone"],
                "action": "SCMP_ACT_ALLOW",
                "args": [
                    {
                        "index": 0,
                        "value": 2080505856,
                        "op": "SCMP_CMP_MASKED_EQ"
                    }
                ]
            }
        ]
    });

    assert_eq!(parsed, expected);
}

#[test]
fn test_capability_builder_default() {
    let caps = CapabilityBuilder::new().build();
    assert_eq!(caps.drop, vec!["ALL"]);
    assert!(caps.add.is_empty());
}

#[test]
fn test_capability_builder_with_adds() {
    let caps = CapabilityBuilder::new()
        .add_capability("CAP_NET_BIND_SERVICE")
        .add_capability("CAP_CHOWN")
        .add_capability("CAP_NET_BIND_SERVICE") // Should deduplicate
        .build();

    assert_eq!(caps.drop, vec!["ALL"]);
    // Sorted and deduplicated
    assert_eq!(caps.add, vec!["CAP_CHOWN", "CAP_NET_BIND_SERVICE"]);
}

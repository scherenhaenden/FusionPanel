#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SeccompProfile {
    pub default_action: SeccompAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<SeccompArchitecture>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syscalls: Option<Vec<SyscallRule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SeccompAction {
    SCMP_ACT_KILL,
    SCMP_ACT_KILL_PROCESS,
    SCMP_ACT_KILL_THREAD,
    SCMP_ACT_TRAP,
    SCMP_ACT_ERRNO,
    SCMP_ACT_TRACE,
    SCMP_ACT_LOG,
    SCMP_ACT_ALLOW,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SeccompArchitecture {
    SCMP_ARCH_X86,
    SCMP_ARCH_X86_64,
    SCMP_ARCH_X32,
    SCMP_ARCH_ARM,
    SCMP_ARCH_AARCH64,
    SCMP_ARCH_MIPS,
    SCMP_ARCH_MIPS64,
    SCMP_ARCH_MIPS64N32,
    SCMP_ARCH_MIPSEL,
    SCMP_ARCH_MIPSEL64,
    SCMP_ARCH_MIPSEL64N32,
    SCMP_ARCH_PPC,
    SCMP_ARCH_PPC64,
    SCMP_ARCH_PPC64LE,
    SCMP_ARCH_S390,
    SCMP_ARCH_S390X,
    SCMP_ARCH_PARISC,
    SCMP_ARCH_PARISC64,
    SCMP_ARCH_RISCV64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyscallRule {
    pub names: Vec<String>,
    pub action: SeccompAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<SyscallArg>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyscallArg {
    pub index: u32,
    pub value: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_two: Option<u64>,
    pub op: SyscallArgOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SyscallArgOp {
    SCMP_CMP_NEQ,
    SCMP_CMP_LT,
    SCMP_CMP_LE,
    SCMP_CMP_EQ,
    SCMP_CMP_GE,
    SCMP_CMP_GT,
    SCMP_CMP_MASKED_EQ,
}

impl Default for SeccompProfile {
    fn default() -> Self {
        Self {
            default_action: SeccompAction::SCMP_ACT_ERRNO,
            architectures: None,
            syscalls: None,
        }
    }
}

impl SeccompProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_action(mut self, action: SeccompAction) -> Self {
        self.default_action = action;
        self
    }

    pub fn add_architecture(mut self, arch: SeccompArchitecture) -> Self {
        self.architectures.get_or_insert_with(Vec::new).push(arch);
        self
    }

    pub fn add_syscall_rule(mut self, rule: SyscallRule) -> Self {
        self.syscalls.get_or_insert_with(Vec::new).push(rule);
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySet {
    pub drop: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
}

pub struct CapabilityBuilder {
    add: Vec<String>,
}

impl Default for CapabilityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityBuilder {
    pub fn new() -> Self {
        Self { add: Vec::new() }
    }

    pub fn add_capability(mut self, cap: impl Into<String>) -> Self {
        self.add.push(cap.into());
        self
    }

    pub fn build(mut self) -> CapabilitySet {
        self.add.sort();
        self.add.dedup();
        CapabilitySet {
            drop: vec!["ALL".to_string()],
            add: self.add,
        }
    }
}

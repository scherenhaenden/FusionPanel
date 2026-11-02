use crate::compose::models::{ComposeFile, PortMapping};
use std::collections::HashSet;
use std::path::{Component, Path};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ValidationError {
    #[error("Host root mount bypass is not allowed: {0}")]
    RootMountBypass(String),
    #[error("Privileged mode is not allowed without explicit override: {0}")]
    PrivilegedMode(String),
    #[error("Overlapping host ports detected: {0}")]
    OverlappingPorts(String),
    #[error("Invalid port mapping format: {0}")]
    InvalidPortFormat(String),
}

pub fn validate_compose(compose: &ComposeFile) -> Result<(), ValidationError> {
    let mut used_ports = HashSet::new();

    for (service_name, service) in &compose.services {
        // Reject privileged mode
        if service.privileged.unwrap_or(false) {
            return Err(ValidationError::PrivilegedMode(service_name.clone()));
        }

        // Reject host root mount bypass
        for volume in &service.volumes {
            if is_root_mount(volume) {
                return Err(ValidationError::RootMountBypass(service_name.clone()));
            }
        }

        // Reject overlapping host ports
        for port_mapping in &service.ports {
            match port_mapping {
                PortMapping::String(mapping) => {
                    let ports = parse_short_port_string(mapping)
                        .map_err(|_| ValidationError::InvalidPortFormat(mapping.clone()))?;
                    for port in ports {
                        if !used_ports.insert(port.clone()) {
                            return Err(ValidationError::OverlappingPorts(port));
                        }
                    }
                }
                PortMapping::Object { published, .. } => {
                    if let Some(pub_port) = published {
                        let ports = parse_published_port(&pub_port.to_string()).map_err(|_| {
                            ValidationError::InvalidPortFormat(pub_port.to_string())
                        })?;
                        for port in ports {
                            if !used_ports.insert(port.clone()) {
                                return Err(ValidationError::OverlappingPorts(port));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn is_root_mount(volume: &str) -> bool {
    let parts: Vec<&str> = volume.split(':').collect();
    if parts.is_empty() {
        return false;
    }

    let host_path = parts[0];

    // Normalize path to prevent ../../etc attacks
    let mut normalized_path = std::path::PathBuf::new();
    for component in Path::new(host_path).components() {
        match component {
            Component::ParentDir => {
                normalized_path.pop();
            }
            Component::CurDir => {}
            _ => normalized_path.push(component),
        }
    }

    let host_path = normalized_path.to_string_lossy().into_owned();

    if host_path == "/" || host_path == "\\" || host_path.is_empty() {
        return true;
    }

    let sensitive_paths = [
        "/",
        "/etc",
        "/var/run/docker.sock",
        "/bin",
        "/sbin",
        "/usr",
        "/sys",
        "/dev",
        "/var/lib/docker",
    ];

    for p in sensitive_paths {
        if host_path == *p || host_path.starts_with(&format!("{}/", p)) {
            return true;
        }
    }

    false
}

fn parse_short_port_string(mapping: &str) -> Result<Vec<String>, ()> {
    // Handle potential IPv6 addresses with brackets like [::1]:8080:80
    // Remove the brackets part if it exists for simpler splitting
    let mapping_no_ipv6 = if mapping.starts_with('[') {
        if let Some(end) = mapping.find(']') {
            &mapping[end + 1..]
        } else {
            mapping
        }
    } else {
        mapping
    };

    let parts: Vec<&str> = mapping_no_ipv6
        .split(':')
        .filter(|s| !s.is_empty())
        .collect();

    let host_part = if parts.len() >= 2 {
        // HOST:CONTAINER or IP:HOST:CONTAINER
        // After removing IP part, it might be 2 or 1 parts left if it was [::1]:8080 or [::1]:8080:80
        parts[parts.len() - 2]
    } else {
        // CONTAINER only
        return Ok(vec![]);
    };

    if host_part.is_empty() {
        return Ok(vec![]);
    }

    parse_host_port_range(host_part)
}

fn parse_published_port(mapping: &str) -> Result<Vec<String>, ()> {
    parse_host_port_range(mapping)
}

fn parse_host_port_range(host_part: &str) -> Result<Vec<String>, ()> {
    if host_part.contains('-') {
        let range: Vec<&str> = host_part.split('-').collect();
        if range.len() == 2 {
            if let (Ok(start), Ok(end)) = (range[0].parse::<u16>(), range[1].parse::<u16>()) {
                let mut ports = Vec::new();
                for p in start..=end {
                    ports.push(p.to_string());
                }
                return Ok(ports);
            }
        }
        return Err(());
    }

    // Just a single port
    if host_part.parse::<u16>().is_ok() {
        Ok(vec![host_part.to_string()])
    } else {
        Err(())
    }
}

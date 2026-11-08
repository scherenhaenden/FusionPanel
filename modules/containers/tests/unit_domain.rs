#![forbid(unsafe_code)]

use containers::domain::*;

#[test]
fn test_container_id_validation() {
    assert!(ContainerId::new("valid-id-123".to_string()).is_ok());

    // Invalid characters
    assert_eq!(
        ContainerId::new("invalid id!".to_string()),
        Err(DomainError::InvalidContainerId)
    );

    // Empty
    assert_eq!(
        ContainerId::new("".to_string()),
        Err(DomainError::InvalidContainerId)
    );

    // Too long (over 128)
    let long_id = "a".repeat(129);
    assert_eq!(
        ContainerId::new(long_id),
        Err(DomainError::InvalidContainerId)
    );

    // Generation works
    let generated = ContainerId::generate();
    assert!(ContainerId::new(generated.as_str().to_string()).is_ok());
}

#[test]
fn test_image_ref_validation() {
    assert!(ImageRef::new("nginx:latest".to_string()).is_ok());
    assert!(ImageRef::new("ubuntu".to_string()).is_ok());

    // Empty
    assert_eq!(
        ImageRef::new("".to_string()),
        Err(DomainError::InvalidImageRef)
    );

    // Contains space
    assert_eq!(
        ImageRef::new("nginx latest".to_string()),
        Err(DomainError::InvalidImageRef)
    );
}

#[test]
fn test_port_mapping_validation() {
    assert!(PortMapping::new(8080, 80).is_ok());

    // Invalid host port
    assert_eq!(
        PortMapping::new(0, 80),
        Err(DomainError::InvalidHostPort(0))
    );

    // Invalid container port
    assert_eq!(
        PortMapping::new(8080, 0),
        Err(DomainError::InvalidContainerPort(0))
    );
}

#[test]
fn test_mount_point_validation() {
    assert!(MountPoint::new(
        "/host/path".to_string(),
        "/container/path".to_string(),
        true
    )
    .is_ok());

    // Empty source
    assert_eq!(
        MountPoint::new("".to_string(), "/container/path".to_string(), true),
        Err(DomainError::InvalidMountPoint(
            "Source path cannot be empty".to_string()
        ))
    );

    // Empty target
    assert_eq!(
        MountPoint::new("/host/path".to_string(), "".to_string(), true),
        Err(DomainError::InvalidMountPoint(
            "Target path cannot be empty".to_string()
        ))
    );
}

#[test]
fn test_container_status_transitions() {
    // Valid transitions
    assert_eq!(
        ContainerStatus::Created.transition_to(ContainerStatus::Running),
        Ok(ContainerStatus::Running)
    );
    assert_eq!(
        ContainerStatus::Running.transition_to(ContainerStatus::Stopped),
        Ok(ContainerStatus::Stopped)
    );
    assert_eq!(
        ContainerStatus::Stopped.transition_to(ContainerStatus::Removed),
        Ok(ContainerStatus::Removed)
    );
    assert_eq!(
        ContainerStatus::Running.transition_to(ContainerStatus::Exited),
        Ok(ContainerStatus::Exited)
    );
    assert_eq!(
        ContainerStatus::Exited.transition_to(ContainerStatus::Running),
        Ok(ContainerStatus::Running)
    );

    // Same state transition (no-op)
    assert_eq!(
        ContainerStatus::Running.transition_to(ContainerStatus::Running),
        Ok(ContainerStatus::Running)
    );

    // Invalid transitions
    assert_eq!(
        ContainerStatus::Removed.transition_to(ContainerStatus::Running),
        Err(DomainError::InvalidStatusTransition {
            from: ContainerStatus::Removed,
            to: ContainerStatus::Running,
        })
    );

    assert_eq!(
        ContainerStatus::Exited.transition_to(ContainerStatus::Created),
        Err(DomainError::InvalidStatusTransition {
            from: ContainerStatus::Exited,
            to: ContainerStatus::Created,
        })
    );
}

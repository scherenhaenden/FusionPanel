use fp_deployments::domain::{
    Deployment, DeploymentId, DeploymentStatus, Environment, ReleaseId, TransitionError,
};
use uuid::Uuid;

#[test]
fn test_valid_transitions() {
    let mut deployment = Deployment::new(
        DeploymentId(Uuid::new_v4()),
        ReleaseId(Uuid::new_v4()),
        Environment::Production,
    );
    assert_eq!(deployment.status, DeploymentStatus::Pending);

    assert!(deployment.transition_to(DeploymentStatus::Building).is_ok());
    assert_eq!(deployment.status, DeploymentStatus::Building);

    assert!(deployment
        .transition_to(DeploymentStatus::Deploying)
        .is_ok());
    assert_eq!(deployment.status, DeploymentStatus::Deploying);

    assert!(deployment.transition_to(DeploymentStatus::Success).is_ok());
    assert_eq!(deployment.status, DeploymentStatus::Success);

    assert!(deployment
        .transition_to(DeploymentStatus::RolledBack)
        .is_ok());
    assert_eq!(deployment.status, DeploymentStatus::RolledBack);
}

#[test]
fn test_failure_transitions() {
    // Pending -> Failed
    let mut d1 = Deployment::new(
        DeploymentId(Uuid::new_v4()),
        ReleaseId(Uuid::new_v4()),
        Environment::Production,
    );
    assert!(d1.transition_to(DeploymentStatus::Failed).is_ok());

    // Building -> Failed
    let mut d2 = Deployment::new(
        DeploymentId(Uuid::new_v4()),
        ReleaseId(Uuid::new_v4()),
        Environment::Production,
    );
    d2.transition_to(DeploymentStatus::Building).unwrap();
    assert!(d2.transition_to(DeploymentStatus::Failed).is_ok());

    // Deploying -> Failed
    let mut d3 = Deployment::new(
        DeploymentId(Uuid::new_v4()),
        ReleaseId(Uuid::new_v4()),
        Environment::Production,
    );
    d3.transition_to(DeploymentStatus::Building).unwrap();
    d3.transition_to(DeploymentStatus::Deploying).unwrap();
    assert!(d3.transition_to(DeploymentStatus::Failed).is_ok());
}

#[test]
fn test_invalid_transitions() {
    let mut deployment = Deployment::new(
        DeploymentId(Uuid::new_v4()),
        ReleaseId(Uuid::new_v4()),
        Environment::Production,
    );

    // Pending -> Success
    let result = deployment.transition_to(DeploymentStatus::Success);
    assert_eq!(
        result,
        Err(TransitionError::InvalidTransition {
            from: DeploymentStatus::Pending,
            to: DeploymentStatus::Success
        })
    );

    // Pending -> Deploying
    let result = deployment.transition_to(DeploymentStatus::Deploying);
    assert_eq!(
        result,
        Err(TransitionError::InvalidTransition {
            from: DeploymentStatus::Pending,
            to: DeploymentStatus::Deploying
        })
    );

    // Building -> Success
    deployment
        .transition_to(DeploymentStatus::Building)
        .unwrap();
    let result = deployment.transition_to(DeploymentStatus::Success);
    assert_eq!(
        result,
        Err(TransitionError::InvalidTransition {
            from: DeploymentStatus::Building,
            to: DeploymentStatus::Success
        })
    );

    // Deploying -> Building
    deployment
        .transition_to(DeploymentStatus::Deploying)
        .unwrap();
    let result = deployment.transition_to(DeploymentStatus::Building);
    assert_eq!(
        result,
        Err(TransitionError::InvalidTransition {
            from: DeploymentStatus::Deploying,
            to: DeploymentStatus::Building
        })
    );

    // Success -> Building
    deployment.transition_to(DeploymentStatus::Success).unwrap();
    let result = deployment.transition_to(DeploymentStatus::Building);
    assert_eq!(
        result,
        Err(TransitionError::InvalidTransition {
            from: DeploymentStatus::Success,
            to: DeploymentStatus::Building
        })
    );
}

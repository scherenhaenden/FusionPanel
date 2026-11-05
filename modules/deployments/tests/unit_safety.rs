use deployments::types::{ReleaseId, DeploymentState};
use deployments::release::ReleaseManager;
use tempfile::TempDir;

#[test]
fn test_release_id_path_traversal_prevention() {
    let hostile_inputs = vec![
        "../../escaped",
        "/etc/passwd",
        "some/path",
        "C:\\Windows\\System32",
        "invalid chars !",
        " ",
        "",
    ];

    for input in hostile_inputs {
        let res = ReleaseId::parse(input);
        assert!(res.is_err(), "Hostile input '{}' should have failed parsing", input);
    }
}

#[test]
fn test_release_id_valid_parsing() {
    let valid_inputs = vec![
        "release-1",
        "rel_2",
        "12345",
        "a-b_c-1_2-3",
    ];

    for input in valid_inputs {
        let res = ReleaseId::parse(input);
        assert!(res.is_ok(), "Valid input '{}' failed to parse", input);
    }
}

#[test]
fn test_release_id_hostile_deserialization() {
    let json = r#"{"release_id":"../../escaped"}"#;
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[allow(dead_code)]
        release_id: ReleaseId,
    }

    let res: Result<Wrapper, _> = serde_json::from_str(json);
    assert!(res.is_err());
}

#[test]
fn test_preexisting_symlink_prevention() {
    let temp = TempDir::new().unwrap();
    let mgr = ReleaseManager::new(temp.path());
    mgr.initialize().unwrap();

    let rel_id = ReleaseId::parse("release-symlink").unwrap();
    let rel_path = mgr.releases_dir().join(rel_id.as_str());

    // Create a hostile symlink where the release dir should be
    std::os::unix::fs::symlink("/etc", &rel_path).unwrap();

    let res = mgr.prepare_release_dir(&rel_id);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_terminal_states_no_transitions() {
    let states = vec![
        DeploymentState::Queued,
        DeploymentState::PendingApproval,
        DeploymentState::Building,
        DeploymentState::Deploying,
        DeploymentState::Active,
        DeploymentState::RolledBack,
        DeploymentState::Failed,
        DeploymentState::Cancelled,
    ];

    let terminal_states = vec![
        DeploymentState::Active,
        DeploymentState::RolledBack,
        DeploymentState::Failed,
        DeploymentState::Cancelled,
    ];

    for term in terminal_states {
        assert!(term.is_terminal());
        for next in &states {
            // Only Active -> RolledBack is allowed among terminals
            if term == DeploymentState::Active && *next == DeploymentState::RolledBack {
                assert!(term.can_transition_to(*next));
            } else {
                assert!(!term.can_transition_to(*next), "{:?} should not transition to {:?}", term, next);
            }
        }
    }
}

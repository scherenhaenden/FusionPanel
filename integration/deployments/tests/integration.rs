use axum::{
    body::Body,
    http::{header, StatusCode},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

use std::fs;
use std::process::Command;

use deployments::build::{execute_build, plan_build};
use deployments::webhook::{ReplayGuard, compute_signature};
use deployments::release::ReleaseManager;
use deployments::types::{DeploymentId, DeploymentState, ReleaseId, BuildSpec, DeploymentPlan, DeploymentStrategy, Environment, GitSource, ProjectType, SiteId};

use deployments_integration::{app, AppState};

#[tokio::test]
async fn test_deployment_lifecycle_and_webhook() {
    let temp_dir = std::env::temp_dir().join(format!("fusionpanel-deploy-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let releases_base_dir = temp_dir.join("site_root");
    let git_repo_dir = temp_dir.join("repo");
    std::fs::create_dir_all(&git_repo_dir).unwrap();

    let webhook_secret = b"super-secret".to_vec();
    let replay_guard = Arc::new(ReplayGuard::new(300));
    let release_manager = Arc::new(Mutex::new(ReleaseManager::new(&releases_base_dir)));
    {
         let rm = release_manager.lock().await;
         rm.initialize().unwrap();
    }
    let deployments = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let approvals = Arc::new(Mutex::new(std::collections::HashMap::new()));

    let state = Arc::new(AppState {
        webhook_secret: webhook_secret.clone(),
        replay_guard,
        release_manager: release_manager.clone(),
        deployments: deployments.clone(),
        approvals: approvals.clone(),
    });

    let router = app(state.clone());

    let run_git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(&git_repo_dir)
            .args(args)
            .output()
            .expect("failed to run git command");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };

    run_git(&["init"]);
    run_git(&["config", "user.email", "deploy-tester@example.com"]);
    run_git(&["config", "user.name", "Deploy Tester"]);

    let index_file = git_repo_dir.join("index.html");
    fs::write(&index_file, b"<h1>Version 1.0</h1>").unwrap();
    run_git(&["add", "index.html"]);
    run_git(&["commit", "-m", "Initial commit (v1.0)"]);

    let out1 = Command::new("git")
        .current_dir(&git_repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let commit1_sha = String::from_utf8_lossy(&out1.stdout).trim().to_string();

    // 1. Webhook - Push Event
    let payload = serde_json::json!({
        "ref": "refs/heads/master",
        "before": "0000000000000000000000000000000000000000",
        "after": commit1_sha.clone(),
        "repository": {
            "clone_url": "https://github.com/scherenhaenden/FusionPanel.git"
        },
        "sender": {
            "login": "octocat"
        }
    });
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let signature = compute_signature(&webhook_secret, &payload_bytes).unwrap();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/webhook")
        .header("X-GitHub-Event", "push")
        .header("X-GitHub-Delivery", "delivery-1")
        .header("X-Hub-Signature-256", &signature)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload_bytes.clone()))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // 2. Create Deployment
    let release1 = ReleaseId::parse(&format!("rel-1-{}", &commit1_sha[..7])).unwrap();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/deployments")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "initiator": "alice",
                "release_id": release1.as_str()
            })).unwrap()
        ))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = http_body_util::BodyExt::collect(response.into_body()).await.unwrap().to_bytes();
    let dep_id: DeploymentId = serde_json::from_slice(&body_bytes).unwrap();

    // 3. Simulate build execution using real components
    {
        let rm = release_manager.lock().await;
        let rel1_dir = rm.prepare_release_dir(&release1).unwrap();

        let plan1 = DeploymentPlan {
            plan_id: "plan-1".to_string(),
            site_id: SiteId("site-integration".to_string()),
            environment: Environment::Production,
            strategy: DeploymentStrategy::ReleaseDirectorySwitch,
            git_source: GitSource {
                repo_url: git_repo_dir.display().to_string(),
                commit_sha: commit1_sha.clone(),
                branch: Some("master".to_string()),
                ref_name: Some("refs/heads/master".to_string()),
            },
            build_spec: BuildSpec {
                project_type: ProjectType::Static,
                command: None,
                artifact_path: None,
            },
            scheduled_at: None,
            requires_approval: false,
        };

        let planned_cmd1 = plan_build(&git_repo_dir, &plan1.build_spec, &rel1_dir).unwrap();
        let build_res1 = execute_build(&git_repo_dir, &planned_cmd1, &[]).unwrap();
        assert!(build_res1.success);
    }

    // 4. Approve Deployment (Bob, not Alice)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/deployments/{}/approve", dep_id.0))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "approver": "bob",
                "release_id": release1.as_str()
            })).unwrap()
        ))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 5. Check Deployment state
    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/deployments/{}", dep_id.0))
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = http_body_util::BodyExt::collect(response.into_body()).await.unwrap().to_bytes();
    let record: deployments::workflow::DeploymentRecord = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(record.current_state, DeploymentState::Active);

    // Check observed content continuity
    {
        let rm = release_manager.lock().await;
        let symlink = rm.current_symlink();
        let content = std::fs::read_to_string(symlink.join("index.html")).unwrap();
        assert_eq!(content, "<h1>Version 1.0</h1>");
    }

    // Commit 2: Update version
    fs::write(&index_file, b"<h1>Version 2.0</h1>").unwrap();
    run_git(&["commit", "-am", "Upgrade to v2.0"]);
    let out2 = Command::new("git")
        .current_dir(&git_repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let commit2_sha = String::from_utf8_lossy(&out2.stdout).trim().to_string();

    // 6. Second deployment (v2)
    let release2 = ReleaseId::parse(&format!("rel-2-{}", &commit2_sha[..7])).unwrap();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/deployments")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "initiator": "alice",
                "release_id": release2.as_str()
            })).unwrap()
        ))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    let body_bytes = http_body_util::BodyExt::collect(response.into_body()).await.unwrap().to_bytes();
    let dep_id2: DeploymentId = serde_json::from_slice(&body_bytes).unwrap();

    {
        let rm = release_manager.lock().await;
        let rel2_dir = rm.prepare_release_dir(&release2).unwrap();
        let plan2 = DeploymentPlan {
            plan_id: "plan-2".to_string(),
            site_id: SiteId("site-integration".to_string()),
            environment: Environment::Production,
            strategy: DeploymentStrategy::ReleaseDirectorySwitch,
            git_source: GitSource {
                repo_url: git_repo_dir.display().to_string(),
                commit_sha: commit2_sha.clone(),
                branch: Some("master".to_string()),
                ref_name: Some("refs/heads/master".to_string()),
            },
            build_spec: BuildSpec {
                project_type: ProjectType::Static,
                command: None,
                artifact_path: None,
            },
            scheduled_at: None,
            requires_approval: false,
        };

        let planned_cmd2 = plan_build(&git_repo_dir, &plan2.build_spec, &rel2_dir).unwrap();
        let build_res2 = execute_build(&git_repo_dir, &planned_cmd2, &[]).unwrap();
        assert!(build_res2.success);
    }

    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/deployments/{}/approve", dep_id2.0))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "approver": "bob",
                "release_id": release2.as_str()
            })).unwrap()
        ))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    {
        let rm = release_manager.lock().await;
        let symlink = rm.current_symlink();
        let content = std::fs::read_to_string(symlink.join("index.html")).unwrap();
        assert_eq!(content, "<h1>Version 2.0</h1>");
    }

    // 7. Health failure rollback
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/deployments/{}/health_failure", dep_id2.0))
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    let status = response.status();
    if status != StatusCode::OK {
        let err_body = http_body_util::BodyExt::collect(response.into_body()).await.unwrap().to_bytes();
        let err_str = String::from_utf8_lossy(&err_body);
        panic!("Health failure rollback failed with status {}: {}", status, err_str);
    }
    assert_eq!(status, StatusCode::OK);

    {
        let rm = release_manager.lock().await;
        let symlink = rm.current_symlink();
        let content = std::fs::read_to_string(symlink.join("index.html")).unwrap();
        assert_eq!(content, "<h1>Version 1.0</h1>");
    }

    // 8. Replay rejection on webhook
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/webhook")
        .header("X-GitHub-Event", "push")
        .header("X-GitHub-Delivery", "delivery-1") // Same delivery ID
        .header("X-Hub-Signature-256", &signature)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload_bytes.clone()))
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    std::fs::remove_dir_all(temp_dir).unwrap();
}

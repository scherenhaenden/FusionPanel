use deployments::build::{execute_build, plan_build};
use deployments::release::ReleaseManager;
use deployments::types::{
    BuildSpec, DeploymentId, DeploymentPlan, DeploymentState, DeploymentStrategy, Environment,
    GitSource, ProjectType, ReleaseId, SiteId,
};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Integration test verifying full local Git repo, real build execution, isolated release directories,
/// atomic symlink cutover, and real rollback verifying prior release content.
#[test]
fn test_real_git_build_deploy_and_rollback_cycle() {
    let base_temp = TempDir::new().unwrap();
    let git_repo_dir = base_temp.path().join("repo");
    let releases_base_dir = base_temp.path().join("site_root");
    fs::create_dir_all(&git_repo_dir).unwrap();

    // 1. Initialize real local Git repository
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

    // Commit 1: Initial version
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

    // Setup release manager
    let rel_mgr = ReleaseManager::new(&releases_base_dir);
    rel_mgr.initialize().unwrap();

    // Build and deploy Release 1
    let rel1_id = ReleaseId::parse(&format!("rel-1-{}", &commit1_sha[..7])).unwrap();
    let rel1_dir = rel_mgr.prepare_release_dir(&rel1_id).unwrap();

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

    let dep1 = DeploymentId::new();
    let applied1 = rel_mgr.activate_directory_switch(&dep1, &rel1_id).unwrap();
    assert_eq!(applied1.state, DeploymentState::Active);

    // Verify currently served content is v1.0
    let served1 = fs::read_to_string(rel_mgr.current_symlink().join("index.html")).unwrap();
    assert_eq!(served1, "<h1>Version 1.0</h1>");

    // Commit 2: Update version
    fs::write(&index_file, b"<h1>Version 2.0</h1>").unwrap();
    run_git(&["commit", "-am", "Upgrade to v2.0"]);

    let out2 = Command::new("git")
        .current_dir(&git_repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let commit2_sha = String::from_utf8_lossy(&out2.stdout).trim().to_string();

    // Build and deploy Release 2
    let rel2_id = ReleaseId::parse(&format!("rel-2-{}", &commit2_sha[..7])).unwrap();
    let rel2_dir = rel_mgr.prepare_release_dir(&rel2_id).unwrap();

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

    let dep2 = DeploymentId::new();
    let applied2 = rel_mgr.activate_directory_switch(&dep2, &rel2_id).unwrap();
    assert_eq!(applied2.state, DeploymentState::Active);
    assert_eq!(applied2.previous_release_id, Some(rel1_id.clone()));

    // Verify currently served content is v2.0
    let served2 = fs::read_to_string(rel_mgr.current_symlink().join("index.html")).unwrap();
    assert_eq!(served2, "<h1>Version 2.0</h1>");

    // Perform atomic rollback to Release 1
    let roll_dep = DeploymentId::new();
    let roll_result = rel_mgr
        .rollback_directory_switch(&roll_dep, Some(&rel1_id))
        .unwrap();
    assert_eq!(roll_result.state, DeploymentState::RolledBack);

    // Verify currently served content is restored to v1.0
    let served_after_rollback =
        fs::read_to_string(rel_mgr.current_symlink().join("index.html")).unwrap();
    assert_eq!(served_after_rollback, "<h1>Version 1.0</h1>");
}

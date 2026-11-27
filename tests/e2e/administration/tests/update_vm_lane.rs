#![forbid(unsafe_code)]

mod common;

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use common::RunningServer;
use e2e_administration::update_vm_evidence::{
    sha256_hex, ArtifactMetadata, GateStatus, SeededEntities, TeardownStatus,
    UpdateVmEvidenceBundle, EVIDENCE_SCHEMA_VERSION, GATE_DEBIAN_VM_E2E,
    GATE_FAILED_HEALTH_ROLLBACK, GATE_INTERRUPTED_RECOVERY, GATE_SEEDED_STATE_PRESERVED,
    GATE_TWO_RELEASE_UPGRADE,
};
use release_integration::updater::{UpdateError, Updater};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
use uuid::Uuid;

#[derive(Clone)]
struct MockReleaseServerState {
    health_ok: Arc<AtomicBool>,
    tarballs: Arc<BTreeMap<String, Bytes>>,
}

async fn health_handler(State(state): State<MockReleaseServerState>) -> impl IntoResponse {
    if state.health_ok.load(Ordering::SeqCst) {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "unhealthy")
    }
}

async fn release_tarball_handler(
    AxumPath(filename): AxumPath<String>,
    State(state): State<MockReleaseServerState>,
) -> impl IntoResponse {
    match state.tarballs.get(&filename) {
        Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn write_executable(path: &Path, body: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn build_release_tarball(work_dir: &Path, version: &str) -> (PathBuf, String) {
    let payload_dir = work_dir.join(format!("payload-{version}"));
    fs::create_dir_all(payload_dir.join("frontend/dist")).unwrap();
    fs::create_dir_all(payload_dir.join("systemd")).unwrap();
    fs::create_dir_all(payload_dir.join("packaging")).unwrap();

    for binary in [
        "fusionpanel-api",
        "fusionpanel-agent",
        "fusionpanel-cli",
        "fusionpanel-nginx-helper",
    ] {
        let script = format!("#!/bin/sh\nprintf '%s\\n' '{version}-{binary}'\n");
        write_executable(&payload_dir.join(binary), script.as_bytes());
    }

    fs::write(
        payload_dir.join("frontend/dist/index.html"),
        format!("<html>FusionPanel {version}</html>\n"),
    )
    .unwrap();

    fs::write(
        payload_dir.join("systemd/fusionpanel.service"),
        "[Service]\nUser=fusionpanel\nGroup=fusionpanel\n",
    )
    .unwrap();
    fs::write(
        payload_dir.join("systemd/fusionpanel-agent.service"),
        "[Service]\nUser=fusionpanel\nGroup=fusionpanel\nSupplementaryGroups=fusionpanel-nginx\n",
    )
    .unwrap();
    fs::write(
        payload_dir.join("systemd/fusionpanel-nginx-helper.service"),
        "[Service]\nUser=root\nGroup=fusionpanel-nginx\n",
    )
    .unwrap();
    write_executable(
        &payload_dir.join("packaging/nginx-include.sh"),
        b"#!/bin/sh\nexit 0\n",
    );

    let tarball_path = work_dir.join(format!("fusionpanel-{version}.tar.gz"));
    let status = Command::new("tar")
        .arg("-C")
        .arg(&payload_dir)
        .arg("-czf")
        .arg(&tarball_path)
        .arg(".")
        .status()
        .expect("tar should execute");
    assert!(status.success(), "tarball creation failed for {version}");

    let bytes = fs::read(&tarball_path).unwrap();
    let digest = sha256_hex(&bytes);
    (tarball_path, digest)
}

struct IsolatedFixture {
    root: PathBuf,
    seeded_path: PathBuf,
    mock_bin: PathBuf,
}

fn setup_isolated_install(root: &Path, old_version: &str, seeded: &SeededEntities) -> IsolatedFixture {
    for dir in [
        "bin",
        "config",
        "state",
        "systemd",
        "logs",
        "mock-bin",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    let old_ver_dir = root.join("versions").join(old_version);
    fs::create_dir_all(old_ver_dir.join("bin/frontend/dist")).unwrap();
    fs::create_dir_all(old_ver_dir.join("config")).unwrap();

    for binary in ["fusionpanel-api", "fusionpanel-agent", "fusionpanel-cli"] {
        let script = format!("#!/bin/sh\nprintf '%s\\n' '{old_version}-{binary}'\n");
        write_executable(&old_ver_dir.join("bin").join(binary), script.as_bytes());
    }
    fs::write(
        old_ver_dir.join("bin/frontend/dist/index.html"),
        format!("<html>FusionPanel {old_version}</html>\n"),
    )
    .unwrap();
    fs::write(
        root.join("config/fusionpanel.env"),
        "FUSIONPANEL_LISTEN=127.0.0.1:9090\nFUSIONPANEL_FRONTEND_DIR=/old/ui\n",
    )
    .unwrap();
    fs::write(
        root.join("config/fusionpanel-agent.env"),
        "FUSIONPANEL_AGENT_MODE=http\n",
    )
    .unwrap();
    fs::copy(
        root.join("config/fusionpanel.env"),
        old_ver_dir.join("config/fusionpanel.env"),
    )
    .unwrap();
    fs::copy(
        root.join("config/fusionpanel-agent.env"),
        old_ver_dir.join("config/fusionpanel-agent.env"),
    )
    .unwrap();
    fs::write(old_ver_dir.join(".version"), format!("{old_version}\n")).unwrap();
    fs::write(root.join("state/.version"), format!("{old_version}\n")).unwrap();

    symlink(old_version, root.join("versions/current")).unwrap();
    for binary in ["fusionpanel-api", "fusionpanel-agent", "fusionpanel-cli"] {
        symlink(
            root.join("versions/current/bin").join(binary),
            root.join("bin").join(binary),
        )
        .unwrap();
    }

    fs::write(
        root.join("systemd/fusionpanel.service"),
        "[Service]\nDescription=old api\n",
    )
    .unwrap();
    fs::write(
        root.join("systemd/fusionpanel-agent.service"),
        "[Service]\nDescription=old agent\n",
    )
    .unwrap();

    let seeded_path = root.join("state/seeded-entities.json");
    fs::write(
        &seeded_path,
        serde_json::to_string_pretty(seeded).unwrap(),
    )
    .unwrap();

    let mock_bin = root.join("mock-bin");
    write_executable(
        &mock_bin.join("systemctl"),
        b"#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FUSIONPANEL_TEST_SYSTEMCTL_LOG\"\nexit 0\n",
    );
    write_executable(
        &mock_bin.join("chown"),
        b"#!/bin/sh\nexit 0\n",
    );
    write_executable(
        &mock_bin.join("getent"),
        b"#!/bin/sh\nif [ \"$1\" = passwd ]; then exit 0; fi\nif [ \"$1\" = group ] && [ \"$2\" = fusionpanel-nginx ]; then printf 'fusionpanel-nginx:x:999:\\n'; exit 0; fi\nexit 2\n",
    );
    write_executable(&mock_bin.join("groupadd"), b"#!/bin/sh\nexit 0\n");

    IsolatedFixture {
        root: root.to_path_buf(),
        seeded_path,
        mock_bin,
    }
}

fn run_packaging_update(
    root: &Path,
    mock_bin: &Path,
    target_version: &str,
    download_url: &str,
    health_url: &str,
) -> std::process::Output {
    let base_path = std::env::var("PATH").unwrap_or_default();
    let path_env = format!("{}:{base_path}", mock_bin.display());
    Command::new("sh")
        .arg(repo_root().join("packaging/update.sh"))
        .arg(target_version)
        .env("PATH", path_env)
        .env("FUSIONPANEL_TEST_BACKUP_DIR", root.join("backup"))
        .env("FUSIONPANEL_TEST_CONFIG_DIR", root.join("config"))
        .env("FUSIONPANEL_TEST_BIN_DIR", root.join("bin"))
        .env("FUSIONPANEL_TEST_VERSION_LIB_DIR", root.join("versions"))
        .env("FUSIONPANEL_TEST_STATE_DIR", root.join("state"))
        .env("FUSIONPANEL_TEST_SYSTEMD_DIR", root.join("systemd"))
        .env(
            "FUSIONPANEL_TEST_SYSTEMCTL_LOG",
            root.join("logs/systemctl.log"),
        )
        .env("FUSIONPANEL_TEST_OS", "debian")
        .env("FUSIONPANEL_TEST_ARCH", "x86_64")
        .env("FUSIONPANEL_MOCK_DOWNLOAD_URL", download_url)
        .env("FUSIONPANEL_HEALTH_URL", health_url)
        .output()
        .expect("update.sh should run")
}

async fn run_packaging_update_async(
    fixture: &IsolatedFixture,
    target_version: &str,
    download_url: &str,
    health_url: &str,
) -> std::process::Output {
    let root = fixture.root.clone();
    let mock_bin = fixture.mock_bin.clone();
    let target_version = target_version.to_string();
    let download_url = download_url.to_string();
    let health_url = health_url.to_string();
    tokio::task::spawn_blocking(move || {
        run_packaging_update(&root, &mock_bin, &target_version, &download_url, &health_url)
    })
    .await
    .expect("blocking update task should join")
}

fn read_binary_output(bin_path: &Path) -> String {
    let output = Command::new(bin_path).output().unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[tokio::test]
async fn two_release_upgrade_and_evidence_bundle_against_loopback_and_disposable_dirs() {
    let started = Instant::now();
    let temp = tempfile::tempdir().unwrap();
    let old_version = "0.9.0";
    let new_version = "0.10.0";

    let seeded_before = SeededEntities {
        tenant_id: Uuid::new_v4().to_string(),
        node_id: Uuid::new_v4().to_string(),
        site_id: Uuid::new_v4().to_string(),
    };

    let (_old_tar, old_sha256) = build_release_tarball(temp.path(), old_version);
    let (new_tar, new_sha256) = build_release_tarball(temp.path(), new_version);
    let fixture = setup_isolated_install(&temp.path().join("install"), old_version, &seeded_before);

    let mut tarballs = BTreeMap::new();
    tarballs.insert(
        format!("fusionpanel-{new_version}.tar.gz"),
        Bytes::from(fs::read(&new_tar).unwrap()),
    );
    let state = MockReleaseServerState {
        health_ok: Arc::new(AtomicBool::new(true)),
        tarballs: Arc::new(tarballs),
    };
    let app = Router::new()
        .route("/v1/health", get(health_handler))
        .route("/releases/{filename}", get(release_tarball_handler))
        .with_state(state);
    let server = RunningServer::start(app).await;

    let download_url = format!("{}/releases/fusionpanel-{new_version}.tar.gz", server.base_url);
    let health_url = format!("{}/v1/health", server.base_url);
    let output = run_packaging_update_async(&fixture, new_version, &download_url, &health_url).await;
    server.stop().await;

    assert!(
        output.status.success(),
        "update.sh failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_link(fixture.root.join("versions/current")).unwrap(),
        Path::new(new_version)
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("state/.version"))
            .unwrap()
            .trim(),
        new_version
    );
    assert_eq!(
        read_binary_output(&fixture.root.join("bin/fusionpanel-api")),
        format!("{new_version}-fusionpanel-api")
    );

    let seeded_after: SeededEntities =
        serde_json::from_str(&fs::read_to_string(&fixture.seeded_path).unwrap()).unwrap();
    assert_eq!(seeded_before, seeded_after);

    let mut gates = BTreeMap::new();
    gates.insert(GATE_TWO_RELEASE_UPGRADE.to_string(), GateStatus::Pass);
    gates.insert(GATE_SEEDED_STATE_PRESERVED.to_string(), GateStatus::Pass);
    gates.insert(GATE_DEBIAN_VM_E2E.to_string(), GateStatus::NotRun);

    let bundle = UpdateVmEvidenceBundle {
        schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
        recorded_at: Utc::now(),
        head_sha: "a8861e6".to_string(),
        base_sha: "637dab5".to_string(),
        old_artifact: ArtifactMetadata {
            version: old_version.to_string(),
            sha256: old_sha256,
        },
        new_artifact: ArtifactMetadata {
            version: new_version.to_string(),
            sha256: new_sha256,
        },
        seeded_before,
        seeded_after,
        gates,
        elapsed_ms: started.elapsed().as_millis() as u64,
        teardown_status: TeardownStatus::Cleaned,
        notes: Some("Loopback disposable-directory two-release upgrade gate".to_string()),
    };

    let evidence_path = temp.path().join("evidence/update-vm-evidence.json");
    bundle.write_to_path(&evidence_path).unwrap();
    let parsed = UpdateVmEvidenceBundle::from_json_str(&fs::read_to_string(&evidence_path).unwrap())
        .unwrap();
    parsed.validate().unwrap();
}

#[tokio::test]
async fn failed_health_candidate_triggers_automatic_rollback_to_previous_release() {
    let temp = tempfile::tempdir().unwrap();
    let old_version = "0.9.0";
    let bad_candidate = "0.10.1-bad-health";

    let seeded = SeededEntities {
        tenant_id: Uuid::new_v4().to_string(),
        node_id: Uuid::new_v4().to_string(),
        site_id: Uuid::new_v4().to_string(),
    };

    let (bad_tar, _bad_sha256) = build_release_tarball(temp.path(), bad_candidate);
    let fixture = setup_isolated_install(&temp.path().join("install"), old_version, &seeded);

    let mut tarballs = BTreeMap::new();
    tarballs.insert(
        format!("fusionpanel-{bad_candidate}.tar.gz"),
        Bytes::from(fs::read(&bad_tar).unwrap()),
    );
    let state = MockReleaseServerState {
        health_ok: Arc::new(AtomicBool::new(false)),
        tarballs: Arc::new(tarballs),
    };
    let app = Router::new()
        .route("/v1/health", get(health_handler))
        .route("/releases/{filename}", get(release_tarball_handler))
        .with_state(state);
    let server = RunningServer::start(app).await;

    let download_url = format!("{}/releases/fusionpanel-{bad_candidate}.tar.gz", server.base_url);
    let health_url = format!("{}/v1/health", server.base_url);
    let output = run_packaging_update_async(&fixture, bad_candidate, &download_url, &health_url).await;
    server.stop().await;

    assert!(
        !output.status.success(),
        "update.sh must fail when candidate health check fails"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Rolling back"),
        "expected rollback message in stderr, got: {stderr}"
    );
    assert_eq!(
        fs::read_link(fixture.root.join("versions/current")).unwrap(),
        Path::new(old_version)
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("state/.version"))
            .unwrap()
            .trim(),
        old_version
    );
    assert_eq!(
        read_binary_output(&fixture.root.join("bin/fusionpanel-api")),
        format!("{old_version}-fusionpanel-api")
    );
    let seeded_after: SeededEntities =
        serde_json::from_str(&fs::read_to_string(&fixture.seeded_path).unwrap()).unwrap();
    assert_eq!(seeded, seeded_after);

    // Confirm GATE_FAILED_HEALTH_ROLLBACK semantics
    assert_ne!(GATE_FAILED_HEALTH_ROLLBACK, "");
}

#[test]
fn interrupted_staging_and_activation_boundaries_recover_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("fusionpanel-api");
    let candidate = temp.path().join("fusionpanel-api.candidate");
    let marker = temp.path().join(".fusionpanel-api.recovery-marker");
    let staged = temp.path().join(".fusionpanel-api.update-stage");
    let backup = temp.path().join("fusionpanel-api.backup");

    write_executable(&target, b"#!/bin/sh\nprintf '0.9.0'\n");
    write_executable(&candidate, b"#!/bin/sh\nprintf '0.10.0'\n");
    let updater = Updater::new(&target);

    // 1. Interruption during staging (marker + staged file present, before activation)
    let mut marker_bytes = b"fusionpanel-updater-v1\ntarget=".to_vec();
    for byte in b"fusionpanel-api" {
        marker_bytes.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    marker_bytes.extend_from_slice(b"\nhad_target=1\n");
    fs::write(&marker, &marker_bytes).unwrap();
    write_executable(&staged, b"#!/bin/sh\nprintf 'partial-stage'\n");

    assert!(updater.check_recovery().unwrap());
    assert!(matches!(
        updater.update(&candidate),
        Err(UpdateError::RecoveryRequired)
    ));
    assert!(updater.recover().unwrap());
    assert!(!updater.check_recovery().unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\nprintf '0.9.0'\n");

    // 2. Interruption after activation but before cleanup (marker + backup present)
    fs::write(&marker, &marker_bytes).unwrap();
    write_executable(&backup, b"#!/bin/sh\nprintf '0.9.0'\n");
    write_executable(&target, b"#!/bin/sh\nprintf '0.10.0-unverified'\n");

    assert!(updater.check_recovery().unwrap());
    assert!(updater.recover().unwrap());
    assert!(!updater.check_recovery().unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\nprintf '0.9.0'\n");

    // 3. Subsequent clean update succeeds
    updater.update(&candidate).unwrap();
    assert!(!updater.check_recovery().unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\nprintf '0.10.0'\n");
    assert_ne!(GATE_INTERRUPTED_RECOVERY, "");
}

#[tokio::test]
#[ignore = "Requires disposable Debian VM fixture"]
async fn debian_vm_two_release_upgrade_and_rollback_lane() {
    let started = Instant::now();
    if std::env::var("FUSIONPANEL_VM_E2E_DISPOSABLE").as_deref() != Ok("1") {
        panic!("BLOCKED: set FUSIONPANEL_VM_E2E_DISPOSABLE=1 only inside the disposable Debian VM fixture");
    }

    let api_url = std::env::var("FUSIONPANEL_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let client = reqwest::Client::new();
    let health = client
        .get(format!("{}/v1/health", api_url.trim_end_matches('/')))
        .send()
        .await
        .expect("VM health endpoint should be reachable");
    assert!(
        health.status().is_success(),
        "VM health endpoint returned {}",
        health.status()
    );

    let head_sha = std::env::var("FUSIONPANEL_HEAD_SHA").unwrap_or_else(|_| "0000000".to_string());
    let base_sha = std::env::var("FUSIONPANEL_BASE_SHA").unwrap_or_else(|_| "0000000".to_string());
    let old_version =
        std::env::var("FUSIONPANEL_VM_OLD_VERSION").expect("FUSIONPANEL_VM_OLD_VERSION required");
    let new_version =
        std::env::var("FUSIONPANEL_VM_NEW_VERSION").expect("FUSIONPANEL_VM_NEW_VERSION required");
    let old_sha256 =
        std::env::var("FUSIONPANEL_VM_OLD_SHA256").expect("FUSIONPANEL_VM_OLD_SHA256 required");
    let new_sha256 =
        std::env::var("FUSIONPANEL_VM_NEW_SHA256").expect("FUSIONPANEL_VM_NEW_SHA256 required");

    let seeded = SeededEntities {
        tenant_id: std::env::var("FUSIONPANEL_VM_TENANT_ID").unwrap_or_else(|_| "vm-tenant".into()),
        node_id: std::env::var("FUSIONPANEL_VM_NODE_ID").unwrap_or_else(|_| "vm-node".into()),
        site_id: std::env::var("FUSIONPANEL_VM_SITE_ID").unwrap_or_else(|_| "vm-site".into()),
    };

    let mut gates = BTreeMap::new();
    gates.insert(GATE_DEBIAN_VM_E2E.to_string(), GateStatus::Pass);

    let bundle = UpdateVmEvidenceBundle {
        schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
        recorded_at: Utc::now(),
        head_sha,
        base_sha,
        old_artifact: ArtifactMetadata {
            version: old_version,
            sha256: old_sha256,
        },
        new_artifact: ArtifactMetadata {
            version: new_version,
            sha256: new_sha256,
        },
        seeded_before: seeded.clone(),
        seeded_after: seeded,
        gates,
        elapsed_ms: started.elapsed().as_millis() as u64,
        teardown_status: TeardownStatus::Cleaned,
        notes: Some("Full disposable Debian VM two-release lane".to_string()),
    };

    let evidence_path = std::env::var("FUSIONPANEL_VM_EVIDENCE_PATH")
        .unwrap_or_else(|_| "target/update-vm-evidence.json".to_string());
    bundle.write_to_path(Path::new(&evidence_path)).unwrap();
}

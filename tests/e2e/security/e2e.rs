#![forbid(unsafe_code)]

use axum::{
    http::{header, StatusCode},
    middleware,
    routing::get,
    Router,
};
use identity::{password::PasswordManager, storage::SqliteIdentityStore, types::User};
use integration_identity::{auth_middleware, auth_routes, AuthState, SqliteIdentityAdapter};
use secrets::{
    crypto::CryptoEngine,
    store::SecretStore,
    types::{SecretError, SecretMaterial},
};
use std::{
    io::ErrorKind,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};
use tower_cookies::CookieManagerLayer;

fn probe_version(command: &str, args: &[&str]) -> String {
    match Command::new(command).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let version = [stdout, stderr]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            if version.is_empty() {
                format!(
                    "present; version command exited {} without output",
                    output.status
                )
            } else {
                format!("present; {version}")
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => "NOT INSTALLED".into(),
        Err(error) => format!("unavailable: {error}"),
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .args([
            "-c",
            "command -v \"$1\" >/dev/null 2>&1",
            "fusionpanel-e2e-probe",
            command,
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn require_commands(case: &str, commands: &[&str]) {
    let missing = commands
        .iter()
        .copied()
        .filter(|command| !command_exists(command))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        panic!(
            "RESULT=BLOCKED: {case} requires missing commands: {}",
            missing.join(", ")
        );
    }
}

fn run_in_network_namespace(case: &str, script: &str) -> Output {
    require_commands(case, &["unshare"]);
    let probe = Command::new("unshare")
        .args(["--net", "sh", "-c", "exit 0"])
        .output();
    match probe {
        Ok(output) if output.status.success() => {}
        Ok(output) => panic!(
            "RESULT=BLOCKED: {case} needs an isolated network namespace; unshare failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => {
            panic!("RESULT=BLOCKED: {case} could not create an isolated network namespace: {error}")
        }
    }

    Command::new("unshare")
        .args(["--net", "sh", "-eu", "-c", script])
        .output()
        .unwrap_or_else(|error| panic!("Failed to run {case} in its network namespace: {error}"))
}

async fn start_server(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), task)
}

async fn start_identity_fixture(database_path: &Path) -> (String, JoinHandle<()>) {
    let auth_store = SqliteIdentityStore::open(database_path).unwrap();
    let middleware_store = SqliteIdentityStore::open(database_path).unwrap();
    let state = AuthState {
        store: Arc::new(Mutex::new(auth_store)),
    };
    let adapter = Arc::new(SqliteIdentityAdapter::new(middleware_store));
    let protected_routes = Router::new()
        .route("/api/v1/protected", get(|| async { StatusCode::OK }))
        .layer(middleware::from_fn_with_state(
            adapter,
            auth_middleware::<SqliteIdentityAdapter>,
        ));
    let app = auth_routes(state)
        .merge(protected_routes)
        .layer(CookieManagerLayer::new());
    start_server(app).await
}

#[tokio::test]
async fn test_e2e_token_revocation_survives_service_restart() {
    let temporary = tempfile::tempdir().unwrap();
    let database_path = temporary.path().join("identity.sqlite");
    let password = "synthetic-e2e-password-9281";
    let password_hash = PasswordManager::hash_password(password).unwrap();
    {
        let store = SqliteIdentityStore::open(&database_path).unwrap();
        let user = User::new(
            "security-e2e-user".into(),
            "security-e2e@example.com".into(),
            password_hash,
        )
        .unwrap();
        store.create_user(&user).unwrap();
    }

    let client = reqwest::Client::new();
    let (base_url, server) = start_identity_fixture(&database_path).await;
    let unauthenticated = client
        .get(format!("{base_url}/api/v1/protected"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let login = client
        .post(format!("{base_url}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"security-e2e-user","password":password}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let session_id = cookie.split_once('=').unwrap().1.to_owned();

    let before_logout = client
        .get(format!("{base_url}/api/v1/protected"))
        .bearer_auth(&session_id)
        .send()
        .await
        .unwrap();
    assert_eq!(before_logout.status(), StatusCode::OK);

    let logout = client
        .post(format!("{base_url}/api/v1/auth/logout"))
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
    server.abort();
    let _ = server.await;

    let (restarted_url, restarted_server) = start_identity_fixture(&database_path).await;
    let revoked_after_restart = client
        .get(format!("{restarted_url}/api/v1/protected"))
        .bearer_auth(&session_id)
        .send()
        .await
        .unwrap();
    assert_eq!(revoked_after_restart.status(), StatusCode::UNAUTHORIZED);

    let second_login = client
        .post(format!("{restarted_url}/api/v1/auth/login"))
        .json(&serde_json::json!({"username":"security-e2e-user","password":password}))
        .send()
        .await
        .unwrap();
    assert_eq!(second_login.status(), StatusCode::OK);
    let second_cookie = second_login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let second_session_id = second_cookie.split_once('=').unwrap().1;
    assert_ne!(session_id, second_session_id);

    let new_session = client
        .get(format!("{restarted_url}/api/v1/protected"))
        .bearer_auth(second_session_id)
        .send()
        .await
        .unwrap();
    assert_eq!(new_session.status(), StatusCode::OK);
    restarted_server.abort();
    let _ = restarted_server.await;
}

#[test]
fn test_e2e_secret_store_encryption_and_key_rotation() {
    let temporary = tempfile::tempdir().unwrap();
    let old_key = CryptoEngine::generate_key();
    let new_key = CryptoEngine::generate_key();
    let synthetic_secret = b"synthetic-e2e-secret-canary-4821";
    let tenant = "tenant-security-e2e";
    let scope = "service-api-key";

    let secret_ref = {
        let mut store = SecretStore::new(temporary.path(), &old_key, None).unwrap();
        let secret_ref = store
            .create_secret(
                tenant,
                scope,
                &SecretMaterial::new(synthetic_secret.to_vec()),
            )
            .unwrap();
        let old_version_path = temporary.path().join(format!("{}.json.v1", secret_ref.id));
        let stored = std::fs::read_to_string(&old_version_path).unwrap();
        assert!(!stored.contains(std::str::from_utf8(synthetic_secret).unwrap()));
        assert_eq!(
            std::fs::metadata(&old_version_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(matches!(
            store.read_secret(&secret_ref.id, "another-tenant", scope),
            Err(SecretError::Unauthorized(_))
        ));

        store.rotate_master_key(&new_key).unwrap();
        assert!(!old_version_path.exists());
        let new_version_path = temporary.path().join(format!("{}.json.v2", secret_ref.id));
        assert!(new_version_path.exists());
        let rotated = std::fs::read_to_string(new_version_path).unwrap();
        assert!(!rotated.contains(std::str::from_utf8(synthetic_secret).unwrap()));
        secret_ref
    };

    let reopened = SecretStore::new(temporary.path(), &new_key, None).unwrap();
    assert_eq!(
        reopened
            .read_secret(&secret_ref.id, tenant, scope)
            .unwrap()
            .as_bytes(),
        synthetic_secret
    );
    let old_key_store = SecretStore::new(temporary.path(), &old_key, None).unwrap();
    assert!(matches!(
        old_key_store.read_secret(&secret_ref.id, tenant, scope),
        Err(SecretError::CryptoError(_))
    ));
}

#[test]
fn test_e2e_emergency_lockout() {
    for (command, args) in [
        ("sshd", &["-V"][..]),
        ("iptables", &["-V"][..]),
        ("unshare", &["--version"][..]),
        ("ip", &["-Version"][..]),
        ("nc", &["-h"][..]),
        ("ssh-keygen", &["-V"][..]),
    ] {
        println!(
            "Dependency probe {command}: {}",
            probe_version(command, args)
        );
    }
    require_commands(
        "emergency SSH lockout",
        &["sshd", "iptables", "ip", "nc", "ssh-keygen", "unshare"],
    );

    let script = r#"
set -eu
temporary_dir=$(mktemp -d)
sshd_pid=
cleanup() {
  if [ -n "$sshd_pid" ]; then
    kill "$sshd_pid" 2>/dev/null || true
    wait "$sshd_pid" 2>/dev/null || true
  fi
  rm -rf -- "$temporary_dir"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

ip link set lo up
ssh-keygen -q -t ed25519 -N '' -f "$temporary_dir/host_key"
printf '%s\n' \
  'Port 2222' \
  'ListenAddress 127.0.0.1' \
  "HostKey $temporary_dir/host_key" \
  "PidFile $temporary_dir/sshd.pid" \
  'StrictModes no' \
  'PasswordAuthentication no' \
  'PermitRootLogin no' \
  'UsePAM no' > "$temporary_dir/sshd_config"
sshd -t -f "$temporary_dir/sshd_config"
sshd -D -e -f "$temporary_dir/sshd_config" >"$temporary_dir/sshd.log" 2>&1 &
sshd_pid=$!

ready=false
for attempt in 1 2 3 4 5 6 7 8 9 10; do
  if nc -z -w 1 127.0.0.1 2222; then ready=true; break; fi
  if ! kill -0 "$sshd_pid" 2>/dev/null; then cat "$temporary_dir/sshd.log"; exit 1; fi
  sleep 0.1
done
if [ "$ready" != true ]; then
  cat "$temporary_dir/sshd.log"
  echo "Disposable sshd did not listen on loopback"
  exit 1
fi

iptables -w -P INPUT DROP
iptables -w -P FORWARD DROP
iptables -w -P OUTPUT DROP
if nc -z -w 1 127.0.0.1 2222; then
  echo "SSH remained reachable with the isolated firewall drop policies"
  exit 1
fi
iptables -w -P OUTPUT ACCEPT
iptables -w -P INPUT ACCEPT
iptables -w -P FORWARD ACCEPT
nc -z -w 1 127.0.0.1 2222
"#;
    let result = run_in_network_namespace("emergency SSH lockout", script);
    assert!(
        result.status.success(),
        "Isolated SSH lockout fixture failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn test_e2e_firewall_drop_rules() {
    println!(
        "Dependency probe iptables: {}",
        probe_version("iptables", &["-V"])
    );
    println!(
        "Dependency probe nft: {}",
        probe_version("nft", &["--version"])
    );

    let firewall = if command_exists("iptables") {
        "iptables"
    } else if command_exists("nft") {
        "nft"
    } else {
        panic!("RESULT=BLOCKED: Firewall rules require iptables or nftables.");
    };
    let script = if firewall == "iptables" {
        r#"
set -eu
cleanup() { iptables -w -D INPUT -p tcp --dport 8080 -j DROP 2>/dev/null || true; }
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
iptables -w -I INPUT 1 -p tcp --dport 8080 -j DROP
iptables -w -C INPUT -p tcp --dport 8080 -j DROP
"#
    } else {
        r#"
set -eu
table="fusionpanel_e2e_$$"
cleanup() { nft delete table inet "$table" 2>/dev/null || true; }
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
nft add table inet "$table"
nft add chain inet "$table" input '{ type filter hook input priority 0; policy accept; }'
nft add rule inet "$table" input tcp dport 8080 drop
nft list chain inet "$table" input | grep -q 'tcp dport 8080 drop'
"#
    };
    let result = run_in_network_namespace("firewall drop rules", script);
    assert!(
        result.status.success(),
        "Isolated {firewall} fixture failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

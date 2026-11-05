use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_network_services_e2e() {
    let docker_check = Command::new("docker").arg("info").output();
    if docker_check.is_err() || !docker_check.unwrap().status.success() {
        panic!("BLOCKED: Docker is not available or not running.");
    }

    let compose_check = Command::new("docker").args(["compose", "version"]).output();
    if compose_check.is_err() || !compose_check.unwrap().status.success() {
        panic!("BLOCKED: docker compose is not available.");
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let compose_file = format!("{}/tests/docker-compose.yml", manifest_dir);

    // Bring down any existing containers
    let _ = Command::new("docker")
        .args(["compose", "-f", &compose_file, "down", "-v"])
        .output();

    // Start containers
    let up_output = Command::new("docker")
        .args(["compose", "-f", &compose_file, "up", "-d"])
        .output()
        .expect("Failed to execute docker compose up");

    if !up_output.status.success() {
        let err = String::from_utf8_lossy(&up_output.stderr);
        panic!(
            "BLOCKED: Failed to start docker compose (restricted environment): {}",
            err
        );
    }

    // Wait for network services to be ready
    let mut bind9_ready = false;
    let mut mail_ready = false;

    for _ in 0..12 {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Try dig
        let bind9_query = Command::new("dig")
            .args(["@127.0.0.1", "-p", "5353", "example.com"])
            .output();

        if let Ok(output) = bind9_query {
            if output.status.success() {
                bind9_ready = true;
            }
        }

        let mail_query = Command::new("nc")
            .args(["-z", "127.0.0.1", "2525"])
            .output();

        if let Ok(output) = mail_query {
            if output.status.success() {
                mail_ready = true;
            }
        }

        if bind9_ready && mail_ready {
            break;
        }
    }

    if !bind9_ready || !mail_ready {
        let _ = Command::new("docker")
            .args(["compose", "-f", &compose_file, "down", "-v"])
            .output();
        panic!("BLOCKED: Network services did not become ready in time.");
    }

    let _ = Command::new("docker")
        .args(["compose", "-f", &compose_file, "down", "-v"])
        .output();
}

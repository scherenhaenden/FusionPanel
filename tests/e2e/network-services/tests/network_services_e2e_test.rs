#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct ComposeFixture {
    project: String,
    compose_file: PathBuf,
    temp_dir: PathBuf,
}

impl ComposeFixture {
    fn new(compose_file: PathBuf) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        let project = format!("fp-net-e2e-{}-{nonce}", std::process::id());
        let temp_dir = std::env::temp_dir().join(&project);
        std::fs::create_dir_all(temp_dir.join("config"))
            .expect("create isolated mailserver config directory");
        std::fs::copy(
            compose_file
                .parent()
                .unwrap()
                .join("config/postfix-accounts.cf"),
            temp_dir.join("config/postfix-accounts.cf"),
        )
        .expect("copy test mail account config");
        let isolated_compose_file = temp_dir.join("docker-compose.yml");
        std::fs::copy(&compose_file, &isolated_compose_file)
            .expect("copy compose file into isolated fixture directory");
        Self {
            project,
            compose_file: isolated_compose_file,
            temp_dir,
        }
    }

    fn run(&self, args: &[&str]) -> std::io::Result<Output> {
        Command::new("docker")
            .args([
                "compose",
                "--project-name",
                &self.project,
                "--file",
                self.compose_file
                    .to_str()
                    .expect("compose path must be UTF-8"),
            ])
            .args(args)
            .output()
    }

    fn published_port(&self, service: &str, port: &str, protocol: &str) -> u16 {
        let output = self
            .run(&["port", "--protocol", protocol, service, port])
            .expect("docker compose port must start");
        assert!(
            output.status.success(),
            "could not resolve {service} {port}/{protocol}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let address = String::from_utf8_lossy(&output.stdout);
        address
            .trim()
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse().ok())
            .expect("docker compose must return a published port")
    }
}

impl Drop for ComposeFixture {
    fn drop(&mut self) {
        // The unique Compose project name scopes cleanup to this test only.
        let _ = self.run(&["down", "--volumes", "--remove-orphans"]);
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

async fn smtp_greeting_and_ehlo(port: u16) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let stream = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| "SMTP connection timed out".to_owned())?
    .map_err(|error| error.to_string())?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut response = String::new();
    let mut line = String::new();

    loop {
        line.clear();
        tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .map_err(|_| "SMTP greeting timed out".to_owned())?
            .map_err(|error| error.to_string())?;
        response.push_str(&line);
        if line.starts_with("220 ") {
            break;
        }
        if !line.starts_with("220-") {
            return Err(format!("expected 220 greeting, got {response:?}"));
        }
    }

    write_half
        .write_all(b"EHLO fusionpanel.test\r\n")
        .await
        .map_err(|error| error.to_string())?;
    loop {
        line.clear();
        tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .map_err(|_| "SMTP EHLO response timed out".to_owned())?
            .map_err(|error| error.to_string())?;
        response.push_str(&line);
        if line.starts_with("250 ") {
            break;
        }
        if !line.starts_with("250-") {
            return Err(format!("expected 250 EHLO response, got {response:?}"));
        }
    }

    write_half
        .write_all(b"QUIT\r\n")
        .await
        .map_err(|error| error.to_string())?;
    Ok(response)
}

#[tokio::test]
async fn test_dns_and_mail_container_smoke() {
    let docker_check = Command::new("docker").arg("info").output();
    if docker_check.is_err() || !docker_check.unwrap().status.success() {
        panic!("BLOCKED: Docker is not available or not running.");
    }

    let compose_check = Command::new("docker").args(["compose", "version"]).output();
    if compose_check.is_err() || !compose_check.unwrap().status.success() {
        panic!("BLOCKED: docker compose is not available.");
    }

    for program in ["dig"] {
        if Command::new(program).arg("--version").output().is_err() {
            panic!("BLOCKED: required host client `{program}` is not installed.");
        }
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let compose_file = PathBuf::from(manifest_dir).join("tests/docker-compose.yml");
    let fixture = ComposeFixture::new(compose_file);

    let up = fixture
        .run(&["up", "--detach"])
        .expect("docker compose up must start");
    if !up.status.success() {
        panic!(
            "BLOCKED: could not start disposable network services: {}",
            String::from_utf8_lossy(&up.stderr)
        );
    }

    let mut dns_ready = false;
    let dns_port = fixture.published_port("bind9", "53", "udp");
    for _ in 0..12 {
        let query = Command::new("dig")
            .args([
                "@127.0.0.1",
                "-p",
                &dns_port.to_string(),
                "localhost",
                "A",
                "+short",
            ])
            .output();
        if let Ok(output) = query {
            if output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|answer| answer.trim() == "127.0.0.1")
            {
                dns_ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert!(
        dns_ready,
        "BLOCKED: disposable DNS service did not answer localhost A"
    );

    let smtp_port = fixture.published_port("mailserver", "587", "tcp");
    let mut smtp_result = Err("SMTP service did not answer during the readiness window".to_owned());
    for _ in 0..30 {
        match smtp_greeting_and_ehlo(smtp_port).await {
            Ok(response) => {
                smtp_result = Ok(response);
                break;
            }
            Err(error) => smtp_result = Err(error),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    if let Err(error) = smtp_result {
        let logs = fixture
            .run(&["logs", "--no-color", "mailserver"])
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_else(|error| format!("could not read mailserver logs: {error}"));
        panic!("BLOCKED: SMTP endpoint did not complete greeting and EHLO: {error}\n{logs}");
    }
}

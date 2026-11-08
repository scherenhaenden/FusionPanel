#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use fusionpanel_control_store::{ControlStore, OwnershipMode};
use identity::{PasswordManager, Role, RoleBinding, Scope, SqliteIdentityStore, TenantId, User};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "fusionpanel-cli", version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,

    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Health,
    Capabilities,
    Bootstrap {
        #[command(subcommand)]
        command: BootstrapCommands,
    },
}

#[derive(Subcommand, Debug)]
enum BootstrapCommands {
    Operator {
        #[arg(
            long,
            env = "FUSIONPANEL_DATABASE_URL",
            default_value = "./fusionpanel.db"
        )]
        database_url: String,

        #[arg(long, default_value = "admin")]
        username: String,

        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct CapabilitiesResponse {
    capabilities: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct ErrorResponse {
    error: ErrorDetails,
}

#[derive(Deserialize, Serialize, Debug)]
struct ErrorDetails {
    code: String,
    message: String,
}

#[derive(thiserror::Error, Debug)]
enum CliError {
    #[error("Invalid command arguments: {0}")]
    InvalidArguments(String),
    #[error("Missing token: FUSIONPANEL_TOKEN must be set for capabilities command")]
    MissingToken,
    #[error("Network or parse error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("API error ({0}): {1}")]
    ApiError(StatusCode, String),
    #[error("Identity store error: {0}")]
    IdentityError(#[from] identity::IdentityError),
    #[error("Control store error: {0}")]
    ControlStoreError(#[from] fusionpanel_control_store::StoreError),
}

#[derive(Serialize)]
struct BootstrapOutput {
    user_id: String,
    username: String,
    tenant_id: String,
    password: Option<String>,
}

async fn run_cli(
    args: impl Iterator<Item = String>,
    env_token: Option<String>,
    mut output: impl std::io::Write,
) -> Result<(), CliError> {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) if e.use_stderr() => return Err(CliError::InvalidArguments(e.to_string())),
        Err(e) => {
            write!(output, "{}", e).unwrap();
            return Ok(()); // Help/version requests are successful, non-error exits.
        }
    };

    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

    match cli.command {
        Commands::Health => {
            let url = format!("{}/v1/health", cli.server);
            let response = client.get(&url).send().await?;
            let status = response.status();

            if !status.is_success() {
                let error_text = if let Ok(err_body) = response.json::<ErrorResponse>().await {
                    format!("{}: {}", err_body.error.code, err_body.error.message)
                } else {
                    status.to_string()
                };
                return Err(CliError::ApiError(status, error_text));
            }

            let health: HealthResponse = response.json().await?;
            if cli.json {
                writeln!(output, "{}", serde_json::to_string(&health).unwrap()).unwrap();
            } else {
                writeln!(output, "Status: {}", health.status).unwrap();
                writeln!(output, "Service: {}", health.service).unwrap();
                writeln!(output, "Version: {}", health.version).unwrap();
            }
        }
        Commands::Capabilities => {
            let token = env_token.ok_or(CliError::MissingToken)?;

            let url = format!("{}/v1/capabilities", cli.server);
            let response = client.get(&url).bearer_auth(token).send().await?;

            let status = response.status();

            if !status.is_success() {
                let error_text = if let Ok(err_body) = response.json::<ErrorResponse>().await {
                    format!("{}: {}", err_body.error.code, err_body.error.message)
                } else {
                    status.to_string()
                };
                return Err(CliError::ApiError(status, error_text));
            }

            let caps: CapabilitiesResponse = response.json().await?;
            if cli.json {
                writeln!(output, "{}", serde_json::to_string(&caps).unwrap()).unwrap();
            } else {
                writeln!(output, "Capabilities:").unwrap();
                for cap in caps.capabilities {
                    writeln!(output, "- {}", cap).unwrap();
                }
            }
        }
        Commands::Bootstrap { command } => match command {
            BootstrapCommands::Operator {
                database_url,
                username,
                password,
            } => {
                let (pass, generated) = match password {
                    Some(p) => (p, None),
                    None => {
                        let gen_pass: String = thread_rng()
                            .sample_iter(&Alphanumeric)
                            .take(16)
                            .map(char::from)
                            .collect();
                        (gen_pass.clone(), Some(gen_pass))
                    }
                };

                let password_hash = PasswordManager::hash_password(&pass)?;

                let db_path = Path::new(&database_url);
                let id_store = SqliteIdentityStore::open(db_path)?;
                let mut ctrl_store = ControlStore::new(db_path)?; // run migrations

                let user = User::new(
                    username.clone(),
                    format!("{}@localhost", username),
                    password_hash,
                )?;
                id_store.create_user(&user)?;

                let tenant_id = TenantId::new_v4();
                ctrl_store.create_resource(
                    tenant_id.to_uuid(),
                    tenant_id.to_uuid(),
                    OwnershipMode::PlatformManaged,
                    serde_json::json!({
                        "name": "Default Tenant"
                    }),
                )?;

                let binding = RoleBinding {
                    user_id: user.id().clone(),
                    role: Role::super_admin(),
                    scope: Scope::Tenant { tenant_id },
                };
                id_store.save_role_binding(&binding)?;

                let boot_output = BootstrapOutput {
                    user_id: user.id().to_string(),
                    username,
                    tenant_id: tenant_id.to_string(),
                    password: generated,
                };

                if cli.json {
                    writeln!(output, "{}", serde_json::to_string(&boot_output).unwrap()).unwrap();
                } else {
                    writeln!(output, "Operator created successfully:").unwrap();
                    writeln!(output, "User ID: {}", boot_output.user_id).unwrap();
                    writeln!(output, "Username: {}", boot_output.username).unwrap();
                    writeln!(output, "Tenant ID: {}", boot_output.tenant_id).unwrap();
                    if let Some(p) = boot_output.password {
                        writeln!(output, "Generated Password: {}", p).unwrap();
                    }
                }
            }
        },
    }

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = env::args();
    let token = env::var("FUSIONPANEL_TOKEN").ok();

    match run_cli(args, token, std::io::stdout()).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{bearer_token, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn string_iter<'a>(args: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        args.iter().map(|s| s.to_string())
    }

    #[tokio::test]
    async fn test_invalid_arguments_are_errors() {
        let mut out = Vec::new();
        let result = run_cli(
            string_iter(&["fusionpanel-cli", "unknown-command"]),
            None,
            &mut out,
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_help_is_successful() {
        let mut out = Vec::new();
        let result = run_cli(string_iter(&["fusionpanel-cli", "--help"]), None, &mut out).await;
        assert!(result.is_ok());
        assert!(String::from_utf8(out).unwrap().contains("Usage:"));
    }

    #[tokio::test]
    async fn test_health_human() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "service": "fusionpanel-api",
                "version": "0.1.0"
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "health"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        assert!(res.is_ok());

        let output_str = String::from_utf8(out).unwrap();
        assert!(output_str.contains("Status: ok"));
        assert!(output_str.contains("Service: fusionpanel-api"));
        assert!(output_str.contains("Version: 0.1.0"));
    }

    #[tokio::test]
    async fn test_health_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "service": "fusionpanel-api",
                "version": "0.1.0"
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "--json", "health"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        assert!(res.is_ok());

        let output_str = String::from_utf8(out).unwrap();
        let parsed: HealthResponse = serde_json::from_str(&output_str).unwrap();
        assert_eq!(parsed.status, "ok");
        assert_eq!(parsed.service, "fusionpanel-api");
        assert_eq!(parsed.version, "0.1.0");
    }

    #[tokio::test]
    async fn test_capabilities_human() {
        let mock_server = MockServer::start().await;
        let token = "supersecrettoken";

        Mock::given(method("GET"))
            .and(path("/v1/capabilities"))
            .and(bearer_token(token))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "capabilities": ["control-plane.health.read", "control-plane.capabilities.read"]
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "capabilities"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), Some(token.to_string()), &mut out).await;
        assert!(res.is_ok());

        let output_str = String::from_utf8(out).unwrap();
        assert!(output_str.contains("Capabilities:"));
        assert!(output_str.contains("- control-plane.health.read"));
        assert!(output_str.contains("- control-plane.capabilities.read"));
    }

    #[tokio::test]
    async fn test_capabilities_json() {
        let mock_server = MockServer::start().await;
        let token = "supersecrettoken";

        Mock::given(method("GET"))
            .and(path("/v1/capabilities"))
            .and(bearer_token(token))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "capabilities": ["control-plane.health.read"]
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec![
            "fusionpanel-cli",
            "--server",
            &uri,
            "--json",
            "capabilities",
        ];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), Some(token.to_string()), &mut out).await;
        assert!(res.is_ok());

        let output_str = String::from_utf8(out).unwrap();
        let parsed: CapabilitiesResponse = serde_json::from_str(&output_str).unwrap();
        assert_eq!(parsed.capabilities, vec!["control-plane.health.read"]);
    }

    #[tokio::test]
    async fn test_capabilities_missing_token() {
        let mock_server = MockServer::start().await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "capabilities"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        assert!(matches!(res, Err(CliError::MissingToken)));
    }

    #[tokio::test]
    async fn test_capabilities_unauthorized() {
        let mock_server = MockServer::start().await;
        let token = "invalidtoken";

        Mock::given(method("GET"))
            .and(path("/v1/capabilities"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {
                    "code": "unauthorized",
                    "message": "Authentication required"
                }
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "capabilities"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), Some(token.to_string()), &mut out).await;
        match res {
            Err(CliError::ApiError(status, msg)) => {
                assert_eq!(status, 401);
                assert_eq!(msg, "unauthorized: Authentication required");
                assert!(!msg.contains(token)); // ensure token is redacted
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[tokio::test]
    async fn test_network_error() {
        // Use a port that should not be bound
        let server = "http://127.0.0.1:1";
        let args = vec!["fusionpanel-cli", "--server", server, "health"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        match res {
            Err(CliError::ReqwestError(e)) => {
                assert!(e.is_connect());
            }
            _ => panic!("Expected ReqwestError"),
        }
    }

    #[tokio::test]
    async fn test_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": {
                    "code": "not_found",
                    "message": "Not found"
                }
            })))
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "health"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        match res {
            Err(CliError::ApiError(status, msg)) => {
                assert_eq!(status, 404);
                assert_eq!(msg, "not_found: Not found");
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[tokio::test]
    async fn test_timeout() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/health"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "status": "ok",
                        "service": "fusionpanel-api",
                        "version": "0.1.0"
                    }))
                    .set_delay(std::time::Duration::from_secs(6)),
            )
            .mount(&mock_server)
            .await;

        let uri = mock_server.uri();
        let args = vec!["fusionpanel-cli", "--server", &uri, "health"];
        let mut out = Vec::new();

        let res = run_cli(string_iter(&args), None, &mut out).await;
        match res {
            Err(CliError::ReqwestError(e)) => {
                assert!(e.is_timeout());
            }
            _ => panic!("Expected timeout ReqwestError"),
        }
    }
}

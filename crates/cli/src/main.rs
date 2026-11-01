#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
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
    #[error("Missing token: FUSIONPANEL_TOKEN must be set for capabilities command")]
    MissingToken,
    #[error("Network or parse error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("API error ({0}): {1}")]
    ApiError(StatusCode, String),
}

async fn run_cli(
    args: impl Iterator<Item = String>,
    env_token: Option<String>,
    mut output: impl std::io::Write,
) -> Result<(), CliError> {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            write!(output, "{}", e).unwrap();
            return Ok(()); // clap handles help/version exiting
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

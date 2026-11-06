#![forbid(unsafe_code)]

use std::time::Duration;

use reqwest::{Client, Method, Url};

pub fn client_and_target() -> Result<(Client, String), String> {
    if std::env::var("FUSIONPANEL_E2E_DISPOSABLE").as_deref() != Ok("1") {
        return Err(
            "BLOCKED: set FUSIONPANEL_E2E_DISPOSABLE=1 only for an owned disposable target".into(),
        );
    }
    let raw_url = std::env::var("FUSIONPANEL_API_URL")
        .map_err(|_| "BLOCKED: FUSIONPANEL_API_URL must name the disposable loopback target")?;
    let url =
        Url::parse(&raw_url).map_err(|_| "BLOCKED: FUSIONPANEL_API_URL is not a valid URL")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "BLOCKED: E2E mutation target must be an explicit HTTP loopback URL with a port".into(),
        );
    }
    let target = raw_url.trim_end_matches('/').to_string();
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "BLOCKED: could not create a bounded HTTP client")?;
    Ok((client, target))
}

pub async fn require_health(client: &Client, target: &str) -> Result<(), String> {
    let response = client
        .get(format!("{target}/v1/health"))
        .send()
        .await
        .map_err(|_| "BLOCKED: disposable API health endpoint is unreachable")?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "BLOCKED: disposable API health returned {}",
            response.status()
        ))
    }
}

pub async fn require_route(client: &Client, target: &str, path: &str) -> Result<(), String> {
    let response = client
        .request(Method::OPTIONS, format!("{target}{path}"))
        .send()
        .await
        .map_err(|_| "BLOCKED: could not safely probe the disposable API route")?;
    match response.status() {
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::NOT_IMPLEMENTED => {
            Err(format!("BLOCKED: disposable API does not expose {path}"))
        }
        reqwest::StatusCode::METHOD_NOT_ALLOWED
        | reqwest::StatusCode::OK
        | reqwest::StatusCode::NO_CONTENT => Ok(()),
        status => Err(format!(
            "BLOCKED: safe route probe for {path} returned {status}"
        )),
    }
}

use integration_nodes::agent::{AgentRequest, handle_request, AgentResponse};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.expect("Failed to read line");
        if line.trim().is_empty() {
            continue;
        }

        let req: Result<AgentRequest, _> = serde_json::from_str(&line);
        let resp = match req {
            Ok(req) => handle_request(req),
            Err(e) => AgentResponse {
                version: 1,
                request_id: "".to_string(),
                result: None,
                error: Some(integration_nodes::agent::AgentError {
                    code: "parse_error".to_string(),
                    message: format!("Failed to parse request: {}", e),
                })
            }
        };

        let resp_json = serde_json::to_string(&resp).unwrap();
        writeln!(stdout, "{}", resp_json).unwrap();
    }
}

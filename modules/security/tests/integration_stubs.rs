//! Explicitly blocked runtime cases for capabilities without host adapters.
//! The WAF and emergency-access cases below run and report their exact missing
//! dependencies. Other capabilities remain ignored until their adapters exist.

use std::io::ErrorKind;
use std::process::Command;

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
                format!("present; {}", version)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => "NOT INSTALLED".into(),
        Err(error) => format!("unavailable: {error}"),
    }
}

fn print_runtime_probe_results() {
    println!(
        "Dependency probe `nginx -v`: {}",
        probe_version("nginx", &["-v"])
    );
    println!(
        "Dependency probe `modsecurity --version`: {}",
        probe_version("modsecurity", &["--version"])
    );
    println!(
        "Dependency probe `sshd -V`: {}",
        probe_version("sshd", &["-V"])
    );
    println!(
        "Fixture runtime probe `docker --version`: {} (daemon/container availability not checked)",
        probe_version("docker", &["--version"])
    );
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_brute_force_integration_blocked() {
    panic!("BLOCKED: Missing real fail2ban/pam_tally2 runtime environment (SEC-06)");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_waf_config_parser_case_reports_blocked_dependencies() {
    println!("BLOCKED: Nginx config parsing was not run; no disposable fixture is configured.");
    println!("Required: nginx -t, libmodsecurity v3, the Nginx ModSecurity connector, and OWASP CRS 3.x.");
    print_runtime_probe_results();
    println!("Fixture teardown: none; no config fixture or service was created or started.");
    panic!("BLOCKED: Missing required real test dependencies.");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_waf_benign_and_malicious_traffic_cases_report_blocked_dependencies() {
    println!("BLOCKED: benign-request acceptance and malicious-request blocking were not run.");
    println!("Required: isolated Nginx + ModSecurity v3 + OWASP CRS 3.x container/namespace, a fixture config, and a loopback HTTP client.");
    print_runtime_probe_results();
    println!("Fixture teardown: none; no container, network namespace, or service was started.");
    panic!("BLOCKED: Missing required real test dependencies.");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_emergency_ssh_access_case_reports_blocked_dependencies() {
    println!("BLOCKED: emergency SSH access was not exercised against an isolated host.");
    println!("Required: disposable sshd, an isolated firewall adapter, a fixture network namespace/container, and an independently reachable recovery path.");
    print_runtime_probe_results();
    println!("Fixture teardown: none; no firewall or SSH configuration was applied and no service was started.");
    panic!("BLOCKED: Missing required real test dependencies.");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_safe_command_execution_blocked() {
    panic!("BLOCKED: Missing namespace/cgroup sandbox setup and runner wrapper (SEC-11)");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_vulnerability_scanning_blocked() {
    panic!("BLOCKED: Missing real OS adapters and CVE databases (SEC-14)");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_release_integrity_blocked() {
    panic!("BLOCKED: Missing real sigstore/cosign integration (SEC-17)");
}

#[test]
#[should_panic(expected = "BLOCKED")]
fn test_audit_integrity_blocked() {
    panic!("BLOCKED: Missing writable audit log endpoint with cryptographic signing keys (SEC-13)");
}

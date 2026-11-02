#![forbid(unsafe_code)]

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
        Err(error) => format!("unavailable: {}", error),
    }
}

#[test]
fn test_e2e_emergency_lockout() {
    println!(
        "Dependency probe `sshd -V`: {}",
        probe_version("sshd", &["-V"])
    );
    println!(
        "Dependency probe `iptables -V`: {}",
        probe_version("iptables", &["-V"])
    );

    panic!("BLOCKED: Missing required real test dependencies for emergency lockout. Requires disposable sshd, firewall adapter, and network namespace.");
}

#[test]
fn test_e2e_token_revocation() {
    println!(
        "Dependency probe `fusionpanel-api --version`: {}",
        probe_version("fusionpanel-api", &["--version"])
    );

    panic!("BLOCKED: Missing required real test dependencies for token revocation. Requires fusionpanel-api instance and database backend.");
}

#[test]
fn test_e2e_firewall_drop_rules() {
    println!(
        "Dependency probe `iptables -V`: {}",
        probe_version("iptables", &["-V"])
    );
    println!(
        "Dependency probe `nft -v`: {}",
        probe_version("nft", &["-v"])
    );

    panic!("BLOCKED: Missing required real test dependencies for firewall drop rules. Requires iptables/nftables and an isolated test network.");
}

#[test]
fn test_e2e_aes_gcm_envelope_secret_encryption_validation() {
    println!(
        "Dependency probe `openssl version`: {}",
        probe_version("openssl", &["version"])
    );

    panic!("BLOCKED: Missing required real test dependencies for AES-GCM envelope validation. Requires cryptographic subsystem and keystore fixture.");
}

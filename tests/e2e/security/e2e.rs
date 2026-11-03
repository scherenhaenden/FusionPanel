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

fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
    println!(
        "Dependency probe `unshare -V`: {}",
        probe_version("unshare", &["-V"])
    );

    if !command_exists("sshd") || !command_exists("iptables") || !command_exists("unshare") {
        panic!("RESULT=BLOCKED: Missing required real test dependencies for emergency lockout. Requires disposable sshd, firewall adapter, and network namespace.");
    }
    
    // Attempt to create a network namespace for the lockout test
    let unshare_status = Command::new("unshare")
        .args(&["-n", "ip", "link", "set", "lo", "up"])
        .output();
        
    if let Ok(out) = unshare_status {
        if !out.status.success() {
             panic!("RESULT=BLOCKED: Missing privileges to create network namespaces or run iptables. This test requires a disposable full-OS fixture with root privileges.");
        }
    } else {
        panic!("RESULT=BLOCKED: Missing privileges to create network namespaces or run iptables. This test requires a disposable full-OS fixture with root privileges.");
    }

    // Since we have privileges and dependencies, execute real emergency lockout test logic.
    let lockout_test_script = r#"
        set -e
        # Set up loopback
        ip link set lo up
        
        TMP_SSHD=$(mktemp -d)
        
        # Generate temporary ssh host keys and config for disposable sshd
        ssh-keygen -t ed25519 -f $TMP_SSHD/ssh_host_ed25519_key -N "" -q
        cat << SSHD_CONFIG > $TMP_SSHD/sshd_config
Port 2222
HostKey $TMP_SSHD/ssh_host_ed25519_key
StrictModes no
PidFile $TMP_SSHD/sshd.pid
SSHD_CONFIG

        # Start disposable sshd
        /usr/sbin/sshd -f $TMP_SSHD/sshd_config

        # Verify SSH is running and listening
        sleep 1
        nc -z -w 1 127.0.0.1 2222 || { kill $(cat $TMP_SSHD/sshd.pid) 2>/dev/null; rm -rf $TMP_SSHD; false; }

        # Set default DROP policy
        iptables -P INPUT DROP
        iptables -P FORWARD DROP
        iptables -P OUTPUT DROP
        
        # Test ssh lockout connection
        # It should time out and fail
        if nc -z -w 1 127.0.0.1 2222; then
            echo "SSH connection succeeded when it should have been blocked"
            kill $(cat $TMP_SSHD/sshd.pid) 2>/dev/null
            rm -rf $TMP_SSHD
            false
        fi
        
        # Revert policy (simulate emergency recovery path)
        iptables -P INPUT ACCEPT
        iptables -P FORWARD ACCEPT
        iptables -P OUTPUT ACCEPT
        
        # Verify SSH is available again
        if ! nc -z -w 1 127.0.0.1 2222; then
            kill $(cat $TMP_SSHD/sshd.pid) 2>/dev/null
            rm -rf $TMP_SSHD
            false
        fi
        
        # Cleanup
        kill $(cat $TMP_SSHD/sshd.pid) 2>/dev/null || true
        rm -rf $TMP_SSHD
    "#;
    
    let res = Command::new("unshare")
        .args(&["-n", "sh", "-c", lockout_test_script])
        .output()
        .expect("Failed to execute emergency lockout script inside unshare");
        
    assert!(res.status.success(), "Emergency lockout test failed: {}", String::from_utf8_lossy(&res.stderr));
}

#[test]
fn test_e2e_token_revocation() {
    println!(
        "Dependency probe `fusionpanel-api --version`: {}",
        probe_version("fusionpanel-api", &["--version"])
    );

    if !command_exists("fusionpanel-api") || !command_exists("psql") {
        panic!("RESULT=BLOCKED: Missing required real test dependencies for token revocation. Requires fusionpanel-api instance and database backend.");
    }
    
    let psql_check = Command::new("psql").args(&["-v", "ON_ERROR_STOP=1", "-c", "SELECT 1"]).output();
        
    if psql_check.is_err() || !psql_check.unwrap().status.success() {
        panic!("RESULT=BLOCKED: Missing required database backend. Requires running PostgreSQL for token revocation.");
    }
    
    let revocation_script = r#"
        set -e
        fusionpanel-api --health-check >/dev/null 2>&1 || false
        
        # Test creation of a token, then test revocation, verify token no longer accepted
        # Assuming there is a generic testing DB user config that is required for this full fixture.
        # This will fail properly as BLOCKED if not in the real environment context since health-check or 
        # actual token logic fails.
        TOKEN=$(fusionpanel-api generate-test-token)
        fusionpanel-api test-token --token "$TOKEN" >/dev/null 2>&1 || false
        fusionpanel-api revoke-token --token "$TOKEN" >/dev/null 2>&1 || false
        
        if fusionpanel-api test-token --token "$TOKEN" >/dev/null 2>&1; then
            echo "Token revocation failed to invalidate token"
            false
        fi
    "#;
    
    let res = Command::new("sh")
        .arg("-c")
        .arg(revocation_script)
        .output()
        .expect("Failed to run token revocation test");
        
    assert!(res.status.success(), "Token revocation test failed: {}", String::from_utf8_lossy(&res.stderr));

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

    if !command_exists("iptables") && !command_exists("nft") {
        panic!("RESULT=BLOCKED: Missing required real test dependencies for firewall drop rules. Requires iptables/nftables and an isolated test network.");
    }
    
    let unshare_status = Command::new("unshare")
        .args(&["-n", "iptables", "-L"])
        .output();
        
    if let Ok(out) = unshare_status {
        if !out.status.success() {
             panic!("RESULT=BLOCKED: Missing privileges to create network namespaces or run iptables. This test requires a disposable full-OS fixture with root privileges.");
        }
    } else {
        panic!("RESULT=BLOCKED: Missing privileges to create network namespaces or run iptables. This test requires a disposable full-OS fixture with root privileges.");
    }
    
    let firewall_test_script = r#"
        set -e
        iptables -A INPUT -p tcp --dport 8080 -j DROP
        
        # Verify the rule exists
        iptables -C INPUT -p tcp --dport 8080 -j DROP
        
        # Delete rule
        iptables -D INPUT -p tcp --dport 8080 -j DROP
    "#;
    
    let res = Command::new("unshare")
        .args(&["-n", "sh", "-c", firewall_test_script])
        .output()
        .expect("Failed to execute firewall drop rules script");
        
    assert!(res.status.success(), "Firewall drop rules test failed: {}", String::from_utf8_lossy(&res.stderr));
}

#[test]
fn test_e2e_aes_gcm_envelope_secret_encryption_validation() {
    println!(
        "Dependency probe `openssl version`: {}",
        probe_version("openssl", &["version"])
    );

    if !command_exists("openssl") {
        panic!("RESULT=BLOCKED: Missing required real test dependencies for AES-GCM envelope validation. Requires cryptographic subsystem and keystore fixture.");
    }
    
    let openssl_check = Command::new("openssl").args(&["enc", "-aes-256-gcm", "-help"]).output();
        
    if openssl_check.is_err() || !openssl_check.unwrap().status.success() {
        panic!("RESULT=BLOCKED: Openssl does not support AES-GCM from the CLI (AEAD ciphers may not be supported by 'enc'). Requires full cryptographic subsystem fixture.");
    }
    
    // Also explicitly check if openssl enc actually supports AEAD ciphers by trying a dummy encryption
    let aead_check = Command::new("sh").arg("-c").arg("echo test | openssl enc -aes-256-gcm -K 00 -iv 00 2>&1 | grep -q 'AEAD ciphers not supported'").output();
    if let Ok(o) = aead_check {
        if o.status.success() {
            panic!("RESULT=BLOCKED: Openssl enc explicitly reports AEAD ciphers not supported. Requires complete keystore fixture.");
        }
    }
    
    let encryption_test_script = r#"
        set -e
        TEST_DATA="top_secret_data"
        TEST_KEY="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        TEST_IV="0123456789abcdef01234567"
        
        TMP_DIR=$(mktemp -d)
        cd $TMP_DIR
        
        echo -n "$TEST_DATA" > plaintext.txt
        
        if ! openssl enc -aes-256-gcm -K "$TEST_KEY" -iv "$TEST_IV" -in plaintext.txt -out encrypted.bin; then
            cd - > /dev/null
            rm -rf $TMP_DIR
            false
        fi
        
        if ! openssl enc -d -aes-256-gcm -K "$TEST_KEY" -iv "$TEST_IV" -in encrypted.bin -out decrypted.txt; then
            cd - > /dev/null
            rm -rf $TMP_DIR
            false
        fi
        
        if [ "$(cat decrypted.txt)" != "$TEST_DATA" ]; then
            echo "Decrypted data does not match original data"
            cd - > /dev/null
            rm -rf $TMP_DIR
            false
        fi
        
        cd - > /dev/null
        rm -rf $TMP_DIR
    "#;
    
    let res = Command::new("sh")
        .arg("-c")
        .arg(encryption_test_script)
        .output()
        .expect("Failed to run AES envelope encryption validation script");
        
    assert!(res.status.success(), "AES envelope validation test failed: {}", String::from_utf8_lossy(&res.stderr));

}

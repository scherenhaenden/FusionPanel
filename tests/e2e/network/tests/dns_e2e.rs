#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn require_binary(name: &str) {
    let output = Command::new("which")
        .arg(name)
        .output()
        .expect("Failed to execute `which`");

    if !output.status.success() {
        panic!("RESULT=BLOCKED: Required binary `{}` is missing", name);
    }
}

fn setup_workspace_tmp() -> PathBuf {
    let target_tmp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("tmp");
    fs::create_dir_all(&target_tmp).expect("Failed to create target/tmp");
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let test_dir = target_tmp.join(format!("dns_test_{}_{}", std::process::id(), test_id));
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

fn find_free_port() -> u16 {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind UDP socket");
    socket.local_addr().expect("Failed to get local address").port()
}

struct NamedGuard {
    child: Child,
    dir: PathBuf,
}

impl Drop for NamedGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn start_named(conf: &Path) -> Child {
    let mut child = Command::new("named")
        .arg("-g")
        .arg("-c")
        .arg(conf)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start named");
    thread::sleep(Duration::from_secs(1));

    // Check if it immediately exited
    if let Ok(Some(status)) = child.try_wait() {
        panic!("named exited immediately with status: {}", status);
    }

    child
}

fn query_record(port: u16, name: &str, rtype: &str) -> String {
    let output = Command::new("dig")
        .arg(format!("@127.0.0.1"))
        .arg("-p")
        .arg(port.to_string())
        .arg(name)
        .arg(rtype)
        .arg("+short")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_authoritative_dns_record_lifecycle() {
    require_binary("named");
    require_binary("rndc");
    require_binary("nsupdate");
    require_binary("dnssec-keygen");
    require_binary("named-checkzone");
    require_binary("dig");

    let test_dir = setup_workspace_tmp();
    let port = find_free_port();

    let named_conf_path = test_dir.join("named.conf");
    let named_conf = format!(r#"
options {{
    directory "{dir}";
    pid-file "{dir}/named.pid";
    listen-on port {port} {{ 127.0.0.1; }};
    allow-query {{ any; }};
    recursion no;
}};

zone "example.com" IN {{
    type master;
    file "{dir}/example.com.zone";
    allow-update {{ 127.0.0.1; }};
}};
"#, dir = test_dir.display(), port = port);
    fs::write(&named_conf_path, named_conf).unwrap();

    let zone_file = test_dir.join("example.com.zone");
    let zone_content = r#"
$ORIGIN example.com.
$TTL 86400
@   IN  SOA ns1.example.com. admin.example.com. (
            2023101001 ; Serial
            3600       ; Refresh
            1800       ; Retry
            604800     ; Expire
            86400 )    ; Minimum TTL
    IN  NS  ns1.example.com.
ns1 IN  A   192.0.2.1
www IN  A   192.0.2.2
www IN  AAAA 2001:db8::1
alias IN CNAME www
mail IN MX  10 mx.example.com.
mx  IN  A   192.0.2.3
@   IN  TXT "v=spf1 mx -all"
default._domainkey IN TXT "v=DKIM1; k=rsa; p=MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQD"
"#;
    fs::write(&zone_file, zone_content).unwrap();

    let child = start_named(&named_conf_path);
    let _guard = NamedGuard { child, dir: test_dir.clone() };

    // Verify records exist initially
    assert!(query_record(port, "example.com", "SOA").contains("2023101001"));
    assert!(query_record(port, "www.example.com", "A").contains("192.0.2.2"));
    assert!(query_record(port, "www.example.com", "AAAA").contains("2001:db8::1"));
    assert!(query_record(port, "alias.example.com", "CNAME").contains("www.example.com."));
    assert!(query_record(port, "mail.example.com", "MX").contains("10 mx.example.com."));
    assert!(query_record(port, "example.com", "TXT").contains("v=spf1 mx -all"));
    assert!(query_record(port, "default._domainkey.example.com", "TXT").contains("v=DKIM1"));

    // Update AAAA
    let nsupdate_script = format!(r#"
server 127.0.0.1 {port}
zone example.com
update delete www.example.com AAAA
update add www.example.com 300 IN AAAA 2001:db8::2
show
send
"#, port = port);

    let update_file = test_dir.join("update.txt");
    fs::write(&update_file, nsupdate_script).unwrap();

    let update_status = Command::new("nsupdate")
        .arg(&update_file)
        .status()
        .expect("Failed to run nsupdate");
    assert!(update_status.success(), "nsupdate failed");

    // Check update
    assert!(query_record(port, "www.example.com", "AAAA").contains("2001:db8::2"));
}

#[test]
fn test_dns_serial_number_increment_and_dnssec_validation() {
    require_binary("named");
    require_binary("rndc");
    require_binary("nsupdate");
    require_binary("dnssec-keygen");
    require_binary("named-checkzone");
    require_binary("dig");

    let test_dir = setup_workspace_tmp();
    let port = find_free_port();

    let named_conf_path = test_dir.join("named.conf");
    let named_conf = format!(r#"
options {{
    directory "{dir}";
    pid-file "{dir}/named.pid";
    listen-on port {port} {{ 127.0.0.1; }};
    allow-query {{ any; }};
    recursion no;
    dnssec-validation auto;
}};

zone "example.com" IN {{
    type master;
    file "{dir}/example.com.zone";
    allow-update {{ 127.0.0.1; }};
    dnssec-policy default;
    inline-signing yes;
}};
"#, dir = test_dir.display(), port = port);
    fs::write(&named_conf_path, named_conf).unwrap();

    let zone_file = test_dir.join("example.com.zone");
    let zone_content = r#"
$ORIGIN example.com.
$TTL 86400
@   IN  SOA ns1.example.com. admin.example.com. (
            2023101001 ; Serial
            3600       ; Refresh
            1800       ; Retry
            604800     ; Expire
            86400 )    ; Minimum TTL
    IN  NS  ns1.example.com.
ns1 IN  A   192.0.2.1
"#;
    fs::write(&zone_file, zone_content).unwrap();

    let child = start_named(&named_conf_path);
    let _guard = NamedGuard { child, dir: test_dir.clone() };

    // Check initial serial
    let soa_initial = query_record(port, "example.com", "SOA");
    assert!(soa_initial.contains("2023101001"), "dig failed to return initial SOA record serial");

    // Perform update
    let nsupdate_script = format!(r#"
server 127.0.0.1 {port}
zone example.com
update add new1.example.com 300 IN A 192.0.2.5
show
send
"#, port = port);
    let update_file = test_dir.join("update.txt");
    fs::write(&update_file, nsupdate_script).unwrap();

    let update_status = Command::new("nsupdate")
        .arg(&update_file)
        .status()
        .expect("Failed to run nsupdate");
    assert!(update_status.success(), "nsupdate failed to increment serial");

    // Poll for serial increment
    let mut incremented = false;
    for _ in 0..10 {
        let soa_new = query_record(port, "example.com", "SOA");
        let parts: Vec<&str> = soa_new.split_whitespace().collect();
        if parts.len() > 2 {
            if let Ok(serial) = parts[2].parse::<u32>() {
                if serial > 2023101001 {
                    incremented = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }

    assert!(incremented, "Serial did not increment after update");

    // Test DNSSEC validation - get RRSIG
    let mut signed = false;
    for _ in 0..10 {
        let dnssec_output = Command::new("dig")
            .arg(format!("@127.0.0.1"))
            .arg("-p")
            .arg(port.to_string())
            .arg("example.com")
            .arg("SOA")
            .arg("+dnssec")
            .output()
            .unwrap();

        let dnssec_stdout = String::from_utf8_lossy(&dnssec_output.stdout);
        if dnssec_stdout.contains("RRSIG") {
            signed = true;
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    assert!(signed, "DNSSEC RRSIG record not found");
}

#[test]
fn test_dynamic_dns_updates_via_tsig() {
    require_binary("named");
    require_binary("rndc");
    require_binary("nsupdate");
    require_binary("dnssec-keygen");
    require_binary("named-checkzone");
    require_binary("dig");

    let test_dir = setup_workspace_tmp();
    let port = find_free_port();

    let named_conf_path = test_dir.join("named.conf");
    let named_conf = format!(r#"
key "tsig-key" {{
    algorithm hmac-sha256;
    secret "C/6fQk3l1T8w2V9h/H8w+Q==";
}};

options {{
    directory "{dir}";
    pid-file "{dir}/named.pid";
    listen-on port {port} {{ 127.0.0.1; }};
    allow-query {{ any; }};
    recursion no;
}};

zone "example.com" IN {{
    type master;
    file "{dir}/example.com.zone";
    allow-update {{ key "tsig-key"; }};
}};
"#, dir = test_dir.display(), port = port);
    fs::write(&named_conf_path, named_conf).unwrap();

    let zone_file = test_dir.join("example.com.zone");
    let zone_content = r#"
$ORIGIN example.com.
$TTL 86400
@   IN  SOA ns1.example.com. admin.example.com. (
            2023101001 ; Serial
            3600       ; Refresh
            1800       ; Retry
            604800     ; Expire
            86400 )    ; Minimum TTL
    IN  NS  ns1.example.com.
ns1 IN  A   192.0.2.1
"#;
    fs::write(&zone_file, zone_content).unwrap();

    let child = start_named(&named_conf_path);
    let _guard = NamedGuard { child, dir: test_dir.clone() };

    let tsig_key_file = test_dir.join("tsig.key");
    let tsig_key_content = r#"
key "tsig-key" {
    algorithm hmac-sha256;
    secret "C/6fQk3l1T8w2V9h/H8w+Q==";
};
"#;
    fs::write(&tsig_key_file, tsig_key_content).unwrap();

    let nsupdate_script = format!(r#"
server 127.0.0.1 {port}
zone example.com
update add dyn.example.com 300 IN A 192.0.2.100
show
send
"#, port = port);

    let update_file = test_dir.join("update.txt");
    fs::write(&update_file, nsupdate_script).unwrap();

    let update_status = Command::new("nsupdate")
        .arg("-k")
        .arg(&tsig_key_file)
        .arg(&update_file)
        .status()
        .expect("Failed to run nsupdate");

    assert!(update_status.success(), "nsupdate failed");

    let stdout = query_record(port, "dyn.example.com", "A");
    assert!(stdout.contains("192.0.2.100"), "dig failed to return TSIG updated A record");
}

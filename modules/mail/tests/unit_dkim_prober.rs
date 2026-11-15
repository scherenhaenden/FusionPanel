#![forbid(unsafe_code)]

use mail::dkim_prober::{probe_dkim_propagation, DkimSelector, OpenDkimConfig};
use std::net::{SocketAddr, UdpSocket};

#[test]
fn test_dkim_selector_new() {
    let selector = DkimSelector::new("example.com", "s1", "/path/to/key.private");
    assert_eq!(selector.domain, "example.com");
    assert_eq!(selector.selector, "s1");
    assert_eq!(selector.private_key_path, "/path/to/key.private");
}

#[test]
fn test_open_dkim_config_key_table() {
    let config = OpenDkimConfig::new(vec![
        DkimSelector::new("example.com", "s1", "/path/to/s1.private"),
        DkimSelector::new("test.com", "s2", "/path/to/s2.private"),
    ]);

    let key_table = config.generate_key_table();
    let expected = "s1._domainkey.example.com example.com:s1:/path/to/s1.private\n\
                    s2._domainkey.test.com test.com:s2:/path/to/s2.private";
    assert_eq!(key_table, expected);
}

#[test]
fn test_open_dkim_config_signing_table() {
    let config = OpenDkimConfig::new(vec![
        DkimSelector::new("example.com", "s1", "/path/to/s1.private"),
        DkimSelector::new("test.com", "s2", "/path/to/s2.private"),
        // Second selector for same domain should be ignored for signing table
        DkimSelector::new("example.com", "s3", "/path/to/s3.private"),
    ]);

    let signing_table = config.generate_signing_table();

    assert!(signing_table.contains("*@example.com s1._domainkey.example.com"));
    assert!(signing_table.contains("*@test.com s2._domainkey.test.com"));
    assert!(!signing_table.contains("*@example.com s3._domainkey.example.com"));
}

fn run_mock_dns_server() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();

    std::thread::spawn(move || {
        let mut buf = vec![0u8; 1024];
        if let Ok((size, peer)) = socket.recv_from(&mut buf) {
            let req = &buf[..size];
            if req.len() < 12 {
                return;
            }
            let transaction_id = [req[0], req[1]];

            // Build response
            let mut res = Vec::new();
            res.extend_from_slice(&transaction_id); // Transaction ID
            res.extend_from_slice(&[0x81, 0x80]); // Flags: Standard query response, No error
            res.extend_from_slice(&[req[4], req[5]]); // Questions
            res.extend_from_slice(&[0x00, 0x01]); // Answer RRs: 1
            res.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
            res.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0

            // Copy questions
            res.extend_from_slice(&req[12..]);

            // Answer section
            // Name: Pointer to QNAME at offset 12
            res.extend_from_slice(&[0xC0, 0x0C]);
            // Type: TXT (16)
            res.extend_from_slice(&[0x00, 0x10]);
            // Class: IN (1)
            res.extend_from_slice(&[0x00, 0x01]);
            // TTL: 300
            res.extend_from_slice(&[0x00, 0x00, 0x01, 0x2C]);

            let txt_data = "v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A";
            // RDLENGTH
            res.extend_from_slice(&[0x00, (txt_data.len() + 1) as u8]);
            // TXT Length
            res.push(txt_data.len() as u8);
            // TXT Data
            res.extend_from_slice(txt_data.as_bytes());

            let _ = socket.send_to(&res, peer);
        }
    });

    addr
}

#[test]
fn test_probe_dkim_propagation_success() {
    let dns_server_addr = run_mock_dns_server();

    let result = probe_dkim_propagation(
        "example.com",
        "s1",
        dns_server_addr.ip(),
        dns_server_addr.port(),
    );

    assert!(result.is_ok());
    assert!(result.unwrap());
}

fn run_mock_dns_server_no_dkim() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();

    std::thread::spawn(move || {
        let mut buf = vec![0u8; 1024];
        if let Ok((size, peer)) = socket.recv_from(&mut buf) {
            let req = &buf[..size];
            if req.len() < 12 {
                return;
            }
            let transaction_id = [req[0], req[1]];

            // Build response
            let mut res = Vec::new();
            res.extend_from_slice(&transaction_id); // Transaction ID
            res.extend_from_slice(&[0x81, 0x80]); // Flags: Standard query response, No error
            res.extend_from_slice(&[req[4], req[5]]); // Questions
            res.extend_from_slice(&[0x00, 0x01]); // Answer RRs: 1
            res.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
            res.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0

            // Copy questions
            res.extend_from_slice(&req[12..]);

            // Answer section
            // Name: Pointer to QNAME at offset 12
            res.extend_from_slice(&[0xC0, 0x0C]);
            // Type: TXT (16)
            res.extend_from_slice(&[0x00, 0x10]);
            // Class: IN (1)
            res.extend_from_slice(&[0x00, 0x01]);
            // TTL: 300
            res.extend_from_slice(&[0x00, 0x00, 0x01, 0x2C]);

            let txt_data = "some other txt record";
            // RDLENGTH
            res.extend_from_slice(&[0x00, (txt_data.len() + 1) as u8]);
            // TXT Length
            res.push(txt_data.len() as u8);
            // TXT Data
            res.extend_from_slice(txt_data.as_bytes());

            let _ = socket.send_to(&res, peer);
        }
    });

    addr
}

#[test]
fn test_probe_dkim_propagation_no_dkim() {
    let dns_server_addr = run_mock_dns_server_no_dkim();

    let result = probe_dkim_propagation(
        "example.com",
        "s1",
        dns_server_addr.ip(),
        dns_server_addr.port(),
    );

    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_probe_dkim_propagation_timeout() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();

    let result = probe_dkim_propagation("example.com", "s1", addr.ip(), addr.port());

    assert!(result.is_ok());
    assert!(!result.unwrap()); // Returns false on timeout
}

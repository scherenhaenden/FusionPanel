#![forbid(unsafe_code)]

use fusionpanel_agent_protocol::{
    DnsProbePayload, DnsRecordSpec, DnsRecordType, DnsZoneApplyPayload, DnsZoneDeletePayload,
};
use std::fmt::Write as _;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::process::Command;

const DEFAULT_ZONES_DIR: &str = "/etc/bind/fusionpanel/zones";
const DEFAULT_CONF_PATH: &str = "/etc/bind/fusionpanel/named.conf.fusionpanel";

fn zones_dir() -> PathBuf {
    std::env::var("FUSIONPANEL_DNS_ZONES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_ZONES_DIR))
}

fn named_conf_path() -> PathBuf {
    std::env::var("FUSIONPANEL_DNS_CONF_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONF_PATH))
}

pub fn render_zone_file(payload: &DnsZoneApplyPayload) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "$ORIGIN {}.", payload.domain);
    let _ = writeln!(out, "$TTL {}", payload.default_ttl);
    let _ = writeln!(
        out,
        "@ IN SOA ns1.{domain}. hostmaster.{domain}. ( {serial} 3600 900 604800 300 )",
        domain = payload.domain,
        serial = payload.serial
    );
    let _ = writeln!(out, "@ IN NS ns1.{}.", payload.domain);

    let has_ns1 = payload.records.iter().any(|r| {
        r.name == "ns1" && matches!(r.record_type, DnsRecordType::A | DnsRecordType::Aaaa)
    });
    if !has_ns1 {
        let _ = writeln!(out, "ns1 {} IN A 127.0.0.1", payload.default_ttl);
    }

    for rec in &payload.records {
        let _ = writeln!(out, "{}", format_record_line(rec));
    }
    out
}

fn format_record_line(rec: &DnsRecordSpec) -> String {
    match rec.record_type {
        DnsRecordType::A => format!("{} {} IN A {}", rec.name, rec.ttl, rec.value),
        DnsRecordType::Aaaa => format!("{} {} IN AAAA {}", rec.name, rec.ttl, rec.value),
        DnsRecordType::Cname => {
            let target = ensure_fqdn_if_dotted(&rec.value);
            format!("{} {} IN CNAME {}", rec.name, rec.ttl, target)
        }
        DnsRecordType::Mx => {
            let prio = rec.priority.unwrap_or(10);
            let target = ensure_fqdn_if_dotted(&rec.value);
            format!("{} {} IN MX {} {}", rec.name, rec.ttl, prio, target)
        }
        DnsRecordType::Txt => {
            let quoted = format_txt_chunks(&rec.value);
            format!("{} {} IN TXT {}", rec.name, rec.ttl, quoted)
        }
    }
}

fn ensure_fqdn_if_dotted(target: &str) -> String {
    if target.contains('.') && !target.ends_with('.') {
        format!("{target}.")
    } else {
        target.to_string()
    }
}

fn format_txt_chunks(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return "\"\"".to_string();
    }
    let mut chunks = Vec::new();
    for chunk in bytes.chunks(255) {
        let s = std::str::from_utf8(chunk).unwrap_or("");
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        chunks.push(format!("\"{escaped}\""));
    }
    chunks.join(" ")
}

pub async fn execute_dns_zone_apply(payload: &DnsZoneApplyPayload) -> Result<(), String> {
    let dir = zones_dir();
    let conf = named_conf_path();
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create DNS zones dir: {e}"))?;
    if let Some(parent) = conf.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create DNS conf dir: {e}"))?;
    }

    let zone_filename = format!("db.{}", payload.domain);
    let zone_path = dir.join(&zone_filename);
    let rendered = render_zone_file(payload);
    atomic_write(&zone_path, rendered.as_bytes())?;
    rebuild_named_conf(&dir, &conf)?;
    run_reload_command().await?;
    Ok(())
}

pub async fn execute_dns_zone_delete(payload: &DnsZoneDeletePayload) -> Result<(), String> {
    let dir = zones_dir();
    let conf = named_conf_path();
    let zone_path = dir.join(format!("db.{}", payload.domain));
    if zone_path.exists() {
        fs::remove_file(&zone_path).map_err(|e| format!("failed to remove zone file: {e}"))?;
    }
    if dir.is_dir() {
        if let Some(parent) = conf.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create DNS conf dir: {e}"))?;
        }
        rebuild_named_conf(&dir, &conf)?;
    }
    run_reload_command().await?;
    Ok(())
}

fn rebuild_named_conf(zones_dir: &Path, conf_path: &Path) -> Result<(), String> {
    let mut domains = Vec::new();
    let entries = fs::read_dir(zones_dir).map_err(|e| format!("failed to read zones dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to iterate zones dir: {e}"))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(domain) = name_str.strip_prefix("db.") {
            if !domain.ends_with(".tmp") && entry.path().is_file() {
                domains.push(domain.to_string());
            }
        }
    }
    domains.sort();

    let mut conf = String::new();
    for domain in domains {
        let zone_file = zones_dir.join(format!("db.{domain}"));
        let _ = writeln!(
            conf,
            "zone \"{domain}\" {{ type master; file \"{}\"; }};",
            zone_file.display()
        );
    }
    atomic_write(conf_path, conf.as_bytes())
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, contents).map_err(|e| format!("failed to write temp DNS file: {e}"))?;
    fs::rename(&tmp_path, path).map_err(|e| format!("failed to commit DNS file: {e}"))?;
    Ok(())
}

async fn run_reload_command() -> Result<(), String> {
    let Ok(cmd_str) = std::env::var("FUSIONPANEL_DNS_RELOAD_CMD") else {
        return Ok(());
    };
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }
    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .await
        .map_err(|e| format!("failed to run DNS reload command: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "DNS reload command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub async fn execute_dns_probe(payload: &DnsProbePayload) -> Result<String, String> {
    let server_addr =
        std::env::var("FUSIONPANEL_DNS_PROBE_ADDR").unwrap_or_else(|_| "127.0.0.1:53".to_string());
    let fqdn = if payload.record_name == "@" {
        payload.domain.clone()
    } else {
        format!("{}.{}", payload.record_name, payload.domain)
    };
    let qtype = record_type_code(payload.record_type);
    let query_id: u16 = 0x4650;
    let packet = build_dns_query(query_id, &fqdn, qtype)?;

    let socket = UuidSocketWrapper::bind_and_query(&server_addr, &packet).await?;
    let answers = parse_dns_answers(query_id, &socket, qtype)?;
    if answers.is_empty() {
        return Err(format!(
            "no {:?} records found for {fqdn} at {server_addr}",
            payload.record_type
        ));
    }
    if let Some(expected) = &payload.expected_substring {
        if !answers.iter().any(|ans| ans.contains(expected)) {
            return Err(format!(
                "expected substring '{expected}' not found in {:?} answers for {fqdn}: {answers:?}",
                payload.record_type
            ));
        }
    }
    Ok(format!(
        "OK: {fqdn} {:?} resolved ({})",
        payload.record_type,
        answers.join(", ")
    ))
}

struct UuidSocketWrapper;

impl UuidSocketWrapper {
    async fn bind_and_query(server_addr: &str, packet: &[u8]) -> Result<Vec<u8>, String> {
        let bind_addr = if server_addr.starts_with('[') {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };
        let sock = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| format!("failed to bind UDP probe socket: {e}"))?;
        sock.connect(server_addr)
            .await
            .map_err(|e| format!("failed to connect to DNS server {server_addr}: {e}"))?;
        sock.send(packet)
            .await
            .map_err(|e| format!("failed to send DNS probe packet: {e}"))?;
        let mut buf = vec![0u8; 4096];
        let len = tokio::time::timeout(Duration::from_secs(3), sock.recv(&mut buf))
            .await
            .map_err(|_| format!("DNS probe timed out waiting for {server_addr}"))?
            .map_err(|e| format!("failed to receive DNS probe response: {e}"))?;
        buf.truncate(len);
        Ok(buf)
    }
}

fn record_type_code(rt: DnsRecordType) -> u16 {
    match rt {
        DnsRecordType::A => 1,
        DnsRecordType::Cname => 5,
        DnsRecordType::Mx => 15,
        DnsRecordType::Txt => 16,
        DnsRecordType::Aaaa => 28,
    }
}

fn build_dns_query(id: u16, fqdn: &str, qtype: u16) -> Result<Vec<u8>, String> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&id.to_be_bytes());
    pkt.extend_from_slice(&0x0100u16.to_be_bytes()); // standard query, RD=1
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT=1
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT=0
    pkt.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT=0
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT=0

    for label in fqdn.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(format!("invalid DNS label in '{fqdn}'"));
        }
        pkt.push(label.len() as u8);
        pkt.extend_from_slice(label.as_bytes());
    }
    pkt.push(0);
    pkt.extend_from_slice(&qtype.to_be_bytes());
    pkt.extend_from_slice(&1u16.to_be_bytes()); // QCLASS=IN
    Ok(pkt)
}

fn parse_dns_answers(
    expected_id: u16,
    pkt: &[u8],
    expected_qtype: u16,
) -> Result<Vec<String>, String> {
    if pkt.len() < 12 {
        return Err("DNS response packet too short".to_string());
    }
    let resp_id = u16::from_be_bytes([pkt[0], pkt[1]]);
    if resp_id != expected_id {
        return Err("DNS response ID mismatch".to_string());
    }
    let flags = u16::from_be_bytes([pkt[2], pkt[3]]);
    let rcode = flags & 0x000f;
    if rcode != 0 {
        return Err(format!("DNS server returned RCODE {rcode}"));
    }
    let qdcount = u16::from_be_bytes([pkt[4], pkt[5]]);
    let ancount = u16::from_be_bytes([pkt[6], pkt[7]]);

    let mut offset = 12;
    for _ in 0..qdcount {
        offset = skip_dns_name(pkt, offset)?;
        if offset + 4 > pkt.len() {
            return Err("truncated DNS question section".to_string());
        }
        offset += 4;
    }

    let mut answers = Vec::new();
    for _ in 0..ancount {
        offset = skip_dns_name(pkt, offset)?;
        if offset + 10 > pkt.len() {
            return Err("truncated DNS answer header".to_string());
        }
        let rtype = u16::from_be_bytes([pkt[offset], pkt[offset + 1]]);
        let rdlength = u16::from_be_bytes([pkt[offset + 8], pkt[offset + 9]]) as usize;
        offset += 10;
        if offset + rdlength > pkt.len() {
            return Err("truncated DNS answer rdata".to_string());
        }
        let rdata_start = offset;
        let rdata = &pkt[offset..offset + rdlength];
        offset += rdlength;

        if rtype != expected_qtype {
            continue;
        }
        let formatted = match rtype {
            1 if rdata.len() == 4 => {
                Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3]).to_string()
            }
            28 if rdata.len() == 16 => {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(rdata);
                Ipv6Addr::from(octets).to_string()
            }
            5 => read_dns_name(pkt, rdata_start)?,
            15 if rdata.len() >= 3 => {
                let pref = u16::from_be_bytes([rdata[0], rdata[1]]);
                let exchange = read_dns_name(pkt, rdata_start + 2)?;
                format!("{pref} {exchange}")
            }
            16 => {
                let mut txt = String::new();
                let mut i = 0;
                while i < rdata.len() {
                    let slen = rdata[i] as usize;
                    i += 1;
                    if i + slen > rdata.len() {
                        return Err("truncated DNS TXT chunk".to_string());
                    }
                    txt.push_str(&String::from_utf8_lossy(&rdata[i..i + slen]));
                    i += slen;
                }
                txt
            }
            _ => continue,
        };
        answers.push(formatted);
    }
    Ok(answers)
}

fn skip_dns_name(pkt: &[u8], mut offset: usize) -> Result<usize, String> {
    let mut steps = 0;
    while offset < pkt.len() {
        steps += 1;
        if steps > 128 {
            return Err("DNS name loop".to_string());
        }
        let len = pkt[offset];
        if len == 0 {
            return Ok(offset + 1);
        }
        if (len & 0xc0) == 0xc0 {
            if offset + 1 >= pkt.len() {
                return Err("truncated DNS pointer".to_string());
            }
            return Ok(offset + 2);
        }
        offset += 1 + (len as usize);
    }
    Err("unterminated DNS name".to_string())
}

fn read_dns_name(pkt: &[u8], mut offset: usize) -> Result<String, String> {
    let mut labels = Vec::new();
    let mut jumps = 0;
    loop {
        if offset >= pkt.len() {
            return Err("truncated DNS name".to_string());
        }
        let len = pkt[offset];
        if len == 0 {
            break;
        }
        if (len & 0xc0) == 0xc0 {
            if offset + 1 >= pkt.len() {
                return Err("truncated DNS pointer".to_string());
            }
            let ptr = (((len & 0x3f) as usize) << 8) | (pkt[offset + 1] as usize);
            offset = ptr;
            jumps += 1;
            if jumps > 16 {
                return Err("DNS pointer loop".to_string());
            }
            continue;
        }
        offset += 1;
        let end = offset + (len as usize);
        if end > pkt.len() {
            return Err("truncated DNS label".to_string());
        }
        labels.push(String::from_utf8_lossy(&pkt[offset..end]).to_string());
        offset = end;
    }
    Ok(labels.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn renders_bind_zone_with_soa_serial_and_chunked_dkim_txt() {
        let long_dkim = format!("v=DKIM1; k=rsa; p={}", "A".repeat(280));
        let payload = DnsZoneApplyPayload {
            zone_id: Uuid::new_v4(),
            domain: "example.com".to_string(),
            serial: 42,
            default_ttl: 300,
            records: vec![
                DnsRecordSpec {
                    name: "www".to_string(),
                    record_type: DnsRecordType::A,
                    value: "203.0.113.10".to_string(),
                    ttl: 300,
                    priority: None,
                },
                DnsRecordSpec {
                    name: "default._domainkey".to_string(),
                    record_type: DnsRecordType::Txt,
                    value: long_dkim,
                    ttl: 300,
                    priority: None,
                },
            ],
        };
        let rendered = render_zone_file(&payload);
        assert!(rendered.contains("$ORIGIN example.com."));
        assert!(rendered.contains("( 42 3600 900 604800 300 )"));
        assert!(rendered.contains("www 300 IN A 203.0.113.10"));
        assert!(rendered.contains("default._domainkey 300 IN TXT \"v=DKIM1; k=rsa; p="));
        assert!(rendered.contains("\" \""));
    }

    #[tokio::test]
    async fn probes_live_udp_dns_response() {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = sock.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut buf = [0u8; 512];
            let (len, peer) = sock.recv_from(&mut buf).await.unwrap();
            assert!(len >= 12);
            let mut resp = Vec::new();
            resp.extend_from_slice(&buf[0..2]); // ID
            resp.extend_from_slice(&0x8180u16.to_be_bytes()); // QR=1, NOERROR
            resp.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT=1
            resp.extend_from_slice(&1u16.to_be_bytes()); // ANCOUNT=1
            resp.extend_from_slice(&0u16.to_be_bytes());
            resp.extend_from_slice(&0u16.to_be_bytes());
            resp.extend_from_slice(&buf[12..len]); // question
            resp.extend_from_slice(&[0xc0, 0x0c]); // pointer to qname
            resp.extend_from_slice(&16u16.to_be_bytes()); // TXT
            resp.extend_from_slice(&1u16.to_be_bytes()); // IN
            resp.extend_from_slice(&300u32.to_be_bytes()); // TTL
            let txt = b"v=DKIM1; k=rsa; p=TEST";
            resp.extend_from_slice(&((txt.len() + 1) as u16).to_be_bytes());
            resp.push(txt.len() as u8);
            resp.extend_from_slice(txt);
            sock.send_to(&resp, peer).await.unwrap();
        });

        std::env::set_var("FUSIONPANEL_DNS_PROBE_ADDR", addr.to_string());
        let res = execute_dns_probe(&DnsProbePayload {
            zone_id: Uuid::new_v4(),
            domain: "example.com".to_string(),
            record_name: "default._domainkey".to_string(),
            record_type: DnsRecordType::Txt,
            expected_substring: Some("v=DKIM1;".to_string()),
        })
        .await
        .unwrap();
        assert!(res.contains("v=DKIM1; k=rsa; p=TEST"));
        server.await.unwrap();
    }
}

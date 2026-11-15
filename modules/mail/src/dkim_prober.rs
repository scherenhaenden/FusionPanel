#![forbid(unsafe_code)]

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DkimSelector {
    pub domain: String,
    pub selector: String,
    pub private_key_path: String,
}

impl DkimSelector {
    pub fn new(domain: &str, selector: &str, private_key_path: &str) -> Self {
        Self {
            domain: domain.to_string(),
            selector: selector.to_string(),
            private_key_path: private_key_path.to_string(),
        }
    }
}

pub struct OpenDkimConfig {
    pub selectors: Vec<DkimSelector>,
}

impl OpenDkimConfig {
    pub fn new(selectors: Vec<DkimSelector>) -> Self {
        Self { selectors }
    }

    /// Generates OpenDKIM KeyTable format
    /// Format: <selector>._domainkey.<domain> <domain>:<selector>:<private_key_path>
    pub fn generate_key_table(&self) -> String {
        let mut lines = Vec::new();
        for s in &self.selectors {
            lines.push(format!(
                "{}._domainkey.{} {}:{}:{}",
                s.selector, s.domain, s.domain, s.selector, s.private_key_path
            ));
        }
        lines.join("\n")
    }

    /// Generates OpenDKIM SigningTable format
    /// Format: *@<domain> <selector>._domainkey.<domain>
    pub fn generate_signing_table(&self) -> String {
        let mut lines = Vec::new();
        let mut seen_domains = std::collections::HashSet::new();

        for s in &self.selectors {
            if !seen_domains.contains(&s.domain) {
                lines.push(format!(
                    "*@{} {}._domainkey.{}",
                    s.domain, s.selector, s.domain
                ));
                seen_domains.insert(s.domain.clone());
            }
        }
        lines.join("\n")
    }
}

fn encode_dns_query(domain: &str, transaction_id: u16) -> Vec<u8> {
    let mut packet = Vec::with_capacity(512);

    // Header (12 bytes)
    packet.extend_from_slice(&transaction_id.to_be_bytes()); // Transaction ID
    packet.extend_from_slice(&[0x01, 0x00]); // Flags: Standard query (recursion desired)
    packet.extend_from_slice(&[0x00, 0x01]); // Questions: 1
    packet.extend_from_slice(&[0x00, 0x00]); // Answer RRs: 0
    packet.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
    packet.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0

    // Question Section
    // QNAME
    for label in domain.split('.') {
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0x00); // Root label (terminator)

    // QTYPE (16 for TXT)
    packet.extend_from_slice(&[0x00, 0x10]);
    // QCLASS (1 for IN)
    packet.extend_from_slice(&[0x00, 0x01]);

    packet
}

fn parse_dns_response(
    response: &[u8],
    expected_transaction_id: u16,
) -> Result<Vec<String>, String> {
    if response.len() < 12 {
        return Err("Response too small".to_string());
    }

    let transaction_id = u16::from_be_bytes([response[0], response[1]]);
    if transaction_id != expected_transaction_id {
        return Err("Mismatched transaction ID".to_string());
    }

    let flags = u16::from_be_bytes([response[2], response[3]]);
    if (flags & 0x8000) == 0 {
        return Err("Not a response".to_string());
    }
    let rcode = flags & 0x000F;
    if rcode != 0 {
        return Err(format!("DNS error rcode: {}", rcode));
    }

    let qdcount = u16::from_be_bytes([response[4], response[5]]);
    let ancount = u16::from_be_bytes([response[6], response[7]]);

    if ancount == 0 {
        return Ok(Vec::new());
    }

    let mut offset = 12;
    // Skip questions
    for _ in 0..qdcount {
        while offset < response.len() {
            let len = response[offset] as usize;
            if (len & 0xC0) == 0xC0 {
                // Pointer
                offset += 2;
                break;
            } else if len == 0 {
                offset += 1;
                break;
            } else {
                offset += 1 + len;
            }
        }
        offset += 4; // Skip QTYPE and QCLASS
    }

    let mut txt_records = Vec::new();

    // Parse answers
    for _ in 0..ancount {
        // Skip NAME
        while offset < response.len() {
            let len = response[offset] as usize;
            if (len & 0xC0) == 0xC0 {
                offset += 2;
                break;
            } else if len == 0 {
                offset += 1;
                break;
            } else {
                offset += 1 + len;
            }
        }

        if offset + 10 > response.len() {
            return Err("Incomplete answer section".to_string());
        }

        let type_code = u16::from_be_bytes([response[offset], response[offset + 1]]);
        offset += 2;
        let class = u16::from_be_bytes([response[offset], response[offset + 1]]);
        offset += 2;
        let _ttl = u32::from_be_bytes([
            response[offset],
            response[offset + 1],
            response[offset + 2],
            response[offset + 3],
        ]);
        offset += 4;
        let rdlength = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
        offset += 2;

        if offset + rdlength > response.len() {
            return Err("RDATA out of bounds".to_string());
        }

        // QTYPE 16 is TXT and QCLASS 1 is IN
        if type_code == 16 && class == 1 {
            let mut rdata_offset = offset;
            let end_of_rdata = offset + rdlength;
            let mut txt_data = String::new();

            while rdata_offset < end_of_rdata {
                let txt_len = response[rdata_offset] as usize;
                rdata_offset += 1;
                if rdata_offset + txt_len > end_of_rdata {
                    return Err("TXT chunk out of bounds".to_string());
                }

                if let Ok(chunk) =
                    std::str::from_utf8(&response[rdata_offset..rdata_offset + txt_len])
                {
                    txt_data.push_str(chunk);
                }
                rdata_offset += txt_len;
            }
            txt_records.push(txt_data);
        }

        offset += rdlength;
    }

    Ok(txt_records)
}

/// Probes DNS for the DKIM TXT record.
/// Queries the authoritative or given DNS server over UDP 53 for the TXT record of <selector>._domainkey.<domain>.
pub fn probe_dkim_propagation(
    domain: &str,
    selector: &str,
    nameserver_ip: IpAddr,
    nameserver_port: u16,
) -> Result<bool, Box<dyn std::error::Error>> {
    let fqdn = format!("{}._domainkey.{}", selector, domain);

    // Bind to any ephemeral port
    let local_bind = match nameserver_ip {
        IpAddr::V4(_) => "0.0.0.0:0",
        IpAddr::V6(_) => "[::]:0",
    };

    let socket = UdpSocket::bind(local_bind)?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;

    let target = SocketAddr::new(nameserver_ip, nameserver_port);

    let transaction_id = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
        % 65536) as u16;
    let packet = encode_dns_query(&fqdn, transaction_id);

    socket.send_to(&packet, target)?;

    let mut buf = vec![0u8; 1024];

    if let Ok((size, _)) = socket.recv_from(&mut buf) {
        let response = &buf[..size];
        if let Ok(records) = parse_dns_response(response, transaction_id) {
            for record in records {
                if record.contains("v=DKIM1") {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

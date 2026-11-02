#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct DnsZone {
    pub name: String,
    pub serial: u32,
    pub records: Vec<DnsRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
    SRV,
    CAA,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: RecordType,
    pub value: String,
    pub ttl: u32,
    pub priority: Option<u16>,
}

impl DnsZone {
    pub fn new(name: String, initial_serial: u32) -> Self {
        Self {
            name,
            serial: initial_serial,
            records: Vec::new(),
        }
    }

    fn increment_serial(&mut self) {
        self.serial = self.serial.wrapping_add(1);
    }

    pub fn add_record(&mut self, record: DnsRecord) -> Result<(), String> {
        validate_record(&record)?;
        self.records.push(record);
        self.increment_serial();
        Ok(())
    }

    pub fn update_record(&mut self, index: usize, record: DnsRecord) -> Result<(), String> {
        if index >= self.records.len() {
            return Err("Record index out of bounds".to_string());
        }
        validate_record(&record)?;
        self.records[index] = record;
        self.increment_serial();
        Ok(())
    }

    pub fn delete_record(&mut self, index: usize) -> Result<(), String> {
        if index >= self.records.len() {
            return Err("Record index out of bounds".to_string());
        }
        self.records.remove(index);
        self.increment_serial();
        Ok(())
    }
}

pub fn validate_record(record: &DnsRecord) -> Result<(), String> {
    if record.name.is_empty() {
        return Err("Record name cannot be empty".to_string());
    }
    if record.value.is_empty() {
        return Err("Record value cannot be empty".to_string());
    }

    match record.record_type {
        RecordType::A => {
            if record.value.parse::<std::net::Ipv4Addr>().is_err() {
                return Err("Invalid IPv4 address".to_string());
            }
        }
        RecordType::AAAA => {
            if record.value.parse::<std::net::Ipv6Addr>().is_err() {
                return Err("Invalid IPv6 address".to_string());
            }
        }
        RecordType::MX => {
            if record.priority.is_none() {
                return Err("MX record requires a priority".to_string());
            }
        }
        RecordType::SRV => {
            if record.priority.is_none() {
                return Err("SRV record requires a priority".to_string());
            }
            // Basic format validation for SRV (weight port target)
            let parts: Vec<&str> = record.value.split_whitespace().collect();
            if parts.len() != 3 {
                return Err("SRV record value must be in format 'weight port target'".to_string());
            }
            if parts[0].parse::<u16>().is_err() || parts[1].parse::<u16>().is_err() {
                return Err("SRV weight and port must be numbers".to_string());
            }
        }
        RecordType::CAA => {
            // Basic format validation for CAA (flags tag value)
            let parts: Vec<&str> = record.value.splitn(3, ' ').collect();
            if parts.len() != 3 {
                return Err("CAA record value must be in format 'flags tag value'".to_string());
            }
            if parts[0].parse::<u8>().is_err() {
                return Err("CAA flags must be a number".to_string());
            }
            let valid_tags = ["issue", "issuewild", "iodef"];
            if !valid_tags.contains(&parts[1]) {
                return Err("Invalid CAA tag".to_string());
            }
            // The value is enclosed in quotes usually, but we keep it simple here
            // Just ensure it's not empty
            if parts[2].is_empty() {
                return Err("CAA value cannot be empty".to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

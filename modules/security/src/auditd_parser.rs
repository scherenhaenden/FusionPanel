#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditRecordType {
    Syscall,
    Execve,
    UserCmd,
    Other(String),
}

impl From<&str> for AuditRecordType {
    fn from(s: &str) -> Self {
        match s {
            "SYSCALL" => AuditRecordType::Syscall,
            "EXECVE" => AuditRecordType::Execve,
            "USER_CMD" => AuditRecordType::UserCmd,
            _ => AuditRecordType::Other(s.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub record_type: AuditRecordType,
    pub msg_id: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditEvent {
    pub msg_id: String,
    pub records: Vec<AuditRecord>,
}

impl AuditEvent {
    pub fn new(msg_id: String) -> Self {
        Self {
            msg_id,
            records: Vec::new(),
        }
    }

    pub fn get_field(&self, record_type: AuditRecordType, field: &str) -> Option<&String> {
        self.records
            .iter()
            .find(|r| r.record_type == record_type)
            .and_then(|r| r.fields.get(field))
    }

    pub fn uid(&self) -> Option<u32> {
        self.get_field(AuditRecordType::Syscall, "uid")
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                self.get_field(AuditRecordType::UserCmd, "uid")
                    .and_then(|s| s.parse().ok())
            })
    }

    pub fn auid(&self) -> Option<u32> {
        self.get_field(AuditRecordType::Syscall, "auid")
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                self.get_field(AuditRecordType::UserCmd, "auid")
                    .and_then(|s| s.parse().ok())
            })
    }

    pub fn exe(&self) -> Option<&String> {
        self.get_field(AuditRecordType::Syscall, "exe")
            .or_else(|| self.get_field(AuditRecordType::UserCmd, "exe"))
    }

    pub fn comm(&self) -> Option<&String> {
        self.get_field(AuditRecordType::Syscall, "comm")
    }

    pub fn success(&self) -> Option<bool> {
        self.get_field(AuditRecordType::Syscall, "success")
            .map(|s| s == "yes")
            .or_else(|| {
                self.get_field(AuditRecordType::UserCmd, "res")
                    .map(|s| s == "success" || s == "success'")
            })
    }

    pub fn is_privilege_escalation(&self) -> bool {
        // Simple heuristic: auid != 0, uid == 0 or euid == 0, success == yes
        if self.success() != Some(true) {
            return false;
        }

        let auid = self.auid().unwrap_or(4294967295);
        if auid == 0 || auid == 4294967295 {
            return false; // Already root or unset auid
        }

        let uid = self.uid().unwrap_or(4294967295);
        let euid = self.get_field(AuditRecordType::Syscall, "euid")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(4294967295);

        uid == 0 || euid == 0
    }

    pub fn get_command_line(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(execve_record) = self.records.iter().find(|r| r.record_type == AuditRecordType::Execve) {
            let mut i = 0;
            while let Some(arg) = execve_record.fields.get(&format!("a{}", i)) {
                args.push(arg.clone());
                i += 1;
            }
            if let Some(argc) = execve_record.fields.get("argc").and_then(|s| s.parse::<usize>().ok()) {
                if args.len() < argc {
                    // Try parsing argc arguments
                    for j in i..argc {
                       if let Some(arg) = execve_record.fields.get(&format!("a{}", j)) {
                           args.push(arg.clone());
                       }
                    }
                }
            }
        } else if let Some(cmd) = self.get_field(AuditRecordType::UserCmd, "cmd") {
            args.push(cmd.clone());
        }
        args
    }

    pub fn is_suspicious_root_command(&self) -> bool {
        let is_root = self.uid() == Some(0)
            || self.auid() == Some(0)
            || self.get_field(AuditRecordType::Syscall, "euid").and_then(|s| s.parse::<u32>().ok()) == Some(0);

        if !is_root {
            return false;
        }

        let args = self.get_command_line();
        if args.is_empty() {
            return false;
        }

        let cmd = args.join(" ");

        let suspicious_patterns = [
            "rm -rf /",
            "chmod 777",
            "chown root:root",
            "nc -e",
            "/bin/bash -i",
            "/dev/tcp",
            "wget http",
            "curl http",
        ];

        suspicious_patterns.iter().any(|pattern| cmd.contains(pattern))
    }
}

pub struct AuditdParser {
    events: HashMap<String, AuditEvent>,
    completed_events: VecDeque<AuditEvent>,
}

impl Default for AuditdParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditdParser {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
            completed_events: VecDeque::new(),
        }
    }

    pub fn parse_line(&mut self, line: &str) {
        if let Some(record) = Self::parse_record(line) {
            let msg_id = record.msg_id.clone();

            let event = self.events.entry(msg_id.clone()).or_insert_with(|| AuditEvent::new(msg_id.clone()));
            let is_proctitle = record.record_type == AuditRecordType::Other("PROCTITLE".to_string());
            let is_eoe = record.record_type == AuditRecordType::Other("EOE".to_string());
            let is_user_cmd = record.record_type == AuditRecordType::UserCmd;

            event.records.push(record);

            // USER_CMD is often stand-alone or ends the user context audit
            if is_proctitle || is_eoe || is_user_cmd {
                if let Some(completed_event) = self.events.remove(&msg_id) {
                    self.completed_events.push_back(completed_event);
                }
            }
        }
    }

    fn parse_record(line: &str) -> Option<AuditRecord> {
        let mut fields = HashMap::new();
        let mut record_type = None;
        let mut msg_id = None;

        let parts = line.split_whitespace();
        for part in parts {
            if let Some((k, v)) = part.split_once('=') {
                if k == "type" {
                    record_type = Some(v.to_string());
                } else if k == "msg" {
                    // It can be msg=audit(...) or msg='cwd="..."'
                    if v.starts_with("audit(") {
                        if let Some(id) = v.strip_prefix("audit(").and_then(|s| s.strip_suffix("):")) {
                            msg_id = Some(id.to_string());
                        }
                    } else if v.starts_with('\'') {
                        // ignore nested msg for id parsing
                    }
                } else {
                    let decoded_v = Self::decode_audit_value(v);
                    fields.insert(k.to_string(), decoded_v);
                }
            }
        }

        // Also look for msg=audit(...) in the original line more reliably
        if msg_id.is_none() {
            if let Some(audit_start) = line.find("audit(") {
                let rest = &line[audit_start + 6..];
                if let Some(audit_end) = rest.find("):") {
                    msg_id = Some(rest[..audit_end].to_string());
                }
            }
        }

        if let (Some(rt), Some(mid)) = (record_type, msg_id) {
            Some(AuditRecord {
                record_type: AuditRecordType::from(rt.as_str()),
                msg_id: mid,
                fields,
            })
        } else {
            None
        }
    }

    fn decode_hex(hex_str: &str) -> Result<Vec<u8>, ()> {
        if hex_str.len() % 2 != 0 {
            return Err(());
        }

        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        let chars: Vec<char> = hex_str.chars().collect();

        for i in (0..chars.len()).step_by(2) {
            let high = chars[i].to_digit(16).ok_or(())? as u8;
            let low = chars[i + 1].to_digit(16).ok_or(())? as u8;
            bytes.push((high << 4) | low);
        }

        Ok(bytes)
    }

    pub fn decode_audit_value(val: &str) -> String {
        let val = val.trim_end_matches('\'').trim_end_matches('"');
        let val = val.trim_start_matches('\'').trim_start_matches('"');

        // If it looks like hex (only 0-9a-fA-F) and even length
        // Must contain at least one letter so we don't treat plain numbers like "1000" as hex string
        if val.len() % 2 == 0 && val.chars().all(|c| c.is_ascii_hexdigit()) && val.chars().any(|c| c.is_ascii_alphabetic()) {
            if let Ok(bytes) = Self::decode_hex(val) {
                if let Ok(s) = String::from_utf8(bytes) {
                    // Null terminated string handling from auditd
                    // Ensure the decoded string is mostly printable ASCII to avoid accidentally decoding valid numbers that happen to contain A-F
                    if s.chars().all(|c| c.is_ascii() && (c.is_ascii_graphic() || c.is_ascii_whitespace() || c == '\0')) {
                        return s.trim_end_matches('\0').to_string();
                    }
                }
            }
        }

        val.to_string()
    }

    pub fn next_event(&mut self) -> Option<AuditEvent> {
        self.completed_events.pop_front()
    }
}

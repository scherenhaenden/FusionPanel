use regex::Regex;
use std::collections::VecDeque;
use std::sync::OnceLock;

static CREDENTIAL_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_credential_regex() -> &'static Regex {
    CREDENTIAL_REGEX.get_or_init(|| {
        Regex::new(r"(?i)(password|token|secret|api_key|apikey)[\s:=]+[^\s]+").unwrap()
    })
}

#[derive(Debug)]
pub struct BoundedLogReader {
    buffer: VecDeque<String>,
    capacity: usize,
}

impl BoundedLogReader {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn append_log(&mut self, log: &str) {
        let regex = get_credential_regex();
        let redacted = regex.replace_all(log, "$1 = ***REDACTED***").into_owned();

        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(redacted);
    }

    pub fn get_logs(&self) -> Vec<String> {
        self.buffer.iter().cloned().collect()
    }
}

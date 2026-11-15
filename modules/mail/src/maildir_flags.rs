#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet, BTreeMap};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use thiserror::Error;

/// Error type for Maildir Flags and Dovecot Keywords
#[derive(Debug, Error)]
pub enum MaildirError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid filename format")]
    InvalidFilename,
    #[error("Invalid dovecot-keywords format")]
    InvalidKeywordsFormat,
    #[error("Keyword limit exceeded (max 26)")]
    KeywordLimitExceeded,
}

/// Represents the standard Maildir flags
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaildirFlags {
    pub passed: bool,
    pub replied: bool,
    pub seen: bool,
    pub trashed: bool,
    pub draft: bool,
    pub flagged: bool,
    pub custom_keywords: HashSet<char>, // Lowercase 'a' to 'z' mapped to 0 to 25
}

impl MaildirFlags {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse Maildir flags from a filename.
    /// Expects the filename to end with `:2,<flags>`
    pub fn parse_from_filename(filename: &str) -> Result<Self, MaildirError> {
        let parts: Vec<&str> = filename.split(":2,").collect();
        let mut flags = MaildirFlags::new();

        if parts.len() != 2 {
            if filename.contains(":2,") {
                // Return empty flags if it ends exactly with `:2,`
                if filename.ends_with(":2,") {
                    return Ok(flags);
                }
            } else if !filename.contains(':') {
                 return Ok(flags); // No flags yet
            }

            // It could be something else entirely, but typically we either have :2,flags or just the filename
            // For this requirement let's just find the last :2, if it exists
            if let Some(pos) = filename.rfind(":2,") {
                let flags_str = &filename[pos + 3..];
                Self::parse_flags_str(flags_str, &mut flags);
                return Ok(flags);
            }
            return Ok(flags);
        }

        let flags_str = parts[1];
        Self::parse_flags_str(flags_str, &mut flags);

        Ok(flags)
    }

    fn parse_flags_str(flags_str: &str, flags: &mut MaildirFlags) {
        for c in flags_str.chars() {
            match c {
                'P' => flags.passed = true,
                'R' => flags.replied = true,
                'S' => flags.seen = true,
                'T' => flags.trashed = true,
                'D' => flags.draft = true,
                'F' => flags.flagged = true,
                'a'..='z' => { flags.custom_keywords.insert(c); },
                _ => {} // Ignore unknown flags
            }
        }
    }

    /// Serialize flags back to the `:2,<flags>` format
    /// The standard flags should be sorted alphabetically: D, F, P, R, S, T.
    /// Custom keywords (a-z) follow alphabetically.
    pub fn serialize(&self) -> String {
        let mut s = String::new();
        if self.draft { s.push('D'); }
        if self.flagged { s.push('F'); }
        if self.passed { s.push('P'); }
        if self.replied { s.push('R'); }
        if self.seen { s.push('S'); }
        if self.trashed { s.push('T'); }

        let mut custom: Vec<char> = self.custom_keywords.iter().cloned().collect();
        custom.sort_unstable();
        for c in custom {
            s.push(c);
        }

        format!(":2,{}", s)
    }

    /// Append or replace the flags in a given filename
    pub fn apply_to_filename(&self, filename: &str) -> String {
        let base = if let Some(pos) = filename.rfind(":2,") {
            &filename[..pos]
        } else {
            filename
        };
        format!("{}{}", base, self.serialize())
    }
}

/// Manages the `dovecot-keywords` file
#[derive(Debug, Clone)]
pub struct DovecotKeywords {
    /// Maps integer bit (0-25) to keyword string
    pub index_to_keyword: BTreeMap<u8, String>,
    /// Maps keyword string to integer bit (0-25)
    pub keyword_to_index: HashMap<String, u8>,
}

impl DovecotKeywords {
    pub fn new() -> Self {
        Self {
            index_to_keyword: BTreeMap::new(),
            keyword_to_index: HashMap::new(),
        }
    }

    /// Loads the `dovecot-keywords` file from the given path
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, MaildirError> {
        let mut keywords = Self::new();
        if !path.as_ref().exists() {
            return Ok(keywords);
        }

        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(MaildirError::InvalidKeywordsFormat);
            }

            let index = parts[0].parse::<u8>().map_err(|_| MaildirError::InvalidKeywordsFormat)?;
            if index > 25 {
                return Err(MaildirError::InvalidKeywordsFormat);
            }

            let keyword = parts[1].to_string();

            keywords.index_to_keyword.insert(index, keyword.clone());
            keywords.keyword_to_index.insert(keyword, index);
        }

        Ok(keywords)
    }

    /// Saves the `dovecot-keywords` to the given path
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), MaildirError> {
        let mut file = fs::File::create(path)?;
        for (index, keyword) in &self.index_to_keyword {
            writeln!(file, "{} {}", index, keyword)?;
        }
        Ok(())
    }

    /// Registers a new keyword, returning its corresponding character flag ('a'-'z')
    /// If the keyword already exists, returns its existing character.
    pub fn register_keyword(&mut self, keyword: &str) -> Result<char, MaildirError> {
        if let Some(&index) = self.keyword_to_index.get(keyword) {
            return Ok((b'a' + index) as char);
        }

        // Find next available index
        let mut next_index = 0;
        while next_index <= 25 && self.index_to_keyword.contains_key(&next_index) {
            next_index += 1;
        }

        if next_index > 25 {
            return Err(MaildirError::KeywordLimitExceeded);
        }

        self.index_to_keyword.insert(next_index, keyword.to_string());
        self.keyword_to_index.insert(keyword.to_string(), next_index);

        Ok((b'a' + next_index) as char)
    }

    /// Resolves a set of character flags ('a'-'z') to their corresponding keyword strings
    pub fn resolve_flags(&self, flags: &HashSet<char>) -> Vec<String> {
        let mut resolved = Vec::new();
        for &c in flags {
            if c.is_ascii_lowercase() {
                let index = (c as u8) - b'a';
                if let Some(keyword) = self.index_to_keyword.get(&index) {
                    resolved.push(keyword.clone());
                }
            }
        }
        resolved
    }
}

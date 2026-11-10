#![forbid(unsafe_code)]

use std::fmt::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogrotateError {
    #[error("Invalid octal mode: {0}")]
    InvalidMode(String),
    #[error("Conflict: both copytruncate and create are specified")]
    ConflictCopytruncateCreate,
    #[error("Formatting error")]
    FmtError(#[from] std::fmt::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone)]
pub struct LogrotateConfig {
    pub paths: Vec<String>,
    pub frequency: Option<Frequency>,
    pub rotate: Option<u32>,
    pub size: Option<String>,
    pub maxsize: Option<String>,
    pub compress: bool,
    pub delaycompress: bool,
    pub missingok: bool,
    pub notifempty: bool,
    pub copytruncate: bool,
    pub create: Option<(String, String, String)>, // mode, owner, group
    pub sharedscripts: bool,
    pub postrotate: Option<String>,
}

pub struct LogrotateConfigBuilder {
    config: LogrotateConfig,
}

impl LogrotateConfigBuilder {
    pub fn new(paths: Vec<String>) -> Self {
        Self {
            config: LogrotateConfig {
                paths,
                frequency: None,
                rotate: None,
                size: None,
                maxsize: None,
                compress: false,
                delaycompress: false,
                missingok: false,
                notifempty: false,
                copytruncate: false,
                create: None,
                sharedscripts: false,
                postrotate: None,
            },
        }
    }

    pub fn frequency(mut self, freq: Frequency) -> Self {
        self.config.frequency = Some(freq);
        self
    }

    pub fn rotate(mut self, count: u32) -> Self {
        self.config.rotate = Some(count);
        self
    }

    pub fn size(mut self, size: impl Into<String>) -> Self {
        self.config.size = Some(size.into());
        self
    }

    pub fn maxsize(mut self, maxsize: impl Into<String>) -> Self {
        self.config.maxsize = Some(maxsize.into());
        self
    }

    pub fn compress(mut self) -> Self {
        self.config.compress = true;
        self
    }

    pub fn delaycompress(mut self) -> Self {
        self.config.delaycompress = true;
        self
    }

    pub fn missingok(mut self) -> Self {
        self.config.missingok = true;
        self
    }

    pub fn notifempty(mut self) -> Self {
        self.config.notifempty = true;
        self
    }

    pub fn copytruncate(mut self) -> Self {
        self.config.copytruncate = true;
        self
    }

    pub fn create(
        mut self,
        mode: impl Into<String>,
        owner: impl Into<String>,
        group: impl Into<String>,
    ) -> Self {
        self.config.create = Some((mode.into(), owner.into(), group.into()));
        self
    }

    pub fn sharedscripts(mut self) -> Self {
        self.config.sharedscripts = true;
        self
    }

    pub fn postrotate(mut self, script: impl Into<String>) -> Self {
        self.config.postrotate = Some(script.into());
        self
    }

    pub fn build(self) -> Result<String, LogrotateError> {
        self.validate()?;

        let mut out = String::new();

        for (i, path) in self.config.paths.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(path);
        }
        out.push_str(" {\n");

        if let Some(freq) = &self.config.frequency {
            match freq {
                Frequency::Daily => out.push_str("    daily\n"),
                Frequency::Weekly => out.push_str("    weekly\n"),
                Frequency::Monthly => out.push_str("    monthly\n"),
            }
        }

        if let Some(r) = self.config.rotate {
            writeln!(out, "    rotate {}", r)?;
        }
        if let Some(s) = &self.config.size {
            writeln!(out, "    size {}", s)?;
        }
        if let Some(ms) = &self.config.maxsize {
            writeln!(out, "    maxsize {}", ms)?;
        }
        if self.config.missingok {
            out.push_str("    missingok\n");
        }
        if self.config.notifempty {
            out.push_str("    notifempty\n");
        }
        if self.config.compress {
            out.push_str("    compress\n");
        }
        if self.config.delaycompress {
            out.push_str("    delaycompress\n");
        }
        if self.config.copytruncate {
            out.push_str("    copytruncate\n");
        }
        if let Some((mode, owner, group)) = &self.config.create {
            writeln!(out, "    create {} {} {}", mode, owner, group)?;
        }
        if self.config.sharedscripts {
            out.push_str("    sharedscripts\n");
        }
        if let Some(script) = &self.config.postrotate {
            out.push_str("    postrotate\n");
            for line in script.lines() {
                writeln!(out, "        {}", line)?;
            }
            out.push_str("    endscript\n");
        }

        out.push_str("}\n");

        Ok(out)
    }

    fn validate(&self) -> Result<(), LogrotateError> {
        if self.config.copytruncate && self.config.create.is_some() {
            return Err(LogrotateError::ConflictCopytruncateCreate);
        }

        if let Some((mode, _, _)) = &self.config.create {
            if !Self::is_valid_octal_mode(mode) {
                return Err(LogrotateError::InvalidMode(mode.clone()));
            }
        }

        Ok(())
    }

    fn is_valid_octal_mode(mode: &str) -> bool {
        if mode.is_empty() || mode.len() > 4 {
            return false;
        }
        for c in mode.chars() {
            if !c.is_ascii_digit() || c > '7' {
                return false;
            }
        }
        true
    }
}

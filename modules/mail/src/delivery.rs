use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::domain::MailAddress;
use crate::error::MailError;

/// Safe Maildir delivery and storage abstraction.
/// Implements standard Maildir structure (`tmp/`, `new/`, `cur/`) with
/// atomic message writing (write to `tmp/`, rename to `new/`), path traversal
/// safety, and message retrieval.
#[derive(Debug, Clone)]
pub struct MaildirDelivery {
    root_path: PathBuf,
}

impl MaildirDelivery {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    /// Resolves the base directory for a specific mailbox address,
    /// enforcing strict path containment to prevent path traversal vulnerabilities.
    pub fn mailbox_dir(&self, address: &MailAddress) -> Result<PathBuf, MailError> {
        let domain_str = address.domain().as_str();
        let local_part = address.local_part();

        // Extra path traversal sanity checks
        if domain_str.contains("..")
            || local_part.contains("..")
            || local_part.contains('/')
            || local_part.contains('\\')
        {
            return Err(MailError::InvalidInput(
                "Path traversal detected in mailbox path".to_string(),
            ));
        }

        let target = self.root_path.join(domain_str).join(local_part);
        Ok(target)
    }

    /// Initializes a Maildir directory structure: `<root>/<domain>/<user>/{tmp,new,cur}`
    pub fn init_mailbox(&self, address: &MailAddress) -> Result<(), MailError> {
        let base = self.mailbox_dir(address)?;
        for folder in &["tmp", "new", "cur"] {
            let path = base.join(folder);
            fs::create_dir_all(&path).map_err(|e| {
                MailError::OperationFailed(format!(
                    "Failed to create Maildir directory {:?}: {}",
                    path, e
                ))
            })?;
        }
        Ok(())
    }

    /// Atomically delivers an email message into the recipient's Maildir.
    /// Following standard Maildir protocol:
    /// 1. Write the message to `tmp/<unique_name>`
    /// 2. Flush and close the file
    /// 3. Atomically rename/move `tmp/<unique_name>` to `new/<unique_name>`
    pub fn deliver_message(
        &self,
        recipient: &MailAddress,
        sender: &MailAddress,
        content: &[u8],
    ) -> Result<String, MailError> {
        let base = self.mailbox_dir(recipient)?;
        let tmp_dir = base.join("tmp");
        let new_dir = base.join("new");

        if !tmp_dir.exists() || !new_dir.exists() {
            self.init_mailbox(recipient)?;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let msg_id = Uuid::new_v4();
        let file_name = format!("{}.{}.localhost", timestamp, msg_id);

        let tmp_file_path = tmp_dir.join(&file_name);
        let new_file_path = new_dir.join(&file_name);

        // Prepend Received header
        let received_header = format!(
            "Return-Path: <{}>\r\nDelivered-To: <{}>\r\nReceived: by FusionPanel Mail Delivery (S14);\r\n\t{}\r\n\r\n",
            sender, recipient, timestamp
        );

        // 1. Write to tmp
        {
            let mut file = File::create(&tmp_file_path).map_err(|e| {
                MailError::OperationFailed(format!("Failed to create message in tmp: {}", e))
            })?;
            file.write_all(received_header.as_bytes()).map_err(|e| {
                MailError::OperationFailed(format!("Failed to write header to tmp message: {}", e))
            })?;
            file.write_all(content).map_err(|e| {
                MailError::OperationFailed(format!("Failed to write content to tmp message: {}", e))
            })?;
            file.flush().map_err(|e| {
                MailError::OperationFailed(format!("Failed to flush message to disk: {}", e))
            })?;
        }

        // 2. Atomic rename from tmp to new
        fs::rename(&tmp_file_path, &new_file_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_file_path);
            MailError::OperationFailed(format!("Failed to move message to new/: {}", e))
        })?;

        Ok(file_name)
    }

    /// Lists messages in a mailbox directory (`new` or `cur`).
    pub fn list_messages(
        &self,
        address: &MailAddress,
        folder: &str,
    ) -> Result<Vec<String>, MailError> {
        if folder != "new" && folder != "cur" && folder != "tmp" {
            return Err(MailError::InvalidInput(
                "Invalid Maildir folder".to_string(),
            ));
        }
        let dir = self.mailbox_dir(address)?.join(folder);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut list = Vec::new();
        let entries = fs::read_dir(dir).map_err(|e| {
            MailError::OperationFailed(format!("Failed to read folder entries: {}", e))
        })?;
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                list.push(name);
            }
        }
        list.sort();
        Ok(list)
    }

    /// Reads the raw content of a specific message from `new/` or `cur/`.
    pub fn read_message(
        &self,
        address: &MailAddress,
        folder: &str,
        file_name: &str,
    ) -> Result<Vec<u8>, MailError> {
        if folder != "new" && folder != "cur" {
            return Err(MailError::InvalidInput(
                "Invalid Maildir folder".to_string(),
            ));
        }
        if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
            return Err(MailError::InvalidInput(
                "Invalid message filename".to_string(),
            ));
        }
        let msg_path = self.mailbox_dir(address)?.join(folder).join(file_name);
        let mut file = File::open(&msg_path).map_err(|e| {
            MailError::OperationFailed(format!("Failed to open message {:?}: {}", msg_path, e))
        })?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| {
            MailError::OperationFailed(format!("Failed to read message content: {}", e))
        })?;
        Ok(buffer)
    }
}

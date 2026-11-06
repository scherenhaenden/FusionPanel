use crate::domain::MailAddress;
use crate::error::MailError;
use crate::models::{MailAlias, MailDomain, Mailbox, SmtpRelayConfig};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Distinguish Plan, AppliedResult, and ObservedState.
/// A Plan represents the generated, validated staged configuration actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailConfigPlan {
    pub plan_id: Uuid,
    pub virtual_mailbox_domains: Vec<String>,
    pub virtual_mailbox_maps: Vec<String>,
    pub virtual_alias_maps: Vec<String>,
    pub relay_host_directive: Option<String>,
    pub dovecot_passwd_entries: Vec<String>,
}

impl MailConfigPlan {
    /// Generates a configuration plan from domains, mailboxes, aliases, and relay settings.
    /// Strictly validates all entries to ensure no config directive injection is possible.
    pub fn generate(
        domains: &[MailDomain],
        mailboxes: &[Mailbox],
        aliases: &[MailAlias],
        relay: Option<&SmtpRelayConfig>,
    ) -> Result<Self, MailError> {
        let mut virtual_mailbox_domains = Vec::new();
        for d in domains {
            if !d.is_active {
                continue;
            }
            let dom = d.name.as_str();
            if dom.contains('\n') || dom.contains('\r') || dom.contains(' ') {
                return Err(MailError::InvalidInput(format!(
                    "Invalid domain in config: {}",
                    dom
                )));
            }
            virtual_mailbox_domains.push(format!("{} OK", dom));
        }
        virtual_mailbox_domains.sort();

        let mut virtual_mailbox_maps = Vec::new();
        let mut dovecot_passwd_entries = Vec::new();
        for mb in mailboxes {
            if !mb.is_active {
                continue;
            }
            let addr = mb.address.full_address();
            let subpath = &mb.maildir_subpath;
            if addr.contains('\n')
                || addr.contains('\r')
                || subpath.contains('\n')
                || subpath.contains('\r')
            {
                return Err(MailError::InvalidInput(
                    "Invalid mailbox entry in config".to_string(),
                ));
            }
            // Format: user@domain.com domain.com/user/
            virtual_mailbox_maps.push(format!("{} {}", addr, subpath));
            // Dovecot passwdfile format: user@domain.com:{CRYPT-HASH}::5000:5000::/var/mail/vhosts/domain.com/user::userdb_quota_rule=*:bytes=...
            dovecot_passwd_entries.push(format!(
                "{}:[REDACTED_SECRET]::5000:5000::/var/mail/vhosts/{}::userdb_quota_rule=*:bytes={}",
                addr, subpath, mb.quota_bytes
            ));
        }
        virtual_mailbox_maps.sort();
        dovecot_passwd_entries.sort();

        let mut virtual_alias_maps = Vec::new();
        for alias in aliases {
            if !alias.is_active {
                continue;
            }
            let src = alias.source.full_address();
            let dsts = alias
                .destinations
                .iter()
                .map(|d: &MailAddress| d.full_address())
                .collect::<Vec<_>>()
                .join(",");
            if src.contains('\n')
                || src.contains('\r')
                || dsts.contains('\n')
                || dsts.contains('\r')
            {
                return Err(MailError::InvalidInput(
                    "Invalid alias entry in config".to_string(),
                ));
            }
            virtual_alias_maps.push(format!("{} {}", src, dsts));
        }
        virtual_alias_maps.sort();

        let relay_host_directive = match relay {
            Some(r) => {
                if r.relay_host.contains('\n') || r.relay_host.contains('\r') {
                    return Err(MailError::InvalidInput(
                        "Invalid relay host directive".to_string(),
                    ));
                }
                Some(format!("[{}]:{}", r.relay_host, r.relay_port))
            }
            None => None,
        };

        Ok(Self {
            plan_id: Uuid::new_v4(),
            virtual_mailbox_domains,
            virtual_mailbox_maps,
            virtual_alias_maps,
            relay_host_directive,
            dovecot_passwd_entries,
        })
    }
}

/// The result of an applied configuration change.
/// Distinguished from a Plan: an operation must not claim success when only a plan was generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedResult {
    pub operation_id: Uuid,
    pub plan_id: Uuid,
    pub status: AppliedStatus,
    pub committed_resources: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppliedStatus {
    Applied,
    PartiallyApplied,
    Failed,
    RolledBack,
}

/// The observed state of mail resources from the system/filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedState {
    pub tenant_id: Uuid,
    pub domains_count: usize,
    pub mailboxes_count: usize,
    pub aliases_count: usize,
    pub relay_configured: bool,
}

#![forbid(unsafe_code)]

use fusionpanel_agent_protocol::{
    MailAliasSpec, MailDkimApplyPayload, MailDkimResult, MailDomainApplyPayload,
    MailDomainDeletePayload, MailMailboxSpec, MailOperationResult,
};
use mail::config::postfix::{render_virtual_mailbox_domains, render_virtual_mailbox_maps};
use mail::config::validator::{is_valid_domain, is_valid_username};
use mail::dkim::{format_dns_txt_record, generate_dkim_keypair};
use mail::dkim_prober::{DkimSelector, OpenDkimConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

const DEFAULT_MAIL_ROOT: &str = "/var/lib/fusionpanel/mail";

static MAIL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn mail_lock() -> &'static Mutex<()> {
    MAIL_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone)]
pub struct MailDriverConfig {
    pub root: PathBuf,
    pub postmap_cmd: Option<String>,
    pub postconf_cmd: Option<String>,
    pub doveconf_cmd: Option<String>,
    pub validate_cmd: Option<String>,
    pub reload_cmd: Option<String>,
}

impl MailDriverConfig {
    pub fn from_env() -> Self {
        Self {
            root: env_non_empty("FUSIONPANEL_MAIL_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_MAIL_ROOT)),
            postmap_cmd: env_non_empty("FUSIONPANEL_MAIL_POSTMAP_CMD"),
            postconf_cmd: env_non_empty("FUSIONPANEL_MAIL_POSTCONF_CMD"),
            doveconf_cmd: env_non_empty("FUSIONPANEL_MAIL_DOVECONF_CMD"),
            validate_cmd: env_non_empty("FUSIONPANEL_MAIL_VALIDATE_CMD"),
            reload_cmd: env_non_empty("FUSIONPANEL_MAIL_RELOAD_CMD"),
        }
    }
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[derive(Debug, Clone)]
pub struct MailExecutionOutcome {
    pub result: MailOperationResult,
    pub deduplicated: bool,
    pub secrets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ManagedDkimState {
    selector: String,
    enabled: bool,
    txt_record: Option<String>,
    public_key_pem: Option<String>,
    private_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ManagedDomainState {
    domain_id: Uuid,
    tenant_id: Uuid,
    domain: String,
    revision: u64,
    active: bool,
    deleted: bool,
    mailboxes: Vec<MailMailboxSpec>,
    aliases: Vec<MailAliasSpec>,
    dkim: Option<ManagedDkimState>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ManagedMailState {
    domains: BTreeMap<Uuid, ManagedDomainState>,
}

pub async fn execute_mail_domain_apply(
    payload: &MailDomainApplyPayload,
) -> Result<MailExecutionOutcome, String> {
    let config = MailDriverConfig::from_env();
    execute_mail_domain_apply_with_config(&config, payload).await
}

pub async fn execute_mail_domain_delete(
    payload: &MailDomainDeletePayload,
) -> Result<MailExecutionOutcome, String> {
    let config = MailDriverConfig::from_env();
    execute_mail_domain_delete_with_config(&config, payload).await
}

pub async fn execute_mail_dkim_apply(
    payload: &MailDkimApplyPayload,
) -> Result<MailExecutionOutcome, String> {
    let config = MailDriverConfig::from_env();
    execute_mail_dkim_apply_with_config(&config, payload).await
}

pub async fn execute_mail_domain_apply_with_config(
    config: &MailDriverConfig,
    payload: &MailDomainApplyPayload,
) -> Result<MailExecutionOutcome, String> {
    let _guard = mail_lock().lock().await;
    validate_domain_segment(&payload.domain)?;

    let mut state = load_state(&config.root)?;
    let existing = state.domains.get(&payload.domain_id).cloned();

    if let Some(prev) = existing.as_ref() {
        if payload.revision < prev.revision {
            return Err(format!(
                "stale mail revision {} for {} (current {})",
                payload.revision, payload.domain, prev.revision
            ));
        }
        if payload.revision == prev.revision && !prev.deleted {
            let dkim = prev.dkim.as_ref().and_then(|d| {
                if d.enabled {
                    d.txt_record.as_ref().map(|txt| MailDkimResult {
                        selector: d.selector.clone(),
                        txt_record: txt.clone(),
                        public_key_pem: d.public_key_pem.clone(),
                    })
                } else {
                    None
                }
            });
            return Ok(MailExecutionOutcome {
                result: MailOperationResult {
                    domain_id: payload.domain_id,
                    revision: payload.revision,
                    dkim,
                },
                deduplicated: true,
                secrets: collect_mailbox_secrets(&payload.mailboxes),
            });
        }
    }

    let mut secrets = collect_mailbox_secrets(&payload.mailboxes);
    let mut mailboxes = payload.mailboxes.clone();
    // Preserve existing password hashes if an update omits password_hash for an existing mailbox.
    if let Some(prev) = existing.as_ref() {
        for mb in &mut mailboxes {
            if mb.password_hash.is_none() {
                if let Some(old_mb) = prev
                    .mailboxes
                    .iter()
                    .find(|m| m.mailbox_id == mb.mailbox_id || m.address == mb.address)
                {
                    mb.password_hash = old_mb.password_hash.clone();
                }
            }
        }
    }

    let (dkim_state, dkim_result) = if let Some(spec) = &payload.dkim {
        if spec.enabled {
            let (dkim_state, dkim_res, mut key_secrets) =
                ensure_dkim_key(&config.root, &payload.domain, &spec.selector, false)?;
            secrets.append(&mut key_secrets);
            (Some(dkim_state), Some(dkim_res))
        } else {
            (
                Some(ManagedDkimState {
                    selector: spec.selector.clone(),
                    enabled: false,
                    txt_record: None,
                    public_key_pem: None,
                    private_key_path: None,
                }),
                None,
            )
        }
    } else if let Some(prev_dkim) = existing.as_ref().and_then(|d| d.dkim.clone()) {
        let res = if prev_dkim.enabled {
            prev_dkim.txt_record.as_ref().map(|txt| MailDkimResult {
                selector: prev_dkim.selector.clone(),
                txt_record: txt.clone(),
                public_key_pem: prev_dkim.public_key_pem.clone(),
            })
        } else {
            None
        };
        (Some(prev_dkim), res)
    } else {
        (None, None)
    };

    let new_domain_state = ManagedDomainState {
        domain_id: payload.domain_id,
        tenant_id: payload.tenant_id,
        domain: payload.domain.clone(),
        revision: payload.revision,
        active: payload.active,
        deleted: false,
        mailboxes,
        aliases: payload.aliases.clone(),
        dkim: dkim_state,
    };

    state.domains.insert(payload.domain_id, new_domain_state);
    stage_validate_switch_reload(config, &state).await?;

    Ok(MailExecutionOutcome {
        result: MailOperationResult {
            domain_id: payload.domain_id,
            revision: payload.revision,
            dkim: dkim_result,
        },
        deduplicated: false,
        secrets,
    })
}

pub async fn execute_mail_domain_delete_with_config(
    config: &MailDriverConfig,
    payload: &MailDomainDeletePayload,
) -> Result<MailExecutionOutcome, String> {
    let _guard = mail_lock().lock().await;
    validate_domain_segment(&payload.domain)?;

    let mut state = load_state(&config.root)?;
    if let Some(prev) = state.domains.get(&payload.domain_id) {
        if payload.revision < prev.revision {
            return Err(format!(
                "stale mail revision {} for {} (current {})",
                payload.revision, payload.domain, prev.revision
            ));
        }
        if payload.revision == prev.revision && prev.deleted {
            return Ok(MailExecutionOutcome {
                result: MailOperationResult {
                    domain_id: payload.domain_id,
                    revision: payload.revision,
                    dkim: None,
                },
                deduplicated: true,
                secrets: Vec::new(),
            });
        }
    }

    state.domains.insert(
        payload.domain_id,
        ManagedDomainState {
            domain_id: payload.domain_id,
            tenant_id: payload.tenant_id,
            domain: payload.domain.clone(),
            revision: payload.revision,
            active: false,
            deleted: true,
            mailboxes: Vec::new(),
            aliases: Vec::new(),
            dkim: None,
        },
    );

    stage_validate_switch_reload(config, &state).await?;

    Ok(MailExecutionOutcome {
        result: MailOperationResult {
            domain_id: payload.domain_id,
            revision: payload.revision,
            dkim: None,
        },
        deduplicated: false,
        secrets: Vec::new(),
    })
}

pub async fn execute_mail_dkim_apply_with_config(
    config: &MailDriverConfig,
    payload: &MailDkimApplyPayload,
) -> Result<MailExecutionOutcome, String> {
    let _guard = mail_lock().lock().await;
    validate_domain_segment(&payload.domain)?;
    validate_selector_segment(&payload.selector)?;

    let mut state = load_state(&config.root)?;
    let prev = state
        .domains
        .get(&payload.domain_id)
        .cloned()
        .ok_or_else(|| format!("mail domain {} is not provisioned on node", payload.domain))?;

    if prev.deleted {
        return Err(format!("mail domain {} has been deleted", payload.domain));
    }
    if payload.revision < prev.revision {
        return Err(format!(
            "stale mail revision {} for {} (current {})",
            payload.revision, payload.domain, prev.revision
        ));
    }
    if payload.revision == prev.revision {
        let dkim = prev.dkim.as_ref().and_then(|d| {
            if d.enabled {
                d.txt_record.as_ref().map(|txt| MailDkimResult {
                    selector: d.selector.clone(),
                    txt_record: txt.clone(),
                    public_key_pem: d.public_key_pem.clone(),
                })
            } else {
                None
            }
        });
        return Ok(MailExecutionOutcome {
            result: MailOperationResult {
                domain_id: payload.domain_id,
                revision: payload.revision,
                dkim,
            },
            deduplicated: true,
            secrets: Vec::new(),
        });
    }

    let mut secrets = Vec::new();
    let (dkim_state, dkim_result) = if payload.enabled {
        let (dkim_state, dkim_res, mut key_secrets) =
            ensure_dkim_key(&config.root, &payload.domain, &payload.selector, payload.rotate)?;
        secrets.append(&mut key_secrets);
        (Some(dkim_state), Some(dkim_res))
    } else {
        (
            Some(ManagedDkimState {
                selector: payload.selector.clone(),
                enabled: false,
                txt_record: None,
                public_key_pem: None,
                private_key_path: None,
            }),
            None,
        )
    };

    let mut updated = prev;
    updated.revision = payload.revision;
    updated.dkim = dkim_state;
    state.domains.insert(payload.domain_id, updated);

    stage_validate_switch_reload(config, &state).await?;

    Ok(MailExecutionOutcome {
        result: MailOperationResult {
            domain_id: payload.domain_id,
            revision: payload.revision,
            dkim: dkim_result,
        },
        deduplicated: false,
        secrets,
    })
}

fn collect_mailbox_secrets(mailboxes: &[MailMailboxSpec]) -> Vec<String> {
    mailboxes
        .iter()
        .filter_map(|m| m.password_hash.clone())
        .filter(|s| !s.is_empty())
        .collect()
}

fn ensure_dkim_key(
    root: &Path,
    domain: &str,
    selector: &str,
    rotate: bool,
) -> Result<(ManagedDkimState, MailDkimResult, Vec<String>), String> {
    validate_domain_segment(domain)?;
    validate_selector_segment(selector)?;

    let key_dir = confined_join(root, &format!("dkim/keys/{domain}"))?;
    fs::create_dir_all(&key_dir)
        .map_err(|e| format!("failed to create DKIM key directory: {e}"))?;

    let private_path = confined_join(&key_dir, &format!("{selector}.private"))?;
    let pub_path = confined_join(&key_dir, &format!("{selector}.pub.pem"))?;
    let txt_path = confined_join(&key_dir, &format!("{selector}.txt"))?;

    let mut secrets = Vec::new();
    let (public_key_pem, txt_record) =
        if rotate || !private_path.exists() || !pub_path.exists() || !txt_path.exists() {
            let pair = generate_dkim_keypair(&private_path)
                .map_err(|e| format!("failed to generate DKIM keypair: {e}"))?;
            fs::set_permissions(&private_path, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("failed to set 0600 permissions on DKIM key: {e}"))?;
            write_atomic_file(&pub_path, pair.public_key.as_bytes(), 0o644)?;
            write_atomic_file(&txt_path, pair.txt_record.as_bytes(), 0o644)?;
            secrets.push(pair.private_key);
            (pair.public_key, pair.txt_record)
        } else {
            fs::set_permissions(&private_path, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("failed to enforce 0600 permissions on DKIM key: {e}"))?;
            let pub_pem = fs::read_to_string(&pub_path)
                .map_err(|e| format!("failed to read DKIM public key: {e}"))?;
            let txt = format_dns_txt_record(&pub_pem)
                .map_err(|e| format!("failed to format DKIM TXT record: {e}"))?;
            (pub_pem, txt)
        };

    verify_dkim_key_file(&private_path)?;

    let state = ManagedDkimState {
        selector: selector.to_string(),
        enabled: true,
        txt_record: Some(txt_record.clone()),
        public_key_pem: Some(public_key_pem.clone()),
        private_key_path: Some(private_path.to_string_lossy().to_string()),
    };
    let result = MailDkimResult {
        selector: selector.to_string(),
        txt_record,
        public_key_pem: Some(public_key_pem),
    };
    Ok((state, result, secrets))
}

fn verify_dkim_key_file(private_path: &Path) -> Result<(), String> {
    let meta = fs::metadata(private_path)
        .map_err(|e| format!("DKIM private key missing at {}: {e}", private_path.display()))?;
    if meta.len() == 0 {
        return Err(format!(
            "DKIM private key is empty at {}",
            private_path.display()
        ));
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(format!(
            "DKIM private key permissions must be 0600, found {:04o}",
            mode
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct RenderedMailConfig {
    virtual_mailbox_domains: String,
    virtual_mailbox_maps: String,
    virtual_alias_maps: String,
    dovecot_users: String,
    dkim_key_table: String,
    dkim_signing_table: String,
    maildirs: Vec<PathBuf>,
    state_json: String,
}

fn render_mail_config(root: &Path, state: &ManagedMailState) -> Result<RenderedMailConfig, String> {
    let mut active_domains: Vec<&ManagedDomainState> = state
        .domains
        .values()
        .filter(|d| d.active && !d.deleted)
        .collect();
    active_domains.sort_by(|a, b| {
        a.domain
            .cmp(&b.domain)
            .then_with(|| a.tenant_id.cmp(&b.tenant_id))
    });

    let domain_names: Vec<&str> = active_domains.iter().map(|d| d.domain.as_str()).collect();
    let virtual_mailbox_domains = render_virtual_mailbox_domains(&domain_names)
        .map_err(|e| format!("failed to render virtual_mailbox_domains: {e:?}"))?;

    let mut mailbox_pairs: Vec<(String, String)> = Vec::new();
    let mut dovecot_lines: Vec<String> = Vec::new();
    let mut maildirs: Vec<PathBuf> = Vec::new();
    let mut alias_entries: Vec<(String, Vec<String>)> = Vec::new();
    let mut dkim_selectors: Vec<DkimSelector> = Vec::new();

    for dom in &active_domains {
        let mut mailboxes: Vec<&MailMailboxSpec> =
            dom.mailboxes.iter().filter(|m| m.active).collect();
        mailboxes.sort_by(|a, b| a.address.cmp(&b.address));

        for mb in mailboxes {
            if !is_valid_username(&mb.local_part) || !is_valid_domain(&dom.domain) {
                return Err(format!("invalid mailbox address {}", mb.address));
            }
            mailbox_pairs.push((mb.local_part.clone(), dom.domain.clone()));
            let maildir = confined_join(root, &format!("vmail/{}/{}", dom.domain, mb.local_part))?;
            maildirs.push(maildir.clone());

            if let Some(hash) = mb.password_hash.as_deref() {
                dovecot_lines.push(format!(
                    "{}:{hash}:5000:5000::{}:userdb_quota_rule=*:bytes={}",
                    mb.address,
                    maildir.display(),
                    mb.quota_bytes
                ));
            }
        }

        let mut aliases: Vec<&MailAliasSpec> = dom.aliases.iter().filter(|a| a.active).collect();
        aliases.sort_by(|a, b| a.source.cmp(&b.source));
        for alias in aliases {
            let mut dests = alias.destinations.clone();
            dests.sort();
            dests.dedup();
            alias_entries.push((alias.source.clone(), dests));
        }

        if let Some(dkim) = &dom.dkim {
            if dkim.enabled {
                let key_path = dkim.private_key_path.as_deref().ok_or_else(|| {
                    format!("DKIM enabled for {} without private key path", dom.domain)
                })?;
                verify_dkim_key_file(Path::new(key_path))?;
                dkim_selectors.push(DkimSelector::new(&dom.domain, &dkim.selector, key_path));
            }
        }
    }

    mailbox_pairs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mailbox_refs: Vec<(&str, &str)> = mailbox_pairs
        .iter()
        .map(|(u, d)| (u.as_str(), d.as_str()))
        .collect();
    let virtual_mailbox_maps = render_virtual_mailbox_maps(&mailbox_refs)
        .map_err(|e| format!("failed to render virtual_mailbox_maps: {e:?}"))?;

    alias_entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut virtual_alias_maps = String::new();
    for (src, dests) in alias_entries {
        virtual_alias_maps.push_str(&format!("{src} {}\n", dests.join(",")));
    }

    dovecot_lines.sort();
    let mut dovecot_users = dovecot_lines.join("\n");
    if !dovecot_users.is_empty() {
        dovecot_users.push('\n');
    }

    dkim_selectors.sort_by(|a, b| a.domain.cmp(&b.domain));
    let opendkim = OpenDkimConfig::new(dkim_selectors);
    let mut dkim_key_table = opendkim.generate_key_table();
    if !dkim_key_table.is_empty() {
        dkim_key_table.push('\n');
    }
    let mut dkim_signing_table = opendkim.generate_signing_table();
    if !dkim_signing_table.is_empty() {
        dkim_signing_table.push('\n');
    }

    let state_json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("failed to serialize mail state: {e}"))?;

    Ok(RenderedMailConfig {
        virtual_mailbox_domains,
        virtual_mailbox_maps,
        virtual_alias_maps,
        dovecot_users,
        dkim_key_table,
        dkim_signing_table,
        maildirs,
        state_json,
    })
}

async fn stage_validate_switch_reload(
    config: &MailDriverConfig,
    new_state: &ManagedMailState,
) -> Result<(), String> {
    let root = &config.root;
    fs::create_dir_all(root).map_err(|e| format!("failed to create mail root: {e}"))?;

    let rendered = render_mail_config(root, new_state)?;

    for maildir in &rendered.maildirs {
        for sub in ["cur", "new", "tmp"] {
            fs::create_dir_all(maildir.join(sub))
                .map_err(|e| format!("failed to create Maildir {}: {e}", maildir.display()))?;
        }
    }

    let tx_id = Uuid::new_v4();
    let staging_dir = confined_join(root, &format!("staging/{tx_id}"))?;
    let backup_dir = confined_join(root, &format!("backup/{tx_id}"))?;
    fs::create_dir_all(staging_dir.join("postfix"))
        .map_err(|e| format!("failed to create postfix staging dir: {e}"))?;
    fs::create_dir_all(staging_dir.join("dovecot"))
        .map_err(|e| format!("failed to create dovecot staging dir: {e}"))?;
    fs::create_dir_all(staging_dir.join("dkim"))
        .map_err(|e| format!("failed to create dkim staging dir: {e}"))?;
    fs::create_dir_all(staging_dir.join("state"))
        .map_err(|e| format!("failed to create state staging dir: {e}"))?;

    let stg_domains = staging_dir.join("postfix/virtual_mailbox_domains");
    let stg_mailboxes = staging_dir.join("postfix/virtual_mailbox_maps");
    let stg_aliases = staging_dir.join("postfix/virtual_alias_maps");
    let stg_dovecot = staging_dir.join("dovecot/users");
    let stg_key_table = staging_dir.join("dkim/KeyTable");
    let stg_signing_table = staging_dir.join("dkim/SigningTable");
    let stg_state = staging_dir.join("state/domains.json");

    write_file_with_mode(&stg_domains, rendered.virtual_mailbox_domains.as_bytes(), 0o644)?;
    write_file_with_mode(&stg_mailboxes, rendered.virtual_mailbox_maps.as_bytes(), 0o644)?;
    write_file_with_mode(&stg_aliases, rendered.virtual_alias_maps.as_bytes(), 0o644)?;
    write_file_with_mode(&stg_dovecot, rendered.dovecot_users.as_bytes(), 0o600)?;
    write_file_with_mode(&stg_key_table, rendered.dkim_key_table.as_bytes(), 0o644)?;
    write_file_with_mode(&stg_signing_table, rendered.dkim_signing_table.as_bytes(), 0o644)?;
    write_file_with_mode(&stg_state, rendered.state_json.as_bytes(), 0o600)?;

    if let Err(err) = validate_staged(
        config,
        &staging_dir,
        &[&stg_domains, &stg_mailboxes, &stg_aliases],
    )
    .await
    {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let mut pairs: Vec<(PathBuf, PathBuf)> = vec![
        (stg_domains.clone(), root.join("postfix/virtual_mailbox_domains")),
        (stg_mailboxes.clone(), root.join("postfix/virtual_mailbox_maps")),
        (stg_aliases.clone(), root.join("postfix/virtual_alias_maps")),
        (stg_dovecot, root.join("dovecot/users")),
        (stg_key_table, root.join("dkim/KeyTable")),
        (stg_signing_table, root.join("dkim/SigningTable")),
        (stg_state, root.join("state/domains.json")),
    ];

    for map_path in [&stg_domains, &stg_mailboxes, &stg_aliases] {
        let db_src = PathBuf::from(format!("{}.db", map_path.display()));
        if db_src.exists() {
            let file_name = db_src
                .file_name()
                .ok_or_else(|| "invalid map db filename".to_string())?;
            let db_dst = root.join("postfix").join(file_name);
            pairs.push((db_src, db_dst));
        }
    }

    let switch_res = switch_and_reload(config, &backup_dir, &pairs).await;
    let _ = fs::remove_dir_all(&staging_dir);
    let _ = fs::remove_dir_all(&backup_dir);
    switch_res
}

async fn validate_staged(
    config: &MailDriverConfig,
    staging_dir: &Path,
    postfix_maps: &[&Path],
) -> Result<(), String> {
    if let Some(cmd) = &config.postmap_cmd {
        for map in postfix_maps {
            run_service_cmd(cmd, &[map.to_string_lossy().as_ref()], &config.root, staging_dir)
                .await
                .map_err(|e| format!("postmap validation failed: {e}"))?;
        }
    }
    if let Some(cmd) = &config.postconf_cmd {
        run_service_cmd(cmd, &[], &config.root, staging_dir)
            .await
            .map_err(|e| format!("postconf validation failed: {e}"))?;
    }
    if let Some(cmd) = &config.doveconf_cmd {
        run_service_cmd(cmd, &[], &config.root, staging_dir)
            .await
            .map_err(|e| format!("doveconf validation failed: {e}"))?;
    }
    if let Some(cmd) = &config.validate_cmd {
        run_service_cmd(
            cmd,
            &[staging_dir.to_string_lossy().as_ref()],
            &config.root,
            staging_dir,
        )
        .await
        .map_err(|e| format!("mail config validation failed: {e}"))?;
    }
    Ok(())
}

async fn switch_and_reload(
    config: &MailDriverConfig,
    backup_dir: &Path,
    pairs: &[(PathBuf, PathBuf)],
) -> Result<(), String> {
    fs::create_dir_all(backup_dir)
        .map_err(|e| format!("failed to create backup dir: {e}"))?;

    let mut backups: Vec<(PathBuf, Option<PathBuf>)> = Vec::with_capacity(pairs.len());

    for (idx, (_, dst)) in pairs.iter().enumerate() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create target dir {}: {e}", parent.display()))?;
        }
        if dst.exists() {
            let bak = backup_dir.join(format!("bak_{idx}"));
            fs::copy(dst, &bak)
                .map_err(|e| format!("failed to snapshot {}: {e}", dst.display()))?;
            if let Ok(meta) = fs::metadata(dst) {
                let _ = fs::set_permissions(&bak, meta.permissions());
            }
            backups.push((dst.clone(), Some(bak)));
        } else {
            backups.push((dst.clone(), None));
        }
    }

    for (src, dst) in pairs {
        if let Err(err) = fs::rename(src, dst) {
            restore_backups(&backups);
            return Err(format!(
                "failed to switch mail config {}: {err}",
                dst.display()
            ));
        }
    }

    if let Some(reload_cmd) = &config.reload_cmd {
        if let Err(err) = run_service_cmd(reload_cmd, &[], &config.root, &config.root).await {
            restore_backups(&backups);
            return Err(format!("mail service reload failed: {err}"));
        }
    }

    Ok(())
}

fn restore_backups(backups: &[(PathBuf, Option<PathBuf>)]) {
    for (dst, bak_opt) in backups {
        match bak_opt {
            Some(bak) => {
                let _ = fs::rename(bak, dst);
            }
            None => {
                let _ = fs::remove_file(dst);
            }
        }
    }
}

async fn run_service_cmd(
    cmd_str: &str,
    extra_args: &[&str],
    mail_root: &Path,
    staging_dir: &Path,
) -> Result<(), String> {
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }
    let output = Command::new(parts[0])
        .args(&parts[1..])
        .args(extra_args)
        .env("FUSIONPANEL_MAIL_ROOT", mail_root)
        .env("FUSIONPANEL_MAIL_STAGING_DIR", staging_dir)
        .output()
        .await
        .map_err(|e| format!("failed to execute {}: {e}", parts[0]))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(format!(
            "{} exited with {}: {detail}",
            parts[0],
            output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string())
        ));
    }
    Ok(())
}

fn load_state(root: &Path) -> Result<ManagedMailState, String> {
    let state_path = confined_join(root, "state/domains.json")?;
    if !state_path.exists() {
        return Ok(ManagedMailState::default());
    }
    let raw = fs::read_to_string(&state_path)
        .map_err(|e| format!("failed to read mail state {}: {e}", state_path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("failed to parse mail state: {e}"))
}

fn write_atomic_file(path: &Path, content: &[u8], mode: u32) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    write_file_with_mode(&tmp, content, mode)?;
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("failed to commit {}: {err}", path.display()));
    }
    Ok(())
}

fn write_file_with_mode(path: &Path, content: &[u8], mode: u32) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    file.write_all(content)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("failed to set mode {:04o} on {}: {e}", mode, path.display()))?;
    Ok(())
}

fn validate_domain_segment(domain: &str) -> Result<(), String> {
    if !is_valid_domain(domain) || domain.contains('/') || domain.contains('\\') {
        return Err(format!("invalid mail domain: {domain}"));
    }
    Ok(())
}

fn validate_selector_segment(selector: &str) -> Result<(), String> {
    if selector.is_empty()
        || selector.len() > 63
        || !selector
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(format!("invalid DKIM selector: {selector}"));
    }
    Ok(())
}

fn confined_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir))
    {
        return Err(format!("path escapes managed mail root: {relative}"));
    }
    Ok(root.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusionpanel_agent_protocol::MailDkimSpec;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_mail_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fusionpanel-mail-test-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn base_config(root: PathBuf) -> MailDriverConfig {
        MailDriverConfig {
            root,
            postmap_cmd: None,
            postconf_cmd: None,
            doveconf_cmd: None,
            validate_cmd: None,
            reload_cmd: None,
        }
    }

    fn mailbox(local: &str, domain: &str, hash: &str) -> MailMailboxSpec {
        MailMailboxSpec {
            mailbox_id: Uuid::new_v4(),
            address: format!("{local}@{domain}"),
            local_part: local.to_string(),
            quota_bytes: 1024 * 1024,
            active: true,
            password_hash: Some(hash.to_string()),
        }
    }

    #[tokio::test]
    async fn mail_render_order_is_deterministic_across_domains_mailboxes_aliases_and_dkim() {
        let root = unique_mail_root("render-order");
        let cfg = base_config(root.clone());
        let tenant = Uuid::new_v4();

        let z_payload = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id: tenant,
            node_id: Uuid::new_v4(),
            domain: "zulu.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![
                mailbox("bob", "zulu.example.test", "{SHA256}secret-bob"),
                mailbox("alice", "zulu.example.test", "{SHA256}secret-alice"),
            ],
            aliases: vec![MailAliasSpec {
                alias_id: Uuid::new_v4(),
                source: "info@zulu.example.test".to_string(),
                destinations: vec![
                    "bob@zulu.example.test".to_string(),
                    "alice@zulu.example.test".to_string(),
                ],
                active: true,
            }],
            dkim: None,
        };

        let a_payload = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id: tenant,
            node_id: Uuid::new_v4(),
            domain: "alpha.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("carol", "alpha.example.test", "{SHA256}secret-carol")],
            aliases: vec![MailAliasSpec {
                alias_id: Uuid::new_v4(),
                source: "abuse@alpha.example.test".to_string(),
                destinations: vec!["carol@alpha.example.test".to_string()],
                active: true,
            }],
            dkim: None,
        };

        execute_mail_domain_apply_with_config(&cfg, &z_payload)
            .await
            .expect("apply zulu");
        execute_mail_domain_apply_with_config(&cfg, &a_payload)
            .await
            .expect("apply alpha");

        let domains_map =
            fs::read_to_string(root.join("postfix/virtual_mailbox_domains")).expect("domains");
        assert_eq!(domains_map, "alpha.example.test OK\nzulu.example.test OK\n");

        let mailboxes_map =
            fs::read_to_string(root.join("postfix/virtual_mailbox_maps")).expect("mailboxes");
        assert_eq!(
            mailboxes_map,
            "carol@alpha.example.test alpha.example.test/carol/\n\
             alice@zulu.example.test zulu.example.test/alice/\n\
             bob@zulu.example.test zulu.example.test/bob/\n"
        );

        let aliases_map =
            fs::read_to_string(root.join("postfix/virtual_alias_maps")).expect("aliases");
        assert_eq!(
            aliases_map,
            "abuse@alpha.example.test carol@alpha.example.test\n\
             info@zulu.example.test alice@zulu.example.test,bob@zulu.example.test\n"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_dkim_private_key_and_dovecot_passwd_have_0600_permissions() {
        let root = unique_mail_root("perms");
        let cfg = base_config(root.clone());

        let payload = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            domain: "perms.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("user1", "perms.example.test", "{ARGON2ID}hash1")],
            aliases: Vec::new(),
            dkim: Some(MailDkimSpec {
                selector: "s2026".to_string(),
                enabled: true,
                dns_zone_id: None,
                txt_record: None,
            }),
        };

        let outcome = execute_mail_domain_apply_with_config(&cfg, &payload)
            .await
            .expect("apply with dkim");
        let dkim = outcome.result.dkim.expect("dkim result");
        assert_eq!(dkim.selector, "s2026");
        assert!(dkim.txt_record.contains("v=DKIM1"));
        assert!(dkim.public_key_pem.is_some());
        assert!(outcome.secrets.iter().any(|s| s.contains("PRIVATE KEY")));

        let key_mode = fs::metadata(root.join("dkim/keys/perms.example.test/s2026.private"))
            .expect("key meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(key_mode, 0o600);

        let dovecot_mode = fs::metadata(root.join("dovecot/users"))
            .expect("dovecot meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dovecot_mode, 0o600);

        let key_table = fs::read_to_string(root.join("dkim/KeyTable")).expect("KeyTable");
        assert!(key_table.contains("s2026._domainkey.perms.example.test"));
        let signing_table =
            fs::read_to_string(root.join("dkim/SigningTable")).expect("SigningTable");
        assert!(signing_table.contains("*@perms.example.test s2026._domainkey.perms.example.test"));

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_deduplicates_replay_and_rejects_stale_revisions() {
        let root = unique_mail_root("revisions");
        let cfg = base_config(root.clone());
        let domain_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let rev2 = MailDomainApplyPayload {
            domain_id,
            tenant_id,
            node_id,
            domain: "rev.example.test".to_string(),
            revision: 2,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("first", "rev.example.test", "{SHA256}h2")],
            aliases: Vec::new(),
            dkim: None,
        };
        let first = execute_mail_domain_apply_with_config(&cfg, &rev2)
            .await
            .expect("rev 2");
        assert!(!first.deduplicated);

        let replay = execute_mail_domain_apply_with_config(&cfg, &rev2)
            .await
            .expect("replay rev 2");
        assert!(replay.deduplicated);

        let stale = MailDomainApplyPayload {
            revision: 1,
            mailboxes: vec![mailbox("stale", "rev.example.test", "{SHA256}h1")],
            ..rev2.clone()
        };
        let err = execute_mail_domain_apply_with_config(&cfg, &stale)
            .await
            .expect_err("stale rev 1 must fail");
        assert!(err.contains("stale mail revision"));

        let maps =
            fs::read_to_string(root.join("postfix/virtual_mailbox_maps")).expect("maps");
        assert!(maps.contains("first@rev.example.test"));
        assert!(!maps.contains("stale@rev.example.test"));

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_validation_failure_keeps_previous_config_and_unaffected_domains() {
        let root = unique_mail_root("validate-fail");
        let mut cfg = base_config(root.clone());

        let good_payload = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            domain: "keep.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("admin", "keep.example.test", "{SHA256}good")],
            aliases: Vec::new(),
            dkim: None,
        };
        execute_mail_domain_apply_with_config(&cfg, &good_payload)
            .await
            .expect("initial apply");

        cfg.validate_cmd = Some("sh -c 'exit 1'".to_string());
        let bad_payload = MailDomainApplyPayload {
            revision: 2,
            mailboxes: vec![mailbox("broken", "keep.example.test", "{SHA256}bad")],
            ..good_payload.clone()
        };
        let err = execute_mail_domain_apply_with_config(&cfg, &bad_payload)
            .await
            .expect_err("validation failure");
        assert!(err.contains("validation failed"));

        let maps =
            fs::read_to_string(root.join("postfix/virtual_mailbox_maps")).expect("maps");
        assert!(maps.contains("admin@keep.example.test"));
        assert!(!maps.contains("broken@keep.example.test"));

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_reload_failure_restores_previous_config_and_preserves_maildir_data() {
        let root = unique_mail_root("reload-fail");
        let mut cfg = base_config(root.clone());

        let initial = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            domain: "reload.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("inbox", "reload.example.test", "{SHA256}v1")],
            aliases: Vec::new(),
            dkim: None,
        };
        execute_mail_domain_apply_with_config(&cfg, &initial)
            .await
            .expect("initial apply");

        // Write a sentinel message file in Maildir to verify message data is preserved.
        let msg_path = root.join("vmail/reload.example.test/inbox/cur/msg-1.eml");
        fs::write(&msg_path, b"Subject: hello\n\nbody").expect("write message");

        cfg.reload_cmd = Some("sh -c 'exit 2'".to_string());
        let updated = MailDomainApplyPayload {
            revision: 2,
            mailboxes: vec![mailbox("inbox2", "reload.example.test", "{SHA256}v2")],
            ..initial.clone()
        };
        let err = execute_mail_domain_apply_with_config(&cfg, &updated)
            .await
            .expect_err("reload failure");
        assert!(err.contains("reload failed"));

        let maps =
            fs::read_to_string(root.join("postfix/virtual_mailbox_maps")).expect("maps");
        assert!(maps.contains("inbox@reload.example.test"));
        assert!(!maps.contains("inbox2@reload.example.test"));
        assert!(msg_path.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_delete_removes_active_config_but_preserves_maildir_and_other_domains() {
        let root = unique_mail_root("delete");
        let cfg = base_config(root.clone());
        let tenant_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let dom1 = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id,
            node_id,
            domain: "one.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("u1", "one.example.test", "{SHA256}1")],
            aliases: Vec::new(),
            dkim: None,
        };
        let dom2 = MailDomainApplyPayload {
            domain_id: Uuid::new_v4(),
            tenant_id,
            node_id,
            domain: "two.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("u2", "two.example.test", "{SHA256}2")],
            aliases: Vec::new(),
            dkim: None,
        };

        execute_mail_domain_apply_with_config(&cfg, &dom1)
            .await
            .expect("dom1");
        execute_mail_domain_apply_with_config(&cfg, &dom2)
            .await
            .expect("dom2");

        let msg_path = root.join("vmail/one.example.test/u1/new/keep.eml");
        fs::write(&msg_path, b"keep-me").expect("write mail");

        let del = MailDomainDeletePayload {
            domain_id: dom1.domain_id,
            tenant_id,
            node_id,
            domain: dom1.domain.clone(),
            revision: 2,
        };
        execute_mail_domain_delete_with_config(&cfg, &del)
            .await
            .expect("delete dom1");

        let domains_map =
            fs::read_to_string(root.join("postfix/virtual_mailbox_domains")).expect("domains");
        assert!(!domains_map.contains("one.example.test"));
        assert!(domains_map.contains("two.example.test"));
        assert!(msg_path.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn mail_dkim_apply_generates_rotates_and_returns_sanitized_result() {
        let root = unique_mail_root("dkim-rotate");
        let cfg = base_config(root.clone());
        let domain_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();

        let dom = MailDomainApplyPayload {
            domain_id,
            tenant_id,
            node_id,
            domain: "dkim.example.test".to_string(),
            revision: 1,
            active: true,
            dns_zone_id: None,
            mailboxes: vec![mailbox("mail", "dkim.example.test", "{SHA256}x")],
            aliases: Vec::new(),
            dkim: None,
        };
        execute_mail_domain_apply_with_config(&cfg, &dom)
            .await
            .expect("apply domain");

        let dkim1 = execute_mail_dkim_apply_with_config(
            &cfg,
            &MailDkimApplyPayload {
                domain_id,
                tenant_id,
                node_id,
                domain: "dkim.example.test".to_string(),
                revision: 2,
                selector: "sel1".to_string(),
                enabled: true,
                dns_zone_id: None,
                rotate: false,
            },
        )
        .await
        .expect("dkim1");
        let res1 = dkim1.result.dkim.expect("dkim1 res");
        assert_eq!(res1.selector, "sel1");
        assert!(res1.txt_record.contains("v=DKIM1"));

        let dkim2 = execute_mail_dkim_apply_with_config(
            &cfg,
            &MailDkimApplyPayload {
                domain_id,
                tenant_id,
                node_id,
                domain: "dkim.example.test".to_string(),
                revision: 3,
                selector: "sel2".to_string(),
                enabled: true,
                dns_zone_id: None,
                rotate: true,
            },
        )
        .await
        .expect("dkim2");
        let res2 = dkim2.result.dkim.expect("dkim2 res");
        assert_eq!(res2.selector, "sel2");
        assert_ne!(res1.public_key_pem, res2.public_key_pem);

        let _ = fs::remove_dir_all(&root);
    }
}

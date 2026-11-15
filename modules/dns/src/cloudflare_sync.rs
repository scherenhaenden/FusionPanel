#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalRecord {
    pub name: String,
    pub record_type: RecordType,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RemoteRecord {
    pub id: String,
    pub name: String,
    pub record_type: RecordType,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncAction {
    Create(LocalRecord),
    Delete {
        id: String,
    },
    Update {
        id: String,
        new_record: LocalRecord,
    },
}

pub struct CloudflareSyncPlanner;

impl CloudflareSyncPlanner {
    pub fn plan(local_records: &[LocalRecord], remote_records: &[RemoteRecord]) -> Vec<SyncAction> {
        let mut actions = Vec::new();

        let mut local_by_group: HashMap<(String, RecordType), Vec<LocalRecord>> = HashMap::new();
        for local in local_records {
            local_by_group.entry((local.name.clone(), local.record_type.clone()))
                .or_default()
                .push(local.clone());
        }

        let mut remote_by_group: HashMap<(String, RecordType), Vec<RemoteRecord>> = HashMap::new();
        for remote in remote_records {
            remote_by_group.entry((remote.name.clone(), remote.record_type.clone()))
                .or_default()
                .push(remote.clone());
        }

        for locals in local_by_group.values_mut() {
            locals.sort();
        }
        for remotes in remote_by_group.values_mut() {
            remotes.sort();
        }

        let mut all_groups = HashSet::new();
        for k in local_by_group.keys() {
            all_groups.insert(k.clone());
        }
        for k in remote_by_group.keys() {
            all_groups.insert(k.clone());
        }

        let mut all_groups: Vec<_> = all_groups.into_iter().collect();
        all_groups.sort();

        for group in all_groups {
            let locals = local_by_group.remove(&group).unwrap_or_default();
            let remotes = remote_by_group.remove(&group).unwrap_or_default();

            let mut matched_local_indices = HashSet::new();
            let mut matched_remote_indices = HashSet::new();

            for (l_idx, local) in locals.iter().enumerate() {
                for (r_idx, remote) in remotes.iter().enumerate() {
                    if !matched_remote_indices.contains(&r_idx) && local.content == remote.content {
                        matched_local_indices.insert(l_idx);
                        matched_remote_indices.insert(r_idx);

                        if local.ttl != remote.ttl || local.proxied != remote.proxied || local.priority != remote.priority {
                            actions.push(SyncAction::Update {
                                id: remote.id.clone(),
                                new_record: local.clone(),
                            });
                        }
                        break;
                    }
                }
            }

            let remaining_locals: Vec<_> = locals.iter().enumerate().filter(|(i, _)| !matched_local_indices.contains(i)).map(|(_, l)| l.clone()).collect();
            let remaining_remotes: Vec<_> = remotes.iter().enumerate().filter(|(i, _)| !matched_remote_indices.contains(i)).map(|(_, r)| r.clone()).collect();

            let min_len = std::cmp::min(remaining_locals.len(), remaining_remotes.len());
            for i in 0..min_len {
                actions.push(SyncAction::Update {
                    id: remaining_remotes[i].id.clone(),
                    new_record: remaining_locals[i].clone(),
                });
            }

            for i in min_len..remaining_locals.len() {
                actions.push(SyncAction::Create(remaining_locals[i].clone()));
            }

            for i in min_len..remaining_remotes.len() {
                actions.push(SyncAction::Delete {
                    id: remaining_remotes[i].id.clone(),
                });
            }
        }

        actions.sort();
        actions
    }
}

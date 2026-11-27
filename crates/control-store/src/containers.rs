#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerState {
    Provisioning,
    Starting,
    Running,
    Stopping,
    Stopped,
    Restarting,
    Deleting,
    Failed,
}

impl ContainerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Restarting => "restarting",
            Self::Deleting => "deleting",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "provisioning" => Ok(Self::Provisioning),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "restarting" => Ok(Self::Restarting),
            "deleting" => Ok(Self::Deleting),
            "failed" => Ok(Self::Failed),
            other => Err(StoreError::Parse(format!(
                "unknown container state '{other}'"
            ))),
        }
    }

    pub fn is_in_progress(self) -> bool {
        matches!(
            self,
            Self::Provisioning
                | Self::Starting
                | Self::Stopping
                | Self::Restarting
                | Self::Deleting
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Provisioning, Self::Running)
                | (Self::Provisioning, Self::Failed)
                | (Self::Stopped, Self::Starting)
                | (Self::Failed, Self::Starting)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::Failed)
                | (Self::Running, Self::Stopping)
                | (Self::Stopping, Self::Stopped)
                | (Self::Stopping, Self::Failed)
                | (Self::Running, Self::Restarting)
                | (Self::Stopped, Self::Restarting)
                | (Self::Failed, Self::Restarting)
                | (Self::Restarting, Self::Running)
                | (Self::Restarting, Self::Failed)
                | (Self::Running, Self::Deleting)
                | (Self::Stopped, Self::Deleting)
                | (Self::Failed, Self::Deleting)
                | (Self::Deleting, Self::Failed)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerPortSpec {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerResourceSpec {
    pub cpu_millicores: u32,
    pub memory_bytes: u64,
    #[serde(default)]
    pub pids_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Option<Uuid>,
    pub name: String,
    pub image: String,
    pub ports: Vec<ContainerPortSpec>,
    pub resources: ContainerResourceSpec,
    pub env: BTreeMap<String, String>,
    pub command: Vec<String>,
    pub restart_policy: String,
    pub state: ContainerState,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContainerOperation {
    #[serde(rename = "container.apply")]
    Apply,
    #[serde(rename = "container.start")]
    Start,
    #[serde(rename = "container.stop")]
    Stop,
    #[serde(rename = "container.restart")]
    Restart,
    #[serde(rename = "container.delete")]
    Delete,
}

impl ContainerOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "container.apply",
            Self::Start => "container.start",
            Self::Stop => "container.stop",
            Self::Restart => "container.restart",
            Self::Delete => "container.delete",
        }
    }

    fn target_in_progress_state(self) -> ContainerState {
        match self {
            Self::Apply => ContainerState::Provisioning,
            Self::Start => ContainerState::Starting,
            Self::Stop => ContainerState::Stopping,
            Self::Restart => ContainerState::Restarting,
            Self::Delete => ContainerState::Deleting,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerOperationReservation {
    pub container: ContainerRecord,
    pub request_id: Uuid,
    pub existing_job_id: Option<Uuid>,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone)]
pub struct NewContainerSpec<'a> {
    pub tenant_id: Uuid,
    pub node_id: Option<Uuid>,
    pub name: &'a str,
    pub image: &'a str,
    pub ports: &'a [ContainerPortSpec],
    pub resources: &'a ContainerResourceSpec,
    pub env: &'a BTreeMap<String, String>,
    pub command: &'a [String],
    pub restart_policy: &'a str,
}

impl ControlStore {
    pub fn list_containers_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<ContainerRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT container_id, tenant_id, node_id, name, image, ports, resources, env, command, restart_policy, state, last_error, created_at, updated_at
             FROM containers WHERE tenant_id = ?1 ORDER BY created_at DESC, container_id DESC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], row_to_container)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_container_for_tenant(
        &self,
        container_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<ContainerRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT container_id, tenant_id, node_id, name, image, ports, resources, env, command, restart_policy, state, last_error, created_at, updated_at
                 FROM containers WHERE container_id = ?1 AND tenant_id = ?2",
                params![container_id.to_string(), tenant_id.to_string()],
                row_to_container,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn reserve_container_create(
        &mut self,
        spec: NewContainerSpec<'_>,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<ContainerOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_container_operation(
            &tx,
            idempotency_key,
            fingerprint,
            ContainerOperation::Apply,
        )? {
            tx.commit()?;
            return Ok(replay);
        }

        let name_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM containers WHERE tenant_id = ?1 AND name = ?2)",
            params![spec.tenant_id.to_string(), spec.name],
            |row| row.get(0),
        )?;
        if name_exists {
            return Err(StoreError::ContainerConflict(format!(
                "Container '{}' already exists for tenant",
                spec.name
            )));
        }

        check_host_port_conflicts(&tx, spec.tenant_id, spec.node_id, spec.ports)?;

        let container_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let record = ContainerRecord {
            id: container_id,
            tenant_id: spec.tenant_id,
            node_id: spec.node_id,
            name: spec.name.to_string(),
            image: spec.image.to_string(),
            ports: spec.ports.to_vec(),
            resources: spec.resources.clone(),
            env: spec.env.clone(),
            command: spec.command.to_vec(),
            restart_policy: spec.restart_policy.to_string(),
            state: ContainerState::Provisioning,
            last_error: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };

        tx.execute(
            "INSERT INTO containers (
                container_id, tenant_id, node_id, name, image, ports, resources, env, command, restart_policy,
                state, last_error, created_at, updated_at, operation_request_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'provisioning', NULL, ?11, ?12, ?13)",
            params![
                record.id.to_string(),
                record.tenant_id.to_string(),
                record.node_id.map(|id| id.to_string()),
                record.name,
                record.image,
                serde_json::to_string(&record.ports)?,
                serde_json::to_string(&record.resources)?,
                serde_json::to_string(&record.env)?,
                serde_json::to_string(&record.command)?,
                record.restart_policy,
                record.created_at,
                record.updated_at,
                request_id.to_string(),
            ],
        )?;

        insert_container_operation(
            &tx,
            idempotency_key,
            fingerprint,
            ContainerOperation::Apply,
            container_id,
            request_id,
            &record,
        )?;
        tx.commit()?;

        Ok(ContainerOperationReservation {
            container: record,
            request_id,
            existing_job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_container_lifecycle(
        &mut self,
        container_id: Uuid,
        tenant_id: Uuid,
        operation: ContainerOperation,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<ContainerOperationReservation, StoreError> {
        if !matches!(
            operation,
            ContainerOperation::Start
                | ContainerOperation::Stop
                | ContainerOperation::Restart
                | ContainerOperation::Delete
        ) {
            return Err(StoreError::ContainerConflict(
                "Unsupported lifecycle operation".to_string(),
            ));
        }

        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) =
            find_existing_container_operation(&tx, idempotency_key, fingerprint, operation)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut record = load_container_for_tenant_tx(&tx, container_id, tenant_id)?;
        if record.state.is_in_progress() {
            return Err(StoreError::ContainerOperationInProgress);
        }

        let next_state = operation.target_in_progress_state();
        if !record.state.can_transition_to(next_state) {
            return Err(StoreError::ContainerConflict(format!(
                "Cannot {} container from state '{}'",
                operation.as_str(),
                record.state.as_str()
            )));
        }

        let request_id = Uuid::new_v4();
        record.state = next_state;
        record.last_error = None;
        record.updated_at = now.to_string();

        tx.execute(
            "UPDATE containers SET state = ?1, last_error = NULL, updated_at = ?2, operation_request_id = ?3 WHERE container_id = ?4",
            params![
                record.state.as_str(),
                record.updated_at,
                request_id.to_string(),
                container_id.to_string(),
            ],
        )?;

        insert_container_operation(
            &tx,
            idempotency_key,
            fingerprint,
            operation,
            container_id,
            request_id,
            &record,
        )?;
        tx.commit()?;

        Ok(ContainerOperationReservation {
            container: record,
            request_id,
            existing_job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn set_container_operation_job_id(
        &mut self,
        idempotency_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE container_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        Ok(())
    }

    pub fn complete_container_operation(
        &mut self,
        request_id: Uuid,
        operation: ContainerOperation,
        worker_node_id: Option<Uuid>,
        succeeded: bool,
        error_text: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let container_id_str: Option<String> = tx
            .query_row(
                "SELECT container_id FROM container_operations WHERE request_id = ?1 AND operation = ?2",
                params![request_id.to_string(), operation.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(container_id_str) = container_id_str else {
            return Ok(false);
        };

        let row: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT operation_request_id, node_id FROM containers WHERE container_id = ?1",
                [&container_id_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((active_req, assigned_node_str)) = row else {
            tx.commit()?;
            return Ok(true);
        };

        if active_req != request_id.to_string() {
            tx.commit()?;
            return Ok(true);
        }

        if let Some(assigned_str) = assigned_node_str {
            let assigned = Uuid::parse_str(&assigned_str)
                .map_err(|e| StoreError::Parse(format!("invalid node_id '{assigned_str}': {e}")))?;
            if worker_node_id != Some(assigned) {
                return Err(StoreError::ContainerConflict(
                    "Worker node does not own this container".to_string(),
                ));
            }
        }

        let worker_node_str = worker_node_id.map(|id| id.to_string());
        match operation {
            ContainerOperation::Apply | ContainerOperation::Start | ContainerOperation::Restart => {
                if succeeded {
                    tx.execute(
                        "UPDATE containers
                         SET state = 'running',
                             node_id = COALESCE(node_id, ?1),
                             last_error = NULL,
                             updated_at = ?2
                         WHERE container_id = ?3",
                        params![worker_node_str, now, container_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE containers
                         SET state = 'failed',
                             last_error = ?1,
                             updated_at = ?2
                         WHERE container_id = ?3",
                        params![
                            error_text.unwrap_or("container operation failed"),
                            now,
                            container_id_str
                        ],
                    )?;
                }
            }
            ContainerOperation::Stop => {
                if succeeded {
                    tx.execute(
                        "UPDATE containers
                         SET state = 'stopped',
                             node_id = COALESCE(node_id, ?1),
                             last_error = NULL,
                             updated_at = ?2
                         WHERE container_id = ?3",
                        params![worker_node_str, now, container_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE containers
                         SET state = 'failed',
                             last_error = ?1,
                             updated_at = ?2
                         WHERE container_id = ?3",
                        params![
                            error_text.unwrap_or("container.stop failed"),
                            now,
                            container_id_str
                        ],
                    )?;
                }
            }
            ContainerOperation::Delete => {
                if succeeded {
                    tx.execute(
                        "DELETE FROM containers WHERE container_id = ?1",
                        [&container_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE containers
                         SET state = 'failed',
                             last_error = ?1,
                             updated_at = ?2
                         WHERE container_id = ?3",
                        params![
                            error_text.unwrap_or("container.delete failed"),
                            now,
                            container_id_str
                        ],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(true)
    }
}

fn load_container_for_tenant_tx(
    tx: &Transaction<'_>,
    container_id: Uuid,
    tenant_id: Uuid,
) -> Result<ContainerRecord, StoreError> {
    tx.query_row(
        "SELECT container_id, tenant_id, node_id, name, image, ports, resources, env, command, restart_policy, state, last_error, created_at, updated_at
         FROM containers WHERE container_id = ?1 AND tenant_id = ?2",
        params![container_id.to_string(), tenant_id.to_string()],
        row_to_container,
    )
    .optional()?
    .ok_or(StoreError::ContainerNotFound(container_id))
}

fn check_host_port_conflicts(
    tx: &Transaction<'_>,
    tenant_id: Uuid,
    node_id: Option<Uuid>,
    ports: &[ContainerPortSpec],
) -> Result<(), StoreError> {
    if ports.is_empty() {
        return Ok(());
    }
    let mut stmt = if node_id.is_some() {
        tx.prepare("SELECT ports FROM containers WHERE node_id = ?1 AND state != 'deleting'")?
    } else {
        tx.prepare("SELECT ports FROM containers WHERE tenant_id = ?1 AND state != 'deleting'")?
    };
    let scope_id = node_id.unwrap_or(tenant_id).to_string();
    let rows = stmt.query_map([scope_id], |row| row.get::<_, String>(0))?;
    for row in rows {
        let existing_json = row?;
        let existing_ports: Vec<ContainerPortSpec> = serde_json::from_str(&existing_json)?;
        for new_port in ports {
            if existing_ports.iter().any(|existing| {
                existing.host_port == new_port.host_port && existing.protocol == new_port.protocol
            }) {
                return Err(StoreError::ContainerConflict(format!(
                    "Host port {}/{} is already bound by another container",
                    new_port.host_port, new_port.protocol
                )));
            }
        }
    }
    Ok(())
}

fn find_existing_container_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    expected_op: ContainerOperation,
) -> Result<Option<ContainerOperationReservation>, StoreError> {
    let row: Option<(String, String, String, Option<String>, String)> = tx
        .query_row(
            "SELECT fingerprint, operation, request_id, job_id, snapshot
             FROM container_operations WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((stored_fp, stored_op, req_id_str, job_id_str, snapshot_str)) = row else {
        return Ok(None);
    };
    if stored_fp != fingerprint || stored_op != expected_op.as_str() {
        return Err(StoreError::IdempotencyConflict);
    }
    let container: ContainerRecord = serde_json::from_str(&snapshot_str)?;
    let request_id = Uuid::parse_str(&req_id_str)
        .map_err(|e| StoreError::Parse(format!("invalid request_id '{req_id_str}': {e}")))?;
    let existing_job_id = job_id_str
        .map(|s| {
            Uuid::parse_str(&s).map_err(|e| StoreError::Parse(format!("invalid job_id '{s}': {e}")))
        })
        .transpose()?;
    Ok(Some(ContainerOperationReservation {
        container,
        request_id,
        existing_job_id,
        idempotent_replay: true,
    }))
}

fn insert_container_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    operation: ContainerOperation,
    container_id: Uuid,
    request_id: Uuid,
    snapshot: &ContainerRecord,
) -> Result<(), StoreError> {
    let snapshot_json = serde_json::to_string(snapshot)?;
    tx.execute(
        "INSERT INTO container_operations (idempotency_key, fingerprint, operation, container_id, request_id, job_id, snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
        params![
            idempotency_key,
            fingerprint,
            operation.as_str(),
            container_id.to_string(),
            request_id.to_string(),
            snapshot_json,
        ],
    )?;
    Ok(())
}

fn row_to_container(row: &Row<'_>) -> rusqlite::Result<ContainerRecord> {
    let id_str: String = row.get(0)?;
    let tenant_str: String = row.get(1)?;
    let node_str: Option<String> = row.get(2)?;
    let ports_json: String = row.get(5)?;
    let resources_json: String = row.get(6)?;
    let env_json: String = row.get(7)?;
    let command_json: String = row.get(8)?;
    let state_str: String = row.get(10)?;

    let parse_err = |msg: String| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, msg)),
        )
    };

    let id = Uuid::parse_str(&id_str).map_err(|e| parse_err(e.to_string()))?;
    let tenant_id = Uuid::parse_str(&tenant_str).map_err(|e| parse_err(e.to_string()))?;
    let node_id = node_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| parse_err(e.to_string()))?;
    let ports: Vec<ContainerPortSpec> =
        serde_json::from_str(&ports_json).map_err(|e| parse_err(e.to_string()))?;
    let resources: ContainerResourceSpec =
        serde_json::from_str(&resources_json).map_err(|e| parse_err(e.to_string()))?;
    let env: BTreeMap<String, String> =
        serde_json::from_str(&env_json).map_err(|e| parse_err(e.to_string()))?;
    let command: Vec<String> =
        serde_json::from_str(&command_json).map_err(|e| parse_err(e.to_string()))?;
    let state = ContainerState::parse(&state_str).map_err(|e| parse_err(e.to_string()))?;

    Ok(ContainerRecord {
        id,
        tenant_id,
        node_id,
        name: row.get(3)?,
        image: row.get(4)?,
        ports,
        resources,
        env,
        command,
        restart_policy: row.get(9)?,
        state,
        last_error: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resources() -> ContainerResourceSpec {
        ContainerResourceSpec {
            cpu_millicores: 500,
            memory_bytes: 256 * 1024 * 1024,
            pids_limit: Some(128),
        }
    }

    #[test]
    fn container_state_machine_and_idempotency() {
        assert!(ContainerState::Provisioning.can_transition_to(ContainerState::Running));
        assert!(ContainerState::Provisioning.can_transition_to(ContainerState::Failed));
        assert!(!ContainerState::Provisioning.can_transition_to(ContainerState::Deleting));
        assert!(ContainerState::Running.can_transition_to(ContainerState::Stopping));
        assert!(!ContainerState::Stopped.can_transition_to(ContainerState::Stopping));

        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        store
            .register_node(node_id, "node-1", "hash-1", "2026-11-27T00:00:00Z")
            .unwrap();

        let ports = vec![ContainerPortSpec {
            host_port: 8080,
            container_port: 80,
            protocol: "tcp".to_string(),
        }];
        let resources = sample_resources();
        let env = BTreeMap::new();
        let cmd = vec![];
        let digest = "a".repeat(64);
        let image = format!("nginx@sha256:{digest}");

        let res1 = store
            .reserve_container_create(
                NewContainerSpec {
                    tenant_id,
                    node_id: Some(node_id),
                    name: "web-1",
                    image: &image,
                    ports: &ports,
                    resources: &resources,
                    env: &env,
                    command: &cmd,
                    restart_policy: "unless_stopped",
                },
                "key-1",
                "fp-1",
                "2026-11-27T00:00:00Z",
            )
            .unwrap();
        assert!(!res1.idempotent_replay);
        assert_eq!(res1.container.state, ContainerState::Provisioning);

        // Exact replay returns the same reservation
        let replay = store
            .reserve_container_create(
                NewContainerSpec {
                    tenant_id,
                    node_id: Some(node_id),
                    name: "web-1",
                    image: &image,
                    ports: &ports,
                    resources: &resources,
                    env: &env,
                    command: &cmd,
                    restart_policy: "unless_stopped",
                },
                "key-1",
                "fp-1",
                "2026-11-27T00:00:01Z",
            )
            .unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(replay.container.id, res1.container.id);

        // Conflicting replay returns IdempotencyConflict
        let conflict = store.reserve_container_create(
            NewContainerSpec {
                tenant_id,
                node_id: Some(node_id),
                name: "web-2",
                image: &image,
                ports: &[],
                resources: &resources,
                env: &env,
                command: &cmd,
                restart_policy: "unless_stopped",
            },
            "key-1",
            "fp-2",
            "2026-11-27T00:00:01Z",
        );
        assert!(matches!(conflict, Err(StoreError::IdempotencyConflict)));

        // Wrong node cannot complete the operation
        let wrong_node = Uuid::new_v4();
        let wrong_complete = store.complete_container_operation(
            res1.request_id,
            ContainerOperation::Apply,
            Some(wrong_node),
            true,
            None,
            "2026-11-27T00:00:02Z",
        );
        assert!(matches!(
            wrong_complete,
            Err(StoreError::ContainerConflict(_))
        ));

        // Assigned node completes successfully -> Running
        assert!(store
            .complete_container_operation(
                res1.request_id,
                ContainerOperation::Apply,
                Some(node_id),
                true,
                None,
                "2026-11-27T00:00:03Z",
            )
            .unwrap());
        let loaded = store
            .get_container_for_tenant(res1.container.id, tenant_id)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.state, ContainerState::Running);

        // Host port conflict on same node rejected
        let port_conflict = store.reserve_container_create(
            NewContainerSpec {
                tenant_id,
                node_id: Some(node_id),
                name: "web-port-dup",
                image: &image,
                ports: &ports,
                resources: &resources,
                env: &env,
                command: &cmd,
                restart_policy: "unless_stopped",
            },
            "key-port-dup",
            "fp-port-dup",
            "2026-11-27T00:00:04Z",
        );
        assert!(matches!(
            port_conflict,
            Err(StoreError::ContainerConflict(_))
        ));

        // Tenant isolation: another tenant cannot see or operate on it
        let other_tenant = Uuid::new_v4();
        assert!(store
            .get_container_for_tenant(res1.container.id, other_tenant)
            .unwrap()
            .is_none());
        let foreign_stop = store.reserve_container_lifecycle(
            res1.container.id,
            other_tenant,
            ContainerOperation::Stop,
            "key-foreign",
            "fp-foreign",
            "2026-11-27T00:00:05Z",
        );
        assert!(matches!(
            foreign_stop,
            Err(StoreError::ContainerNotFound(_))
        ));
    }
}

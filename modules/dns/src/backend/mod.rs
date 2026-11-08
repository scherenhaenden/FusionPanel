use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::domain::{DomainName, Record, RecordType, Zone};
use crate::provider::{
    AppliedResult, DnssecStatus, ObservedState, Plan, Provider, ProviderError, RecordChange,
};

#[derive(Debug, Clone)]
pub struct ExportedZone {
    pub name: DomainName,
    pub records: Vec<Record>,
    pub serial: u32,
}

#[derive(Debug)]
struct ZoneState {
    zone: Zone,
    serial: u32,
    dnssec_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AuthoritativeBackend {
    zones: Arc<Mutex<HashMap<Uuid, ZoneState>>>,
}

impl AuthoritativeBackend {
    pub fn new() -> Self {
        Self {
            zones: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Internal helper to fetch zone
    fn with_zone<T, F>(&self, zone_id: Uuid, f: F) -> Result<T, ProviderError>
    where
        F: FnOnce(&mut ZoneState) -> Result<T, ProviderError>,
    {
        let mut map = self.zones.lock().unwrap();
        let state = map
            .get_mut(&zone_id)
            .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", zone_id)))?;
        f(state)
    }

    pub fn enable_dnssec(&self, zone_id: Uuid) -> Result<(), ProviderError> {
        self.with_zone(zone_id, |state| {
            state.dnssec_enabled = true;
            Ok(())
        })
    }

    pub fn export_zone(&self, zone_id: Uuid) -> Result<ExportedZone, ProviderError> {
        let map = self.zones.lock().unwrap();
        let state = map
            .get(&zone_id)
            .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", zone_id)))?;

        Ok(ExportedZone {
            name: state.zone.name().clone(),
            records: state.zone.records().to_vec(),
            serial: state.serial,
        })
    }

    pub fn restore_zone(&self, zone: Zone, serial: u32) -> Result<(), ProviderError> {
        let mut map = self.zones.lock().unwrap();
        let id = zone.id();
        map.insert(
            id,
            ZoneState {
                zone,
                serial,
                dnssec_enabled: false,
            },
        );
        Ok(())
    }

    pub fn query(
        &self,
        zone_id: Uuid,
        name: &DomainName,
        record_type: RecordType,
    ) -> Result<Vec<Record>, ProviderError> {
        let map = self.zones.lock().unwrap();
        let state = map
            .get(&zone_id)
            .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", zone_id)))?;

        let records = state
            .zone
            .records()
            .iter()
            .filter(|r| r.name() == name && r.record_type() == record_type)
            .cloned()
            .collect();

        Ok(records)
    }
}

impl Provider for AuthoritativeBackend {
    fn capabilities(&self) -> Vec<String> {
        vec!["authoritative".to_string(), "dnssec".to_string()]
    }

    fn plan_change(&self, plan: Plan) -> Result<Plan, ProviderError> {
        match &plan {
            Plan::CreateZone(zone) => {
                let map = self.zones.lock().unwrap();
                if map.contains_key(&zone.id()) {
                    return Err(ProviderError::Conflict(format!(
                        "Zone {} already exists",
                        zone.id()
                    )));
                }
            }
            Plan::DeleteZone(id) => {
                let map = self.zones.lock().unwrap();
                if !map.contains_key(id) {
                    return Err(ProviderError::InvalidInput(format!(
                        "Zone {} not found",
                        id
                    )));
                }
            }
            Plan::UpdateRecords(id, changes) => {
                let map = self.zones.lock().unwrap();
                let state = map
                    .get(id)
                    .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", id)))?;

                for change in changes {
                    match change {
                        RecordChange::Add(record) => {
                            if !record.name().is_within(state.zone.name()) {
                                return Err(ProviderError::InvalidInput(
                                    "Record outside zone".to_string(),
                                ));
                            }
                        }
                        RecordChange::Remove(record) => {
                            if !state.zone.records().contains(record) {
                                return Err(ProviderError::Conflict(
                                    "Target record not found for removal (serial conflict)"
                                        .to_string(),
                                ));
                            }
                        }
                        RecordChange::Modify { old, new } => {
                            if !state.zone.records().contains(old) {
                                return Err(ProviderError::Conflict(
                                    "Old record not found for modification (serial conflict)"
                                        .to_string(),
                                ));
                            }
                            if !new.name().is_within(state.zone.name()) {
                                return Err(ProviderError::InvalidInput(
                                    "New record outside zone".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(plan)
    }

    fn apply_change(&self, plan: Plan) -> Result<AppliedResult, ProviderError> {
        match plan {
            Plan::CreateZone(zone) => {
                let id = zone.id();
                let mut map = self.zones.lock().unwrap();
                if map.contains_key(&id) {
                    return Err(ProviderError::Conflict(format!(
                        "Zone {} already exists",
                        id
                    )));
                }
                map.insert(
                    id,
                    ZoneState {
                        zone,
                        serial: 1,
                        dnssec_enabled: false,
                    },
                );
                Ok(AppliedResult {
                    plan_id: Uuid::new_v4(),
                    status: "created".to_string(),
                    effects: vec!["Zone created".to_string()],
                })
            }
            Plan::DeleteZone(id) => {
                let mut map = self.zones.lock().unwrap();
                if map.remove(&id).is_none() {
                    return Err(ProviderError::InvalidInput(format!(
                        "Zone {} not found",
                        id
                    )));
                }
                Ok(AppliedResult {
                    plan_id: Uuid::new_v4(),
                    status: "deleted".to_string(),
                    effects: vec!["Zone deleted".to_string()],
                })
            }
            Plan::UpdateRecords(id, changes) => {
                self.with_zone(id, |state| {
                    for change in changes {
                        match change {
                            RecordChange::Add(record) => {
                                state.zone.add_record(record)?;
                            }
                            RecordChange::Remove(record) => {
                                if !state.zone.remove_record(&record) {
                                    return Err(ProviderError::Conflict(
                                        "Record not found for removal".to_string(),
                                    ));
                                }
                            }
                            RecordChange::Modify { old, new } => {
                                if !state.zone.remove_record(&old) {
                                    return Err(ProviderError::Conflict(
                                        "Old record not found for modification".to_string(),
                                    ));
                                }
                                state.zone.add_record(new)?;
                            }
                        }
                    }
                    state.serial += 1;
                    Ok(())
                })?;

                Ok(AppliedResult {
                    plan_id: Uuid::new_v4(),
                    status: "updated".to_string(),
                    effects: vec!["Records updated".to_string()],
                })
            }
        }
    }

    fn observe_zone(&self, zone_id: Uuid) -> Result<ObservedState, ProviderError> {
        let map = self.zones.lock().unwrap();
        let state = map
            .get(&zone_id)
            .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", zone_id)))?;

        let dnssec = if state.dnssec_enabled {
            DnssecStatus {
                enabled: true,
                key_material: vec![], // In a real system, keys would be generated
            }
        } else {
            DnssecStatus {
                enabled: false,
                key_material: vec![],
            }
        };

        Ok(ObservedState {
            zone: state.zone.clone(),
            dnssec,
        })
    }

    fn get_dnssec_status(&self, zone_id: Uuid) -> Result<DnssecStatus, ProviderError> {
        let map = self.zones.lock().unwrap();
        let state = map
            .get(&zone_id)
            .ok_or_else(|| ProviderError::InvalidInput(format!("Zone {} not found", zone_id)))?;

        if state.dnssec_enabled {
            Ok(DnssecStatus {
                enabled: true,
                key_material: vec![], // In a real system, keys would be generated
            })
        } else {
            Ok(DnssecStatus {
                enabled: false,
                key_material: vec![],
            })
        }
    }
}

#![forbid(unsafe_code)]

/// Resource ownership tiers for FusionPanel.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OwnershipTier {
    /// Fully managed by the panel.
    PanelOwned,
    /// Adopted with a boundary (e.g., panel manages config, but user manages data).
    AdoptedWithBoundary,
    /// Externally managed resource.
    ExternallyManaged,
}

/// A resource in the system.
#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub tier: OwnershipTier,
    pub is_active: bool,
    pub is_deleted: bool,
}

impl Resource {
    pub fn new(name: &str, tier: OwnershipTier) -> Self {
        Self {
            name: name.to_string(),
            tier,
            is_active: true,
            is_deleted: false,
        }
    }
}

/// Simulates a lifecycle operation on a resource.
pub fn apply_lifecycle_action(resource: &mut Resource, action: &str) -> Result<(), &'static str> {
    match action {
        "suspend" => {
            if resource.tier == OwnershipTier::ExternallyManaged {
                return Err("Cannot suspend externally managed resources");
            }
            resource.is_active = false;
            Ok(())
        }
        "delete" => {
            if resource.tier == OwnershipTier::ExternallyManaged {
                return Err("Cannot delete externally managed resources");
            }
            resource.is_deleted = true;
            Ok(())
        }
        "restart" => {
            if resource.tier == OwnershipTier::ExternallyManaged {
                return Err("Cannot restart externally managed resources");
            }
            resource.is_active = true; // Assuming restart makes it active.
            Ok(())
        }
        _ => Err("Unknown action"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_owned_lifecycle() {
        let mut resource = Resource::new("panel_db", OwnershipTier::PanelOwned);

        assert_eq!(apply_lifecycle_action(&mut resource, "suspend"), Ok(()));
        assert!(!resource.is_active);

        assert_eq!(apply_lifecycle_action(&mut resource, "restart"), Ok(()));
        assert!(resource.is_active);

        assert_eq!(apply_lifecycle_action(&mut resource, "delete"), Ok(()));
        assert!(resource.is_deleted);
    }

    #[test]
    fn test_externally_managed_lifecycle() {
        let mut resource = Resource::new("external_db", OwnershipTier::ExternallyManaged);

        assert_eq!(
            apply_lifecycle_action(&mut resource, "suspend"),
            Err("Cannot suspend externally managed resources")
        );
        assert!(resource.is_active);

        assert_eq!(
            apply_lifecycle_action(&mut resource, "restart"),
            Err("Cannot restart externally managed resources")
        );
        assert!(resource.is_active);

        assert_eq!(
            apply_lifecycle_action(&mut resource, "delete"),
            Err("Cannot delete externally managed resources")
        );
        assert!(!resource.is_deleted);
    }

    #[test]
    fn test_adopted_with_boundary_lifecycle() {
        let mut resource = Resource::new("adopted_service", OwnershipTier::AdoptedWithBoundary);

        assert_eq!(apply_lifecycle_action(&mut resource, "suspend"), Ok(()));
        assert!(!resource.is_active);

        assert_eq!(apply_lifecycle_action(&mut resource, "restart"), Ok(()));
        assert!(resource.is_active);

        assert_eq!(apply_lifecycle_action(&mut resource, "delete"), Ok(()));
        assert!(resource.is_deleted);
    }

    #[test]
    fn test_deleting_managed_site_keeps_external_workloads_intact() {
        let mut managed_site = Resource::new("site1", OwnershipTier::PanelOwned);
        let mut external_db = Resource::new("ext_db", OwnershipTier::ExternallyManaged);

        // Simulating deleting managed site.
        assert_eq!(apply_lifecycle_action(&mut managed_site, "delete"), Ok(()));
        assert!(managed_site.is_deleted);

        // Simulated check that external_db was unaffected
        let delete_result = apply_lifecycle_action(&mut external_db, "delete");
        assert_eq!(delete_result, Err("Cannot delete externally managed resources"));
        assert!(!external_db.is_deleted);
    }
}

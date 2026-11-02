use fusionpanel_administration::tenants::models::{AccountStatus, Organization, PlanTier};

#[test]
fn test_create_organization() {
    let org = Organization::new("Acme Corp").unwrap();
    assert_eq!(org.name, "Acme Corp");
    assert_eq!(org.plan, PlanTier::Free);
    assert_eq!(org.status, AccountStatus::Pending);
}

#[test]
fn test_empty_organization_name() {
    let err = Organization::new("").unwrap_err();
    assert_eq!(err, "Organization name cannot be empty");

    let err = Organization::new("   ").unwrap_err();
    assert_eq!(err, "Organization name cannot be empty");
}

#[test]
fn test_long_organization_name() {
    let name = "a".repeat(101);
    let err = Organization::new(name).unwrap_err();
    assert_eq!(err, "Organization name too long");
}

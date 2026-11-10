#![forbid(unsafe_code)]

use databases::privilege_auditor::{Grant, PrivilegeAuditor};

#[test]
fn test_grant_parsing_basic() {
    let raw = "GRANT SELECT, INSERT ON *.* TO 'user'@'%' WITH GRANT OPTION";
    let grant = Grant::parse(raw).unwrap();
    assert_eq!(grant.privileges, vec!["SELECT", "INSERT"]);
    assert_eq!(grant.object, "*.*");
    assert_eq!(grant.user, "user");
    assert_eq!(grant.host, "%");
    assert_eq!(grant.has_grant_option, true);
    assert_eq!(grant.raw, raw);
}

#[test]
fn test_grant_parsing_all_privileges() {
    let raw = "GRANT ALL PRIVILEGES ON `db`.* TO 'some user'@'192.168.%' IDENTIFIED BY 'pass'";
    let grant = Grant::parse(raw).unwrap();
    assert_eq!(grant.privileges, vec!["ALL PRIVILEGES"]);
    assert_eq!(grant.object, "`db`.*");
    assert_eq!(grant.user, "some user");
    assert_eq!(grant.host, "192.168.%");
    assert_eq!(grant.has_grant_option, false);
}

#[test]
fn test_grant_parsing_edge_cases() {
    let raw = "GRANT SELECT (id, name), UPDATE (name) ON db.table TO user@localhost";
    let grant = Grant::parse(raw).unwrap();
    assert_eq!(grant.privileges, vec!["SELECT (ID, NAME)", "UPDATE (NAME)"]);
    assert_eq!(grant.object, "db.table");
    assert_eq!(grant.user, "user");
    assert_eq!(grant.host, "localhost");
    assert_eq!(grant.has_grant_option, false);
}

#[test]
fn test_grant_parsing_invalid() {
    assert!(Grant::parse("REVOKE ALL ON *.* FROM user").is_none());
    assert!(Grant::parse("NOT A GRANT STATEMENT").is_none());
}

#[test]
fn test_auditor_dangerous_privileges() {
    let raw = "GRANT SUPER, FILE, SHUTDOWN, SELECT ON *.* TO 'root'@'localhost'";
    let violations = PrivilegeAuditor::parse_and_audit(&[raw]);

    let types: Vec<String> = violations.iter().map(|v| v.violation_type.clone()).collect();
    let descriptions: Vec<String> = violations.iter().map(|v| v.description.clone()).collect();

    assert_eq!(types.iter().filter(|&t| t == "DangerousPrivilege").count(), 3);
    assert!(descriptions.contains(&"User has SUPER privilege".to_string()));
    assert!(descriptions.contains(&"User has FILE privilege".to_string()));
    assert!(descriptions.contains(&"User has SHUTDOWN privilege".to_string()));
}

#[test]
fn test_auditor_host_wildcards() {
    let raw1 = "GRANT SELECT ON db.* TO 'dev'@'%'";
    let violations1 = PrivilegeAuditor::parse_and_audit(&[raw1]);
    assert_eq!(violations1[0].violation_type, "WildcardHost");
    assert_eq!(violations1[0].description, "Host is set to global wildcard '%'");

    let raw2 = "GRANT SELECT ON db.* TO 'dev'@'10.0.%'";
    let violations2 = PrivilegeAuditor::parse_and_audit(&[raw2]);
    assert_eq!(violations2[0].violation_type, "PartialWildcardHost");
    assert_eq!(violations2[0].description, "Host contains wildcard: 10.0.%");
}

#[test]
fn test_auditor_grant_option() {
    let raw = "GRANT INSERT ON *.* TO 'app'@'localhost' WITH GRANT OPTION";
    let violations = PrivilegeAuditor::parse_and_audit(&[raw]);

    let types: Vec<String> = violations.iter().map(|v| v.violation_type.clone()).collect();
    assert!(types.contains(&"GrantOption".to_string()));
}

#[test]
fn test_auditor_global_all_privileges() {
    let raw1 = "GRANT ALL PRIVILEGES ON *.* TO 'admin'@'localhost'";
    let violations1 = PrivilegeAuditor::parse_and_audit(&[raw1]);
    assert_eq!(violations1[0].violation_type, "GlobalAllPrivileges");

    let raw2 = "GRANT ALL PRIVILEGES ON db.* TO 'admin'@'localhost'";
    let violations2 = PrivilegeAuditor::parse_and_audit(&[raw2]);
    assert!(violations2.is_empty());
}

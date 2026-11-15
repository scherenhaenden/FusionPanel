#![forbid(unsafe_code)]

use hosting::modsecurity_rules::{ModSecurityExclusionsBuilder, Preset};

#[test]
fn test_empty_builder() {
    let builder = ModSecurityExclusionsBuilder::new();
    assert_eq!(builder.build(), "");
}

#[test]
fn test_presets() {
    let builder = ModSecurityExclusionsBuilder::new().add_preset(Preset::WordPress);
    assert_eq!(
        builder.build(),
        "SecAction \"id:10000,phase:1,nolog,pass,t:none,setvar:tx.crs_exclusions_wordpress=1\"\n"
    );

    let builder2 = ModSecurityExclusionsBuilder::new()
        .preset_rule_id(99999)
        .add_preset(Preset::Nextcloud)
        .add_preset(Preset::WordPress);
    assert_eq!(
        builder2.build(),
        "SecAction \"id:99999,phase:1,nolog,pass,t:none,setvar:tx.crs_exclusions_nextcloud=1,setvar:tx.crs_exclusions_wordpress=1\"\n"
    );
}

#[test]
fn test_global_removals() {
    let builder = ModSecurityExclusionsBuilder::new()
        .remove_rule_by_id(920350)
        .remove_rule_by_id(920360);
    assert_eq!(
        builder.build(),
        "SecRuleRemoveById 920350 920360\n"
    );
}

#[test]
fn test_custom_uri_rules() {
    let builder = ModSecurityExclusionsBuilder::new()
        .remove_rule_by_id_for_uri("/api/upload", 10001, 941100)
        .remove_target_by_id_for_uri("/admin/settings", 10002, 942100, "ARGS:json_payload");

    assert_eq!(
        builder.build(),
        "SecRule REQUEST_URI \"@beginsWith /api/upload\" \"id:10001,phase:1,pass,nolog,ctl:ruleRemoveById=941100\"\n\
         \n\
         SecRule REQUEST_URI \"@beginsWith /admin/settings\" \"id:10002,phase:1,pass,nolog,ctl:ruleRemoveTargetById=942100;ARGS:json_payload\"\n"
    );
}

#[test]
fn test_combined() {
    let builder = ModSecurityExclusionsBuilder::new()
        .add_preset(Preset::WordPress)
        .remove_rule_by_id(920350)
        .remove_rule_by_id_for_uri("/wp-admin", 10001, 941100);

    assert_eq!(
        builder.build(),
        "SecAction \"id:10000,phase:1,nolog,pass,t:none,setvar:tx.crs_exclusions_wordpress=1\"\n\
         \n\
         SecRuleRemoveById 920350\n\
         \n\
         SecRule REQUEST_URI \"@beginsWith /wp-admin\" \"id:10001,phase:1,pass,nolog,ctl:ruleRemoveById=941100\"\n"
    );
}

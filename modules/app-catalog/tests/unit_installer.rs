use app_catalog::installer::CatalogInstaller;
use std::collections::HashMap;

#[test]
fn test_render_compose() {
    let template = "version: '3'\nservices:\n  app:\n    image: myapp:${TAG}\n    environment:\n      - DB_PASS=${DB_PASSWORD}";
    let mut config = HashMap::new();
    config.insert("TAG".to_string(), "latest".to_string());
    config.insert("DB_PASSWORD".to_string(), "secret".to_string());

    let rendered = CatalogInstaller::render_compose(template, &config);

    assert!(rendered.contains("image: myapp:latest"));
    assert!(rendered.contains("- DB_PASS=secret"));
}

#[test]
fn test_render_compose_brackets_and_no_brackets() {
    let template = "var1=$VAR1 var2=${VAR2}";
    let mut config = HashMap::new();
    config.insert("VAR1".to_string(), "val1".to_string());
    config.insert("VAR2".to_string(), "val2".to_string());

    let rendered = CatalogInstaller::render_compose(template, &config);

    assert_eq!(rendered, "var1=val1 var2=val2");
}

#[test]
fn test_allocate_port() {
    let used_ports = vec![8080, 8081];

    let port1 = CatalogInstaller::allocate_port(8080, 8085, &used_ports);
    assert_eq!(port1, Some(8082));

    let port2 = CatalogInstaller::allocate_port(8080, 8081, &used_ports);
    assert_eq!(port2, None);
}

#[test]
fn test_generate_password() {
    let pw1 = CatalogInstaller::generate_password(16).unwrap();
    let pw2 = CatalogInstaller::generate_password(16).unwrap();

    assert_eq!(pw1.len(), 16);
    assert_eq!(pw2.len(), 16);
    assert_ne!(pw1, pw2);
}

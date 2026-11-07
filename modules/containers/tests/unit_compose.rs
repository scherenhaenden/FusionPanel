use containers::compose::models::PortMapping;
use containers::compose::{parse_and_validate, parse_and_validate_with_env, validator::ValidationError, ComposeError};
use std::collections::HashMap;

#[test]
fn test_valid_compose() {
    let yaml = r#"
version: '3'
services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
    volumes:
      - "./html:/usr/share/nginx/html"
  db:
    image: postgres:13
    ports:
      - "5432:5432"
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok(), "Valid compose file should pass validation");
}

#[test]
fn test_reject_privileged_mode() {
    let yaml = r#"
version: '3'
services:
  app:
    image: myapp
    privileged: true
"#;
    let result = parse_and_validate(yaml);
    match result {
        Err(ComposeError::Validation(ValidationError::PrivilegedMode(svc))) => {
            assert_eq!(svc, "app");
        }
        _ => panic!("Expected PrivilegedMode error"),
    }
}

#[test]
fn test_reject_root_mount_bypass() {
    let yaml = r#"
version: '3'
services:
  hacker:
    image: ubuntu
    volumes:
      - "/:/host_root"
"#;
    let result = parse_and_validate(yaml);
    match result {
        Err(ComposeError::Validation(ValidationError::RootMountBypass(svc))) => {
            assert_eq!(svc, "hacker");
        }
        _ => panic!("Expected RootMountBypass error"),
    }

    let yaml2 = r#"
version: '3'
services:
  hacker2:
    image: ubuntu
    volumes:
      - "/etc:/host_etc"
"#;
    let result2 = parse_and_validate(yaml2);
    match result2 {
        Err(ComposeError::Validation(ValidationError::RootMountBypass(svc))) => {
            assert_eq!(svc, "hacker2");
        }
        _ => panic!("Expected RootMountBypass error for /etc"),
    }
}

#[test]
fn test_reject_overlapping_ports() {
    let yaml = r#"
version: '3'
services:
  app1:
    image: app1
    ports:
      - "8080:80"
  app2:
    image: app2
    ports:
      - "8080:8080"
"#;
    let result = parse_and_validate(yaml);
    match result {
        Err(ComposeError::Validation(ValidationError::OverlappingPorts(port))) => {
            assert_eq!(port, "8080");
        }
        _ => panic!("Expected OverlappingPorts error"),
    }

    let yaml_range = r#"
version: '3'
services:
  app1:
    image: app1
    ports:
      - "8000-8010:8000-8010"
  app2:
    image: app2
    ports:
      - "8005:80"
"#;
    let result_range = parse_and_validate(yaml_range);
    match result_range {
        Err(ComposeError::Validation(ValidationError::OverlappingPorts(port))) => {
            assert_eq!(port, "8005");
        }
        _ => panic!("Expected OverlappingPorts error for range overlap"),
    }
}

#[test]
fn test_port_mapping_object_format() {
    let yaml = r#"
version: '3.9'
services:
  web:
    image: nginx
    ports:
      - target: 80
        published: "8080"
        protocol: tcp
        mode: host
  api:
    image: api
    ports:
      - target: 8080
        published: "8080"
"#;
    let result = parse_and_validate(yaml);
    match result {
        Err(ComposeError::Validation(ValidationError::OverlappingPorts(port))) => {
            assert_eq!(port, "8080");
        }
        _ => panic!("Expected OverlappingPorts error for object format"),
    }
}

#[test]
fn test_reject_root_mount_bypass_path_traversal() {
    let yaml = r#"
version: '3'
services:
  hacker:
    image: ubuntu
    volumes:
      - "/tmp/../../etc:/host_etc"
"#;
    let result = parse_and_validate(yaml);
    match result {
        Err(ComposeError::Validation(ValidationError::RootMountBypass(svc))) => {
            assert_eq!(svc, "hacker");
        }
        _ => panic!("Expected RootMountBypass error for path traversal"),
    }
}

#[test]
fn test_ipv6_port_mapping() {
    let yaml = r#"
version: '3'
services:
  web:
    image: nginx
    ports:
      - "[::1]:8080:80"
"#;
    let result = parse_and_validate(yaml);
    assert!(result.is_ok());

    let yaml_overlap = r#"
version: '3'
services:
  web1:
    image: nginx
    ports:
      - "[::1]:8080:80"
  web2:
    image: nginx
    ports:
      - "8080:80"
"#;
    let result_overlap = parse_and_validate(yaml_overlap);
    match result_overlap {
        Err(ComposeError::Validation(ValidationError::OverlappingPorts(port))) => {
            assert_eq!(port, "8080");
        }
        _ => panic!("Expected OverlappingPorts error for ipv6 overlap"),
    }
}

#[test]
fn test_env_substitution() {
    let yaml = r#"
version: '3'
services:
  web:
    image: ${IMAGE_NAME:-nginx:latest}
    ports:
      - "$PORT:80"
  db:
    image: postgres:${DB_VERSION}
    volumes:
      - "./data:/var/lib/postgresql/data"
"#;
    
    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "8080".to_string());
    env.insert("DB_VERSION".to_string(), "13".to_string());

    let result = parse_and_validate_with_env(yaml, &env).expect("Should parse and validate");
    
    let web_service = result.services.get("web").unwrap();
    assert_eq!(web_service.image.as_deref(), Some("nginx:latest"));
    match &web_service.ports[0] {
        PortMapping::String(mapping) => assert_eq!(mapping, "8080:80"),
        _ => panic!("Expected String port mapping"),
    }

    let db_service = result.services.get("db").unwrap();
    assert_eq!(db_service.image.as_deref(), Some("postgres:13"));
}

#[test]
fn test_env_substitution_escaped() {
    let yaml = r#"
version: '3'
services:
  web:
    image: nginx:latest
    environment:
      - VAR=$$ESCAPED
"#;
    
    let env = HashMap::new();
    let result = parse_and_validate_with_env(yaml, &env).expect("Should parse and validate");
    
    // We don't have environment modeled in Service, but we just verify it doesn't fail parsing.
    // However, the test itself ensures no panic or error during parsing due to substitution logic
}

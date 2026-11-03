use observability::{alerts::*, incidents::*, metrics::*, probes::*};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use std::io::{Read, Write};

#[test]
fn test_metrics_registry() {
    let registry = MetricsRegistry::new().unwrap();
    registry.cpu_usage.set(42.5);

    use prometheus::Encoder;
    let mut buffer = vec![];
    let encoder = prometheus::TextEncoder::new();
    let metric_families = registry.registry.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert!(output.contains("host_cpu_usage_percent 42.5"));
}

#[test]
fn test_incident_states() {
    let mut incident = Incident::new("inc-1".into(), "Test".into(), 100);
    assert_eq!(incident.state, IncidentState::Triggered);

    assert!(incident.acknowledge(101).is_ok());
    assert_eq!(incident.state, IncidentState::Acknowledged);

    assert!(incident.resolve(102).is_ok());
    assert_eq!(incident.state, IncidentState::Resolved);

    assert!(incident.acknowledge(103).is_err());
}

#[test]
fn test_tcp_probe() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("127.0.0.1:{}", port);

    thread::spawn(move || {
        for _ in listener.incoming() {
            // just accept and close
        }
    });

    let result = check_tcp(&addr, 1);
    assert!(result.success);
}

#[test]
fn test_tcp_probe_fail() {
    // Port 0 cannot be connected to, so this should fail
    let result = check_tcp("127.0.0.1:0", 1);
    assert!(!result.success);
}

#[test]
fn test_tcp_probe_multiple_candidates() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("127.0.0.1:{}", port);

    thread::spawn(move || {
        for _ in listener.incoming() {
            // just accept and close
        }
    });

    let result = check_tcp(&addr, 2);
    assert!(result.success);
}

#[test]
fn test_http_probe() {
    // Simple test for HTTP probe against a known failure domain
    let result = check_http("http://127.0.0.1:0", 1);
    assert!(!result.success);
}

#[test]
fn test_http_probe_success() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 1024];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    // Give the server a moment to start
    thread::sleep(Duration::from_millis(50));

    let result = check_http(&url, 1);
    assert!(result.success);
    assert_eq!(result.status_code, Some(200));
}

#[test]
fn test_alert_serialize() {
    let alert = Alert {
        id: "a1".into(),
        title: "Test Alert".into(),
        description: "Something is wrong".into(),
        severity: AlertSeverity::Critical,
        source: "probe-1".into(),
        timestamp: 12345,
    };

    let json = serde_json::to_string(&alert).unwrap();
    assert!(json.contains("Critical"));

    let parsed: Alert = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.id, alert.id);
    assert_eq!(parsed.severity, AlertSeverity::Critical);
}

#[test]
fn test_webhook_delivery_success() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}/alert", port);

    let (tx, rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request_str = String::from_utf8_lossy(&buf[..n]).to_string();

            tx.send(request_str).unwrap();

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    thread::sleep(Duration::from_millis(50));

    let alert = Alert {
        id: "test-alert".into(),
        title: "Webhook Test".into(),
        description: "Testing delivery".into(),
        severity: AlertSeverity::Info,
        source: "tests".into(),
        timestamp: 12345,
    };

    let delivery = WebhookDelivery { url };
    assert!(delivery.deliver(&alert).is_ok());

    let received_request = rx.recv_timeout(Duration::from_secs(2)).unwrap();

    // Assert HTTP method and path
    assert!(received_request.starts_with("POST /alert HTTP/1.1\r\n"));

    // Assert Headers
    assert!(received_request.to_lowercase().contains("content-type: application/json"));

    // Assert JSON payload structure
    let body_start = received_request.find("\r\n\r\n").unwrap() + 4;
    let body = &received_request[body_start..];
    let parsed: Alert = serde_json::from_str(body).unwrap();
    assert_eq!(parsed.id, "test-alert");
    assert_eq!(parsed.title, "Webhook Test");
    assert_eq!(parsed.description, "Testing delivery");
    assert_eq!(parsed.severity, AlertSeverity::Info);
    assert_eq!(parsed.source, "tests");
    assert_eq!(parsed.timestamp, 12345);
}

#[test]
fn test_webhook_delivery_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}/alert", port);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 1024];
            let _ = stream.read(&mut buf);
            // Simulate 500 Internal Server Error
            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nError";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    thread::sleep(Duration::from_millis(50));

    let alert = Alert {
        id: "fail-alert".into(),
        title: "Webhook Fail".into(),
        description: "Testing failure".into(),
        severity: AlertSeverity::Warning,
        source: "tests".into(),
        timestamp: 67890,
    };

    let delivery = WebhookDelivery { url };
    let res = delivery.deliver(&alert);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("status: 500"));
}

#[test]
fn test_alert_thresholds() {
    let rule_gt = AlertRule {
        metric_name: "cpu".into(),
        threshold: 90.0,
        operator: AlertOperator::GreaterThan,
        severity: AlertSeverity::Critical,
    };

    assert!(!rule_gt.evaluate(85.0));
    assert!(rule_gt.evaluate(95.0));

    let rule_lt = AlertRule {
        metric_name: "disk_free".into(),
        threshold: 10.0,
        operator: AlertOperator::LessThan,
        severity: AlertSeverity::Warning,
    };

    assert!(rule_lt.evaluate(5.0));
    assert!(!rule_lt.evaluate(15.0));
}

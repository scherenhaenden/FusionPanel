#![forbid(unsafe_code)]

pub mod alert_notifier;
pub mod collector;
pub mod cpu_anomaly;
pub mod governor;
pub mod incident;
pub mod prometheus;
pub mod rules;

#[cfg(test)]
mod tests {
    use super::collector::{HostMetrics, MetricCollector};
    use super::incident::{Incident, IncidentState};
    use super::rules::{MetricType, Operator, Rule, RuleEngine};

    #[test]
    fn test_threshold_evaluation_math() {
        let metrics = HostMetrics {
            cpu_usage_percent: 85.0,
            memory_usage_percent: 92.0,
            disk_usage_percent: 95.0,
            network_rx_bytes: 1000,
            network_tx_bytes: 500,
        };

        let rule1 = Rule {
            id: "cpu_alert".to_string(),
            metric_type: MetricType::Cpu,
            operator: Operator::GreaterThan,
            threshold_percent: 80.0,
        };

        let rule2 = Rule {
            id: "mem_alert".to_string(),
            metric_type: MetricType::Memory,
            operator: Operator::GreaterThan,
            threshold_percent: 95.0,
        };

        assert!(rule1.evaluate(&metrics));
        assert!(!rule2.evaluate(&metrics));

        let mut engine = RuleEngine::new();
        engine.add_rule(rule1.clone());
        engine.add_rule(rule2.clone());

        let violated = engine.evaluate_all(&metrics);
        assert_eq!(violated.len(), 1);
        assert_eq!(violated[0].id, "cpu_alert");
    }

    #[test]
    fn test_incident_state_transitions() {
        let mut incident = Incident::new("rule_1".to_string());
        assert_eq!(incident.state, IncidentState::Open);

        assert!(incident.acknowledge().is_ok());
        assert_eq!(incident.state, IncidentState::Acknowledged);

        // Cannot ack twice
        assert!(incident.acknowledge().is_err());

        assert!(incident.resolve().is_ok());
        assert_eq!(incident.state, IncidentState::Resolved);

        // Cannot resolve again
        assert!(incident.resolve().is_err());
    }

    #[test]
    fn test_collector_fail_closed_if_proc_missing() {
        // We'll just test that initialization fails if /proc doesn't exist.
        // Assuming we are in a normal linux container, /proc/stat exists.
        // Let's create an instance to verify it works normally.
        let collector_res = MetricCollector::new();

        if std::path::Path::new("/proc/stat").exists() {
            assert!(collector_res.is_ok());
            let mut collector = collector_res.unwrap();
            let metrics = collector.collect();
            assert!(metrics.is_ok());
        } else {
            let err = collector_res.unwrap_err();
            assert!(err.starts_with("BLOCKED:"));
        }
    }
}

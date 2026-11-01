use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Alert {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub source: String,
    pub timestamp: u64,
}

pub trait AlertDelivery {
    fn deliver(&self, alert: &Alert) -> Result<(), String>;
}

pub struct WebhookDelivery {
    pub url: String,
}

impl AlertDelivery for WebhookDelivery {
    fn deliver(&self, alert: &Alert) -> Result<(), String> {
        let client = reqwest::blocking::Client::new();
        match client.post(&self.url).json(alert).send() {
            Ok(res) if res.status().is_success() => Ok(()),
            Ok(res) => Err(format!("Webhook failed with status: {}", res.status())),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct AlertRule {
    pub metric_name: String,
    pub threshold: f64,
    pub operator: AlertOperator,
    pub severity: AlertSeverity,
}

pub enum AlertOperator {
    GreaterThan,
    LessThan,
    Equal,
}

impl AlertRule {
    pub fn evaluate(&self, value: f64) -> bool {
        match self.operator {
            AlertOperator::GreaterThan => value > self.threshold,
            AlertOperator::LessThan => value < self.threshold,
            AlertOperator::Equal => (value - self.threshold).abs() < f64::EPSILON,
        }
    }
}

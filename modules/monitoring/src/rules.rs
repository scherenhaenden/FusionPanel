use crate::collector::HostMetrics;

#[derive(Debug, Clone, PartialEq)]
pub enum MetricType {
    Cpu,
    Memory,
    Disk,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub metric_type: MetricType,
    pub operator: Operator,
    pub threshold_percent: f32,
}

impl Rule {
    pub fn evaluate(&self, metrics: &HostMetrics) -> bool {
        let value = match self.metric_type {
            MetricType::Cpu => metrics.cpu_usage_percent,
            MetricType::Memory => metrics.memory_usage_percent,
            MetricType::Disk => metrics.disk_usage_percent,
        };

        match self.operator {
            Operator::GreaterThan => value > self.threshold_percent,
            Operator::LessThan => value < self.threshold_percent,
            Operator::GreaterThanOrEqual => value >= self.threshold_percent,
            Operator::LessThanOrEqual => value <= self.threshold_percent,
            Operator::Equal => (value - self.threshold_percent).abs() < f32::EPSILON,
        }
    }
}

pub struct RuleEngine {
    pub rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn evaluate_all(&self, metrics: &HostMetrics) -> Vec<&Rule> {
        self.rules.iter().filter(|r| r.evaluate(metrics)).collect()
    }
}

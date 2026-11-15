#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadPoolMetrics {
    pub idle_threads: u32,
    pub active_threads: u32,
    pub queued_requests: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadPoolState {
    Healthy,
    WorkerStarvation,
    QueueLatencySpike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadPoolGuard {
    pub metrics: ThreadPoolMetrics,
    pub state: ThreadPoolState,
    pub recommended_size: Option<u32>,
}

impl ThreadPoolMetrics {
    pub fn parse(variables: &[(String, String)]) -> Self {
        let mut metrics = ThreadPoolMetrics::default();

        for (key, value) in variables {
            match key.as_str() {
                "ThreadPool_idle_threads" => {
                    metrics.idle_threads = value.parse().unwrap_or(0);
                }
                "ThreadPool_active_threads" => {
                    metrics.active_threads = value.parse().unwrap_or(0);
                }
                "ThreadPool_queued_requests" => {
                    metrics.queued_requests = value.parse().unwrap_or(0);
                }
                _ => {}
            }
        }

        metrics
    }
}

impl ThreadPoolGuard {
    pub fn evaluate(metrics: ThreadPoolMetrics) -> Self {
        let mut state = ThreadPoolState::Healthy;
        let mut recommended_size = None;

        if metrics.active_threads > 0 && metrics.idle_threads == 0 && metrics.queued_requests > 0 {
            state = ThreadPoolState::WorkerStarvation;
            recommended_size = Some(metrics.active_threads + metrics.queued_requests.min(16)); // Recommend scaling up
        } else if metrics.active_threads > 0 && metrics.queued_requests > metrics.active_threads * 2 {
            // Threshold for latency spike
            state = ThreadPoolState::QueueLatencySpike;
        }

        Self {
            metrics,
            state,
            recommended_size,
        }
    }
}

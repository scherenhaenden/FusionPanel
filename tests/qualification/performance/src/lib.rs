#![forbid(unsafe_code)]

/// The core types and helpers for performance testing.
pub mod runner {
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct TestResult {
        pub duration: Duration,
        pub success_count: usize,
        pub error_count: usize,
        pub latencies: Vec<Duration>,
    }

    impl TestResult {
        fn percentile(&self, p: f64) -> Option<Duration> {
            if self.latencies.is_empty() {
                return None;
            }
            let idx = ((self.latencies.len() - 1) as f64 * p) as usize;
            Some(self.latencies[idx])
        }

        pub fn p50(&self) -> Option<Duration> { self.percentile(0.50) }
        pub fn p95(&self) -> Option<Duration> { self.percentile(0.95) }
        pub fn p99(&self) -> Option<Duration> { self.percentile(0.99) }
    }

    pub async fn run_load_test<F, Fut>(
        concurrency: usize,
        requests_per_worker: usize,
        task: F,
    ) -> TestResult
    where
        F: FnMut() -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = Result<(), ()>> + Send,
    {
        let start = Instant::now();
        let mut handles = Vec::new();

        for _ in 0..concurrency {
            let mut task_clone = task.clone();
            let handle = tokio::spawn(async move {
                let mut success = 0;
                let mut error = 0;
                let mut latencies = Vec::with_capacity(requests_per_worker);
                for _ in 0..requests_per_worker {
                    let task_start = Instant::now();
                    if task_clone().await.is_ok() {
                        success += 1;
                        latencies.push(task_start.elapsed());
                    } else {
                        error += 1;
                    }
                }
                (success, error, latencies)
            });
            handles.push(handle);
        }

        let mut total_success = 0;
        let mut total_error = 0;
        let mut all_latencies = Vec::with_capacity(concurrency * requests_per_worker);

        for handle in handles {
            if let Ok((s, e, mut l)) = handle.await {
                total_success += s;
                total_error += e;
                all_latencies.append(&mut l);
            } else {
                total_error += requests_per_worker;
            }
        }
        
        all_latencies.sort();

        TestResult {
            duration: start.elapsed(),
            success_count: total_success,
            error_count: total_error,
            latencies: all_latencies,
        }
    }
}

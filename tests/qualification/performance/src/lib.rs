#![forbid(unsafe_code)]

/// The core types and helpers for performance testing.
pub mod runner {
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct TestResult {
        pub duration: Duration,
        pub success_count: usize,
        pub error_count: usize,
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
                for _ in 0..requests_per_worker {
                    if task_clone().await.is_ok() {
                        success += 1;
                    } else {
                        error += 1;
                    }
                }
                (success, error)
            });
            handles.push(handle);
        }

        let mut total_success = 0;
        let mut total_error = 0;

        for handle in handles {
            if let Ok((s, e)) = handle.await {
                total_success += s;
                total_error += e;
            } else {
                total_error += requests_per_worker;
            }
        }

        TestResult {
            duration: start.elapsed(),
            success_count: total_success,
            error_count: total_error,
        }
    }
}

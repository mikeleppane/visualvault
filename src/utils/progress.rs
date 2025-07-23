use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Progress {
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub started_at: Instant,
    pub is_complete: bool,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            current: 0,
            total: 0,
            message: String::new(),
            started_at: Instant::now(),
            is_complete: false,
        }
    }
}

impl Progress {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.current = 0;
        self.total = 0;
        self.message.clear();
        self.started_at = Instant::now();
        self.is_complete = false;
    }

    #[allow(dead_code)]
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
    }

    #[allow(dead_code)]
    pub fn set_current(&mut self, current: usize) {
        self.current = current;
        if self.current >= self.total && self.total > 0 {
            self.is_complete = true;
        }
    }

    #[allow(dead_code)]
    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
    #[must_use]
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    #[allow(clippy::missing_docs_in_private_items)]
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn eta(&self) -> Option<std::time::Duration> {
        if self.current == 0 || self.total == 0 {
            return None;
        }

        // If we've already completed or exceeded the total, return 0 duration
        if self.current >= self.total {
            return Some(std::time::Duration::from_secs(0));
        }

        let elapsed = self.elapsed().as_secs_f64();
        let rate = self.current as f64 / elapsed;

        // Avoid division by zero
        if rate == 0.0 {
            return None;
        }

        let remaining = (self.total - self.current) as f64 / rate;

        Some(std::time::Duration::from_secs_f64(remaining))
    }
}

// ...existing code...

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_new_progress() {
        let progress = Progress::new();
        assert_eq!(progress.current, 0);
        assert_eq!(progress.total, 0);
        assert!(progress.message.is_empty());
        assert!(!progress.is_complete);
    }

    #[test]
    fn test_default_progress() {
        let progress = Progress::default();
        assert_eq!(progress.current, 0);
        assert_eq!(progress.total, 0);
        assert!(progress.message.is_empty());
        assert!(!progress.is_complete);
    }

    #[test]
    fn test_reset() {
        let mut progress = Progress::new();

        // Set some values
        progress.set_total(100);
        progress.set_current(50);
        progress.set_message("Processing...".to_string());
        progress.is_complete = true;

        // Reset
        progress.reset();

        // Verify all fields are reset
        assert_eq!(progress.current, 0);
        assert_eq!(progress.total, 0);
        assert!(progress.message.is_empty());
        assert!(!progress.is_complete);

        // Verify started_at is updated (should be very recent)
        assert!(progress.elapsed().as_millis() < 10);
    }

    #[test]
    fn test_set_total() {
        let mut progress = Progress::new();

        progress.set_total(100);
        assert_eq!(progress.total, 100);

        progress.set_total(200);
        assert_eq!(progress.total, 200);

        progress.set_total(0);
        assert_eq!(progress.total, 0);
    }

    #[test]
    fn test_set_current() {
        let mut progress = Progress::new();
        progress.set_total(100);

        // Normal update
        progress.set_current(50);
        assert_eq!(progress.current, 50);
        assert!(!progress.is_complete);

        // Update to total
        progress.set_current(100);
        assert_eq!(progress.current, 100);
        assert!(progress.is_complete);

        // Update beyond total
        progress.set_current(150);
        assert_eq!(progress.current, 150);
        assert!(progress.is_complete);
    }

    #[test]
    fn test_set_current_with_zero_total() {
        let mut progress = Progress::new();

        // When total is 0, is_complete should remain false
        progress.set_current(50);
        assert_eq!(progress.current, 50);
        assert!(!progress.is_complete);
    }

    #[test]
    fn test_set_message() {
        let mut progress = Progress::new();

        progress.set_message("Starting...".to_string());
        assert_eq!(progress.message, "Starting...");

        progress.set_message("Processing file 1".to_string());
        assert_eq!(progress.message, "Processing file 1");

        progress.set_message(String::new());
        assert!(progress.message.is_empty());
    }

    #[test]
    fn test_percentage_calculation() {
        let mut progress = Progress::new();

        // Zero total
        assert_eq!(progress.percentage(), 0.0);

        // Normal cases
        progress.set_total(100);
        progress.set_current(0);
        assert_eq!(progress.percentage(), 0.0);

        progress.set_current(25);
        assert_eq!(progress.percentage(), 25.0);

        progress.set_current(50);
        assert_eq!(progress.percentage(), 50.0);

        progress.set_current(75);
        assert_eq!(progress.percentage(), 75.0);

        progress.set_current(100);
        assert_eq!(progress.percentage(), 100.0);

        // Beyond total
        progress.set_current(150);
        assert_eq!(progress.percentage(), 150.0);
    }

    #[test]
    fn test_percentage_with_non_round_numbers() {
        let mut progress = Progress::new();

        progress.set_total(3);
        progress.set_current(1);
        assert!((progress.percentage() - 33.333_333_333_333_336).abs() < 0.0001);

        progress.set_current(2);
        assert!((progress.percentage() - 66.666_666_666_666_67).abs() < 0.0001);
    }

    #[test]
    fn test_elapsed() {
        let progress = Progress::new();

        // Sleep for a short time
        thread::sleep(Duration::from_millis(50));

        let elapsed = progress.elapsed();
        assert!(elapsed.as_millis() >= 50);
        assert!(elapsed.as_millis() < 100); // Should not take too long
    }

    #[test]
    fn test_eta_calculation() {
        let mut progress = Progress::new();

        // No progress yet
        assert!(progress.eta().is_none());

        progress.set_total(100);
        assert!(progress.eta().is_none()); // Still no current progress

        // Simulate some progress with time
        thread::sleep(Duration::from_millis(100));
        progress.set_current(10);

        let eta = progress.eta();
        assert!(eta.is_some());

        // With 10% done in ~100ms, ETA should be around 900ms
        // Allow for some variance in timing
        let eta_ms = eta.unwrap().as_millis();
        assert!(eta_ms > 500, "ETA should be reasonable: {eta_ms} ms");
        assert!(eta_ms < 2000, "ETA should not be too high: {eta_ms} ms");
    }

    #[test]
    fn test_eta_edge_cases() {
        let mut progress = Progress::new();

        // Zero total
        progress.set_total(0);
        progress.set_current(10);
        assert!(progress.eta().is_none());

        // Complete
        progress.set_total(100);
        progress.set_current(100);
        let eta = progress.eta();
        assert!(eta.is_some());
        assert_eq!(eta.unwrap().as_secs(), 0);

        // Beyond complete
        progress.set_current(150);
        let eta = progress.eta();
        assert!(eta.is_some());
        // Should return 0 or negative time (clamped to 0)
    }

    #[test]
    fn test_eta_with_fast_progress() {
        let mut progress = Progress::new();
        progress.set_total(1000);

        // Simulate very fast progress
        thread::sleep(Duration::from_millis(10));
        progress.set_current(500);

        let eta = progress.eta();
        assert!(eta.is_some());

        // Should complete quickly
        let eta_ms = eta.unwrap().as_millis();
        assert!(eta_ms < 50, "Fast progress should have low ETA: {eta_ms} ms");
    }

    #[test]
    fn test_clone() {
        let mut progress = Progress::new();
        progress.set_total(100);
        progress.set_current(50);
        progress.set_message("Test message".to_string());

        let cloned = progress.clone();

        assert_eq!(cloned.current, progress.current);
        assert_eq!(cloned.total, progress.total);
        assert_eq!(cloned.message, progress.message);
        assert_eq!(cloned.is_complete, progress.is_complete);

        // Verify it's a deep clone
        progress.set_current(75);
        assert_eq!(cloned.current, 50); // Cloned value should not change
    }

    #[test]
    fn test_debug_impl() {
        let mut progress = Progress::new();
        progress.set_total(100);
        progress.set_current(50);
        progress.set_message("Processing".to_string());

        let debug_str = format!("{progress:?}");
        assert!(debug_str.contains("current: 50"));
        assert!(debug_str.contains("total: 100"));
        assert!(debug_str.contains("message: \"Processing\""));
        assert!(debug_str.contains("is_complete: false"));
    }

    #[test]
    fn test_concurrent_usage() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};
        use std::thread;

        let progress = Arc::new(Mutex::new(Progress::new()));
        let counter = Arc::new(AtomicUsize::new(0));

        progress.lock().unwrap().set_total(100);

        let mut handles = vec![];

        // Spawn multiple threads that update progress based on shared work counter
        for i in 0..10 {
            let progress_clone = Arc::clone(&progress);
            let counter_clone = Arc::clone(&counter);

            let handle = thread::spawn(move || {
                for j in 0..10 {
                    // Simulate doing work
                    thread::sleep(std::time::Duration::from_millis(1));

                    // Atomically increment shared counter
                    let work_done = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;

                    // Update progress with the total work done
                    {
                        let mut prog = progress_clone.lock().unwrap();
                        prog.set_current(work_done);
                        prog.set_message(format!("Thread {i} completed item {j} (total: {work_done})"));
                        drop(prog); // Explicitly drop to release lock
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final state - should have exactly 100 work items completed
        let final_progress = progress.lock().unwrap();
        assert_eq!(final_progress.current, 100);
        assert!(final_progress.is_complete); // 100 >= 100, so complete
        assert_eq!(counter.load(Ordering::SeqCst), 100);
        drop(final_progress);
    }
}

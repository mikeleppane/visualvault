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

        let elapsed = self.elapsed().as_secs_f64();
        let rate = self.current as f64 / elapsed;
        let remaining = (self.total - self.current) as f64 / rate;

        Some(std::time::Duration::from_secs_f64(remaining))
    }
}

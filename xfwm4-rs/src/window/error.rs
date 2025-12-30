use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, warn, debug};

/// Error tracking for X11 operations
pub struct ErrorTracker {
    x11_errors: AtomicU64,
    compositor_errors: AtomicU64,
    window_errors: AtomicU64,
}

impl ErrorTracker {
    pub fn new() -> Self {
        Self {
            x11_errors: AtomicU64::new(0),
            compositor_errors: AtomicU64::new(0),
            window_errors: AtomicU64::new(0),
        }
    }

    pub fn record_x11_error(&self, operation: &str, error: impl std::fmt::Display) {
        self.x11_errors.fetch_add(1, Ordering::Relaxed);
        error!("X11 error in {}: {}", operation, error);
    }

    pub fn record_compositor_error(&self, operation: &str, error: impl std::fmt::Display) {
        self.compositor_errors.fetch_add(1, Ordering::Relaxed);
        error!("Compositor error in {}: {}", operation, error);
    }

    pub fn record_window_error(&self, operation: &str, error: impl std::fmt::Display) {
        self.window_errors.fetch_add(1, Ordering::Relaxed);
        error!("Window management error in {}: {}", operation, error);
    }

    pub fn warn_if_failed<T, E: std::fmt::Display>(
        &self,
        result: Result<T, E>,
        operation: &str,
        category: ErrorCategory,
    ) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                match category {
                    ErrorCategory::X11 => self.record_x11_error(operation, e),
                    ErrorCategory::Compositor => self.record_compositor_error(operation, e),
                    ErrorCategory::Window => self.record_window_error(operation, e),
                }
                None
            }
        }
    }

    pub fn health_check(&self) -> HealthStatus {
        let x11 = self.x11_errors.load(Ordering::Relaxed);
        let comp = self.compositor_errors.load(Ordering::Relaxed);
        let win = self.window_errors.load(Ordering::Relaxed);
        
        HealthStatus {
            x11_errors: x11,
            compositor_errors: comp,
            window_errors: win,
            is_healthy: x11 < 10 && comp < 5 && win < 10,
        }
    }
}

pub enum ErrorCategory {
    X11,
    Compositor,
    Window,
}

pub struct HealthStatus {
    pub x11_errors: u64,
    pub compositor_errors: u64,
    pub window_errors: u64,
    pub is_healthy: bool,
}

impl Default for ErrorTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Log and ignore X11 errors (for cleanup operations)
pub fn log_and_ignore<T, E: std::fmt::Display>(result: Result<T, E>, operation: &str) {
    if let Err(e) = result {
        debug!("Ignoring error in {}: {}", operation, e);
    }
}

/// Log warning for non-critical errors
pub fn log_warn<T, E: std::fmt::Display>(result: Result<T, E>, operation: &str) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("Warning in {}: {}", operation, e);
            None
        }
    }
}



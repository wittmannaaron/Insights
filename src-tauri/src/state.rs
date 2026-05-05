use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct AppState {
    focused: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_focused(&self, v: bool) {
        self.focused.store(v, Ordering::Relaxed);
    }

    pub fn is_focused(&self) -> bool {
        self.focused.load(Ordering::Relaxed)
    }

    pub fn sample_interval(&self) -> Duration {
        if self.is_focused() {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(2000)
        }
    }
}

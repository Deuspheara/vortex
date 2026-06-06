use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AgentError;

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn check_cancelled(&self) -> Result<(), AgentError> {
        if self.is_cancelled() {
            Err(AgentError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

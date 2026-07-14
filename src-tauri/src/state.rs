use std::{collections::HashSet, sync::Mutex};

use crate::utils::utc_now;

const MAX_RUNTIME_LOGS: usize = 500;

pub struct RuntimeState {
    pub worker_active: Mutex<bool>,
    pub cancel_requests: Mutex<HashSet<String>>,
    pub deleted_tasks: Mutex<HashSet<String>>,
    runtime_logs: Mutex<Vec<String>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            worker_active: Mutex::new(false),
            cancel_requests: Mutex::new(HashSet::new()),
            deleted_tasks: Mutex::new(HashSet::new()),
            runtime_logs: Mutex::new(Vec::new()),
        }
    }

    pub fn push_log(&self, event: &str, message: impl AsRef<str>) {
        let Ok(mut logs) = self.runtime_logs.lock() else {
            return;
        };
        logs.push(format!(
            "{} [{}] {}",
            utc_now(),
            event,
            message.as_ref().replace(['\n', '\r'], " ")
        ));
        let overflow = logs.len().saturating_sub(MAX_RUNTIME_LOGS);
        if overflow > 0 {
            logs.drain(0..overflow);
        }
    }

    pub fn logs_text(&self) -> String {
        self.runtime_logs
            .lock()
            .map(|logs| logs.join("\n"))
            .unwrap_or_default()
    }
}

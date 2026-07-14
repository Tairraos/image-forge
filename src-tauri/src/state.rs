use std::{collections::HashSet, sync::Mutex};

pub struct RuntimeState {
    pub worker_active: Mutex<bool>,
    pub cancel_requests: Mutex<HashSet<String>>,
    pub deleted_tasks: Mutex<HashSet<String>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            worker_active: Mutex::new(false),
            cancel_requests: Mutex::new(HashSet::new()),
            deleted_tasks: Mutex::new(HashSet::new()),
        }
    }
}

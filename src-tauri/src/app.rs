use crate::model::{CommandError, Operation, OperationStatus, SavedData, Snapshot};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::ipc::Channel;
use uuid::Uuid;

pub struct Core {
    snapshot: Snapshot,
    revision: u64,
    subscriber: Option<Channel<Snapshot>>,
    requests: HashMap<String, Vec<String>>,
    pub stopped: bool,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            snapshot: Snapshot {
                session_id: Uuid::new_v4().to_string(),
                revision: "0".into(),
                app_version: env!("CARGO_PKG_VERSION").into(),
                operations: vec![],
                saved_data: SavedData::Loading,
            },
            revision: 0,
            subscriber: None,
            requests: HashMap::new(),
            stopped: false,
        }
    }
}

impl Core {
    pub fn saved_data(&mut self, status: SavedData) {
        if !self.stopped {
            self.snapshot.saved_data = status;
            self.changed();
        }
    }
    pub fn watch(&mut self, channel: Channel<Snapshot>) -> Result<(), CommandError> {
        channel.send(self.snapshot.clone()).map_err(|_| {
            CommandError::new("connection_failed", "Could not connect to desktop state.")
        })?;
        self.subscriber = Some(channel);
        Ok(())
    }

    pub fn publish(&mut self) {
        if let Some(channel) = &self.subscriber
            && channel.send(self.snapshot.clone()).is_err()
        {
            self.subscriber = None;
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        self.snapshot.revision = self.revision.to_string();
        self.publish();
    }

    pub fn start(&mut self, request: &str) -> Result<(Vec<String>, bool), CommandError> {
        Uuid::parse_str(request).map_err(|_| {
            CommandError::new("invalid_request", "The diagnostic request ID is invalid.")
        })?;
        if let Some(ids) = self.requests.get(request) {
            return Ok((ids.clone(), false));
        }
        if self.snapshot.operations.iter().any(|op| {
            matches!(
                op.status,
                OperationStatus::Running | OperationStatus::Cancelling
            )
        }) {
            return Err(CommandError::new(
                "operation_running",
                "Wait for the current diagnostic or cancel it first.",
            ));
        }
        if self.stopped || self.requests.len() >= 256 {
            return Err(CommandError::new(
                "session_limit",
                "Restart Starframe before running another diagnostic.",
            ));
        }
        // Only the latest diagnostic is displayed; request IDs remain valid for this session.
        self.snapshot.operations = (1..=3)
            .map(|lane| Operation {
                id: Uuid::new_v4().to_string(),
                request_id: request.into(),
                label: format!("Diagnostic worker {lane}"),
                progress: 0,
                status: OperationStatus::Running,
                message: "Simulated transfer and memory hashing; no game files.".into(),
            })
            .collect();
        let ids: Vec<String> = self
            .snapshot
            .operations
            .iter()
            .map(|op| op.id.clone())
            .collect();
        self.requests.insert(request.into(), ids.clone());
        self.changed();
        Ok((ids, true))
    }

    pub fn cancel(&mut self, id: &str) -> Result<(), CommandError> {
        let op = self
            .snapshot
            .operations
            .iter_mut()
            .find(|op| op.id == id)
            .ok_or_else(|| {
                CommandError::new(
                    "missing_operation",
                    "This operation is no longer available.",
                )
            })?;
        if op.status == OperationStatus::Running {
            op.status = OperationStatus::Cancelling;
            op.message = "Stopping at the next work boundary.".into();
            self.changed();
        }
        Ok(())
    }

    pub fn advance(&mut self, ids: &[String], fail: bool) -> bool {
        if self.stopped {
            return false;
        }
        let mut changed = false;
        for op in &mut self.snapshot.operations {
            if !ids.contains(&op.id) {
                continue;
            }
            match op.status {
                OperationStatus::Cancelling => {
                    op.status = OperationStatus::Cancelled;
                    op.message = "Diagnostic cancelled. No files were changed.".into();
                    changed = true;
                }
                OperationStatus::Running => {
                    op.progress += 1;
                    if fail && op.progress == 50 {
                        op.status = OperationStatus::Failed;
                        op.message =
                            "Requested diagnostic failure. Run another check to retry.".into();
                    } else if op.progress == 100 {
                        op.status = OperationStatus::Completed;
                        op.message =
                            "Diagnostic completed. No mods were downloaded or applied.".into();
                    }
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            self.changed();
        }
        self.snapshot.operations.iter().any(|op| {
            ids.contains(&op.id)
                && matches!(
                    op.status,
                    OperationStatus::Running | OperationStatus::Cancelling
                )
        })
    }
}

pub type Shared = Arc<Mutex<Core>>;

pub fn start_worker(state: Shared, ids: Vec<String>, fail: bool) {
    tauri::async_runtime::spawn_blocking(move || {
        use std::hash::{Hash, Hasher};
        let bytes = vec![0x5au8; 8 * 1024 * 1024];
        loop {
            // Bounded CPU work runs outside the state lock and the webview thread.
            for _ in 0..3 {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                bytes.hash(&mut hasher);
                std::hint::black_box(hasher.finish());
            }
            std::thread::sleep(Duration::from_millis(100));
            if !state.lock().expect("state lock").advance(&ids, fail) {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_do_not_start_another_job_and_conflicts_are_rejected() {
        let mut app = Core::default();
        let request = Uuid::new_v4().to_string();
        let (ids, start) = app.start(&request).unwrap();
        assert!(start);
        assert_eq!(app.start(&request).unwrap(), (ids.clone(), false));
        assert!(app.start(&Uuid::new_v4().to_string()).is_err());
        assert!(app.start("../bad").is_err());
        for _ in 0..100 {
            app.advance(&ids, false);
        }
        assert!(
            app.snapshot
                .operations
                .iter()
                .all(|op| op.status == OperationStatus::Completed)
        );
        assert!(!app.start(&request).unwrap().1);
    }

    #[test]
    fn cancellation_waits_for_the_worker_and_failure_stays_visible() {
        let mut app = Core::default();
        let (ids, _) = app.start(&Uuid::new_v4().to_string()).unwrap();
        app.cancel(&ids[0]).unwrap();
        assert_eq!(
            app.snapshot.operations[0].status,
            OperationStatus::Cancelling
        );
        for _ in 0..50 {
            app.advance(&ids, true);
        }
        assert_eq!(
            app.snapshot.operations[0].status,
            OperationStatus::Cancelled
        );
        assert_eq!(app.snapshot.operations[1].status, OperationStatus::Failed);
        let revision = app.revision;
        assert!(!app.advance(&ids, false));
        assert_eq!(app.revision, revision);
        assert!(app.cancel("unknown").is_err());
    }

    #[test]
    fn a_replacement_subscription_gets_current_state_and_retires_the_old_one() {
        let mut app = Core::default();
        let first = Arc::new(Mutex::new(Vec::new()));
        let sink = first.clone();
        app.watch(Channel::new(move |body| {
            sink.lock().unwrap().push(body);
            Ok(())
        }))
        .unwrap();
        let (ids, _) = app.start(&Uuid::new_v4().to_string()).unwrap();
        let previous = first.lock().unwrap().len();
        let second = Arc::new(Mutex::new(Vec::new()));
        let sink = second.clone();
        app.watch(Channel::new(move |body| {
            sink.lock().unwrap().push(body);
            Ok(())
        }))
        .unwrap();
        app.advance(&ids, false);
        assert_eq!(first.lock().unwrap().len(), previous);
        assert_eq!(second.lock().unwrap().len(), 2);
        app.stopped = true;
        assert!(!app.advance(&ids, false));
    }
}

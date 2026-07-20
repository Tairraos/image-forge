use std::{fs, path::PathBuf};

use image_forge_lib::integration_checks::{
    verify_cancellation_recovery, verify_network_failure, verify_reference_cleanup,
    verify_session_and_task_group_recovery, verify_tool_call_loop,
};
use uuid::Uuid;

fn integration_root(label: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("integration-tests")
        .join(format!("{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
}

fn recycle(root: &PathBuf) {
    if root.exists() {
        trash::delete(root).unwrap();
    }
}

#[test]
fn session_and_task_group_recover_across_persistence_boundary() {
    let root = integration_root("session-recovery");
    verify_session_and_task_group_recovery(&root).unwrap();
    recycle(&root);
}

#[test]
fn tool_call_loop_parses_call_and_followup_response() {
    verify_tool_call_loop().unwrap();
}

#[test]
fn cancellation_and_waiting_queue_recover_together() {
    let root = integration_root("cancellation");
    verify_cancellation_recovery(&root).unwrap();
    recycle(&root);
}

#[test]
fn network_failure_is_reported_as_agent_error() {
    verify_network_failure().unwrap();
}

#[test]
fn referenced_image_survives_cleanup_and_orphan_is_recycled() {
    let root = integration_root("reference-cleanup");
    verify_reference_cleanup(&root).unwrap();
    recycle(&root);
}

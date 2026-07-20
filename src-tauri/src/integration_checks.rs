//! 供 `src-tauri/tests/` 使用的跨模块场景入口。

use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;

use crate::{
    models::{AgentMessage, AgentTaskGroupSummary, ApiProvider, QueueState, AGENT_SCHEMA_VERSION},
    services::{
        agent_store::{create_session, recover_sessions},
        chat::{agent_completion, parse_agent_tool_response},
        queue::recover_stale_records,
        references::{prune_unreferenced_files_with_data, scan_orphan_files},
    },
    store::{fallback_failed_record, request_path, write_agent_session},
    utils::utc_now,
};

pub fn verify_session_and_task_group_recovery(root: &Path) -> Result<(), String> {
    let mut session = create_session(root, "integration-chat")?;
    session.status = "tool_running".into();
    session.messages.push(AgentMessage {
        id: "message-running-group".into(),
        role: "assistant".into(),
        status: "running".into(),
        content: String::new(),
        task_group: Some(AgentTaskGroupSummary {
            schema_version: AGENT_SCHEMA_VERSION,
            id: "group-1".into(),
            skill_content_hash: String::new(),
            task_ids: vec!["task-1".into()],
            titles: Vec::new(),
            prompt_summaries: Vec::new(),
            status: "running".into(),
        }),
        attachments: Vec::new(),
        tool_call: None,
        questions: Vec::new(),
        skill_id: String::new(),
        skill_content_hash: String::new(),
        error: String::new(),
        created_at: utc_now(),
    });
    write_agent_session(root, &session)?;

    let recovered = recover_sessions(root)?;
    let session = recovered
        .into_iter()
        .find(|item| item.id == session.id)
        .ok_or_else(|| "恢复后找不到会话".to_string())?;
    if session.status != "interrupted" {
        return Err(format!("会话恢复状态错误：{}", session.status));
    }
    let group = session.messages[0]
        .task_group
        .as_ref()
        .ok_or_else(|| "恢复后任务组丢失".to_string())?;
    if group.id != "group-1" || group.task_ids != ["task-1"] {
        return Err("恢复后任务组内容不一致".into());
    }
    Ok(())
}

pub fn verify_tool_call_loop() -> Result<(), String> {
    let first = parse_agent_tool_response(
        r#"{"choices":[{"message":{"content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"list_skills","arguments":"{\"query\":\"水彩\"}"}}]}}]}"#,
        "non-stream",
        None,
        &mut |_| {},
    )?;
    if first.tool_calls.len() != 1 || first.tool_calls[0].name != "list_skills" {
        return Err("第一轮 Tool Call 解析失败".into());
    }
    let arguments: serde_json::Value = serde_json::from_str(&first.tool_calls[0].arguments)
        .map_err(|error| format!("Tool Call 参数无效：{error}"))?;
    if arguments["query"] != "水彩" {
        return Err("Tool Call 参数拼装错误".into());
    }

    let second = parse_agent_tool_response(
        r#"{"choices":[{"message":{"content":"已根据工具结果继续处理","tool_calls":[]}}]}"#,
        "non-stream",
        None,
        &mut |_| {},
    )?;
    if second.text != "已根据工具结果继续处理" || !second.tool_calls.is_empty() {
        return Err("Tool Call 后续轮次解析失败".into());
    }
    Ok(())
}

pub fn verify_cancellation_recovery(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join("requests")).map_err(|error| error.to_string())?;
    let mut queued = fallback_failed_record("queued-task", "running");
    queued.status = "running".into();
    fs::write(request_path(root, "queued-task"), b"{}").map_err(|error| error.to_string())?;
    let mut cancelling = fallback_failed_record("cancel-task", "running");
    cancelling.status = "cancelling".into();
    let mut history = vec![queued, cancelling];
    let mut queue = QueueState {
        waiting: vec!["cancel-task".into()],
        ..QueueState::default()
    };
    let changed = recover_stale_records(
        root,
        &mut history,
        &mut queue,
        &["queued-task".into(), "cancel-task".into()],
    );
    if !changed || history[0].status != "queued" || history[1].status != "cancelled" {
        return Err("取消或队列恢复状态错误".into());
    }
    if queue.waiting != ["queued-task"] {
        return Err("恢复后的等待队列错误".into());
    }
    Ok(())
}

pub fn verify_network_failure() -> Result<(), String> {
    let provider = ApiProvider {
        id: "network-failure".into(),
        base_url: "http://127.0.0.1:9/v1".into(),
        api_key: "integration-test-key".into(),
        image_model: "test-chat".into(),
        ..ApiProvider::default()
    };
    let result = tauri::async_runtime::block_on(agent_completion(
        &provider,
        &[json!({ "role": "user", "content": "hello" })],
        &[],
        None,
        |_| {},
    ));
    let error = result.err().ok_or_else(|| "断网请求意外成功".to_string())?;
    if !error.contains("Agent 请求失败") {
        return Err(format!("网络错误没有被归一化：{error}"));
    }
    Ok(())
}

pub fn verify_reference_cleanup(root: &Path) -> Result<(), String> {
    for directory in ["references", "outputs", "requests", "clipboard"] {
        fs::create_dir_all(root.join(directory)).map_err(|error| error.to_string())?;
    }
    fs::write(root.join("history.json"), "[]").map_err(|error| error.to_string())?;
    fs::write(root.join("queue.json"), r#"{"waiting":[],"running":[]}"#)
        .map_err(|error| error.to_string())?;
    fs::write(root.join("templates.json"), "[]").map_err(|error| error.to_string())?;
    let kept = root.join("references/kept.png");
    let orphan = root.join("references/orphan.png");
    fs::write(&kept, b"kept").map_err(|error| error.to_string())?;
    fs::write(&orphan, b"orphan").map_err(|error| error.to_string())?;
    fs::write(
        root.join("agent-session.json"),
        serde_json::to_vec(&BTreeMap::from([(
            "attachmentPath",
            kept.to_string_lossy().into_owned(),
        )]))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let candidates = scan_orphan_files(root)?;
    if candidates
        .iter()
        .any(|item| item.path == kept.to_string_lossy())
        || !candidates
            .iter()
            .any(|item| item.path == orphan.to_string_lossy())
    {
        return Err("引用扫描没有正确区分已用和孤岛文件".into());
    }
    prune_unreferenced_files_with_data(root, &[], &[])?;
    if !kept.is_file() || orphan.exists() {
        return Err("参考图清理结果错误".into());
    }
    Ok(())
}

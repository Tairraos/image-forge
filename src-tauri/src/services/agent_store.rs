use std::path::Path;

use uuid::Uuid;

use crate::{
    history_db,
    models::{AgentMessage, AgentSession, AgentTaskGroupSummary, AGENT_SCHEMA_VERSION},
    store::{
        agent_session_path, list_agent_sessions, read_agent_session, read_history,
        write_agent_session,
    },
    utils::{recycle_path, utc_now},
};

pub(crate) fn create_session(data_dir: &Path, provider_id: &str) -> Result<AgentSession, String> {
    let now = utc_now();
    let session = AgentSession {
        schema_version: AGENT_SCHEMA_VERSION,
        id: Uuid::new_v4().to_string(),
        title: "新对话".into(),
        model_provider_id: provider_id.trim().to_string(),
        messages: Vec::new(),
        summary: String::new(),
        status: "idle".into(),
        task_group_ids: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    write_agent_session(data_dir, &session)?;
    Ok(session)
}

pub(crate) fn sessions(data_dir: &Path) -> Result<Vec<AgentSession>, String> {
    list_agent_sessions(data_dir)
}

pub(crate) fn session(data_dir: &Path, session_id: &str) -> Result<AgentSession, String> {
    ensure_session_id(session_id)?;
    read_agent_session(data_dir, session_id)
}

pub(crate) fn delete_session(data_dir: &Path, session_id: &str) -> Result<(), String> {
    ensure_session_id(session_id)?;
    // 会话已迁移到 SQLite，删除以数据库行为准
    history_db::delete_agent_session(data_dir, session_id)?;
    // JSON 时代迁移遗留的会话文件已不再被读取；只有安全 ID（历史上是 UUID）
    // 才可能对应真实文件路径，其余 ID 直接跳过文件清理。
    if validate_legacy_session_id(session_id) {
        let path = agent_session_path(data_dir, session_id);
        if path.exists() {
            recycle_path(&path).map_err(|error| format!("将 Agent 会话移入回收站失败: {error}"))?;
        }
    }
    Ok(())
}

pub(crate) fn append_message(
    data_dir: &Path,
    session_id: &str,
    message: AgentMessage,
) -> Result<AgentSession, String> {
    let mut session = session(data_dir, session_id)?;
    if session.messages.is_empty() && message.role == "user" {
        session.title = title_from_message(&message.content);
    }
    session.messages.push(message);
    session.updated_at = utc_now();
    write_agent_session(data_dir, &session)?;
    Ok(session)
}

pub(crate) fn save_session(
    data_dir: &Path,
    mut session: AgentSession,
) -> Result<AgentSession, String> {
    ensure_session_id(&session.id)?;
    session.updated_at = utc_now();
    write_agent_session(data_dir, &session)?;
    Ok(session)
}

/// 队列 worker 在任务达到终态后调用：刷新会话内任务组摘要状态；
/// 整组到达 completed / failed 时追加一条 task_result 消息，让下一轮对话感知生图结果。
/// 重复调用是幂等的（同组只追加一次），并发场景由进程内互斥锁串行化。
pub(crate) fn record_agent_task_result(
    data_dir: &Path,
    session_id: &str,
    task_group_id: &str,
) -> Result<(), String> {
    if session_id.trim().is_empty() || task_group_id.trim().is_empty() {
        return Ok(());
    }
    static TASK_RESULT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = TASK_RESULT_LOCK
        .lock()
        .map_err(|_| "任务组回写状态锁定失败")?;
    let records = read_history(data_dir)?
        .into_iter()
        .filter(|record| record.task_group_id == task_group_id)
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Ok(());
    }
    let statuses = records
        .iter()
        .map(|record| record.status.as_str())
        .collect::<Vec<_>>();
    let group_status = summarize_group_status(&statuses);
    let mut session = session(data_dir, session_id)?;
    let mut changed = false;
    for message in &mut session.messages {
        if let Some(group) = &mut message.task_group {
            if group.id == task_group_id && group.status != group_status {
                group.status = group_status.clone();
                changed = true;
            }
        }
    }
    let terminal = matches!(group_status.as_str(), "completed" | "failed");
    let already_recorded = session.messages.iter().any(|message| {
        message.status == "task_result" && message.content.contains(task_group_id)
    });
    if terminal && !already_recorded {
        let succeeded = records
            .iter()
            .filter(|record| record.status == "completed")
            .count();
        let failed = records
            .iter()
            .filter(|record| record.status == "failed")
            .count();
        let content = if group_status == "completed" {
            let paths = records
                .iter()
                .flat_map(|record| record.outputs.iter().map(|output| output.path.clone()))
                .collect::<Vec<_>>();
            if paths.is_empty() {
                format!("[taskGroupId={task_group_id}] 绘图任务组已完成，共 {} 张", records.len())
            } else {
                format!(
                    "[taskGroupId={task_group_id}] 绘图任务组已完成，共 {} 张：{}",
                    paths.len(),
                    paths.join("、")
                )
            }
        } else {
            let first_error = records
                .iter()
                .find_map(|record| record.error.clone())
                .unwrap_or_default();
            format!(
                "[taskGroupId={task_group_id}] 绘图任务组未全部成功：成功 {succeeded} 张，失败 {failed} 张。{first_error}"
            )
        };
        session.messages.push(AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: "tool".into(),
            status: "task_result".into(),
            content,
            attachments: Vec::new(),
            tool_call: None,
            questions: Vec::new(),
            task_group: None,
            error: String::new(),
            created_at: utc_now(),
        });
        changed = true;
    }
    if changed {
        save_session(data_dir, session)?;
    }
    Ok(())
}

fn summarize_group_status(statuses: &[&str]) -> String {
    if statuses.is_empty() {
        return "missing".into();
    }
    if statuses.iter().any(|status| *status == "cancelling") {
        return "cancelling".into();
    }
    if statuses.iter().any(|status| *status == "running") {
        return "running".into();
    }
    if statuses.iter().any(|status| *status == "queued") {
        return "queued".into();
    }
    if statuses.iter().all(|status| *status == "completed") {
        return "completed".into();
    }
    if statuses.iter().any(|status| *status == "failed") {
        return "failed".into();
    }
    if statuses.iter().any(|status| *status == "cancelled") {
        return "cancelled".into();
    }
    "missing".into()
}
pub(crate) fn rename_session(
    data_dir: &Path,
    session_id: &str,
    title: &str,
) -> Result<AgentSession, String> {
    let mut session = session(data_dir, session_id)?;
    let title = title.trim();
    session.title = if title.is_empty() {
        "新对话".into()
    } else {
        title.chars().take(60).collect::<String>()
    };
    write_agent_session(data_dir, &session)?;
    Ok(session)
}

pub(crate) fn prepare_context(session: &mut AgentSession) -> Vec<AgentMessage> {
    const CONTEXT_CHAR_BUDGET: usize = 48_000;
    const PRIORITY_CHAR_BUDGET: usize = 16_000;
    const RECENT_MESSAGES: usize = 24;
    let mut selected = Vec::new();
    let mut priority_size = 0;
    for (index, message) in session.messages.iter().enumerate().rev() {
        if !is_unfinished_context(message) {
            continue;
        }
        let size = context_size(message);
        if priority_size + size <= PRIORITY_CHAR_BUDGET || selected.is_empty() {
            selected.push(index);
            priority_size += size;
        }
    }
    let mut total_size = priority_size;
    let mut recent_count = 0;
    for (index, message) in session.messages.iter().enumerate().rev() {
        if selected.contains(&index) {
            continue;
        }
        let size = context_size(message);
        if recent_count >= RECENT_MESSAGES
            || (total_size + size > CONTEXT_CHAR_BUDGET && recent_count > 0)
        {
            break;
        }
        selected.push(index);
        total_size += size;
        recent_count += 1;
    }
    selected.sort_unstable();
    let first_selected = selected.first().copied().unwrap_or(session.messages.len());
    if first_selected > 0 {
        session.summary = session.messages[..first_selected]
            .iter()
            .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|message| {
                let content = message.content.chars().take(160).collect::<String>();
                format!("{}: {}", message.role, content)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    selected
        .into_iter()
        .map(|index| session.messages[index].clone())
        .collect()
}

fn context_size(message: &AgentMessage) -> usize {
    message.content.chars().count() + 512
}

fn is_unfinished_context(message: &AgentMessage) -> bool {
    message
        .tool_call
        .as_ref()
        .is_some_and(|call| matches!(call.status.as_str(), "pending" | "running"))
        || message.task_group.as_ref().is_some_and(|group| {
            !matches!(
                group.status.as_str(),
                "completed" | "failed" | "cancelled" | "missing"
            )
        })
}

pub(crate) fn recover_sessions(data_dir: &Path) -> Result<Vec<AgentSession>, String> {
    let mut values = sessions(data_dir)?;

    for session in &values {
        validate_schema_version("会话", session.schema_version)?;
        for message in &session.messages {
            if let Some(tool_call) = &message.tool_call {
                validate_schema_version("Tool Call", tool_call.schema_version)?;
            }
            if let Some(task_group) = &message.task_group {
                validate_schema_version("任务组", task_group.schema_version)?;
            }
        }
    }

    for session in &mut values {
        let mut changed = false;
        if session.schema_version == 0 {
            session.schema_version = AGENT_SCHEMA_VERSION;
            changed = true;
        }
        for message in &mut session.messages {
            if let Some(tool_call) = &mut message.tool_call {
                if tool_call.schema_version == 0 {
                    tool_call.schema_version = AGENT_SCHEMA_VERSION;
                    changed = true;
                }
            }
            if let Some(task_group) = &mut message.task_group {
                if task_group.schema_version == 0 {
                    task_group.schema_version = AGENT_SCHEMA_VERSION;
                    changed = true;
                }
            }
        }
        if matches!(session.status.as_str(), "running" | "tool_running") {
            session.status = "interrupted".into();
            session.updated_at = utc_now();
            changed = true;
        }
        if changed {
            write_agent_session(data_dir, session)?;
        }
    }
    Ok(values)
}

fn validate_schema_version(kind: &str, version: u32) -> Result<(), String> {
    if version > AGENT_SCHEMA_VERSION {
        return Err(format!("不支持的 Agent {kind} schemaVersion：{version}"));
    }
    Ok(())
}

/// SQLite 时代会话 ID 不再要求 UUID（Web 端同步/导入的会话使用 `web-session-*` 等 ID），
/// 只要求非空且不能构造出文件路径。
fn ensure_session_id(value: &str) -> Result<(), String> {
    let id = value.trim();
    if id.is_empty()
        || id.len() > 200
        || id.contains('/')
        || id.contains('\\')
        || id.contains("..")
        || id.contains('\0')
    {
        return Err("Agent 会话 ID 无效".into());
    }
    Ok(())
}

/// 旧 JSON 文件按 ID 落盘，只有当年的 UUID 格式 ID 才可能对应真实文件。
fn validate_legacy_session_id(value: &str) -> bool {
    Uuid::parse_str(value.trim()).is_ok()
}

fn validate_session_id(value: &str) -> Result<(), String> {
    if validate_legacy_session_id(value) {
        Ok(())
    } else {
        Err("Agent 会话 ID 无效".into())
    }
}

fn title_from_message(content: &str) -> String {
    let title = content.lines().next().unwrap_or_default().trim();
    let shortened = title.chars().take(30).collect::<String>();
    if shortened.is_empty() {
        "新对话".into()
    } else {
        shortened
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn recover_sessions_marks_running_sessions_as_interrupted() {
        let data_dir = temp_data_dir("recover-sessions");
        let mut session = create_session(&data_dir, "chat-provider").unwrap();
        session.schema_version = 0;
        session.status = "running".into();
        session.messages.push(AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".into(),
            status: "running".into(),
            content: "处理中".into(),
            attachments: Vec::new(),
            tool_call: None,
            questions: Vec::new(),
            task_group: None,
            error: String::new(),
            created_at: utc_now(),
        });
        write_agent_session(&data_dir, &session).unwrap();

        let recovered = recover_sessions(&data_dir).unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].schema_version, 1);
        assert_eq!(recovered[0].status, "interrupted");
        assert_eq!(recovered[0].messages.len(), 1);

        let reread = read_agent_session(&data_dir, &session.id).unwrap();
        assert_eq!(reread.status, "interrupted");
        recycle(&data_dir);
    }

    #[test]
    fn non_uuid_sessions_can_be_read_and_deleted() {
        let data_dir = temp_data_dir("non-uuid-session");
        let mut created = create_session(&data_dir, "chat-provider").unwrap();
        // 模拟 Web 端同步 / 导入来的非 UUID 会话
        created.id = "web-session-1787487024202".into();
        write_agent_session(&data_dir, &created).unwrap();

        let loaded = session(&data_dir, "web-session-1787487024202").unwrap();
        assert_eq!(loaded.id, "web-session-1787487024202");

        delete_session(&data_dir, "web-session-1787487024202").unwrap();
        assert!(read_agent_session(&data_dir, "web-session-1787487024202").is_err());

        // 路径不安全 ID 仍然拒绝
        assert!(session(&data_dir, "../escape").is_err());
        assert!(session(&data_dir, "  ").is_err());
        recycle(&data_dir);
    }

    #[test]
    fn delete_session_removes_sqlite_row_and_recycles_legacy_file() {        let data_dir = temp_data_dir("delete-session");
        let session = create_session(&data_dir, "chat-provider").unwrap();
        // 模拟 JSON 时代迁移遗留的会话文件
        let legacy_path = agent_session_path(&data_dir, &session.id);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_path, "{}").unwrap();

        delete_session(&data_dir, &session.id).unwrap();
        assert!(read_agent_session(&data_dir, &session.id).is_err());
        assert!(!legacy_path.exists());

        let error = delete_session(&data_dir, &session.id).unwrap_err();
        assert!(error.contains("找不到 Agent 会话"));
        recycle(&data_dir);
    }

    #[test]
    fn recover_sessions_rejects_future_nested_schema_without_overwriting_file() {
        let data_dir = temp_data_dir("recover-future-schema");
        let mut session = create_session(&data_dir, "chat-provider").unwrap();
        session.messages.push(AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".into(),
            status: "completed".into(),
            content: String::new(),
            attachments: Vec::new(),
            tool_call: Some(crate::models::AgentToolCall {
                schema_version: AGENT_SCHEMA_VERSION + 1,
                id: "future-call".into(),
                name: "future_tool".into(),
                arguments: serde_json::Value::Null,
                result: None,
                error: None,
                status: "completed".into(),
                created_at: utc_now(),
                completed_at: None,
            }),
            questions: Vec::new(),
            task_group: None,
            error: String::new(),
            created_at: utc_now(),
        });
        write_agent_session(&data_dir, &session).unwrap();

        let error = recover_sessions(&data_dir).unwrap_err();
        assert!(error.contains("Tool Call"));
        let reread = read_agent_session(&data_dir, &session.id).unwrap();
        assert_eq!(
            reread.messages[0]
                .tool_call
                .as_ref()
                .unwrap()
                .schema_version,
            2
        );
        recycle(&data_dir);
    }

    #[test]
    fn task_result_recorded_once_when_group_reaches_terminal_status() {
        let data_dir = temp_data_dir("task-result");
        let session = create_session(&data_dir, "chat-provider").unwrap();
        let group_id = "group-result-x";
        append_message(
            &data_dir,
            &session.id,
            AgentMessage {
                id: Uuid::new_v4().to_string(),
                role: "tool".into(),
                status: "task_group".into(),
                content: "已创建 2 个绘图任务".into(),
                attachments: Vec::new(),
                tool_call: None,
                questions: Vec::new(),
                task_group: Some(AgentTaskGroupSummary {
                    schema_version: AGENT_SCHEMA_VERSION,
                    id: group_id.into(),
                    task_ids: vec!["t-1".into(), "t-2".into()],
                    titles: Vec::new(),
                    prompt_summaries: Vec::new(),
                    status: "queued".into(),
                }),
                error: String::new(),
                created_at: utc_now(),
            },
        )
        .unwrap();

        let mut first = crate::store::fallback_failed_record("t-1", "x");
        first.status = "completed".into();
        first.error = None;
        first.task_group_id = group_id.into();
        let mut second = crate::store::fallback_failed_record("t-2", "x");
        second.status = "completed".into();
        second.error = None;
        second.task_group_id = group_id.into();
        crate::store::write_history(&data_dir, &[first, second]).unwrap();

        record_agent_task_result(&data_dir, &session.id, group_id).unwrap();

        let saved = read_agent_session(&data_dir, &session.id).unwrap();
        let result_count = saved
            .messages
            .iter()
            .filter(|message| message.status == "task_result")
            .count();
        assert_eq!(result_count, 1);
        let group_message = saved
            .messages
            .iter()
            .find(|message| message.task_group.is_some())
            .unwrap();
        assert_eq!(group_message.task_group.as_ref().unwrap().status, "completed");
        let result_message = saved
            .messages
            .iter()
            .find(|message| message.status == "task_result")
            .unwrap();
        assert!(result_message.content.contains(group_id));

        // 幂等：重复回写不追加
        record_agent_task_result(&data_dir, &session.id, group_id).unwrap();
        let saved = read_agent_session(&data_dir, &session.id).unwrap();
        assert_eq!(
            saved
                .messages
                .iter()
                .filter(|message| message.status == "task_result")
                .count(),
            1
        );
        recycle(&data_dir);
    }

    #[test]
    fn prepare_context_keeps_recent_messages_and_builds_summary() {        let data_dir = temp_data_dir("prepare-context");
        let mut session = AgentSession {
            schema_version: AGENT_SCHEMA_VERSION,
            id: Uuid::new_v4().to_string(),
            title: String::new(),
            model_provider_id: String::new(),
            messages: (0..30)
                .map(|index| AgentMessage {
                    id: Uuid::new_v4().to_string(),
                    role: if index % 2 == 0 {
                        "user".into()
                    } else {
                        "assistant".into()
                    },
                    status: String::new(),
                    content: format!("第 {index} 条消息"),
                    attachments: Vec::new(),
                    tool_call: None,
                    questions: Vec::new(),
                    task_group: None,
                    error: String::new(),
                    created_at: utc_now(),
                })
                .collect(),
            summary: String::new(),
            status: "idle".into(),
            task_group_ids: Vec::new(),
            created_at: utc_now(),
            updated_at: utc_now(),
        };

        let context = prepare_context(&mut session);
        assert_eq!(context.len(), 24);
        assert!(!session.summary.is_empty());
        assert!(session.summary.contains("user: 第 0 条消息"));
        recycle(&data_dir);
    }

    fn temp_data_dir(name: &str) -> PathBuf {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("agent-store-tests")
            .join(format!("{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn recycle(path: &Path) {
        if path.exists() {
            let _ = trash::delete(path);
        }
    }
}

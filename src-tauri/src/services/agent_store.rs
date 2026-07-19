use std::path::Path;

use uuid::Uuid;

use crate::{
    models::{AgentMessage, AgentSession, AGENT_SCHEMA_VERSION},
    store::{agent_session_path, list_agent_sessions, read_agent_session, write_agent_session},
    utils::utc_now,
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
    validate_session_id(session_id)?;
    read_agent_session(data_dir, session_id)
}

pub(crate) fn delete_session(data_dir: &Path, session_id: &str) -> Result<(), String> {
    validate_session_id(session_id)?;
    let path = agent_session_path(data_dir, session_id);
    if !path.exists() {
        return Err("找不到 Agent 会话".into());
    }
    trash::delete(&path).map_err(|error| format!("将 Agent 会话移入回收站失败: {error}"))
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
    validate_session_id(&session.id)?;
    session.updated_at = utc_now();
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

fn validate_session_id(value: &str) -> Result<(), String> {
    Uuid::parse_str(value.trim())
        .map(|_| ())
        .map_err(|_| "Agent 会话 ID 无效".into())
}

fn title_from_message(content: &str) -> String {
    let title = content.lines().next().unwrap_or_default().trim();
    let shortened = title.chars().take(28).collect::<String>();
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
            skill_id: String::new(),
            skill_content_hash: String::new(),
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
            skill_id: String::new(),
            skill_content_hash: String::new(),
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
    fn prepare_context_keeps_recent_messages_and_builds_summary() {
        let data_dir = temp_data_dir("prepare-context");
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
                    skill_id: String::new(),
                    skill_content_hash: String::new(),
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

use std::path::Path;

use uuid::Uuid;

use crate::{
    models::{AgentMessage, AgentSession},
    store::{agent_session_path, list_agent_sessions, read_agent_session, write_agent_session},
    utils::utc_now,
};

pub(crate) fn create_session(data_dir: &Path, provider_id: &str) -> Result<AgentSession, String> {
    let now = utc_now();
    let session = AgentSession {
        schema_version: 1,
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
    const RECENT_MESSAGES: usize = 24;
    const SUMMARY_MESSAGES: usize = 20;
    if session.messages.len() > RECENT_MESSAGES && session.summary.trim().is_empty() {
        session.summary = session
            .messages
            .iter()
            .take(session.messages.len().saturating_sub(RECENT_MESSAGES))
            .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
            .take(SUMMARY_MESSAGES)
            .map(|message| {
                let content = message.content.chars().take(160).collect::<String>();
                format!("{}: {}", message.role, content)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    session
        .messages
        .iter()
        .rev()
        .take(RECENT_MESSAGES)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub(crate) fn recover_sessions(data_dir: &Path) -> Result<Vec<AgentSession>, String> {
    let mut values = sessions(data_dir)?;
    for session in &mut values {
        if session.schema_version == 0 {
            session.schema_version = 1;
        }
        if matches!(session.status.as_str(), "running" | "tool_running") {
            session.status = "interrupted".into();
            session.updated_at = utc_now();
            write_agent_session(data_dir, session)?;
        }
    }
    Ok(values)
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
    fn prepare_context_keeps_recent_messages_and_builds_summary() {
        let data_dir = temp_data_dir("prepare-context");
        let mut session = AgentSession {
            schema_version: 1,
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

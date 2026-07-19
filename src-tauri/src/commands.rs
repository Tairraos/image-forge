use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{
    defaults::APP_BUILD_TIME,
    models::{
        AboutInfo, AgentAttachment, AgentImagePlan, AgentMessage, AgentProgressEvent, AgentSession,
        AgentSkillContext, AgentTaskGroup, ApiProvider, AppState, CleanupCandidate,
        GenerateRequest, PromptTemplate, QueueSnapshot, ReferencePreview, Settings,
        SkillAuditResult, SkillEntry, SkillFetchResult, TaskRecord, TemplateFillEvent,
        TemplateImportResult,
    },
    services::{
        agent::run_turn,
        agent_store::{
            append_message, create_session, delete_session, prepare_context, recover_sessions,
            save_session, session,
        },
        chat::fill_template_response,
        images::reference_preview,
        provider_bundle::{export_providers_json, read_providers_json},
        queue::{
            build_queue_snapshot, emit_queue_updated, ensure_queue_worker, recover_stale_running,
        },
        references::{
            cleanup_orphan_files, persist_reference_paths, prune_unreferenced_files,
            prune_unreferenced_files_with_data, scan_orphan_files,
        },
        skill::fetch_skill_markdown as fetch_skill_markdown_from_url,
        skill_installer::{
            audit_skill_directory, install_skill_source, read_verified_manifest, save_skill_entry,
        },
        template_bundle::{export_templates_archive, import_templates_archive},
    },
    state::{record_operation, runtime_logs_text, RuntimeState},
    store::{
        enqueue_task, ensure_data_dir, is_safe_skill_directory, next_template_id,
        normalize_request, normalize_settings, normalize_template, params_from_request,
        provider_for_request, read_history, read_json, read_queue, read_settings, read_skills,
        read_templates, refresh_history_output_sizes, request_path, skills_dir, templates_path,
        upsert_history, write_generation_batch, write_history, write_json, write_queue,
        write_settings, write_skill_index,
    },
    utils::utc_now,
};

#[tauri::command]
/// 返回关于弹窗需要的版本和编译时间。
pub(crate) fn about_info() -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        build_time: APP_BUILD_TIME.into(),
    }
}

#[tauri::command]
/// 返回本次运行的结构化日志文本。
pub(crate) fn runtime_logs() -> String {
    runtime_logs_text()
}

#[tauri::command]
pub(crate) fn create_agent_session(
    app: AppHandle,
    provider_id: String,
) -> Result<AgentSession, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = create_session(&data_dir, &provider_id);
    record_result("创建 Agent 会话", "", None, &result);
    result
}

#[tauri::command]
pub(crate) fn list_agent_sessions(app: AppHandle) -> Result<Vec<AgentSession>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = recover_sessions(&data_dir);
    record_result("读取 Agent 会话", "", None, &result);
    result
}

#[tauri::command]
pub(crate) fn delete_agent_session(app: AppHandle, session_id: String) -> Result<(), String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = delete_session(&data_dir, &session_id);
    record_result(
        "删除 Agent 会话",
        &format!("session_id={session_id}"),
        None,
        &result,
    );
    result
}

#[tauri::command]
pub(crate) fn get_agent_session(
    app: AppHandle,
    session_id: String,
) -> Result<AgentSession, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = session(&data_dir, &session_id);
    record_result(
        "读取 Agent 会话",
        format!("session_id={session_id}").as_str(),
        None,
        &result,
    );
    result
}

#[tauri::command]
pub(crate) async fn send_agent_message(
    app: AppHandle,
    session_id: String,
    provider_id: String,
    skill_id: String,
    content: String,
    attachments: Vec<AgentAttachment>,
) -> Result<AgentSession, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut current = session(&data_dir, &session_id)?;
    if !provider_id.trim().is_empty() {
        current.model_provider_id = provider_id.trim().to_string();
    }
    save_session(&data_dir, current)?;
    let user = AgentMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".into(),
        status: "user".into(),
        content: content.trim().to_string(),
        attachments,
        tool_call: None,
        questions: Vec::new(),
        skill_id: String::new(),
        skill_content_hash: String::new(),
        task_group: None,
        error: String::new(),
        created_at: utc_now(),
    };
    if user.content.is_empty() {
        return Err("消息不能为空".into());
    }
    current = append_message(&data_dir, &session_id, user)?;
    current.status = "running".into();
    let context_messages = prepare_context(&mut current);
    current = save_session(&data_dir, current)?;
    let settings = read_settings(&data_dir)?;
    let provider = agent_chat_provider(&settings, &current.model_provider_id)?;
    let mut context = if current.summary.trim().is_empty() {
        String::new()
    } else {
        format!("历史摘要：\n{}", current.summary)
    };
    let mut selected_skill_hash = String::new();
    if !skill_id.trim().is_empty() {
        let skill_context = use_skill(app.clone(), skill_id.clone())?;
        selected_skill_hash = skill_context.manifest.content_hash.clone();
        context.push_str(&format!(
            "\n\n<skill id=\"{}\" name=\"{}\" content_hash=\"{}\">\n{}\n",
            skill_context.skill_id,
            skill_context.name,
            skill_context.manifest.content_hash,
            skill_context.content
        ));
        for reference in skill_context.references {
            context.push_str(&format!(
                "\n<skill_reference>\n{}\n</skill_reference>\n",
                reference
            ));
        }
        context.push_str("</skill>");
    }
    let mut chat_messages = vec![serde_json::json!({
        "role": "system",
        "content": agent_system_prompt(&context),
    })];
    chat_messages.extend(context_messages.iter().map(agent_message_to_chat_value));
    let task_provider = provider.clone();
    let task_messages = chat_messages;
    let user_confirmation = content.clone();
    let session_for_task = session_id.clone();
    let app_for_task = app.clone();
    let task = tokio::spawn(async move {
        let session_for_event = session_for_task.clone();
        let app_for_event = app_for_task.clone();
        let mut messages = task_messages;
        run_turn(
            &task_provider,
            &mut messages,
            Some(app_for_task.state::<RuntimeState>().inner()),
            |name, arguments| {
                let app = app_for_task.clone();
                let session_id = session_for_task.clone();
                let user_confirmation = user_confirmation.clone();
                async move {
                    execute_agent_tool(&app, &session_id, &user_confirmation, &name, &arguments)
                        .await
                }
            },
            move |event| {
                let _ = app_for_event.emit(
                    "agent-progress",
                    AgentProgressEvent {
                        session_id: session_for_event.clone(),
                        phase: event.phase.into(),
                        mode: event.mode.into(),
                        chunk: event.chunk,
                        message: event.message,
                        tool_call_id: event.tool_call_id,
                        tool_name: event.tool_name,
                    },
                );
            },
        )
        .await
    });
    app.state::<RuntimeState>()
        .agent_tasks
        .lock()
        .map_err(|_| "Agent 任务状态锁定失败")?
        .insert(session_id.clone(), task.abort_handle());
    let output = match task.await {
        Ok(result) => result,
        Err(error) if error.is_cancelled() => Err("Agent 对话已停止".into()),
        Err(error) => Err(format!("Agent 对话任务失败: {error}")),
    };
    if let Ok(mut tasks) = app.state::<RuntimeState>().agent_tasks.lock() {
        tasks.remove(&session_id);
    }
    let result = match output {
        Ok(mut output) => {
            if output.skill_id.trim().is_empty() && !skill_id.trim().is_empty() {
                output.skill_id = skill_id.clone();
            }
            if output.skill_content_hash.trim().is_empty() && !selected_skill_hash.is_empty() {
                output.skill_content_hash = selected_skill_hash.clone();
            }
            for tool_call in output.tool_calls {
                let content = serde_json::to_string(&serde_json::json!({
                    "result": tool_call.result,
                    "error": tool_call.error,
                }))
                .unwrap_or_default();
                append_message(
                    &data_dir,
                    &session_id,
                    AgentMessage {
                        id: Uuid::new_v4().to_string(),
                        role: "tool".into(),
                        status: "tool".into(),
                        content,
                        attachments: Vec::new(),
                        tool_call: Some(tool_call),
                        questions: Vec::new(),
                        skill_id: String::new(),
                        skill_content_hash: String::new(),
                        task_group: None,
                        error: String::new(),
                        created_at: utc_now(),
                    },
                )?;
            }
            current = append_message(
                &data_dir,
                &session_id,
                AgentMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".into(),
                    status: output.status.clone(),
                    content: output.text,
                    attachments: Vec::new(),
                    tool_call: None,
                    questions: output.questions,
                    skill_id: output.skill_id,
                    skill_content_hash: output.skill_content_hash,
                    task_group: None,
                    error: String::new(),
                    created_at: utc_now(),
                },
            )?;
            current.status = "idle".into();
            current = save_session(&data_dir, current)?;
            Ok(current)
        }
        Err(error) => {
            if let Ok(mut failed) = session(&data_dir, &session_id) {
                failed.status = if error.contains("已停止") {
                    "interrupted".into()
                } else {
                    "error".into()
                };
                failed.messages.push(AgentMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".into(),
                    status: "error".into(),
                    content: "本轮 Agent 对话未完成".into(),
                    attachments: Vec::new(),
                    tool_call: None,
                    questions: Vec::new(),
                    skill_id: String::new(),
                    skill_content_hash: String::new(),
                    task_group: None,
                    error: error.clone(),
                    created_at: utc_now(),
                });
                let _ = save_session(&data_dir, failed);
            }
            Err(error)
        }
    };
    record_result(
        "Agent 对话",
        format!("session_id={session_id} model={}", provider.image_model).as_str(),
        Some(!provider.proxy_url.trim().is_empty()),
        &result,
    );
    result
}

#[tauri::command]
pub(crate) fn cancel_agent_turn(app: AppHandle, session_id: String) -> Result<bool, String> {
    let handle = app
        .state::<RuntimeState>()
        .agent_tasks
        .lock()
        .map_err(|_| "Agent 任务状态锁定失败")?
        .remove(&session_id);
    if let Some(handle) = handle {
        handle.abort();
        record_operation(
            "停止 Agent 对话",
            "成功",
            format!("session_id={session_id}"),
            None,
            None,
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

fn agent_system_prompt(context: &str) -> String {
    format!(
        "你是 Image Forge 本地 Agent。普通聊天直接回答；需要 Skill 或绘图时必须调用已注册工具。禁止声称执行终端、脚本、任意文件读写、任意 HTTP、浏览器、数据库或插件。调用 use_skill 后，如果缺信息必须返回 schemaVersion=1 的 assistant envelope，status=needs_input 并在 questions 中提出最多 3 个问题；Skill 无法执行时返回 status=rejected 和原因；信息完整时返回 status=ready 及逐图 plans，或调用 create_image_tasks。每个 plan 必须明确 resolution、ratio、quality、promptFidelity、referencePolicy 和 referenceIds。参考图只有 ID 和元数据；不支持视觉的模型不能假装看到了图片内容。\n\n当前会话上下文：\n{}",
        context.trim()
    )
}

fn agent_chat_provider(
    settings: &Settings,
    preferred_provider_id: &str,
) -> Result<ApiProvider, String> {
    let provider = settings
        .providers
        .iter()
        .find(|provider| {
            provider.id == preferred_provider_id.trim() && provider.model_type == "chat"
        })
        .or_else(|| {
            settings.providers.iter().find(|provider| {
                provider.id == settings.active_chat_provider_id && provider.model_type == "chat"
            })
        })
        .ok_or("还没有配置对话模型")?;
    if provider.api_key.trim().is_empty() {
        return Err(format!("对话模型「{}」还没有填写 API Key", provider.name));
    }
    Ok(provider.clone())
}

fn agent_message_to_chat_value(message: &AgentMessage) -> serde_json::Value {
    if let Some(call) = &message.tool_call {
        if message.role == "tool" {
            return serde_json::json!({
                "role": "tool",
                "tool_call_id": call.id,
                "name": call.name,
                "content": message.content,
            });
        }
        return serde_json::json!({
            "role": "assistant",
            "content": if message.content.trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(message.content.clone()) },
            "tool_calls": [{
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".into()),
                }
            }]
        });
    }
    let attachment_metadata = message
        .attachments
        .iter()
        .map(|attachment| {
            serde_json::json!({
                "id": attachment.id,
                "fileName": attachment.file_name,
                "mimeType": attachment.mime_type,
                "width": attachment.width,
                "height": attachment.height,
            })
        })
        .collect::<Vec<_>>();
    let mut content = if attachment_metadata.is_empty() {
        message.content.clone()
    } else {
        format!(
            "{}\n\n<reference_attachments>{}</reference_attachments>",
            message.content,
            serde_json::to_string(&attachment_metadata).unwrap_or_default()
        )
    };
    if message.role == "assistant" && !message.questions.is_empty() {
        content.push_str(&format!(
            "\n\n<agent_questions status=\"{}\">{}</agent_questions>",
            message.status,
            serde_json::to_string(&message.questions).unwrap_or_default()
        ));
    }
    serde_json::json!({
        "role": message.role,
        "content": content,
    })
}

async fn execute_agent_tool(
    app: &AppHandle,
    session_id: &str,
    user_message: &str,
    name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match name {
        "list_skills" => {
            let data_dir = ensure_data_dir(app)?;
            let query = arguments
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            Ok(serde_json::Value::Array(list_skill_summaries(
                &data_dir, query,
            )?))
        }
        "install_skill" => {
            let source = arguments
                .get("source")
                .and_then(serde_json::Value::as_str)
                .ok_or("install_skill.source 不能为空")?;
            let replace = arguments
                .get("replace")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if replace && !explicit_confirmation(user_message) {
                return Err("覆盖安装需要用户在当前消息中明确确认".into());
            }
            let data_dir = ensure_data_dir(app)?;
            let (skill, audit) = install_skill_source(&data_dir, source, replace).await?;
            Ok(serde_json::json!({
                "skill": skill,
                "warnings": audit.warnings,
            }))
        }
        "use_skill" => {
            let skill_id = arguments
                .get("skillId")
                .and_then(serde_json::Value::as_str)
                .ok_or("use_skill.skillId 不能为空")?;
            serde_json::to_value(use_skill(app.clone(), skill_id.to_string())?)
                .map_err(|error| format!("序列化 Skill 上下文失败: {error}"))
        }
        "create_image_tasks" => {
            let plans = serde_json::from_value::<Vec<AgentImagePlan>>(
                arguments.get("plans").cloned().unwrap_or_default(),
            )
            .map_err(|error| format!("解析图片计划失败: {error}"))?;
            let skill_id = arguments
                .get("skillId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            if plans.len() > 4 && !explicit_confirmation(user_message) {
                return Err("一次创建超过 4 张图片需要用户在当前消息中明确确认".into());
            }
            let group =
                create_agent_image_tasks(app.clone(), session_id.to_string(), skill_id, plans)?;
            let _ = app.emit("agent-task-group", &group);
            serde_json::to_value(group).map_err(|error| format!("序列化任务组失败: {error}"))
        }
        "get_task_status" => {
            let data_dir = ensure_data_dir(app)?;
            let task_id = arguments
                .get("taskId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let task_group_id = arguments
                .get("taskGroupId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let tasks = task_status_records(&data_dir, task_group_id, task_id)?;
            Ok(serde_json::json!({ "taskGroupId": task_group_id, "tasks": tasks }))
        }
        _ => Err(format!("不允许的 Agent 工具：{name}")),
    }
}

fn task_status_records(
    data_dir: &Path,
    task_group_id: &str,
    task_id: &str,
) -> Result<Vec<TaskRecord>, String> {
    if task_group_id.trim().is_empty() && task_id.trim().is_empty() {
        return Err("需要 taskGroupId 或 taskId".into());
    }
    let records = read_history(data_dir)?
        .into_iter()
        .filter(|task| {
            (!task_id.is_empty() && task.id == task_id)
                || (!task_group_id.is_empty() && task.task_group_id == task_group_id)
        })
        .collect::<Vec<_>>();
    if records.is_empty() {
        Err("找不到任务或任务组".into())
    } else {
        Ok(records)
    }
}

fn list_skill_summaries(data_dir: &Path, query: &str) -> Result<Vec<serde_json::Value>, String> {
    let query = query.trim().to_lowercase();
    read_skills(data_dir)?
        .into_iter()
        .filter(|skill| skill_matches_query(skill, &query))
        .map(|skill| skill_summary_value(data_dir, skill))
        .collect()
}

fn skill_matches_query(skill: &SkillEntry, query: &str) -> bool {
    query.is_empty()
        || [
            skill.name.as_str(),
            skill.notes.as_str(),
            skill.source_url.as_str(),
        ]
        .join(" ")
        .to_lowercase()
        .contains(query)
}

fn skill_summary_value(data_dir: &Path, skill: SkillEntry) -> Result<serde_json::Value, String> {
    let package_dir = skills_dir(data_dir).join(&skill.directory);
    let manifest = audit_skill_directory(&package_dir)
        .ok()
        .and_then(|audit| audit.manifest);
    Ok(serde_json::json!({
        "id": skill.id,
        "name": skill.name,
        "notes": skill.notes,
        "sourceUrl": skill.source_url,
        "capabilities": manifest.map(|value| value.capabilities).unwrap_or_default(),
    }))
}

fn update_agent_task_group_summary(data_dir: &Path, task_group_id: &str, status: &str) {
    let Ok(records) = read_history(data_dir) else {
        return;
    };
    let Some(session_id) = records
        .iter()
        .find(|record| record.task_group_id == task_group_id)
        .map(|record| record.agent_session_id.clone())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(mut agent_session) = session(data_dir, &session_id) else {
        return;
    };
    for message in &mut agent_session.messages {
        if let Some(summary) = &mut message.task_group {
            if summary.id == task_group_id {
                summary.status = status.into();
            }
        }
    }
    let _ = save_session(data_dir, agent_session);
}

fn explicit_confirmation(message: &str) -> bool {
    let message = message.trim().to_lowercase();
    ["确认", "同意", "覆盖", "继续", "yes", "confirm"]
        .iter()
        .any(|keyword| message.contains(keyword))
}

#[tauri::command]
pub(crate) fn audit_skill_package(
    app: AppHandle,
    path: String,
) -> Result<SkillAuditResult, String> {
    let _ = ensure_data_dir(&app)?;
    let root = PathBuf::from(path.trim());
    let result = audit_skill_directory(&root);
    record_result(
        "审查 Skill 包",
        format!("path={}", root.display()).as_str(),
        None,
        &result,
    );
    result
}

#[tauri::command]
pub(crate) async fn install_skill(
    app: AppHandle,
    source: String,
    replace: bool,
) -> Result<SkillEntry, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = install_skill_source(&data_dir, &source, replace)
        .await
        .map(|(skill, _)| skill);
    record_result(
        "安装 Skill",
        format!("source={source}").as_str(),
        None,
        &result,
    );
    result
}

#[tauri::command]
pub(crate) fn use_skill(app: AppHandle, skill_id: String) -> Result<AgentSkillContext, String> {
    let data_dir = ensure_data_dir(&app)?;
    load_skill_context(&data_dir, &skill_id)
}

fn load_skill_context(data_dir: &Path, skill_id: &str) -> Result<AgentSkillContext, String> {
    let skills = read_skills(data_dir)?;
    let skill = skills
        .iter()
        .find(|item| item.id == skill_id)
        .ok_or("找不到 Skill")?;
    if !is_safe_skill_directory(&skill.directory) {
        return Err("Skill 目录名不安全".into());
    }
    let package_dir = skills_dir(&data_dir).join(&skill.directory);
    let audit = audit_skill_directory(&package_dir)?;
    if !audit.allowed {
        return Err(format!("Skill 审查失败：{}", audit.reasons.join("；")));
    }
    let manifest = read_verified_manifest(&package_dir)?;
    let content = read_skill_entry_content(&package_dir)?;
    let references = read_skill_markdown_references(&package_dir)?;
    Ok(AgentSkillContext {
        skill_id: skill.id.clone(),
        name: skill.name.clone(),
        content,
        manifest,
        references,
    })
}

fn read_skill_entry_content(package_dir: &Path) -> Result<String, String> {
    let entry = [package_dir.join("SKILL.md"), package_dir.join("skill.md")]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or("Skill 包缺少 SKILL.md")?;
    fs::read_to_string(entry).map_err(|error| format!("读取 Skill 内容失败: {error}"))
}

fn read_skill_markdown_references(package_dir: &Path) -> Result<Vec<String>, String> {
    let references_dir = package_dir.join("references");
    if references_dir.is_dir() {
        let mut files = Vec::new();
        collect_skill_reference_markdown_files(&references_dir, &references_dir, &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
            .into_iter()
            .map(|(_, path)| {
                fs::read_to_string(&path).map_err(|error| {
                    format!("读取 Skill reference {} 失败: {error}", path.display())
                })
            })
            .collect()
    } else {
        Ok(Vec::new())
    }
}

fn collect_skill_reference_markdown_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("读取 Skill references 失败: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取 Skill references 失败: {error}"))?;
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("读取 Skill reference 元数据失败: {error}"))?;
        if metadata.is_dir() {
            collect_skill_reference_markdown_files(root, &path, files)?;
            continue;
        }
        let is_markdown = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("md") || value.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false);
        if is_markdown {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            files.push((relative, path));
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn create_agent_image_tasks(
    app: AppHandle,
    session_id: String,
    skill_id: String,
    plans: Vec<AgentImagePlan>,
) -> Result<AgentTaskGroup, String> {
    let data_dir = ensure_data_dir(&app)?;
    let group = create_agent_image_tasks_in_data_dir(&data_dir, session_id, skill_id, plans)?;
    let _ = emit_queue_updated(&app, &data_dir);
    ensure_queue_worker(&app);
    Ok(group)
}

fn create_agent_image_tasks_in_data_dir(
    data_dir: &Path,
    session_id: String,
    skill_id: String,
    plans: Vec<AgentImagePlan>,
) -> Result<AgentTaskGroup, String> {
    if plans.is_empty() || plans.len() > 12 {
        return Err("图片计划数量必须在 1 到 12 之间".into());
    }
    let settings = read_settings(data_dir)?;
    let agent_session = session(data_dir, &session_id)?;
    let attachment_paths = agent_session
        .messages
        .iter()
        .flat_map(|message| message.attachments.iter())
        .map(|attachment| (attachment.id.clone(), attachment.path.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    let task_group_id = Uuid::new_v4().to_string();
    let mut prepared = Vec::with_capacity(plans.len());
    for plan in plans {
        let policy = match plan.reference_policy.trim() {
            "use" | "optional" | "none" => plan.reference_policy.trim(),
            "" => "optional",
            _ => return Err("参考图策略必须是 use、optional 或 none".into()),
        };
        if policy == "use" && plan.reference_ids.is_empty() {
            return Err("referencePolicy=use 时必须指定参考图".into());
        }
        if policy == "none" && !plan.reference_ids.is_empty() {
            return Err("referencePolicy=none 时不能指定参考图".into());
        }
        let mut request = GenerateRequest {
            provider_id: Some(if plan.provider_id.trim().is_empty() {
                settings.active_image_provider_id.clone()
            } else {
                plan.provider_id
            }),
            prompt: plan.prompt,
            reference_paths: if policy == "none" {
                Vec::new()
            } else {
                plan.reference_ids
                    .iter()
                    .map(|id| {
                        attachment_paths
                            .get(id)
                            .cloned()
                            .ok_or_else(|| format!("参考图 ID 不属于当前 Agent 会话：{id}"))
                    })
                    .collect::<Result<Vec<_>, String>>()?
            },
            resolution: if plan.resolution.trim().is_empty() {
                "standard".into()
            } else {
                plan.resolution
            },
            ratio: if plan.ratio.trim().is_empty() {
                "1:1".into()
            } else {
                plan.ratio
            },
            quality: if plan.quality.trim().is_empty() {
                "auto".into()
            } else {
                plan.quality
            },
            prompt_fidelity: if plan.prompt_fidelity.trim().is_empty() {
                "original".into()
            } else {
                plan.prompt_fidelity
            },
            ..GenerateRequest::default()
        };
        request = normalize_request(request)?;
        let provider = provider_for_request(&settings, request.provider_id.as_deref())?;
        if provider.api_key.trim().is_empty() {
            return Err(format!("API 源「{}」还没有填写 API Key", provider.name));
        }
        request.provider_id = Some(provider.id.clone());
        request.reference_paths = persist_reference_paths(data_dir, &request.reference_paths)?;
        prepared.push((plan.title, request, provider));
    }
    let now = utc_now();
    let mut request_files = Vec::with_capacity(prepared.len());
    let mut tasks = Vec::with_capacity(prepared.len());
    let mut titles = Vec::with_capacity(prepared.len());
    for (title, request, provider) in prepared {
        let id = Uuid::new_v4().to_string();
        let record = task_record_from_request(
            id.clone(),
            &request,
            &provider,
            Some((&session_id, &task_group_id, &skill_id)),
        );
        titles.push(if title.trim().is_empty() {
            "图片".into()
        } else {
            title
        });
        request_files.push((id, request));
        tasks.push(record);
    }
    write_generation_batch(data_dir, &request_files, &tasks)?;
    let mut agent_session = session(data_dir, &session_id)?;
    if !agent_session.task_group_ids.contains(&task_group_id) {
        agent_session.task_group_ids.push(task_group_id.clone());
    }
    agent_session.messages.push(AgentMessage {
        id: Uuid::new_v4().to_string(),
        role: "tool".into(),
        status: "task_group".into(),
        content: format!("已创建 {} 个绘图任务", tasks.len()),
        attachments: Vec::new(),
        tool_call: None,
        questions: Vec::new(),
        skill_id: skill_id.clone(),
        skill_content_hash: String::new(),
        task_group: Some(crate::models::AgentTaskGroupSummary {
            id: task_group_id.clone(),
            task_ids: tasks.iter().map(|task| task.id.clone()).collect(),
            titles,
            status: "queued".into(),
        }),
        error: String::new(),
        created_at: now,
    });
    let _ = save_session(data_dir, agent_session)?;
    Ok(AgentTaskGroup {
        id: task_group_id,
        session_id,
        skill_id,
        tasks,
        created_at: utc_now(),
    })
}

#[tauri::command]
/// 按任务组或任务 ID 返回当前队列、运行和历史状态。
pub(crate) fn get_task_status(
    app: AppHandle,
    task_group_id: String,
    task_id: String,
) -> Result<Vec<TaskRecord>, String> {
    let data_dir = ensure_data_dir(&app)?;
    task_status_records(&data_dir, &task_group_id, &task_id)
}

#[tauri::command]
/// 取消 Agent 任务组；运行中的任务由 worker 收尾，失败/取消后仍可重试。
pub(crate) fn cancel_agent_task_group(
    app: AppHandle,
    task_group_id: String,
) -> Result<Vec<TaskRecord>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut history = read_history(&data_dir)?;
    let mut queue = read_queue(&data_dir)?;
    let mut found = false;
    let now = utc_now();
    for record in history
        .iter_mut()
        .filter(|record| record.task_group_id == task_group_id)
    {
        found = true;
        if matches!(record.status.as_str(), "queued" | "running" | "cancelling") {
            app.state::<RuntimeState>()
                .cancel_requests
                .lock()
                .map_err(|_| "取消状态锁定失败")?
                .insert(record.id.clone());
            if record.status == "queued" {
                record.status = "cancelled".into();
                record.error = Some("任务组已取消".into());
                record.completed_at = Some(now.clone());
                queue.waiting.retain(|id| id != &record.id);
            } else {
                record.status = "cancelling".into();
            }
            record.updated_at = now.clone();
        }
    }
    if !found {
        return Err("找不到任务组".into());
    }
    write_history(&data_dir, &history)?;
    write_queue(&data_dir, &queue)?;
    update_agent_task_group_summary(&data_dir, &task_group_id, "cancelling");
    let _ = emit_queue_updated(&app, &data_dir);
    Ok(history
        .into_iter()
        .filter(|record| record.task_group_id == task_group_id)
        .collect())
}

#[tauri::command]
/// 重试 Agent 任务组中失败或取消的任务，并一次性恢复到等待队列。
pub(crate) fn retry_agent_task_group(
    app: AppHandle,
    task_group_id: String,
) -> Result<Vec<TaskRecord>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut history = read_history(&data_dir)?;
    let settings = read_settings(&data_dir)?;
    let mut requests = Vec::new();
    let mut records = Vec::new();
    let now = utc_now();
    for record in history.iter_mut().filter(|record| {
        record.task_group_id == task_group_id
            && matches!(record.status.as_str(), "failed" | "cancelled")
    }) {
        let request: GenerateRequest = read_json(&request_path(&data_dir, &record.id))?;
        let provider = provider_for_request(&settings, request.provider_id.as_deref())?;
        record.status = "queued".into();
        record.started_at = None;
        record.completed_at = None;
        record.error = None;
        record.outputs.clear();
        record.provider_id = provider.id.clone();
        record.provider_name = provider.name.clone();
        record.model = provider.image_model;
        record.updated_at = now.clone();
        requests.push((record.id.clone(), request));
        records.push(record.clone());
        if let Ok(mut cancelled) = app.state::<RuntimeState>().cancel_requests.lock() {
            cancelled.remove(&record.id);
        }
    }
    if records.is_empty() {
        return Err("任务组没有可重试的失败或取消任务".into());
    }
    write_generation_batch(&data_dir, &requests, &records)?;
    update_agent_task_group_summary(&data_dir, &task_group_id, "queued");
    ensure_queue_worker(&app);
    let _ = emit_queue_updated(&app, &data_dir);
    Ok(records)
}

#[tauri::command]
/// 扫描四个资源目录中的孤岛文件；只读，不会删除任何文件。
pub(crate) fn scan_cleanup_candidates(app: AppHandle) -> Result<Vec<CleanupCandidate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = scan_orphan_files(&data_dir);
    match &result {
        Ok(candidates) => record_operation(
            "扫描孤岛文件",
            "成功",
            format!("candidate_count={}", candidates.len()),
            None,
            None,
        ),
        Err(error) => record_operation("扫描孤岛文件", "失败", "", None, Some(error)),
    }
    result
}

#[tauri::command]
/// 重新扫描并把当前孤岛文件移入系统回收站。
pub(crate) fn cleanup_data_files(app: AppHandle) -> Result<Vec<CleanupCandidate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = cleanup_orphan_files(&data_dir);
    match &result {
        Ok(candidates) => record_operation(
            "清理孤岛文件",
            "成功",
            format!("file_count={}", candidates.len()),
            None,
            None,
        ),
        Err(error) => record_operation("清理孤岛文件", "失败", "", None, Some(error)),
    }
    result
}

fn record_result<T>(
    operation: &str,
    params: &str,
    proxy_used: Option<bool>,
    result: &Result<T, String>,
) {
    match result {
        Ok(_) => record_operation(operation, "成功", params, proxy_used, None),
        Err(error) => record_operation(operation, "失败", params, proxy_used, Some(error)),
    }
}

#[tauri::command]
/// 加载前端启动所需的完整应用状态，并恢复异常退出遗留的运行中任务。
pub(crate) fn load_app_state(app: AppHandle) -> Result<AppState, String> {
    let data_dir = ensure_data_dir(&app)?;
    let settings = read_settings(&data_dir)?;
    let mut history = read_history(&data_dir)?;
    let templates = read_templates(&data_dir)?;
    recover_stale_running(&app, &data_dir, Some(&mut history))?;
    if refresh_history_output_sizes(&mut history) {
        write_history(&data_dir, &history)?;
    }
    prune_unreferenced_files_with_data(&data_dir, &history, &templates)?;
    if settings.auto_start_queue {
        ensure_queue_worker(&app);
    }
    Ok(AppState {
        settings,
        history: history.clone(),
        queue: build_queue_snapshot(&app, &data_dir, history)?,
        templates,
        skills: read_skills(&data_dir)?,
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
/// 保存 API 源与基础设置，随后唤醒队列 worker 处理可能恢复的任务。
pub(crate) fn save_settings(
    app: AppHandle,
    settings: crate::models::Settings,
) -> Result<crate::models::Settings, String> {
    let data_dir = ensure_data_dir(&app)?;
    let settings = normalize_settings(settings);
    write_settings(&data_dir, &settings)?;
    ensure_queue_worker(&app);
    Ok(settings)
}

#[tauri::command]
/// 将当前 API 源导出为可再次批量导入的 JSON 文件。
pub(crate) fn export_api_providers(
    _app: AppHandle,
    destination: String,
    providers: Vec<ApiProvider>,
) -> Result<String, String> {
    let params = format!("path={} provider_count={}", destination, providers.len());
    let result = export_providers_json(Path::new(&destination), &providers);
    record_result("导出 API 源文件", &params, None, &result);
    result
}

#[tauri::command]
/// 读取用户拖入导入框的 API 源 JSON 文件。
pub(crate) fn read_api_providers_file(_app: AppHandle, path: String) -> Result<String, String> {
    let params = format!("path={path}");
    let result = read_providers_json(Path::new(&path));
    record_result("读取 API 源文件", &params, None, &result);
    result
}

#[tauri::command]
/// 读取拖入的 Markdown Skill 文件或包含 SKILL.md 的目录。
pub(crate) fn read_skill_markdown_file(_app: AppHandle, path: String) -> Result<String, String> {
    let input_path = Path::new(path.trim());
    let params = format!("path={}", input_path.display());
    let result = (|| {
        let path = if input_path.is_dir() {
            ["SKILL.md", "skill.md"]
                .into_iter()
                .map(|name| input_path.join(name))
                .find(|candidate| candidate.is_file())
                .ok_or("目录中没有找到 SKILL.md")?
        } else {
            input_path.to_path_buf()
        };
        if !path.is_file() {
            return Err("找不到拖入的文件".into());
        }
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            return Err("只支持拖入 .md 文件".into());
        }
        let metadata =
            fs::metadata(&path).map_err(|error| format!("读取 Skill 文件失败: {error}"))?;
        if metadata.len() > 1_048_576 {
            return Err("Skill 文件超过 1 MB".into());
        }
        fs::read_to_string(&path).map_err(|error| format!("读取 Skill 文件失败: {error}"))
    })();
    record_result("读取 Skill Markdown 文件", &params, None, &result);
    result
}

#[tauri::command]
/// 返回队列快照，供前端轮询刷新运行中和等待中的任务。
pub(crate) fn queue_snapshot(app: AppHandle) -> Result<QueueSnapshot, String> {
    let data_dir = ensure_data_dir(&app)?;
    build_queue_snapshot(&app, &data_dir, read_history(&data_dir)?)
}

#[tauri::command]
/// 创建新的生图任务：保存原始请求、写入历史、放入等待队列。
pub(crate) fn enqueue_generation(
    app: AppHandle,
    request: GenerateRequest,
) -> Result<TaskRecord, String> {
    let data_dir = ensure_data_dir(&app)?;
    let settings = read_settings(&data_dir)?;
    let record = enqueue_generation_request(&app, &data_dir, &settings, request, None)?;
    ensure_queue_worker(&app);
    Ok(record)
}

#[tauri::command]
/// 批量创建生图任务，适用于 Skill 一次规划出多张图的场景。
pub(crate) fn enqueue_generation_batch(
    app: AppHandle,
    requests: Vec<GenerateRequest>,
) -> Result<Vec<TaskRecord>, String> {
    if requests.is_empty() {
        return Err("没有可加入队列的任务".into());
    }
    let data_dir = ensure_data_dir(&app)?;
    let settings = read_settings(&data_dir)?;
    let mut records = Vec::with_capacity(requests.len());
    for request in requests {
        records.push(enqueue_generation_request(
            &app, &data_dir, &settings, request, None,
        )?);
    }
    ensure_queue_worker(&app);
    Ok(records)
}

fn enqueue_generation_request(
    app: &AppHandle,
    data_dir: &Path,
    settings: &Settings,
    request: GenerateRequest,
    agent_origin: Option<(&str, &str, &str)>,
) -> Result<TaskRecord, String> {
    let mut request = normalize_request(request)?;
    let provider = provider_for_request(settings, request.provider_id.as_deref())?;
    request.provider_id = Some(provider.id.clone());
    if provider.api_key.trim().is_empty() {
        return Err(format!("API 源「{}」还没有填写 API Key", provider.name));
    }
    request.reference_paths = persist_reference_paths(data_dir, &request.reference_paths)?;

    let id = Uuid::new_v4().to_string();
    let record = task_record_from_request(id.clone(), &request, &provider, agent_origin);

    write_json(&request_path(data_dir, &id), &request)?;
    upsert_history(data_dir, record.clone())?;
    enqueue_task(data_dir, &id)?;
    let _ = emit_queue_updated(app, data_dir);
    Ok(record)
}

fn task_record_from_request(
    id: String,
    request: &GenerateRequest,
    provider: &ApiProvider,
    agent_origin: Option<(&str, &str, &str)>,
) -> TaskRecord {
    let now = utc_now();
    let mut record = TaskRecord {
        id: id.clone(),
        created_at: now.clone(),
        updated_at: now,
        started_at: None,
        completed_at: None,
        prompt: request.prompt.clone(),
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        mode: "images".into(),
        model: provider.image_model.clone(),
        status: "queued".into(),
        params: params_from_request(request),
        reference_paths: request.reference_paths.clone(),
        outputs: Vec::new(),
        attempts: 0,
        error: None,
        origin: String::new(),
        agent_session_id: String::new(),
        task_group_id: String::new(),
        skill_id: String::new(),
    };
    if let Some((session_id, task_group_id, skill_id)) = agent_origin {
        record.origin = "agent".into();
        record.agent_session_id = session_id.into();
        record.task_group_id = task_group_id.into();
        record.skill_id = skill_id.into();
    }

    record
}

#[tauri::command]
/// 使用已保存的原始请求重新排队失败或完成的历史任务。
pub(crate) fn retry_task(app: AppHandle, task_id: String) -> Result<TaskRecord, String> {
    let data_dir = ensure_data_dir(&app)?;
    let request: GenerateRequest = read_json(&request_path(&data_dir, &task_id))?;
    let mut history = read_history(&data_dir)?;
    let record = history
        .iter_mut()
        .find(|item| item.id == task_id)
        .ok_or("找不到任务")?;
    if record.status == "running" || record.status == "queued" {
        return Err("任务已经在队列中".into());
    }
    let settings = read_settings(&data_dir)?;
    let provider = provider_for_request(&settings, request.provider_id.as_deref())?;
    record.status = "queued".into();
    record.updated_at = utc_now();
    record.started_at = None;
    record.completed_at = None;
    record.error = None;
    record.outputs.clear();
    record.provider_id = provider.id;
    record.provider_name = provider.name;
    record.mode = "images".into();
    record.model = provider.image_model;
    let next = record.clone();
    write_history(&data_dir, &history)?;
    enqueue_task(&data_dir, &task_id)?;
    ensure_queue_worker(&app);
    let _ = emit_queue_updated(&app, &data_dir);
    Ok(next)
}

#[tauri::command]
/// 删除任务记录、队列项和请求文件，并把已生成图片移入回收站。
pub(crate) fn delete_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut history = read_history(&data_dir)?;
    let Some(index) = history.iter().position(|item| item.id == task_id) else {
        return Err("找不到任务".into());
    };
    let defer_reference_cleanup =
        matches!(history[index].status.as_str(), "running" | "cancelling");
    if matches!(
        history[index].status.as_str(),
        "queued" | "running" | "cancelling"
    ) {
        app.state::<RuntimeState>()
            .cancel_requests
            .lock()
            .map_err(|_| "取消状态锁定失败")?
            .insert(task_id.clone());
        app.state::<RuntimeState>()
            .deleted_tasks
            .lock()
            .map_err(|_| "删除状态锁定失败")?
            .insert(task_id.clone());
    }

    delete_output_files_for_task(&history[index])?;
    history.remove(index);
    let mut queue = read_queue(&data_dir)?;
    queue.waiting.retain(|id| id != &task_id);
    queue.running.retain(|run| run.task_id != task_id);
    write_history(&data_dir, &history)?;
    write_queue(&data_dir, &queue)?;

    let request_file = request_path(&data_dir, &task_id);
    if request_file.exists() {
        trash::delete(&request_file)
            .map_err(|error| format!("将任务请求移入回收站失败: {error}"))?;
    }
    if !defer_reference_cleanup {
        prune_unreferenced_files(&data_dir)?;
    }
    let _ = emit_queue_updated(&app, &data_dir);
    Ok(())
}

/// 将任务输出图移入系统回收站，避免误删后无法找回。
fn delete_output_files_for_task(record: &TaskRecord) -> Result<(), String> {
    for output in &record.outputs {
        let path = PathBuf::from(&output.path);
        if path.is_file() {
            if let Err(error) = trash::delete(&path) {
                let message = format!(
                    "将生成图片移到回收站失败（{}）: {error}",
                    path.to_string_lossy()
                );
                record_operation(
                    "删除生成图片",
                    "失败",
                    format!("task_id={} path={}", record.id, path.display()),
                    None,
                    Some(&message),
                );
                return Err(message);
            }
            record_operation(
                "删除生成图片",
                "成功",
                format!("task_id={} path={}", record.id, path.display()),
                None,
                None,
            );
        }
    }
    Ok(())
}

#[tauri::command]
/// 根据本地图片路径生成参考图预览信息。
pub(crate) fn reference_from_path(
    _app: AppHandle,
    path: String,
) -> Result<ReferencePreview, String> {
    let params = format!("path={path}");
    let result = reference_preview(Path::new(&path));
    record_result("读取图片文件", &params, None, &result);
    result
}

#[tauri::command]
/// 从系统剪贴板读取 Finder 文件引用或图片，并保存为参考图资源。
pub(crate) fn reference_from_clipboard(app: AppHandle) -> Result<Option<ReferencePreview>, String> {
    let result = crate::services::clipboard::reference_from_clipboard(&app);
    let params = match &result {
        Ok(Some(preview)) => format!("path={} bytes={}", preview.path, preview.file_size),
        _ => "source=system_clipboard".into(),
    };
    record_result("读取剪贴板图片", &params, None, &result);
    result
}

#[tauri::command]
/// 新增或更新提示词模板，空 ID 会按数字序列自动分配。
pub(crate) fn save_template(
    app: AppHandle,
    template: PromptTemplate,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    let mut next = normalize_template(template)?;
    next.reference_paths = persist_reference_paths(&data_dir, &next.reference_paths)?;
    next.effect_image_path = persist_optional_image_path(&data_dir, &next.effect_image_path)?;
    if next.id.is_empty() {
        next.id = next_template_id(&templates);
        next.created_at = utc_now();
    }
    next.updated_at = utc_now();
    if let Some(index) = templates.iter().position(|item| item.id == next.id) {
        next.created_at = templates[index].created_at.clone();
        next.usage_count = templates[index].usage_count;
        templates[index] = next;
    } else {
        templates.push(next);
    }
    write_json(&templates_path(&data_dir), &templates)?;
    prune_unreferenced_files(&data_dir)?;
    Ok(templates)
}

#[tauri::command]
/// 导出全部提示词模板、Markdown 清单和去重后的参考图资源。
pub(crate) fn export_templates(app: AppHandle, destination: String) -> Result<String, String> {
    let data_dir = ensure_data_dir(&app)?;
    let templates = read_templates(&data_dir)?;
    let params = format!("path={} template_count={}", destination, templates.len());
    let result = export_templates_archive(&templates, Path::new(&destination))
        .map(|archive| archive.to_string_lossy().into_owned());
    record_result("导出模板文件", &params, None, &result);
    result
}

#[tauri::command]
/// 导入模板 ZIP，为新模板分配数字 ID，并跳过内容和参考图完全相同的模板。
pub(crate) fn import_templates(
    app: AppHandle,
    archive_path: String,
) -> Result<TemplateImportResult, String> {
    let data_dir = ensure_data_dir(&app)?;
    let params = format!("path={archive_path}");
    let imported = match import_templates_archive(&data_dir, Path::new(&archive_path)) {
        Ok(templates) => templates,
        Err(error) => {
            let _ = prune_unreferenced_files(&data_dir);
            record_operation("导入模板文件", "失败", &params, None, Some(&error));
            return Err(error);
        }
    };
    let templates = read_templates(&data_dir)?;
    let result = merge_imported_templates(templates, imported)?;
    if let Err(error) = write_json(&templates_path(&data_dir), &result.templates) {
        let _ = prune_unreferenced_files(&data_dir);
        record_operation("导入模板文件", "失败", &params, None, Some(&error));
        return Err(error);
    }
    prune_unreferenced_files(&data_dir)?;
    record_operation(
        "导入模板文件",
        "成功",
        format!(
            "{params} imported={} skipped={}",
            result.imported_count, result.skipped_count
        ),
        None,
        None,
    );
    Ok(result)
}

fn merge_imported_templates(
    mut templates: Vec<PromptTemplate>,
    imported: Vec<PromptTemplate>,
) -> Result<TemplateImportResult, String> {
    let mut signatures = templates
        .iter()
        .map(template_signature)
        .collect::<HashSet<_>>();
    let mut imported_count = 0;
    let mut skipped_count = 0;

    for template in imported {
        let mut next = normalize_template(template)?;
        let signature = template_signature(&next);
        if !signatures.insert(signature) {
            skipped_count += 1;
            continue;
        }
        next.id = next_template_id(&templates);
        next.created_at = utc_now();
        next.updated_at = next.created_at.clone();
        templates.push(next);
        imported_count += 1;
    }

    Ok(TemplateImportResult {
        templates,
        imported_count,
        skipped_count,
    })
}

fn template_signature(template: &PromptTemplate) -> (String, String, Vec<String>, String) {
    let mut references = template.reference_paths.clone();
    references.sort();
    references.dedup();
    (
        template.title.trim().to_string(),
        template.content.trim().to_string(),
        references,
        template.effect_image_path.trim().to_string(),
    )
}

fn persist_optional_image_path(data_dir: &Path, path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(String::new());
    }
    Ok(persist_reference_paths(data_dir, &[path.to_string()])?
        .into_iter()
        .next()
        .unwrap_or_default())
}

#[tauri::command]
/// 删除指定提示词模板并返回更新后的模板列表。
pub(crate) fn delete_template(
    app: AppHandle,
    template_id: String,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    templates.retain(|template| template.id != template_id);
    write_json(&templates_path(&data_dir), &templates)?;
    prune_unreferenced_files(&data_dir)?;
    Ok(templates)
}

#[tauri::command]
/// 新增或更新 Skill；保存与导入使用相同安全门并生成 manifest。
pub(crate) fn save_skill(
    app: AppHandle,
    skill: SkillEntry,
    replace: bool,
) -> Result<Vec<SkillEntry>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let result = save_skill_entry(&data_dir, skill, replace).and_then(|_| read_skills(&data_dir));
    record_result("保存 Skill", "source=editor", None, &result);
    result
}

#[tauri::command]
/// 删除指定 Skill 并返回更新后的列表。
pub(crate) fn delete_skill(app: AppHandle, skill_id: String) -> Result<Vec<SkillEntry>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut skills = read_skills(&data_dir)?;
    if let Some(skill) = skills
        .iter()
        .find(|skill| skill.id == skill_id && is_safe_skill_directory(&skill.directory))
    {
        let package_dir = skills_dir(&data_dir).join(&skill.directory);
        if package_dir.starts_with(skills_dir(&data_dir)) && package_dir.is_dir() {
            trash::delete(&package_dir)
                .map_err(|error| format!("将 Skill 目录移入回收站失败: {error}"))?;
        }
    }
    skills.retain(|skill| skill.id != skill_id);
    write_skill_index(&data_dir, &skills)?;
    Ok(skills)
}

#[tauri::command]
/// 提取 URL 指向的 Markdown Skill，目录 URL 会继续尝试大小写文件名。
pub(crate) async fn fetch_skill_markdown(
    _app: AppHandle,
    source_url: String,
) -> Result<SkillFetchResult, String> {
    let params = format!("url={source_url}");
    record_operation("从 URL 提取 Skill", "开始", &params, Some(false), None);
    let result = fetch_skill_markdown_from_url(&source_url).await;
    record_result("从 URL 提取 Skill", &params, Some(false), &result);
    result
}

#[tauri::command]
/// 交换两个模板在持久化数组中的位置，用于维护界面的手动排序。
pub(crate) fn move_template(
    app: AppHandle,
    template_id: String,
    target_template_id: String,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    swap_template_order(&mut templates, &template_id, &target_template_id)?;
    write_json(&templates_path(&data_dir), &templates)?;
    Ok(templates)
}

fn swap_template_order(
    templates: &mut [PromptTemplate],
    template_id: &str,
    target_template_id: &str,
) -> Result<(), String> {
    let index = templates
        .iter()
        .position(|template| template.id == template_id)
        .ok_or("找不到要移动的模板")?;
    let target_index = templates
        .iter()
        .position(|template| template.id == target_template_id)
        .ok_or("找不到目标模板")?;
    templates.swap(index, target_index);
    Ok(())
}

#[tauri::command]
/// 引用模板后增加使用次数，用于后续排序或维护参考。
pub(crate) fn mark_template_used(
    app: AppHandle,
    template_id: String,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    if let Some(template) = templates
        .iter_mut()
        .find(|template| template.id == template_id)
    {
        template.usage_count = template.usage_count.saturating_add(1);
        template.updated_at = utc_now();
    }
    write_json(&templates_path(&data_dir), &templates)?;
    Ok(templates)
}

#[tauri::command]
/// 调用对话模型填充模板中的占位描述，返回完整提示词文本。
pub(crate) async fn fill_prompt_template(
    app: AppHandle,
    session_id: String,
    provider_id: String,
    template: String,
) -> Result<String, String> {
    let content = template.trim();
    if content.is_empty() {
        return Err("模板内容不能为空".into());
    }
    let data_dir = ensure_data_dir(&app)?;
    let runtime_state = app.state::<RuntimeState>();
    runtime_state.push_log(
        "ai_fill.command.start",
        format!(
            "provider_id={} template_len={}",
            provider_id,
            content.chars().count()
        ),
    );
    let settings = read_settings(&data_dir)?;
    let Some(provider) = settings
        .providers
        .iter()
        .find(|provider| provider.id == provider_id && provider.model_type == "chat")
    else {
        runtime_state.push_log(
            "ai_fill.command.provider_not_found",
            format!("provider_id={}", provider_id),
        );
        return Err("请选择可用的对话模型".into());
    };
    if provider.api_key.trim().is_empty() {
        runtime_state.push_log(
            "ai_fill.command.missing_key",
            format!(
                "provider_id={} provider_name={}",
                provider.id, provider.name
            ),
        );
        return Err(format!("对话模型「{}」还没有填写 API Key", provider.name));
    }
    runtime_state.push_log(
        "ai_fill.command.provider",
        format!(
            "provider_id={} provider_name={} base_url={} model={}",
            provider.id, provider.name, provider.base_url, provider.image_model
        ),
    );
    let result = fill_template_response(provider, content, Some(&runtime_state), |event| {
        let _ = app.emit(
            "template-fill",
            TemplateFillEvent {
                session_id: session_id.clone(),
                phase: event.phase.to_string(),
                mode: event.mode.to_string(),
                chunk: event.chunk,
            },
        );
    })
    .await;
    match &result {
        Ok(value) => runtime_state.push_log(
            "ai_fill.command.success",
            format!(
                "provider_id={} output_len={}",
                provider.id,
                value.text.chars().count()
            ),
        ),
        Err(error) => runtime_state.push_log(
            "ai_fill.command.error",
            format!("provider_id={} error={}", provider.id, error),
        ),
    }
    result.map(|output| output.text)
}

#[tauri::command]
/// 从兼容 OpenAI 的 API 源读取可选模型列表。
pub(crate) async fn list_provider_models(provider: ApiProvider) -> Result<Vec<String>, String> {
    let params = format!(
        "provider_id={} provider_name={} model_type={} base_url={}",
        provider.id, provider.name, provider.model_type, provider.base_url
    );
    let proxy_used = !provider.proxy_url.trim().is_empty();
    record_operation("获取模型列表", "开始", &params, Some(proxy_used), None);
    let result = crate::services::models::list_provider_models(&provider).await;
    match &result {
        Ok(models) => record_operation(
            "获取模型列表",
            "成功",
            format!("{params} model_count={}", models.len()),
            Some(proxy_used),
            None,
        ),
        Err(error) => record_operation(
            "获取模型列表",
            "失败",
            params,
            Some(proxy_used),
            Some(error),
        ),
    }
    result
}

#[tauri::command]
/// 把生成图片复制到系统下载目录，并自动处理重名文件。
pub(crate) fn download_output(app: AppHandle, path: String) -> Result<String, String> {
    let source = PathBuf::from(path);
    if !source.is_file() {
        return Err("找不到要下载的图片".into());
    }
    let downloads_dir = app
        .path()
        .download_dir()
        .map_err(|error| format!("找不到下载目录: {error}"))?;
    fs::create_dir_all(&downloads_dir).map_err(|error| format!("创建下载目录失败: {error}"))?;
    let file_name = source
        .file_name()
        .ok_or("图片文件名无效")?
        .to_string_lossy()
        .into_owned();
    let target = unique_download_path(&downloads_dir, &file_name);
    let params = format!("source={} target={}", source.display(), target.display());
    let result = fs::copy(&source, &target)
        .map(|_| target.to_string_lossy().into_owned())
        .map_err(|error| format!("保存到下载目录失败: {error}"));
    record_result("复制生成图片到下载目录", &params, None, &result);
    result
}

#[tauri::command]
/// 将图片复制到系统剪贴板。
pub(crate) fn copy_image_to_clipboard(path: String) -> Result<(), String> {
    let params = format!("path={path}");
    let result = crate::services::clipboard::copy_image_to_clipboard(Path::new(&path));
    record_result("读取图片并写入剪贴板", &params, None, &result);
    result
}

#[tauri::command]
/// 在系统文件管理器中打开目录或定位指定文件。
pub(crate) fn reveal_path(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("路径不存在".into());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        if path.is_dir() {
            command.arg(&path);
        } else {
            command.arg("-R").arg(&path);
        }
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg("/select,").arg(&path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or_else(|| Path::new(".")));
        command
    };

    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("打开路径失败: {error}"))?;
    Ok(())
}

/// 为下载文件生成不冲突的目标路径。
fn unique_download_path(downloads_dir: &Path, file_name: &str) -> PathBuf {
    let candidate = downloads_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let source_path = Path::new(file_name);
    let stem = source_path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let extension = source_path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty());

    for index in 1.. {
        let next_name = if let Some(extension) = &extension {
            format!("{stem}-{index}.{extension}")
        } else {
            format!("{stem}-{index}")
        };
        let next = downloads_dir.join(next_name);
        if !next.exists() {
            return next;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_templates_skip_duplicates_and_continue_numeric_ids() {
        let existing = vec![template(
            "4",
            "相同标题",
            "相同模板",
            &["/references/a.png", "/references/b.png"],
        )];
        let imported = vec![
            template(
                "",
                "相同标题",
                " 相同模板 ",
                &["/references/b.png", "/references/a.png"],
            ),
            template("", "", "新增模板", &[]),
            template("", "新增模板", "新增模板", &[]),
        ];
        let result = merge_imported_templates(existing, imported).unwrap();
        assert_eq!(result.imported_count, 1);
        assert_eq!(result.skipped_count, 2);
        assert_eq!(result.templates.len(), 2);
        assert_eq!(result.templates[1].id, "5");
        assert_eq!(result.templates[1].title, "新增模板");
        assert_eq!(result.templates[1].content, "新增模板");
    }

    #[test]
    fn empty_title_uses_only_the_first_line() {
        let normalized =
            normalize_template(template("", "", "第一行标题\n第二行不应进入标题", &[])).unwrap();
        assert_eq!(normalized.title, "第一行标题");
    }

    #[test]
    fn templates_can_swap_persisted_order() {
        let mut templates = vec![
            template("1", "模板一", "内容一", &[]),
            template("2", "模板二", "内容二", &[]),
            template("3", "模板三", "内容三", &[]),
        ];
        swap_template_order(&mut templates, "3", "1").unwrap();
        assert_eq!(
            templates
                .iter()
                .map(|template| template.id.as_str())
                .collect::<Vec<_>>(),
            vec!["3", "2", "1"]
        );
    }

    #[test]
    fn agent_skill_list_returns_filtered_summaries_without_content() {
        let data_dir = command_test_data_dir("skill-list");
        std::fs::create_dir_all(data_dir.join("skills").join("camera-director")).unwrap();
        std::fs::create_dir_all(data_dir.join(".staging")).unwrap();
        std::fs::write(
            data_dir.join("skills").join("camera-director").join("SKILL.md"),
            "---\nname: 镜头导演\ncapabilities: [chat, image_plan]\n---\n# 镜头导演\n正文不应出现在 list_skills 结果里",
        )
        .unwrap();
        write_skill_index(
            &data_dir,
            &[SkillEntry {
                id: "skill-camera".into(),
                name: "镜头导演".into(),
                source_url: "https://example.com/skills/camera".into(),
                notes: "电影感构图".into(),
                content: "正文不应出现在 list_skills 结果里".into(),
                directory: "camera-director".into(),
                source_path: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
            }],
        )
        .unwrap();

        let summaries = list_skill_summaries(&data_dir, "电影").unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0]["id"], "skill-camera");
        assert_eq!(summaries[0]["name"], "镜头导演");
        assert_eq!(summaries[0]["notes"], "电影感构图");
        assert_eq!(
            summaries[0]["capabilities"],
            serde_json::json!(["chat", "image_plan"])
        );
        assert!(summaries[0].get("content").is_none());

        let empty = list_skill_summaries(&data_dir, "不存在").unwrap();
        assert!(empty.is_empty());
        recycle(&data_dir);
    }

    #[test]
    fn use_skill_loads_package_content_and_nested_references_in_order() {
        let data_dir = command_test_data_dir("use-skill-context");
        let package = data_dir.join("skills").join("camera-director");
        std::fs::create_dir_all(package.join("references").join("nested")).unwrap();
        let content =
            "---\nname: 镜头导演\ncapabilities: [chat, image_plan]\n---\n# 镜头导演\n真实正文";
        std::fs::write(package.join("SKILL.md"), content).unwrap();
        std::fs::write(package.join("references").join("z.md"), "第三份").unwrap();
        std::fs::write(package.join("references").join("a.md"), "第一份").unwrap();
        std::fs::write(
            package.join("references").join("nested").join("b.md"),
            "第二份",
        )
        .unwrap();
        write_skill_manifest(&package);
        write_skill_index(
            &data_dir,
            &[SkillEntry {
                id: "skill-camera".into(),
                name: "镜头导演".into(),
                source_url: String::new(),
                notes: String::new(),
                content: "过期缓存正文".into(),
                directory: "camera-director".into(),
                source_path: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
            }],
        )
        .unwrap();

        let context = load_skill_context(&data_dir, "skill-camera").unwrap();
        assert_eq!(context.content, content);
        assert_eq!(context.references, vec!["第一份", "第二份", "第三份"]);
        assert_eq!(context.manifest.name, "镜头导演");
        recycle(&data_dir);
    }

    #[test]
    fn use_skill_rejects_package_when_manifest_hash_is_stale() {
        let data_dir = command_test_data_dir("use-skill-stale-manifest");
        let package = data_dir.join("skills").join("camera-director");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("SKILL.md"), "# 镜头导演\n原始正文").unwrap();
        write_skill_manifest(&package);
        std::fs::write(package.join("SKILL.md"), "# 镜头导演\n被改动的正文").unwrap();
        write_skill_index(
            &data_dir,
            &[SkillEntry {
                id: "skill-camera".into(),
                name: "镜头导演".into(),
                source_url: String::new(),
                notes: String::new(),
                content: String::new(),
                directory: "camera-director".into(),
                source_path: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
            }],
        )
        .unwrap();

        let error = load_skill_context(&data_dir, "skill-camera").unwrap_err();
        assert!(error.contains("manifest 哈希校验失败"));
        recycle(&data_dir);
    }

    #[test]
    fn agent_image_tasks_create_a_group_atomically() {
        let (data_dir, agent_session, reference_id) = agent_task_data_dir("agent-image-group");
        let group = create_agent_image_tasks_in_data_dir(
            &data_dir,
            agent_session.id.clone(),
            "skill-camera".into(),
            vec![agent_plan("电影感构图", "use", &[&reference_id])],
        )
        .unwrap();

        assert_eq!(group.tasks.len(), 1);
        assert_eq!(group.skill_id, "skill-camera");
        let task = &group.tasks[0];
        assert_eq!(task.origin, "agent");
        assert_eq!(task.agent_session_id, agent_session.id);
        assert_eq!(task.task_group_id, group.id);
        assert_eq!(task.skill_id, "skill-camera");
        assert_eq!(task.reference_paths.len(), 1);
        assert!(Path::new(&task.reference_paths[0]).starts_with(data_dir.join("references")));

        let history = read_history(&data_dir).unwrap();
        assert_eq!(history.len(), 1);
        let queue = read_queue(&data_dir).unwrap();
        assert_eq!(queue.waiting, vec![task.id.clone()]);
        let saved_session = session(&data_dir, &agent_session.id).unwrap();
        assert!(saved_session.task_group_ids.contains(&group.id));
        let summary = saved_session
            .messages
            .iter()
            .find_map(|message| message.task_group.as_ref())
            .unwrap();
        assert_eq!(summary.id, group.id);
        assert_eq!(summary.task_ids, vec![task.id.clone()]);
        recycle(&data_dir);
    }

    #[test]
    fn agent_image_tasks_reject_reference_ids_when_policy_is_none() {
        let (data_dir, agent_session, reference_id) = agent_task_data_dir("agent-image-none");
        let error = create_agent_image_tasks_in_data_dir(
            &data_dir,
            agent_session.id,
            "skill-camera".into(),
            vec![agent_plan("不使用参考图", "none", &[&reference_id])],
        )
        .unwrap_err();

        assert!(error.contains("referencePolicy=none"));
        assert!(read_history(&data_dir).unwrap().is_empty());
        assert!(read_queue(&data_dir).unwrap().waiting.is_empty());
        recycle(&data_dir);
    }

    #[test]
    fn agent_message_to_chat_value_keeps_attachment_metadata_only() {
        let message = AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".into(),
            status: "user".into(),
            content: "参考这张图".into(),
            attachments: vec![AgentAttachment {
                id: "ref-1".into(),
                path: "/Users/xiaole/secret/reference.png".into(),
                file_name: "reference.png".into(),
                mime_type: "image/png".into(),
                width: Some(640),
                height: Some(480),
            }],
            tool_call: None,
            questions: Vec::new(),
            skill_id: String::new(),
            skill_content_hash: String::new(),
            task_group: None,
            error: String::new(),
            created_at: utc_now(),
        };

        let value = agent_message_to_chat_value(&message);
        let content = value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        assert!(content.contains("<reference_attachments>"));
        assert!(content.contains("\"id\":\"ref-1\""));
        assert!(content.contains("\"fileName\":\"reference.png\""));
        assert!(content.contains("\"mimeType\":\"image/png\""));
        assert!(!content.contains("/Users/xiaole/secret/reference.png"));
    }

    #[test]
    fn agent_chat_provider_prefers_session_binding_and_checks_api_key() {
        let provider_a = ApiProvider {
            id: "chat-a".into(),
            name: "对话 A".into(),
            model_type: "chat".into(),
            base_url: "https://example.com/a".into(),
            api_key: "key-a".into(),
            proxy_url: String::new(),
            image_model: "model-a".into(),
            images_concurrency: 1,
            enabled: true,
            notes: String::new(),
        };
        let provider_b = ApiProvider {
            id: "chat-b".into(),
            name: "对话 B".into(),
            model_type: "chat".into(),
            base_url: "https://example.com/b".into(),
            api_key: "key-b".into(),
            proxy_url: String::new(),
            image_model: "model-b".into(),
            images_concurrency: 1,
            enabled: true,
            notes: String::new(),
        };
        let settings = Settings {
            active_chat_provider_id: provider_b.id.clone(),
            providers: vec![provider_a.clone(), provider_b.clone()],
            ..Settings::default()
        };
        let selected = agent_chat_provider(&settings, "chat-a").unwrap();
        assert_eq!(selected.id, provider_a.id);

        let keyless = Settings {
            active_chat_provider_id: provider_b.id.clone(),
            providers: vec![ApiProvider {
                api_key: String::new(),
                ..provider_b.clone()
            }],
            ..Settings::default()
        };
        let error = agent_chat_provider(&keyless, "chat-b").unwrap_err();
        assert!(error.contains("还没有填写 API Key"));
    }

    #[test]
    fn explicit_confirmation_accepts_clear_confirmation_phrases() {
        assert!(explicit_confirmation("请继续"));
        assert!(explicit_confirmation("yes"));
        assert!(explicit_confirmation("确认覆盖"));
        assert!(!explicit_confirmation("先看看再说"));
    }

    #[test]
    fn task_status_reports_missing_task_groups() {
        let data_dir = command_test_data_dir("missing-task-status");
        let error = task_status_records(&data_dir, "missing-group", "").unwrap_err();
        assert!(error.contains("找不到任务"));
        recycle(&data_dir);
    }

    fn command_test_data_dir(name: &str) -> PathBuf {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("agent-command-tests")
            .join(format!("{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn agent_task_data_dir(name: &str) -> (PathBuf, AgentSession, String) {
        let data_dir = command_test_data_dir(name);
        let provider = ApiProvider {
            id: "image-provider".into(),
            name: "测试生图模型".into(),
            model_type: "image-gpt".into(),
            base_url: "https://example.com/v1".into(),
            api_key: "test-key".into(),
            proxy_url: String::new(),
            image_model: "gpt-image-1".into(),
            images_concurrency: 1,
            enabled: true,
            notes: String::new(),
        };
        write_settings(
            &data_dir,
            &Settings {
                active_provider_id: provider.id.clone(),
                active_image_provider_id: provider.id.clone(),
                providers: vec![provider],
                ..Settings::default()
            },
        )
        .unwrap();

        let reference_path = data_dir.join("agent-reference.png");
        std::fs::write(&reference_path, b"\x89PNG\r\n\x1a\nagent-reference").unwrap();
        let mut agent_session = create_session(&data_dir, "chat-provider").unwrap();
        let reference_id = "ref-1".to_string();
        agent_session.messages.push(AgentMessage {
            id: "message-1".into(),
            role: "user".into(),
            status: "user".into(),
            content: "用参考图画一张图".into(),
            attachments: vec![AgentAttachment {
                id: reference_id.clone(),
                path: reference_path.to_string_lossy().into_owned(),
                file_name: "agent-reference.png".into(),
                mime_type: "image/png".into(),
                width: Some(1),
                height: Some(1),
            }],
            tool_call: None,
            questions: Vec::new(),
            skill_id: String::new(),
            skill_content_hash: String::new(),
            task_group: None,
            error: String::new(),
            created_at: utc_now(),
        });
        let agent_session = save_session(&data_dir, agent_session).unwrap();
        (data_dir, agent_session, reference_id)
    }

    fn agent_plan(title: &str, reference_policy: &str, reference_ids: &[&str]) -> AgentImagePlan {
        AgentImagePlan {
            title: title.into(),
            prompt: "一张完整的测试图片提示词".into(),
            provider_id: String::new(),
            resolution: "standard".into(),
            ratio: "1:1".into(),
            quality: "auto".into(),
            prompt_fidelity: "original".into(),
            reference_policy: reference_policy.into(),
            reference_ids: reference_ids.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn write_skill_manifest(package: &Path) {
        let audit = audit_skill_directory(package).unwrap();
        assert!(audit.allowed, "{:?}", audit.reasons);
        let manifest = audit.manifest.unwrap();
        std::fs::write(
            package.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn recycle(path: &Path) {
        if path.exists() {
            trash::delete(path).unwrap();
        }
    }

    fn template(id: &str, title: &str, content: &str, reference_paths: &[&str]) -> PromptTemplate {
        PromptTemplate {
            id: id.into(),
            title: title.into(),
            short_title: String::new(),
            category: String::new(),
            content: content.into(),
            reference_paths: reference_paths.iter().map(|path| (*path).into()).collect(),
            effect_image_path: String::new(),
            notes: String::new(),
            tags: Vec::new(),
            favorite: false,
            usage_count: 0,
            model_hint: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

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
        AboutInfo, ApiProvider, AppState, GenerateRequest, PromptTemplate, QueueSnapshot,
        ReferencePreview, Settings, SkillConversationMessage, SkillEntry, SkillFetchResult,
        SkillImagePlan, SkillPlanResult, SkillPlannerEvent, SkillPlannerQuestion, TaskRecord,
        TemplateFillEvent, TemplateImportResult,
    },
    services::{
        chat::{
            fill_skill_prompt as generate_prompt_from_skill, fill_template_response,
            plan_skill_response,
        },
        images::reference_preview,
        provider_bundle::{export_providers_json, read_providers_json},
        queue::{
            build_queue_snapshot, emit_queue_updated, ensure_queue_worker, recover_stale_running,
        },
        references::{
            persist_reference_paths, prune_unreferenced_files, prune_unreferenced_files_with_data,
        },
        skill::fetch_skill_markdown as fetch_skill_markdown_from_url,
        template_bundle::{export_templates_archive, import_templates_archive},
    },
    state::{record_operation, runtime_logs_text, RuntimeState},
    store::{
        enqueue_task, ensure_data_dir, next_template_id, normalize_request, normalize_settings,
        normalize_skill, normalize_template, params_from_request, provider_for_request,
        read_history, read_json, read_queue, read_settings, read_skills, read_templates,
        refresh_history_output_sizes, request_path, templates_path, upsert_history,
        is_safe_skill_directory, skill_directory_name, skill_package_path, skills_dir,
        write_history, write_json, write_queue, write_settings, write_skill_index,
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
    let record = enqueue_generation_request(&app, &data_dir, &settings, request)?;
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
            &app, &data_dir, &settings, request,
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
) -> Result<TaskRecord, String> {
    let mut request = normalize_request(request)?;
    let provider = provider_for_request(settings, request.provider_id.as_deref())?;
    request.provider_id = Some(provider.id.clone());
    if provider.api_key.trim().is_empty() {
        return Err(format!("API 源「{}」还没有填写 API Key", provider.name));
    }
    request.reference_paths = persist_reference_paths(data_dir, &request.reference_paths)?;

    let now = utc_now();
    let id = Uuid::new_v4().to_string();
    let record = TaskRecord {
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
        params: params_from_request(&request),
        reference_paths: request.reference_paths.clone(),
        outputs: Vec::new(),
        attempts: 0,
        error: None,
    };

    write_json(&request_path(data_dir, &id), &request)?;
    upsert_history(data_dir, record.clone())?;
    enqueue_task(data_dir, &id)?;
    let _ = emit_queue_updated(app, data_dir);
    Ok(record)
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
        fs::remove_file(request_file).map_err(|error| format!("删除任务请求失败: {error}"))?;
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
/// 新增或更新纯 Markdown Skill，名称始终从内容中自动提取。
pub(crate) fn save_skill(app: AppHandle, skill: SkillEntry) -> Result<Vec<SkillEntry>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut skills = read_skills(&data_dir)?;
    let mut next = normalize_skill(skill)?;
    let previous_directory = skills
        .iter()
        .find(|item| item.id == next.id)
        .map(|item| item.directory.clone());
    if next.id.is_empty() {
        next.id = Uuid::new_v4().to_string();
        next.created_at = utc_now();
    }
    let mut directory = skill_directory_name(&next.name, &next.id);
    let base_directory = directory.clone();
    let mut suffix = 2;
    while skills
        .iter()
        .any(|item| item.id != next.id && item.directory == directory)
    {
        directory = format!("{}-{}", base_directory, suffix);
        suffix += 1;
    }
    next.directory = directory;
    let package_dir = skills_dir(&data_dir).join(&next.directory);
    if let Some(previous_directory) = previous_directory
        .filter(|value| value != &next.directory && is_safe_skill_directory(value))
    {
        let previous_package_dir = skills_dir(&data_dir).join(previous_directory);
        if previous_package_dir.is_dir() {
            if package_dir.exists() {
                return Err(format!("Skill 目录已存在：{}", package_dir.display()));
            }
            fs::rename(previous_package_dir, &package_dir)
                .map_err(|error| format!("重命名 Skill 目录失败: {error}"))?;
        }
    }
    fs::create_dir_all(&package_dir).map_err(|error| format!("创建 Skill 目录失败: {error}"))?;
    fs::write(skill_package_path(&data_dir, &next.directory), format!("{}\n", next.content))
        .map_err(|error| format!("写入 SKILL.md 失败: {error}"))?;
    copy_skill_references(&next.source_path, &package_dir)?;
    next.updated_at = utc_now();
    if let Some(index) = skills.iter().position(|item| item.id == next.id) {
        next.created_at = skills[index].created_at.clone();
        skills[index] = next;
    } else {
        skills.push(next);
    }
    write_skill_index(&data_dir, &skills)?;
    Ok(skills)
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
            fs::remove_dir_all(package_dir).map_err(|error| format!("删除 Skill 目录失败: {error}"))?;
        }
    }
    skills.retain(|skill| skill.id != skill_id);
    write_skill_index(&data_dir, &skills)?;
    Ok(skills)
}

fn copy_skill_references(source_path: &str, package_dir: &Path) -> Result<(), String> {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return Ok(());
    }
    let source = Path::new(source_path);
    let source_dir = if source.is_dir() {
        source.to_path_buf()
    } else {
        source.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
    };
    let source_references = source_dir.join("references");
    if !source_references.is_dir() {
        return Ok(());
    }
    let target_references = package_dir.join("references");
    fs::create_dir_all(&target_references)
        .map_err(|error| format!("创建 Skill references 目录失败: {error}"))?;
    for entry in fs::read_dir(&source_references)
        .map_err(|error| format!("读取 Skill references 目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取 Skill reference 文件失败: {error}"))?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        let file_name = path
            .file_name()
            .ok_or("Skill reference 文件名无效")?;
        let bytes = fs::read(&path).map_err(|error| format!("读取 Skill reference 失败: {error}"))?;
        if bytes.len() > 1_048_576 {
            return Err(format!("Skill reference 文件超过 1 MB：{}", path.display()));
        }
        fs::write(target_references.join(file_name), bytes)
            .map_err(|error| format!("写入 Skill reference 失败: {error}"))?;
    }
    Ok(())
}

fn load_skill_package_content(data_dir: &Path, skill: &SkillEntry) -> Result<String, String> {
    let entry_path = skill_package_path(data_dir, &skill.directory);
    let content = if entry_path.is_file() {
        fs::read_to_string(&entry_path)
            .map_err(|error| format!("读取 Skill.md 失败: {error}"))?
    } else {
        skill.content.clone()
    };
    let references_dir = entry_path
        .parent()
        .map(|path| path.join("references"))
        .unwrap_or_default();
    if !references_dir.is_dir() {
        return Ok(content.trim().to_string());
    }
    let mut reference_paths = fs::read_dir(&references_dir)
        .map_err(|error| format!("读取 Skill references 目录失败: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        })
        .collect::<Vec<_>>();
    reference_paths.sort();
    let mut routed = content.trim().to_string();
    for path in reference_paths {
        let reference = fs::read_to_string(&path)
            .map_err(|error| format!("读取 Skill reference 失败: {error}"))?;
        if reference.trim().is_empty() {
            continue;
        }
        routed.push_str("\n\n<skill_reference path=\"");
        routed.push_str(&path.file_name().unwrap_or_default().to_string_lossy());
        routed.push_str("\">\n");
        routed.push_str(reference.trim());
        routed.push_str("\n</skill_reference>");
    }
    Ok(routed)
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
/// 把 Skill 规范和用户任务交给选定的对话模型，返回最终生图提示词。
pub(crate) async fn fill_skill_prompt(
    app: AppHandle,
    provider_id: String,
    skill: String,
    request: String,
) -> Result<String, String> {
    let skill = skill.trim();
    let request = request.trim();
    if skill.is_empty() {
        return Err("请先选择 Skill".into());
    }
    if request.is_empty() {
        return Err("请输入要交给 Skill 处理的任务".into());
    }

    let data_dir = ensure_data_dir(&app)?;
    let runtime_state = app.state::<RuntimeState>();
    runtime_state.push_log(
        "skill_fill.command.start",
        format!(
            "provider_id={} skill_len={} request_len={}",
            provider_id,
            skill.chars().count(),
            request.chars().count()
        ),
    );
    let settings = read_settings(&data_dir)?;
    let Some(provider) = settings
        .providers
        .iter()
        .find(|provider| provider.id == provider_id && provider.model_type == "chat")
    else {
        runtime_state.push_log(
            "skill_fill.command.provider_not_found",
            format!("provider_id={provider_id}"),
        );
        return Err("请选择可用的对话模型".into());
    };
    if provider.api_key.trim().is_empty() {
        return Err(format!("对话模型「{}」还没有填写 API Key", provider.name));
    }

    let result = generate_prompt_from_skill(provider, skill, request, Some(&runtime_state)).await;
    match &result {
        Ok(value) => runtime_state.push_log(
            "skill_fill.command.success",
            format!(
                "provider_id={} output_len={}",
                provider.id,
                value.chars().count()
            ),
        ),
        Err(error) => runtime_state.push_log(
            "skill_fill.command.error",
            format!("provider_id={} error={error}", provider.id),
        ),
    }
    result
}

#[tauri::command]
/// 根据用户显式指定的 Skill，规划单图或多图任务，并在需要时返回追问问题。
pub(crate) async fn plan_skill_generation(
    app: AppHandle,
    session_id: String,
    provider_id: String,
    skill_id: String,
    prompt: String,
    conversation: Vec<SkillConversationMessage>,
    has_reference_images: bool,
) -> Result<SkillPlanResult, String> {
    let prompt = prompt.trim().to_string();
    if session_id.trim().is_empty() {
        return Err("Skill 会话 ID 不能为空".into());
    }

    let data_dir = ensure_data_dir(&app)?;
    let runtime_state = app.state::<RuntimeState>();
    let settings = read_settings(&data_dir)?;
    let Some(provider) = settings
        .providers
        .iter()
        .find(|provider| provider.id == provider_id && provider.model_type == "chat")
    else {
        return Err("请选择可用的对话模型".into());
    };
    if provider.api_key.trim().is_empty() {
        return Err(format!("对话模型「{}」还没有填写 API Key", provider.name));
    }
    let skills = read_skills(&data_dir)?;
    let skill = skills
        .into_iter()
        .find(|item| item.id == skill_id)
        .ok_or("找不到指定的 Skill")?;

    let conversation = normalize_skill_conversation(conversation);
    let skill_content = load_skill_package_content(&data_dir, &skill)?;
    let routed_skill = route_skill_content(&skill_content, &prompt, &conversation);
    let system_prompt = build_skill_planner_system_prompt(&skill.name, &routed_skill);
    let user_content =
        build_skill_planner_user_content(&prompt, &conversation, has_reference_images);
    let request_summary = format!(
        "skill_id={} skill_name={} prompt_len={} conversation_turns={}",
        skill.id,
        skill.name,
        prompt.chars().count(),
        conversation.len()
    );

    let output = plan_skill_response(
        provider,
        &system_prompt,
        &user_content,
        request_summary,
        Some(&runtime_state),
        |event| {
            emit_skill_planner_event(
                &app,
                &session_id,
                event.phase,
                event.mode,
                event.chunk,
                event.message,
                event.elapsed_ms,
            )
        },
    )
    .await
    .map_err(|error| {
        emit_skill_planner_event(
            &app,
            &session_id,
            "error",
            "pending",
            String::new(),
            error.clone(),
            None,
        );
        error
    })?;

    let mut result = parse_skill_plan_result(&output.text)?;
    finalize_skill_plan_result(&mut result, &skill.name)?;
    result.stream_mode = output.mode;

    emit_skill_planner_event(
        &app,
        &session_id,
        "result",
        &result.stream_mode,
        String::new(),
        result.message.clone(),
        None,
    );
    Ok(result)
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

fn emit_skill_planner_event(
    app: &AppHandle,
    session_id: &str,
    phase: &str,
    mode: &str,
    chunk: String,
    message: String,
    elapsed_ms: Option<u64>,
) {
    let _ = app.emit(
        "skill-planner",
        SkillPlannerEvent {
            session_id: session_id.to_string(),
            phase: phase.to_string(),
            mode: mode.to_string(),
            chunk,
            message,
            elapsed_ms,
        },
    );
}

fn normalize_skill_conversation(
    conversation: Vec<SkillConversationMessage>,
) -> Vec<SkillConversationMessage> {
    conversation
        .into_iter()
        .filter_map(|message| {
            let role = message.role.trim().to_string();
            let content = message.content.trim().to_string();
            if role.is_empty() || content.is_empty() {
                None
            } else {
                Some(SkillConversationMessage { role, content })
            }
        })
        .collect()
}

fn build_skill_planner_system_prompt(skill_name: &str, routed_skill: &str) -> String {
    format!(
        concat!(
            "你是 Image Forge 的 Skill 路由规划器。用户已经显式指定了 Skill「{}」，你只能根据这个 Skill 工作，不要切换到别的 Skill。\n",
            "你会收到三类信息：\n",
            "1. 经过路由筛选后的 Skill 片段\n",
            "2. 用户原始需求\n",
            "3. 用户与产品弹窗之间的补充对话\n\n",
            "你的任务：\n",
            "- 判断信息是否足够开始生成图片。\n",
            "- 如果信息不足，返回 1 到 3 个最关键的补充问题，避免一次问太多。\n",
            "- 如果信息已经足够，直接产出最终图片计划。\n",
            "- 判断这次输出是否属于深度提示词；深度提示词请把 promptDepth 设为 deep，否则设为 normal。\n",
            "- 判断 promptFidelity，必须是 original、strict、off 之一。通常 deep 对应 strict。\n",
            "- 如果需要多张图，images 数组里一张图一个对象，不要把多张图塞进同一条 prompt。\n",
            "- 每条图片 prompt 都要可以直接交给生图模型使用，不要再写解释。\n\n",
            "- 必须明确输出 referenceImageUsage：use 表示提示词要配合参考图，not_needed 表示不需要参考图，optional 表示参考图可选。没有参考图时也必须填写 not_needed。\n",
            "严格返回 JSON，不要输出 Markdown、代码块或说明文字。JSON 结构如下：\n",
            "{{\n",
            "  \"status\": \"needs_input\" | \"ready\",\n",
            "  \"message\": \"一句中文说明\",\n",
            "  \"promptDepth\": \"deep\" | \"normal\",\n",
            "  \"promptFidelity\": \"original\" | \"strict\" | \"off\",\n",
            "  \"referenceImageUsage\": \"use\" | \"not_needed\" | \"optional\",\n",
            "  \"questions\": [{{ \"key\": \"field_key\", \"label\": \"要问用户的问题\", \"placeholder\": \"可选占位\", \"required\": true }}],\n",
            "  \"images\": [{{ \"title\": \"图片标题\", \"prompt\": \"最终生图提示词\" }}]\n",
            "}}\n\n",
            "规则：\n",
            "- status=needs_input 时，questions 必须非空，images 必须为空。\n",
            "- status=ready 时，images 必须非空，questions 必须为空。\n",
            "- key 使用英文或下划线命名。\n",
            "- prompt 里不要写“第几张图提示词如下”这类解释语。\n\n",
            "<skill_routed>\n{}\n</skill_routed>"
        ),
        skill_name.trim(),
        routed_skill.trim()
    )
}

fn build_skill_planner_user_content(
    prompt: &str,
    conversation: &[SkillConversationMessage],
    has_reference_images: bool,
) -> String {
    let prompt = if prompt.trim().is_empty() {
        "用户目前只指定了 Skill，尚未提供额外说明。请根据 Skill 自己判断需要补问什么。"
    } else {
        prompt.trim()
    };
    let mut content = format!("<user_request>\n{}\n</user_request>", prompt);
    content.push_str(if has_reference_images {
        "\n\n<reference_images>用户附加了参考图。你必须明确判断提示词是否需要配合这些参考图。</reference_images>"
    } else {
        "\n\n<reference_images>用户没有附加参考图。referenceImageUsage 必须为 not_needed。</reference_images>"
    });
    if !conversation.is_empty() {
        content.push_str("\n\n<dialogue>");
        for message in conversation {
            content.push_str(&format!(
                "\n{}: {}",
                if message.role == "assistant" {
                    "助手"
                } else {
                    "用户"
                },
                message.content
            ));
        }
        content.push_str("\n</dialogue>");
    }
    content
}

fn route_skill_content(
    skill_content: &str,
    prompt: &str,
    conversation: &[SkillConversationMessage],
) -> String {
    let skill_content = skill_content.trim();
    if skill_content.chars().count() <= 5000 {
        return skill_content.to_string();
    }

    let sections = split_skill_sections(skill_content);
    if sections.len() <= 6 {
        return skill_content.to_string();
    }

    let query = build_skill_query(prompt, conversation);
    let mut picked = Vec::new();
    for (index, (heading, body)) in sections.iter().enumerate() {
        let haystack = format!("{heading}\n{body}").to_lowercase();
        let hits = query
            .iter()
            .filter(|term| haystack.contains(term.as_str()))
            .count();
        if index == 0 || hits > 0 {
            picked.push((index, heading.as_str(), body.as_str(), hits));
        }
    }

    if picked.len() <= 1 {
        picked = sections
            .iter()
            .enumerate()
            .take(6)
            .map(|(index, (heading, body))| (index, heading.as_str(), body.as_str(), 0))
            .collect();
    } else {
        picked.sort_by(|left, right| right.3.cmp(&left.3).then(left.0.cmp(&right.0)));
        picked.truncate(6);
        picked.sort_by_key(|item| item.0);
    }

    let mut routed = String::new();
    for (_, heading, body, _) in picked {
        if !heading.is_empty() {
            routed.push_str(heading);
            routed.push('\n');
        }
        routed.push_str(body.trim());
        routed.push_str("\n\n");
    }
    routed.trim().to_string()
}

fn split_skill_sections(skill_content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_heading = String::new();
    let mut current_body = Vec::new();

    for line in skill_content.lines() {
        let trimmed = line.trim_start();
        let heading_text = trimmed.trim_start_matches('#');
        if trimmed.starts_with('#') && heading_text.starts_with(' ') {
            if !current_heading.is_empty() || !current_body.is_empty() {
                sections.push((current_heading.clone(), current_body.join("\n")));
            }
            current_heading = trimmed.to_string();
            current_body.clear();
        } else {
            current_body.push(line.to_string());
        }
    }

    if !current_heading.is_empty() || !current_body.is_empty() {
        sections.push((current_heading, current_body.join("\n")));
    }
    sections
}

fn build_skill_query(prompt: &str, conversation: &[SkillConversationMessage]) -> HashSet<String> {
    let mut terms = HashSet::new();
    collect_query_terms(prompt, &mut terms);
    for message in conversation {
        collect_query_terms(&message.content, &mut terms);
    }
    terms
}

fn collect_query_terms(value: &str, terms: &mut HashSet<String>) {
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || is_cjk(ch) {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            push_query_term(&current, terms);
            current.clear();
        }
    }
    if !current.is_empty() {
        push_query_term(&current, terms);
    }
}

fn push_query_term(value: &str, terms: &mut HashSet<String>) {
    let trimmed = value.trim();
    if trimmed.chars().count() < 2 {
        return;
    }
    terms.insert(trimmed.to_string());
    if trimmed.chars().count() > 8 {
        terms.insert(trimmed.chars().take(8).collect());
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

fn parse_skill_plan_result(text: &str) -> Result<SkillPlanResult, String> {
    let candidate = extract_json_body(text).unwrap_or(text.trim());
    serde_json::from_str(candidate).map_err(|error| {
        format!(
            "Skill 规划结果不是有效 JSON：{}。原始返回片段：{}",
            error,
            candidate.chars().take(200).collect::<String>()
        )
    })
}

fn extract_json_body(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
    {
        return Some(inner.trim());
    }
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|value| value.strip_suffix("```"))
    {
        return Some(inner.trim());
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end > start {
        Some(trimmed[start..=end].trim())
    } else {
        None
    }
}

fn finalize_skill_plan_result(
    result: &mut SkillPlanResult,
    skill_name: &str,
) -> Result<(), String> {
    result.skill_name = skill_name.trim().to_string();
    result.status = normalize_skill_status(&result.status, &result.questions, &result.images);
    result.message = result.message.trim().to_string();
    result.prompt_depth = match result.prompt_depth.trim() {
        "deep" => "deep".into(),
        _ => "normal".into(),
    };
    result.prompt_fidelity =
        normalize_prompt_fidelity_choice(&result.prompt_fidelity, &result.prompt_depth);
    result.reference_image_usage = match result.reference_image_usage.trim() {
        "use" | "optional" | "not_needed" => result.reference_image_usage.trim().to_string(),
        _ => {
            return Err(
                "Skill 必须明确说明是否需要配合参考图（use / optional / not_needed）".into(),
            )
        }
    };
    result.questions = result
        .questions
        .drain(..)
        .filter_map(normalize_skill_question)
        .take(3)
        .collect();
    result.images = result
        .images
        .drain(..)
        .filter_map(normalize_skill_image)
        .collect();

    match result.status.as_str() {
        "needs_input" => {
            result.images.clear();
            if result.questions.is_empty() {
                return Err("Skill 认为信息不足，但没有返回补充问题".into());
            }
        }
        "ready" => {
            result.questions.clear();
            if result.images.is_empty() {
                return Err("Skill 已准备生成，但没有返回图片提示词".into());
            }
        }
        _ => unreachable!(),
    }

    if result.message.is_empty() {
        result.message = if result.status == "ready" {
            format!(
                "Skill「{}」已生成 {} 条图片任务",
                result.skill_name,
                result.images.len()
            )
        } else {
            format!("Skill「{}」需要补充信息", result.skill_name)
        };
    }
    Ok(())
}

fn normalize_skill_status(
    status: &str,
    questions: &[SkillPlannerQuestion],
    images: &[SkillImagePlan],
) -> String {
    match status.trim() {
        "ready" if !images.is_empty() => "ready".into(),
        "needs_input" if !questions.is_empty() => "needs_input".into(),
        _ if !images.is_empty() => "ready".into(),
        _ => "needs_input".into(),
    }
}

fn normalize_prompt_fidelity_choice(value: &str, prompt_depth: &str) -> String {
    match value.trim() {
        "original" | "strict" | "off" => value.trim().to_string(),
        _ if prompt_depth == "deep" => "strict".into(),
        _ => "off".into(),
    }
}

fn normalize_skill_question(question: SkillPlannerQuestion) -> Option<SkillPlannerQuestion> {
    let key = question.key.trim().to_string();
    let label = question.label.trim().to_string();
    if key.is_empty() || label.is_empty() {
        return None;
    }
    Some(SkillPlannerQuestion {
        key,
        label,
        placeholder: question.placeholder.trim().to_string(),
        required: question.required,
    })
}

fn normalize_skill_image(image: SkillImagePlan) -> Option<SkillImagePlan> {
    let prompt = image.prompt.trim().to_string();
    if prompt.is_empty() {
        return None;
    }
    Some(SkillImagePlan {
        title: image.title.trim().to_string(),
        prompt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_package_content_includes_markdown_references() {
        let root = std::env::temp_dir().join(format!("image-forge-skill-{}", Uuid::new_v4()));
        let package = root.join("skills").join("image-director");
        fs::create_dir_all(package.join("references")).unwrap();
        fs::write(package.join("SKILL.md"), "# Image Director\n\nMain rules").unwrap();
        fs::write(package.join("references").join("camera.md"), "# Camera\n\nUse 50mm").unwrap();
        let skill = SkillEntry {
            id: "skill-id".into(),
            name: "image-director".into(),
            source_url: String::new(),
            notes: String::new(),
            content: String::new(),
            directory: "image-director".into(),
            source_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let content = load_skill_package_content(&root, &skill).unwrap();
        assert!(content.contains("Main rules"));
        assert!(content.contains("<skill_reference path=\"camera.md\">"));
        assert!(content.contains("Use 50mm"));
        fs::remove_dir_all(root).unwrap();
    }

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

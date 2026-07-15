use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    defaults::APP_BUILD_TIME,
    models::{
        AboutInfo, ApiProvider, AppState, GenerateRequest, PromptTemplate, QueueSnapshot,
        ReferencePreview, TaskRecord, TemplateImportResult,
    },
    services::{
        chat::fill_template,
        images::reference_preview,
        provider_bundle::{export_providers_json, read_providers_json},
        queue::{build_queue_snapshot, ensure_queue_worker, recover_stale_running},
        references::{persist_reference_paths, prune_unreferenced_files},
        template_bundle::{export_templates_archive, import_templates_archive},
    },
    state::RuntimeState,
    store::{
        enqueue_task, ensure_data_dir, next_template_id, normalize_request, normalize_settings,
        normalize_template, params_from_request, provider_for_request, read_history, read_json,
        read_queue, read_settings, read_templates, refresh_history_output_sizes, request_path,
        templates_path, upsert_history, write_history, write_json, write_queue, write_settings,
    },
    utils::utc_now,
};

#[tauri::command]
/// 返回关于弹窗需要的版本、编译时间和本次运行日志。
pub(crate) fn about_info(app: AppHandle) -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        build_time: APP_BUILD_TIME.into(),
        logs: app.state::<RuntimeState>().logs_text(),
    }
}

#[tauri::command]
/// 加载前端启动所需的完整应用状态，并恢复异常退出遗留的运行中任务。
pub(crate) fn load_app_state(app: AppHandle) -> Result<AppState, String> {
    let data_dir = ensure_data_dir(&app)?;
    recover_stale_running(&app, &data_dir)?;
    prune_unreferenced_files(&data_dir)?;
    let settings = read_settings(&data_dir)?;
    let mut history = read_history(&data_dir)?;
    if refresh_history_output_sizes(&mut history) {
        write_history(&data_dir, &history)?;
    }
    Ok(AppState {
        settings,
        history: history.clone(),
        queue: build_queue_snapshot(&app, &data_dir, history)?,
        templates: read_templates(&data_dir)?,
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
    destination: String,
    providers: Vec<ApiProvider>,
) -> Result<String, String> {
    export_providers_json(Path::new(&destination), &providers)
}

#[tauri::command]
/// 读取用户拖入导入框的 API 源 JSON 文件。
pub(crate) fn read_api_providers_file(path: String) -> Result<String, String> {
    read_providers_json(Path::new(&path))
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
    let mut request = normalize_request(request)?;
    let settings = read_settings(&data_dir)?;
    let provider = provider_for_request(&settings, request.provider_id.as_deref())?;
    request.provider_id = Some(provider.id.clone());

    if provider.api_key.trim().is_empty() {
        return Err(format!("API 源「{}」还没有填写 API Key", provider.name));
    }
    request.reference_paths = persist_reference_paths(&data_dir, &request.reference_paths)?;

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

    write_json(&request_path(&data_dir, &id), &request)?;
    upsert_history(&data_dir, record.clone())?;
    enqueue_task(&data_dir, &id)?;
    ensure_queue_worker(&app);
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
    Ok(())
}

/// 将任务输出图移入系统回收站，避免误删后无法找回。
fn delete_output_files_for_task(record: &TaskRecord) -> Result<(), String> {
    for output in &record.outputs {
        let path = PathBuf::from(&output.path);
        if path.is_file() {
            trash::delete(&path).map_err(|error| {
                format!(
                    "将生成图片移到回收站失败（{}）: {error}",
                    path.to_string_lossy()
                )
            })?;
        }
    }
    Ok(())
}

#[tauri::command]
/// 根据本地图片路径生成参考图预览信息。
pub(crate) fn reference_from_path(path: String) -> Result<ReferencePreview, String> {
    reference_preview(Path::new(&path))
}

#[tauri::command]
/// 从系统剪贴板读取 Finder 文件引用或图片，并保存为参考图资源。
pub(crate) fn reference_from_clipboard(app: AppHandle) -> Result<Option<ReferencePreview>, String> {
    crate::services::clipboard::reference_from_clipboard(&app)
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
    templates.sort_by(compare_template_id);
    write_json(&templates_path(&data_dir), &templates)?;
    prune_unreferenced_files(&data_dir)?;
    Ok(templates)
}

#[tauri::command]
/// 导出全部提示词模板、Markdown 清单和去重后的参考图资源。
pub(crate) fn export_templates(app: AppHandle, destination: String) -> Result<String, String> {
    let data_dir = ensure_data_dir(&app)?;
    let templates = read_templates(&data_dir)?;
    let archive = export_templates_archive(&templates, Path::new(&destination))?;
    Ok(archive.to_string_lossy().into_owned())
}

#[tauri::command]
/// 导入模板 ZIP，为新模板分配数字 ID，并跳过内容和参考图完全相同的模板。
pub(crate) fn import_templates(
    app: AppHandle,
    archive_path: String,
) -> Result<TemplateImportResult, String> {
    let data_dir = ensure_data_dir(&app)?;
    let imported = match import_templates_archive(&data_dir, Path::new(&archive_path)) {
        Ok(templates) => templates,
        Err(error) => {
            let _ = prune_unreferenced_files(&data_dir);
            return Err(error);
        }
    };
    let templates = read_templates(&data_dir)?;
    let result = merge_imported_templates(templates, imported)?;
    if let Err(error) = write_json(&templates_path(&data_dir), &result.templates) {
        let _ = prune_unreferenced_files(&data_dir);
        return Err(error);
    }
    prune_unreferenced_files(&data_dir)?;
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

    templates.sort_by(compare_template_id);
    Ok(TemplateImportResult {
        templates,
        imported_count,
        skipped_count,
    })
}

fn template_signature(template: &PromptTemplate) -> (String, String, Vec<String>) {
    let mut references = template.reference_paths.clone();
    references.sort();
    references.dedup();
    (
        template.title.trim().to_string(),
        template.content.trim().to_string(),
        references,
    )
}

/// 优先按数字 ID 排序模板，兼容旧数据中的非数字 ID。
fn compare_template_id(left: &PromptTemplate, right: &PromptTemplate) -> std::cmp::Ordering {
    match (left.id.parse::<u64>(), right.id.parse::<u64>()) {
        (Ok(left_id), Ok(right_id)) => left_id.cmp(&right_id),
        _ => left.id.cmp(&right.id),
    }
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
    let result = fill_template(provider, content, Some(&runtime_state)).await;
    match &result {
        Ok(value) => runtime_state.push_log(
            "ai_fill.command.success",
            format!(
                "provider_id={} output_len={}",
                provider.id,
                value.chars().count()
            ),
        ),
        Err(error) => runtime_state.push_log(
            "ai_fill.command.error",
            format!("provider_id={} error={}", provider.id, error),
        ),
    }
    result
}

#[tauri::command]
/// 从兼容 OpenAI 的 API 源读取可选模型列表。
pub(crate) async fn list_provider_models(provider: ApiProvider) -> Result<Vec<String>, String> {
    crate::services::models::list_provider_models(&provider).await
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
    fs::copy(&source, &target).map_err(|error| format!("保存到下载目录失败: {error}"))?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
/// 将图片复制到系统剪贴板。
pub(crate) fn copy_image_to_clipboard(path: String) -> Result<(), String> {
    crate::services::clipboard::copy_image_to_clipboard(Path::new(&path))
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
        ];
        let result = merge_imported_templates(existing, imported).unwrap();
        assert_eq!(result.imported_count, 1);
        assert_eq!(result.skipped_count, 1);
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

    fn template(id: &str, title: &str, content: &str, reference_paths: &[&str]) -> PromptTemplate {
        PromptTemplate {
            id: id.into(),
            title: title.into(),
            short_title: String::new(),
            category: String::new(),
            content: content.into(),
            reference_paths: reference_paths.iter().map(|path| (*path).into()).collect(),
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

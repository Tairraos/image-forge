use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    models::{
        AppState, GalleryItem, GalleryPayload, GalleryState, GalleryUpdate, GenerateRequest,
        PromptSnippet, PromptTemplate, QueueSnapshot, ReferencePreview, TaskRecord,
    },
    services::{
        images::reference_preview,
        queue::{build_queue_snapshot, ensure_queue_worker, recover_stale_running},
    },
    state::RuntimeState,
    store::{
        enqueue_task, ensure_data_dir, gallery_image_dir, normalize_request, normalize_settings,
        normalize_snippet, normalize_template, params_from_request, provider_for_request,
        read_gallery, read_history, read_json, read_queue, read_settings, read_snippets,
        read_templates, request_path, snippets_path, sync_gallery_categories, templates_path,
        upsert_history, write_gallery, write_history, write_json, write_queue, write_settings,
    },
    utils::{clean_text, extension_for_mime, file_stem, image_mime_type, utc_now},
};

#[tauri::command]
pub(crate) fn load_app_state(app: AppHandle) -> Result<AppState, String> {
    let data_dir = ensure_data_dir(&app)?;
    recover_stale_running(&app, &data_dir)?;
    let settings = read_settings(&data_dir)?;
    let history = read_history(&data_dir)?;
    Ok(AppState {
        settings,
        history: history.clone(),
        queue: build_queue_snapshot(&app, &data_dir, history)?,
        gallery: read_gallery(&data_dir)?,
        snippets: read_snippets(&data_dir)?,
        templates: read_templates(&data_dir)?,
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
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
pub(crate) fn queue_snapshot(app: AppHandle) -> Result<QueueSnapshot, String> {
    let data_dir = ensure_data_dir(&app)?;
    build_queue_snapshot(&app, &data_dir, read_history(&data_dir)?)
}

#[tauri::command]
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
pub(crate) fn cancel_task(app: AppHandle, task_id: String) -> Result<TaskRecord, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut queue = read_queue(&data_dir)?;
    let mut history = read_history(&data_dir)?;
    let record = history
        .iter_mut()
        .find(|item| item.id == task_id)
        .ok_or("找不到任务")?;

    if record.status == "queued" {
        queue.waiting.retain(|id| id != &task_id);
        record.status = "cancelled".into();
        record.completed_at = Some(utc_now());
    } else if record.status == "running" {
        app.state::<RuntimeState>()
            .cancel_requests
            .lock()
            .map_err(|_| "取消状态锁定失败")?
            .insert(task_id.clone());
        record.status = "cancelling".into();
    }

    record.updated_at = utc_now();
    let next = record.clone();
    write_history(&data_dir, &history)?;
    write_queue(&data_dir, &queue)?;
    Ok(next)
}

#[tauri::command]
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
pub(crate) fn delete_task(app: AppHandle, task_id: String) -> Result<(), String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut history = read_history(&data_dir)?;
    let Some(index) = history.iter().position(|item| item.id == task_id) else {
        return Err("找不到任务".into());
    };
    let status = history[index].status.as_str();
    if matches!(status, "queued" | "running" | "cancelling") {
        return Err("任务仍在执行或排队中，不能删除".into());
    }

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
    Ok(())
}

#[tauri::command]
pub(crate) fn promote_task(app: AppHandle, task_id: String) -> Result<QueueSnapshot, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut queue = read_queue(&data_dir)?;
    if let Some(index) = queue.waiting.iter().position(|id| id == &task_id) {
        let id = queue.waiting.remove(index);
        queue.waiting.insert(0, id);
        queue.updated_at = utc_now();
        write_queue(&data_dir, &queue)?;
    }
    queue_snapshot(app)
}

#[tauri::command]
pub(crate) fn reference_from_path(path: String) -> Result<ReferencePreview, String> {
    reference_preview(Path::new(&path))
}

#[tauri::command]
pub(crate) fn add_gallery_item(
    app: AppHandle,
    payload: GalleryPayload,
) -> Result<GalleryState, String> {
    let data_dir = ensure_data_dir(&app)?;
    let source = PathBuf::from(payload.path);
    if !source.is_file() {
        return Err("找不到图库图片".into());
    }
    let bytes = fs::read(&source).map_err(|error| format!("读取图库图片失败: {error}"))?;
    let mime_type = image_mime_type(&source, &bytes)?;
    let id = Uuid::new_v4().to_string();
    let extension = extension_for_mime(&mime_type);
    let image_dir = gallery_image_dir(&data_dir);
    fs::create_dir_all(&image_dir).map_err(|error| format!("创建图库目录失败: {error}"))?;
    let stored_path = image_dir.join(format!("{id}.{extension}"));
    fs::write(&stored_path, bytes).map_err(|error| format!("保存图库图片失败: {error}"))?;

    let now = utc_now();
    let mut gallery = read_gallery(&data_dir)?;
    let category = clean_text(payload.category, "默认");
    gallery.items.push(GalleryItem {
        id,
        name: clean_text(payload.name, &file_stem(&source)),
        category: category.clone(),
        note: payload.note.trim().to_string(),
        path: stored_path.to_string_lossy().into_owned(),
        mime_type,
        created_at: now.clone(),
        updated_at: now,
    });
    sync_gallery_categories(&mut gallery);
    write_gallery(&data_dir, &gallery)?;
    Ok(gallery)
}

#[tauri::command]
pub(crate) fn update_gallery_item(
    app: AppHandle,
    payload: GalleryUpdate,
) -> Result<GalleryState, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut gallery = read_gallery(&data_dir)?;
    let item = gallery
        .items
        .iter_mut()
        .find(|item| item.id == payload.id)
        .ok_or("找不到图库条目")?;
    if !payload.name.trim().is_empty() {
        item.name = payload.name.trim().to_string();
    }
    if !payload.category.trim().is_empty() {
        item.category = payload.category.trim().to_string();
    }
    item.note = payload.note.trim().to_string();
    item.updated_at = utc_now();
    sync_gallery_categories(&mut gallery);
    write_gallery(&data_dir, &gallery)?;
    Ok(gallery)
}

#[tauri::command]
pub(crate) fn delete_gallery_item(app: AppHandle, item_id: String) -> Result<GalleryState, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut gallery = read_gallery(&data_dir)?;
    if let Some(item) = gallery.items.iter().find(|item| item.id == item_id) {
        let _ = fs::remove_file(&item.path);
    }
    gallery.items.retain(|item| item.id != item_id);
    sync_gallery_categories(&mut gallery);
    write_gallery(&data_dir, &gallery)?;
    Ok(gallery)
}

#[tauri::command]
pub(crate) fn save_snippet(
    app: AppHandle,
    snippet: PromptSnippet,
) -> Result<Vec<PromptSnippet>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut snippets = read_snippets(&data_dir)?;
    let mut next = normalize_snippet(snippet)?;
    if next.id.is_empty() {
        next.id = Uuid::new_v4().to_string();
        next.created_at = utc_now();
    }
    next.updated_at = utc_now();
    if let Some(index) = snippets.iter().position(|item| item.id == next.id) {
        next.created_at = snippets[index].created_at.clone();
        snippets[index] = next;
    } else {
        snippets.push(next);
    }
    snippets.sort_by(|left, right| left.tag.cmp(&right.tag));
    write_json(&snippets_path(&data_dir), &snippets)?;
    Ok(snippets)
}

#[tauri::command]
pub(crate) fn delete_snippet(
    app: AppHandle,
    snippet_id: String,
) -> Result<Vec<PromptSnippet>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut snippets = read_snippets(&data_dir)?;
    snippets.retain(|snippet| snippet.id != snippet_id);
    write_json(&snippets_path(&data_dir), &snippets)?;
    Ok(snippets)
}

#[tauri::command]
pub(crate) fn save_template(
    app: AppHandle,
    template: PromptTemplate,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    let mut next = normalize_template(template)?;
    if next.id.is_empty() {
        next.id = Uuid::new_v4().to_string();
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
    templates.sort_by(|left, right| {
        right
            .favorite
            .cmp(&left.favorite)
            .then(left.title.cmp(&right.title))
    });
    write_json(&templates_path(&data_dir), &templates)?;
    Ok(templates)
}

#[tauri::command]
pub(crate) fn delete_template(
    app: AppHandle,
    template_id: String,
) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    templates.retain(|template| template.id != template_id);
    write_json(&templates_path(&data_dir), &templates)?;
    Ok(templates)
}

#[tauri::command]
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

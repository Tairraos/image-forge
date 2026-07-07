use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::Duration,
};

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, USER_AGENT},
    multipart, Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};
use url::Url;
use uuid::Uuid;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_IMAGE_MODEL: &str = "gpt-image-2";
const DEFAULT_PROVIDER_ID: &str = "default";
const APP_USER_AGENT: &str = "image-forge/0.2.3";
const MAX_HISTORY_ITEMS: usize = 300;
const MAX_PROVIDER_CONCURRENCY: u8 = 32;

struct RuntimeState {
    worker_active: Mutex<bool>,
    cancel_requests: Mutex<HashSet<String>>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            worker_active: Mutex::new(false),
            cancel_requests: Mutex::new(HashSet::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProvider {
    #[serde(default = "default_provider_id")]
    id: String,
    #[serde(default = "default_provider_name")]
    name: String,
    #[serde(default = "default_base_url")]
    base_url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_image_model")]
    image_model: String,
    #[serde(default = "default_provider_concurrency")]
    images_concurrency: u8,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    notes: String,
}

impl Default for ApiProvider {
    fn default() -> Self {
        Self {
            id: default_provider_id(),
            name: default_provider_name(),
            base_url: default_base_url(),
            api_key: String::new(),
            image_model: default_image_model(),
            images_concurrency: default_provider_concurrency(),
            enabled: true,
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default = "default_provider_id")]
    active_provider_id: String,
    #[serde(default)]
    providers: Vec<ApiProvider>,
    #[serde(default)]
    output_dir: Option<String>,
    #[serde(default)]
    input_dir: Option<String>,
    #[serde(default = "default_true")]
    auto_start_queue: bool,
    #[serde(default)]
    auto_retry: bool,
    #[serde(default = "default_true")]
    notifications_enabled: bool,
    #[serde(default, skip_serializing)]
    base_url: String,
    #[serde(default, skip_serializing)]
    api_key: String,
    #[serde(default, skip_serializing)]
    image_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_provider_id: default_provider_id(),
            providers: vec![ApiProvider::default()],
            output_dir: None,
            input_dir: None,
            auto_start_queue: true,
            auto_retry: false,
            notifications_enabled: true,
            base_url: String::new(),
            api_key: String::new(),
            image_model: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateRequest {
    #[serde(default)]
    provider_id: Option<String>,
    prompt: String,
    #[serde(default)]
    reference_paths: Vec<String>,
    #[serde(default)]
    mask_path: Option<String>,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default = "default_quality")]
    quality: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_count")]
    count: u8,
    #[serde(default)]
    background: String,
    #[serde(default)]
    output_compression: Option<u8>,
    #[serde(default)]
    input_fidelity: String,
    #[serde(default)]
    moderation: String,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            provider_id: None,
            prompt: String::new(),
            reference_paths: Vec::new(),
            mask_path: None,
            size: default_size(),
            quality: default_quality(),
            output_format: default_output_format(),
            count: default_count(),
            background: String::new(),
            output_compression: None,
            input_fidelity: String::new(),
            moderation: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationParams {
    size: String,
    quality: String,
    output_format: String,
    count: u8,
    background: String,
    output_compression: Option<u8>,
    input_fidelity: String,
    moderation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRecord {
    id: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
    prompt: String,
    provider_id: String,
    provider_name: String,
    mode: String,
    model: String,
    status: String,
    params: GenerationParams,
    reference_paths: Vec<String>,
    outputs: Vec<OutputImage>,
    #[serde(default)]
    attempts: u32,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputImage {
    path: String,
    file_name: String,
    mime_type: String,
    output_format: String,
    size: String,
    background: String,
    quality: String,
    revised_prompt: String,
    usage: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueRun {
    task_id: String,
    provider_id: String,
    provider_name: String,
    started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueState {
    #[serde(default)]
    waiting: Vec<String>,
    #[serde(default)]
    running: Vec<QueueRun>,
    #[serde(default = "utc_now")]
    updated_at: String,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            waiting: Vec::new(),
            running: Vec::new(),
            updated_at: utc_now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueSnapshot {
    waiting: Vec<TaskRecord>,
    running: Vec<TaskRecord>,
    recent: Vec<TaskRecord>,
    worker_active: bool,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    settings: Settings,
    history: Vec<TaskRecord>,
    queue: QueueSnapshot,
    gallery: GalleryState,
    snippets: Vec<PromptSnippet>,
    templates: Vec<PromptTemplate>,
    data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferencePreview {
    path: String,
    file_name: String,
    mime_type: String,
    file_size: u64,
    data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GalleryItem {
    id: String,
    name: String,
    category: String,
    note: String,
    path: String,
    mime_type: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GalleryState {
    #[serde(default)]
    items: Vec<GalleryItem>,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GalleryPayload {
    path: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GalleryUpdate {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptSnippet {
    #[serde(default)]
    id: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    content: String,
    #[serde(default = "utc_now")]
    created_at: String,
    #[serde(default = "utc_now")]
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptTemplate {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    short_title: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    usage_count: u32,
    #[serde(default)]
    model_hint: String,
    #[serde(default = "utc_now")]
    created_at: String,
    #[serde(default = "utc_now")]
    updated_at: String,
}

#[derive(Debug, Clone)]
struct ApiImageResult {
    bytes: Vec<u8>,
    revised_prompt: String,
    output_format: String,
    size: String,
    background: String,
    quality: String,
    usage: Value,
}

#[tauri::command]
fn load_app_state(app: AppHandle) -> Result<AppState, String> {
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
fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let data_dir = ensure_data_dir(&app)?;
    let settings = normalize_settings(settings);
    write_json(&settings_path(&data_dir), &settings)?;
    ensure_queue_worker(&app);
    Ok(settings)
}

#[tauri::command]
fn queue_snapshot(app: AppHandle) -> Result<QueueSnapshot, String> {
    let data_dir = ensure_data_dir(&app)?;
    build_queue_snapshot(&app, &data_dir, read_history(&data_dir)?)
}

#[tauri::command]
fn enqueue_generation(app: AppHandle, request: GenerateRequest) -> Result<TaskRecord, String> {
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
fn cancel_task(app: AppHandle, task_id: String) -> Result<TaskRecord, String> {
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
fn retry_task(app: AppHandle, task_id: String) -> Result<TaskRecord, String> {
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
fn promote_task(app: AppHandle, task_id: String) -> Result<QueueSnapshot, String> {
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
fn reference_from_path(path: String) -> Result<ReferencePreview, String> {
    reference_preview(Path::new(&path))
}

#[tauri::command]
fn add_gallery_item(app: AppHandle, payload: GalleryPayload) -> Result<GalleryState, String> {
    let data_dir = ensure_data_dir(&app)?;
    let source = PathBuf::from(payload.path);
    if !source.is_file() {
        return Err("找不到图库图片".into());
    }
    let bytes = fs::read(&source).map_err(|error| format!("读取图库图片失败: {error}"))?;
    let mime_type = image_mime_type(&source, &bytes)?;
    let id = Uuid::new_v4().to_string();
    let extension = extension_for_mime(&mime_type);
    let image_dir = data_dir.join("gallery").join("images");
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
fn update_gallery_item(app: AppHandle, payload: GalleryUpdate) -> Result<GalleryState, String> {
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
fn delete_gallery_item(app: AppHandle, item_id: String) -> Result<GalleryState, String> {
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
fn save_snippet(app: AppHandle, snippet: PromptSnippet) -> Result<Vec<PromptSnippet>, String> {
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
fn delete_snippet(app: AppHandle, snippet_id: String) -> Result<Vec<PromptSnippet>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut snippets = read_snippets(&data_dir)?;
    snippets.retain(|snippet| snippet.id != snippet_id);
    write_json(&snippets_path(&data_dir), &snippets)?;
    Ok(snippets)
}

#[tauri::command]
fn save_template(app: AppHandle, template: PromptTemplate) -> Result<Vec<PromptTemplate>, String> {
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
fn delete_template(app: AppHandle, template_id: String) -> Result<Vec<PromptTemplate>, String> {
    let data_dir = ensure_data_dir(&app)?;
    let mut templates = read_templates(&data_dir)?;
    templates.retain(|template| template.id != template_id);
    write_json(&templates_path(&data_dir), &templates)?;
    Ok(templates)
}

#[tauri::command]
fn mark_template_used(app: AppHandle, template_id: String) -> Result<Vec<PromptTemplate>, String> {
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
fn reveal_path(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("找不到文件".into());
    }
    Command::new("open")
        .arg("-R")
        .arg(path)
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法打开 Finder: {error}"))?;
    Ok(())
}

fn ensure_queue_worker(app: &AppHandle) {
    let state = app.state::<RuntimeState>();
    let Ok(mut active) = state.worker_active.lock() else {
        return;
    };
    if *active {
        return;
    }
    *active = true;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        worker_loop(app.clone()).await;
        if let Ok(mut active) = app.state::<RuntimeState>().worker_active.lock() {
            *active = false;
        }
        if let Ok(data_dir) = ensure_data_dir(&app) {
            if read_queue(&data_dir)
                .map(|queue| !queue.waiting.is_empty())
                .unwrap_or(false)
            {
                ensure_queue_worker(&app);
            }
        }
    });
}

async fn worker_loop(app: AppHandle) {
    loop {
        let mut started = false;
        while start_next_runnable_task(&app).unwrap_or(false) {
            started = true;
        }

        let done = ensure_data_dir(&app)
            .and_then(|data_dir| read_queue(&data_dir))
            .map(|queue| queue.waiting.is_empty() && queue.running.is_empty())
            .unwrap_or(true);
        if done {
            break;
        }

        let delay = if started { 200 } else { 700 };
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
}

fn start_next_runnable_task(app: &AppHandle) -> Result<bool, String> {
    let data_dir = ensure_data_dir(app)?;
    recover_stale_running(app, &data_dir)?;
    let settings = read_settings(&data_dir)?;
    let Some((task_id, provider)) = pop_next_runnable(&data_dir, &settings)? else {
        return Ok(false);
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_task(app.clone(), task_id.clone(), provider).await {
            if let Ok(data_dir) = ensure_data_dir(&app) {
                let mut record = read_history(&data_dir)
                    .ok()
                    .and_then(|history| history.into_iter().find(|item| item.id == task_id))
                    .unwrap_or_else(|| fallback_failed_record(&task_id, &error));
                record.status = "failed".into();
                record.error = Some(error);
                record.updated_at = utc_now();
                record.completed_at = Some(utc_now());
                let _ = upsert_history(&data_dir, record);
                let _ = clear_running_task(&data_dir, &task_id);
            }
        }
    });
    Ok(true)
}

async fn run_task(app: AppHandle, task_id: String, provider: ApiProvider) -> Result<(), String> {
    let data_dir = ensure_data_dir(&app)?;
    let request: GenerateRequest = read_json(&request_path(&data_dir, &task_id))?;
    let settings = read_settings(&data_dir)?;
    let mut record = history_record(&data_dir, &task_id)?.ok_or("找不到任务记录")?;
    let now = utc_now();
    record.status = "running".into();
    record.started_at.get_or_insert_with(|| now.clone());
    record.updated_at = now;
    record.attempts = record.attempts.saturating_add(1);
    record.error = None;
    upsert_history(&data_dir, record.clone())?;

    if is_cancel_requested(&app, &task_id) {
        mark_cancelled(&app, &data_dir, &task_id)?;
        clear_running_task(&data_dir, &task_id)?;
        return Ok(());
    }

    let client = http_client()?;
    let result = execute_generation(&client, &provider, &request).await;

    if is_cancel_requested(&app, &task_id) {
        mark_cancelled(&app, &data_dir, &task_id)?;
        clear_running_task(&data_dir, &task_id)?;
        return Ok(());
    }

    match result {
        Ok(images) => {
            let output_dir = output_dir_for(&data_dir, &settings)?;
            let outputs = save_outputs(&output_dir, &task_id, &request, images)?;
            record.outputs = outputs;
            record.status = "completed".into();
            record.error = None;
            record.completed_at = Some(utc_now());
            record.updated_at = utc_now();
            upsert_history(&data_dir, record)?;
            clear_running_task(&data_dir, &task_id)?;
            Ok(())
        }
        Err(error) => {
            clear_running_task(&data_dir, &task_id)?;
            if settings.auto_retry && record.attempts < 2 {
                record.status = "queued".into();
                record.error = Some(error);
                record.updated_at = utc_now();
                upsert_history(&data_dir, record)?;
                enqueue_task(&data_dir, &task_id)?;
                ensure_queue_worker(&app);
                Ok(())
            } else {
                record.status = "failed".into();
                record.error = Some(error);
                record.completed_at = Some(utc_now());
                record.updated_at = utc_now();
                upsert_history(&data_dir, record)?;
                Ok(())
            }
        }
    }
}

async fn execute_generation(
    client: &Client,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    call_images_api(client, provider, request).await
}

async fn call_images_api(
    client: &Client,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let base_url = normalize_base_url(&provider.base_url)?;
    if request.reference_paths.is_empty() && request.mask_path.is_none() {
        call_images_generation(client, &base_url, provider, request).await
    } else {
        call_images_edit(client, &base_url, provider, request).await
    }
}

async fn call_images_generation(
    client: &Client,
    base_url: &str,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let mut payload = Map::new();
    payload.insert("model".into(), json!(provider.image_model));
    payload.insert("prompt".into(), json!(request.prompt));
    payload.insert("n".into(), json!(request.count));
    payload.insert("output_format".into(), json!(request.output_format));
    insert_optional_text(&mut payload, "size", &request.size);
    insert_optional_text(&mut payload, "quality", &request.quality);
    insert_optional_text(&mut payload, "background", &request.background);
    insert_optional_text(&mut payload, "moderation", &request.moderation);
    if should_send_input_fidelity(&provider.image_model, &request.input_fidelity) {
        payload.insert("input_fidelity".into(), json!(request.input_fidelity));
    }
    if let Some(compression) = request.output_compression {
        payload.insert("output_compression".into(), json!(compression));
    }

    let url = format!("{base_url}/images/generations");
    let response = client
        .post(url)
        .bearer_auth(&provider.api_key)
        .header(ACCEPT, "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Images API 请求失败: {error}"))?;

    let body = checked_body(response, "Images API").await?;
    parse_images_response(client, &provider.api_key, &body, request).await
}

async fn call_images_edit(
    client: &Client,
    base_url: &str,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    if request.reference_paths.is_empty() {
        return Err("图像编辑需要至少一张参考图".into());
    }

    let mut form = multipart::Form::new()
        .text("model", provider.image_model.clone())
        .text("prompt", request.prompt.clone())
        .text("n", request.count.to_string())
        .text("output_format", request.output_format.clone());

    form = add_optional_text_part(form, "size", &request.size);
    form = add_optional_text_part(form, "quality", &request.quality);
    form = add_optional_text_part(form, "background", &request.background);
    form = add_optional_text_part(form, "moderation", &request.moderation);
    if should_send_input_fidelity(&provider.image_model, &request.input_fidelity) {
        form = form.text("input_fidelity", request.input_fidelity.clone());
    }
    if let Some(compression) = request.output_compression {
        form = form.text("output_compression", compression.to_string());
    }
    for path in &request.reference_paths {
        form = add_image_part(form, "image", Path::new(path))?;
    }
    if let Some(mask_path) = &request.mask_path {
        form = add_image_part(form, "mask", Path::new(mask_path))?;
    }

    let url = format!("{base_url}/images/edits");
    let response = client
        .post(url)
        .bearer_auth(&provider.api_key)
        .header(ACCEPT, "application/json")
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("Images API 编辑请求失败: {error}"))?;

    let body = checked_body(response, "Images API").await?;
    parse_images_response(client, &provider.api_key, &body, request).await
}

async fn parse_images_response(
    client: &Client,
    api_key: &str,
    body: &[u8],
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| format!("Images API 返回了无效 JSON: {error}"))?;
    if let Some(error) = value.get("error") {
        return Err(format_api_error("Images API", error));
    }
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Images API 未返回图像数据")?;
    let usage = value.get("usage").cloned().unwrap_or(Value::Null);
    let mut outputs = Vec::new();
    for item in data {
        let Some(object) = item.as_object() else {
            continue;
        };
        let bytes = if let Some(b64) = object.get("b64_json").and_then(Value::as_str) {
            general_purpose::STANDARD
                .decode(b64)
                .map_err(|error| format!("Images API 返回了无效 base64 图像: {error}"))?
        } else if let Some(url) = object.get("url").and_then(Value::as_str) {
            download_image_url(client, api_key, url).await?
        } else {
            continue;
        };
        outputs.push(ApiImageResult {
            bytes,
            revised_prompt: object
                .get("revised_prompt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            output_format: object
                .get("output_format")
                .or_else(|| value.get("output_format"))
                .and_then(Value::as_str)
                .unwrap_or(&request.output_format)
                .to_string(),
            size: object
                .get("size")
                .or_else(|| value.get("size"))
                .and_then(Value::as_str)
                .unwrap_or(&request.size)
                .to_string(),
            background: object
                .get("background")
                .or_else(|| value.get("background"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            quality: object
                .get("quality")
                .or_else(|| value.get("quality"))
                .and_then(Value::as_str)
                .unwrap_or(&request.quality)
                .to_string(),
            usage: usage.clone(),
        });
    }
    if outputs.is_empty() {
        Err("Images API 完成了请求，但没有图像数据".into())
    } else {
        Ok(outputs)
    }
}

async fn download_image_url(client: &Client, api_key: &str, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .header(ACCEPT, "image/*,*/*")
        .header(USER_AGENT, APP_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("下载图像失败: {error}"))?;
    if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
        response = client
            .get(url)
            .header(ACCEPT, "image/*,*/*")
            .header(USER_AGENT, APP_USER_AGENT)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|error| format!("带认证下载图像失败: {error}"))?;
    }
    checked_body(response, "图像下载").await
}

async fn checked_body(response: reqwest::Response, label: &str) -> Result<Vec<u8>, String> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("{label} 读取响应失败: {error}"))?
        .to_vec();
    if !status.is_success() {
        let text = String::from_utf8_lossy(&body);
        return Err(format!(
            "{label} 请求失败: HTTP {}: {}",
            status.as_u16(),
            text.trim()
        ));
    }
    Ok(body)
}

fn save_outputs(
    output_dir: &Path,
    task_id: &str,
    request: &GenerateRequest,
    images: Vec<ApiImageResult>,
) -> Result<Vec<OutputImage>, String> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let mut outputs = Vec::with_capacity(images.len());
    for (index, image) in images.into_iter().enumerate() {
        let output_format = normalize_output_format(if image.output_format.is_empty() {
            &request.output_format
        } else {
            &image.output_format
        });
        let extension = extension_for_format(&output_format, &image.bytes);
        let file_name = format!("{timestamp}-{task_id}-{:02}.{extension}", index + 1);
        let path = output_dir.join(&file_name);
        fs::write(&path, &image.bytes).map_err(|error| format!("保存生成图片失败: {error}"))?;
        outputs.push(OutputImage {
            path: path.to_string_lossy().into_owned(),
            file_name,
            mime_type: mime_for_format(&output_format).to_string(),
            output_format,
            size: image.size,
            background: image.background,
            quality: image.quality,
            revised_prompt: image.revised_prompt,
            usage: image.usage,
        });
    }
    Ok(outputs)
}

fn add_image_part(
    form: multipart::Form,
    field: &'static str,
    path: &Path,
) -> Result<multipart::Form, String> {
    if !path.is_file() {
        return Err(format!("找不到图像文件: {}", path.display()));
    }
    let bytes = fs::read(path).map_err(|error| format!("读取图像失败: {error}"))?;
    let mime_type = image_mime_type(path, &bytes)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image.png".into());
    let part = multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(&mime_type)
        .map_err(|error| format!("图像 MIME 类型无效: {error}"))?;
    Ok(form.part(field, part))
}

fn reference_preview(path: &Path) -> Result<ReferencePreview, String> {
    if !path.is_file() {
        return Err("找不到参考图文件".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("读取参考图失败: {error}"))?;
    let mime_type = image_mime_type(path, &bytes)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let data_url = format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(&bytes)
    );
    Ok(ReferencePreview {
        path: path.to_string_lossy().into_owned(),
        file_name,
        mime_type,
        file_size: bytes.len() as u64,
        data_url,
    })
}

fn image_mime_type(path: &Path, bytes: &[u8]) -> Result<String, String> {
    let mime_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".to_string()
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg".to_string()
    } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp".to_string()
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif".to_string()
    } else {
        mime_guess::from_path(path)
            .first()
            .map(|mime| mime.to_string())
            .unwrap_or_else(|| "application/octet-stream".into())
    };

    if mime_type.starts_with("image/") {
        Ok(mime_type)
    } else {
        Err("只支持图像文件".into())
    }
}

fn ensure_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("找不到应用数据目录: {error}"))?;
    for dir in [
        data_dir.join("outputs"),
        data_dir.join("requests"),
        data_dir.join("gallery").join("images"),
    ] {
        fs::create_dir_all(dir).map_err(|error| format!("创建应用目录失败: {error}"))?;
    }
    Ok(data_dir)
}

fn output_dir_for(data_dir: &Path, settings: &Settings) -> Result<PathBuf, String> {
    let dir = settings
        .output_dir
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("outputs"));
    fs::create_dir_all(&dir).map_err(|error| format!("创建输出目录失败: {error}"))?;
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err("输出目录不可用".into())
    }
}

fn read_settings(data_dir: &Path) -> Result<Settings, String> {
    read_json(&settings_path(data_dir)).map(normalize_settings)
}

fn normalize_settings(mut settings: Settings) -> Settings {
    if settings.providers.is_empty() {
        settings.providers = vec![ApiProvider {
            id: default_provider_id(),
            name: default_provider_name(),
            base_url: if settings.base_url.trim().is_empty() {
                default_base_url()
            } else {
                settings.base_url.clone()
            },
            api_key: settings.api_key.clone(),
            image_model: if settings.image_model.trim().is_empty() {
                default_image_model()
            } else {
                settings.image_model.clone()
            },
            images_concurrency: default_provider_concurrency(),
            enabled: true,
            notes: String::new(),
        }];
    }

    let mut seen = HashSet::new();
    let mut providers = Vec::new();
    for (index, provider) in settings.providers.into_iter().enumerate() {
        let mut provider = normalize_provider(provider, index + 1);
        if seen.contains(&provider.id) {
            provider.id = format!("{}-{}", provider.id, index + 1);
        }
        seen.insert(provider.id.clone());
        providers.push(provider);
    }
    if providers.is_empty() {
        providers.push(ApiProvider::default());
    }

    settings.active_provider_id = sanitize_id(&settings.active_provider_id);
    if !providers
        .iter()
        .any(|provider| provider.id == settings.active_provider_id)
    {
        settings.active_provider_id = providers[0].id.clone();
    }
    settings.providers = providers;
    settings.output_dir = settings.output_dir.and_then(|path| {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    settings.input_dir = settings.input_dir.and_then(|path| {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    settings
}

fn normalize_provider(provider: ApiProvider, index: usize) -> ApiProvider {
    let fallback_id;
    let id_source = if provider.id.trim().is_empty() {
        fallback_id = format!("provider-{index}");
        fallback_id.as_str()
    } else {
        provider.id.as_str()
    };
    let id = sanitize_id(id_source);
    ApiProvider {
        id,
        name: clean_text(provider.name, &format!("Provider {index}")),
        base_url: normalize_base_url(&provider.base_url).unwrap_or_else(|_| default_base_url()),
        api_key: provider.api_key.trim().to_string(),
        image_model: clean_text(provider.image_model, DEFAULT_IMAGE_MODEL),
        images_concurrency: provider
            .images_concurrency
            .clamp(1, MAX_PROVIDER_CONCURRENCY),
        enabled: provider.enabled,
        notes: provider.notes.trim().to_string(),
    }
}

fn provider_for_request(
    settings: &Settings,
    provider_id: Option<&str>,
) -> Result<ApiProvider, String> {
    let target = provider_id
        .map(sanitize_id)
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| settings.active_provider_id.clone());
    settings
        .providers
        .iter()
        .find(|provider| provider.id == target)
        .or_else(|| {
            settings
                .providers
                .iter()
                .find(|provider| provider.id == settings.active_provider_id)
        })
        .or_else(|| settings.providers.first())
        .cloned()
        .ok_or("还没有配置 API 源".into())
}

fn normalize_request(mut request: GenerateRequest) -> Result<GenerateRequest, String> {
    request.prompt = request.prompt.trim().to_string();
    if request.prompt.is_empty() {
        return Err("提示词不能为空".into());
    }
    request.reference_paths = request
        .reference_paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();
    request.count = request.count.clamp(1, 8);
    request.output_format = normalize_output_format(&request.output_format);
    request.quality = clean_text(request.quality, "auto");
    request.size = clean_text(request.size, "1024x1024");
    request.background = request.background.trim().to_string();
    request.input_fidelity = request.input_fidelity.trim().to_string();
    request.moderation = request.moderation.trim().to_string();
    request.output_compression = request.output_compression.map(|value| value.min(100));
    Ok(request)
}

fn params_from_request(request: &GenerateRequest) -> GenerationParams {
    GenerationParams {
        size: request.size.clone(),
        quality: request.quality.clone(),
        output_format: request.output_format.clone(),
        count: request.count,
        background: request.background.clone(),
        output_compression: request.output_compression,
        input_fidelity: request.input_fidelity.clone(),
        moderation: request.moderation.clone(),
    }
}

fn read_history(data_dir: &Path) -> Result<Vec<TaskRecord>, String> {
    let mut history: Vec<TaskRecord> = read_json(&history_path(data_dir))?;
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(history)
}

fn write_history(data_dir: &Path, history: &[TaskRecord]) -> Result<(), String> {
    let mut history = history.to_vec();
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    if history.len() > MAX_HISTORY_ITEMS {
        history.truncate(MAX_HISTORY_ITEMS);
    }
    write_json(&history_path(data_dir), &history)
}

fn upsert_history(data_dir: &Path, record: TaskRecord) -> Result<(), String> {
    let mut history = read_history(data_dir)?;
    if let Some(index) = history.iter().position(|item| item.id == record.id) {
        history[index] = record;
    } else {
        history.push(record);
    }
    write_history(data_dir, &history)
}

fn history_record(data_dir: &Path, task_id: &str) -> Result<Option<TaskRecord>, String> {
    Ok(read_history(data_dir)?
        .into_iter()
        .find(|record| record.id == task_id))
}

fn fallback_failed_record(task_id: &str, error: &str) -> TaskRecord {
    let now = utc_now();
    TaskRecord {
        id: task_id.into(),
        created_at: now.clone(),
        updated_at: now.clone(),
        started_at: None,
        completed_at: Some(now),
        prompt: String::new(),
        provider_id: String::new(),
        provider_name: String::new(),
        mode: String::new(),
        model: String::new(),
        status: "failed".into(),
        params: GenerationParams {
            size: String::new(),
            quality: String::new(),
            output_format: String::new(),
            count: 1,
            background: String::new(),
            output_compression: None,
            input_fidelity: String::new(),
            moderation: String::new(),
        },
        reference_paths: Vec::new(),
        outputs: Vec::new(),
        attempts: 0,
        error: Some(error.into()),
    }
}

fn read_queue(data_dir: &Path) -> Result<QueueState, String> {
    read_json(&queue_path(data_dir))
}

fn write_queue(data_dir: &Path, queue: &QueueState) -> Result<(), String> {
    let mut queue = queue.clone();
    queue.updated_at = utc_now();
    write_json(&queue_path(data_dir), &queue)
}

fn enqueue_task(data_dir: &Path, task_id: &str) -> Result<(), String> {
    let mut queue = read_queue(data_dir)?;
    queue.running.retain(|run| run.task_id != task_id);
    queue.waiting.retain(|id| id != task_id);
    queue.waiting.push(task_id.to_string());
    write_queue(data_dir, &queue)
}

fn pop_next_runnable(
    data_dir: &Path,
    settings: &Settings,
) -> Result<Option<(String, ApiProvider)>, String> {
    let mut queue = read_queue(data_dir)?;
    let running_counts = running_counts_by_provider(&queue);
    for index in 0..queue.waiting.len() {
        let task_id = queue.waiting[index].clone();
        let request: GenerateRequest = read_json(&request_path(data_dir, &task_id))?;
        let provider = provider_for_request(settings, request.provider_id.as_deref())?;
        let running = running_counts
            .get(&provider.id)
            .copied()
            .unwrap_or_default();
        if running >= provider.images_concurrency as usize {
            continue;
        }
        queue.waiting.remove(index);
        queue.running.push(QueueRun {
            task_id: task_id.clone(),
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            started_at: utc_now(),
        });
        write_queue(data_dir, &queue)?;
        return Ok(Some((task_id, provider)));
    }
    Ok(None)
}

fn running_counts_by_provider(queue: &QueueState) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for run in &queue.running {
        *counts.entry(run.provider_id.clone()).or_insert(0) += 1;
    }
    counts
}

fn clear_running_task(data_dir: &Path, task_id: &str) -> Result<(), String> {
    let mut queue = read_queue(data_dir)?;
    queue.running.retain(|run| run.task_id != task_id);
    write_queue(data_dir, &queue)
}

fn build_queue_snapshot(
    app: &AppHandle,
    data_dir: &Path,
    history: Vec<TaskRecord>,
) -> Result<QueueSnapshot, String> {
    let queue = read_queue(data_dir)?;
    let by_id: HashMap<String, TaskRecord> = history
        .iter()
        .map(|record| (record.id.clone(), record.clone()))
        .collect();
    let waiting = queue
        .waiting
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect();
    let running = queue
        .running
        .iter()
        .filter_map(|run| by_id.get(&run.task_id).cloned())
        .collect();
    let recent = history.into_iter().take(80).collect();
    let worker_active = app
        .state::<RuntimeState>()
        .worker_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);
    Ok(QueueSnapshot {
        waiting,
        running,
        recent,
        worker_active,
        updated_at: queue.updated_at,
    })
}

fn recover_stale_running(app: &AppHandle, data_dir: &Path) -> Result<(), String> {
    let worker_active = app
        .state::<RuntimeState>()
        .worker_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);
    if worker_active {
        return Ok(());
    }
    let mut queue = read_queue(data_dir)?;
    if queue.running.is_empty() {
        return Ok(());
    }
    let stale: Vec<String> = queue
        .running
        .iter()
        .map(|run| run.task_id.clone())
        .collect();
    queue.running.clear();
    for task_id in stale {
        if !queue.waiting.contains(&task_id) {
            queue.waiting.insert(0, task_id.clone());
        }
        if let Some(mut record) = history_record(data_dir, &task_id)? {
            if record.status == "running" || record.status == "cancelling" {
                record.status = "queued".into();
                record.updated_at = utc_now();
                upsert_history(data_dir, record)?;
            }
        }
    }
    write_queue(data_dir, &queue)
}

fn is_cancel_requested(app: &AppHandle, task_id: &str) -> bool {
    app.state::<RuntimeState>()
        .cancel_requests
        .lock()
        .map(|requests| requests.contains(task_id))
        .unwrap_or(false)
}

fn mark_cancelled(app: &AppHandle, data_dir: &Path, task_id: &str) -> Result<(), String> {
    if let Ok(mut requests) = app.state::<RuntimeState>().cancel_requests.lock() {
        requests.remove(task_id);
    }
    if let Some(mut record) = history_record(data_dir, task_id)? {
        record.status = "cancelled".into();
        record.error = Some("任务已取消".into());
        record.completed_at = Some(utc_now());
        record.updated_at = utc_now();
        upsert_history(data_dir, record)?;
    }
    Ok(())
}

fn read_gallery(data_dir: &Path) -> Result<GalleryState, String> {
    let mut gallery: GalleryState = read_json(&gallery_path(data_dir))?;
    sync_gallery_categories(&mut gallery);
    Ok(gallery)
}

fn write_gallery(data_dir: &Path, gallery: &GalleryState) -> Result<(), String> {
    write_json(&gallery_path(data_dir), gallery)
}

fn sync_gallery_categories(gallery: &mut GalleryState) {
    let mut categories: Vec<String> = gallery
        .items
        .iter()
        .map(|item| clean_text(item.category.clone(), "默认"))
        .collect();
    categories.sort();
    categories.dedup();
    if categories.is_empty() {
        categories.push("默认".into());
    }
    gallery.categories = categories;
}

fn read_snippets(data_dir: &Path) -> Result<Vec<PromptSnippet>, String> {
    read_json(&snippets_path(data_dir))
}

fn read_templates(data_dir: &Path) -> Result<Vec<PromptTemplate>, String> {
    read_json(&templates_path(data_dir))
}

fn normalize_snippet(mut snippet: PromptSnippet) -> Result<PromptSnippet, String> {
    snippet.tag = snippet.tag.trim().trim_start_matches('~').to_string();
    snippet.title = snippet.title.trim().to_string();
    snippet.category = clean_text(snippet.category, "常用");
    snippet.content = snippet.content.trim().to_string();
    if snippet.tag.is_empty() {
        snippet.tag = snippet
            .title
            .chars()
            .take(12)
            .collect::<String>()
            .trim()
            .to_string();
    }
    if snippet.tag.is_empty() || snippet.content.is_empty() {
        return Err("片段需要短标签和内容".into());
    }
    if snippet.title.is_empty() {
        snippet.title = snippet.tag.clone();
    }
    Ok(snippet)
}

fn normalize_template(mut template: PromptTemplate) -> Result<PromptTemplate, String> {
    template.title = template.title.trim().to_string();
    template.short_title = template.short_title.trim().to_string();
    template.category = clean_text(template.category, "常用");
    template.content = template.content.trim().to_string();
    template.notes = template.notes.trim().to_string();
    template.model_hint = template.model_hint.trim().to_string();
    template.tags = template
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .take(8)
        .collect();
    if template.title.is_empty() || template.content.is_empty() {
        return Err("模板需要标题和内容".into());
    }
    if template.short_title.is_empty() {
        template.short_title = template.title.chars().take(8).collect();
    }
    Ok(template)
}

fn read_json<T>(path: &Path) -> Result<T, String>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("解析 {} 失败: {error}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化 JSON 失败: {error}"))?;
    fs::write(path, text).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

fn queue_path(data_dir: &Path) -> PathBuf {
    data_dir.join("queue.json")
}

fn history_path(data_dir: &Path) -> PathBuf {
    data_dir.join("history.json")
}

fn request_path(data_dir: &Path, task_id: &str) -> PathBuf {
    data_dir.join("requests").join(format!("{task_id}.json"))
}

fn gallery_path(data_dir: &Path) -> PathBuf {
    data_dir.join("gallery").join("gallery.json")
}

fn snippets_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prompt-snippets.json")
}

fn templates_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prompt-templates.json")
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent(APP_USER_AGENT)
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))
}

fn normalize_base_url(base_url: &str) -> Result<String, String> {
    let raw = if base_url.trim().is_empty() {
        DEFAULT_BASE_URL
    } else {
        base_url.trim().trim_end_matches('/')
    };
    let mut url = Url::parse(raw).map_err(|_| "Base URL 必须是完整 URL".to_string())?;
    let mut path = url.path().trim_end_matches('/').to_string();
    for suffix in ["/images/generations", "/images/edits"] {
        if path.ends_with(suffix) {
            path.truncate(path.len() - suffix.len());
        }
    }
    if path.is_empty() {
        path = "/v1".into();
    }
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn insert_optional_text(map: &mut Map<String, Value>, key: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        map.insert(key.into(), json!(value));
    }
}

fn add_optional_text_part(
    form: multipart::Form,
    key: &'static str,
    value: &str,
) -> multipart::Form {
    let value = value.trim();
    if value.is_empty() {
        form
    } else {
        form.text(key, value.to_string())
    }
}

fn should_send_input_fidelity(model: &str, input_fidelity: &str) -> bool {
    !input_fidelity.trim().is_empty() && model.trim().to_lowercase() != "gpt-image-2"
}

fn normalize_output_format(format: &str) -> String {
    match format.trim().to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".into(),
        "webp" => "webp".into(),
        _ => "png".into(),
    }
}

fn extension_for_format(format: &str, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return "jpg";
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return "webp";
    }
    match format {
        "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn mime_for_format(format: &str) -> &'static str {
    match format {
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn format_api_error(label: &str, error: &Value) -> String {
    if let Some(object) = error.as_object() {
        let code = object
            .get("code")
            .or_else(|| object.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let message = object
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !code.is_empty() && !message.is_empty() {
            return format!("{label}: {code}: {message}");
        }
        if !message.is_empty() {
            return format!("{label}: {message}");
        }
        if !code.is_empty() {
            return format!("{label}: {code}");
        }
    }
    format!("{label}: {error}")
}

fn clean_text(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        DEFAULT_PROVIDER_ID.into()
    } else {
        out
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into())
}

fn utc_now() -> String {
    Utc::now().to_rfc3339()
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.into()
}

fn default_image_model() -> String {
    DEFAULT_IMAGE_MODEL.into()
}

fn default_provider_id() -> String {
    DEFAULT_PROVIDER_ID.into()
}

fn default_provider_name() -> String {
    "Default".into()
}

fn default_provider_concurrency() -> u8 {
    4
}

fn default_size() -> String {
    "1024x1024".into()
}

fn default_quality() -> String {
    "auto".into()
}

fn default_output_format() -> String {
    "png".into()
}

fn default_count() -> u8 {
    1
}

fn default_true() -> bool {
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState::new())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            if let Ok(data_dir) = ensure_data_dir(&handle) {
                let _ = recover_stale_running(&handle, &data_dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_gallery_item,
            cancel_task,
            delete_gallery_item,
            delete_snippet,
            delete_template,
            enqueue_generation,
            load_app_state,
            mark_template_used,
            promote_task,
            queue_snapshot,
            reference_from_path,
            retry_task,
            reveal_path,
            save_settings,
            save_snippet,
            save_template,
            update_gallery_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_url_strips_known_endpoints() {
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/images/generations").unwrap(),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn input_fidelity_skips_gpt_image_2() {
        assert!(!should_send_input_fidelity("gpt-image-2", "high"));
        assert!(should_send_input_fidelity("gpt-image-1", "high"));
    }

    #[test]
    fn provider_ids_are_stable() {
        assert_eq!(sanitize_id("OpenAI Official"), "OpenAI-Official");
        assert_eq!(sanitize_id(""), "default");
    }
}

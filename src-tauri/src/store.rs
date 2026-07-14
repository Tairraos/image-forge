use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::{
    defaults::{
        default_base_url, default_image_model, default_model_type, default_provider_concurrency,
        default_provider_id, default_provider_name, DEFAULT_IMAGE_MODEL, MAX_HISTORY_ITEMS,
        MAX_PROVIDER_CONCURRENCY,
    },
    models::{
        ApiProvider, GalleryState, GenerateRequest, GenerationParams, PromptTemplate, QueueRun,
        QueueState, Settings, TaskRecord,
    },
    utils::{
        clean_text, normalize_base_url, normalize_output_format, normalize_prompt_fidelity,
        normalize_quality, normalize_ratio, normalize_resolution, orientation_for_ratio,
        sanitize_id, size_for_preset, utc_now,
    },
};

pub(crate) fn ensure_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
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

pub(crate) fn output_dir_for(data_dir: &Path, settings: &Settings) -> Result<PathBuf, String> {
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

pub(crate) fn read_settings(data_dir: &Path) -> Result<Settings, String> {
    read_json(&settings_path(data_dir)).map(normalize_settings)
}

pub(crate) fn write_settings(data_dir: &Path, settings: &Settings) -> Result<(), String> {
    write_json(&settings_path(data_dir), settings)
}

pub(crate) fn normalize_settings(mut settings: Settings) -> Settings {
    if settings.providers.is_empty() {
        settings.providers = vec![ApiProvider {
            id: default_provider_id(),
            name: default_provider_name(),
            model_type: default_model_type(),
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

    let legacy_active_id = sanitize_id(&settings.active_provider_id);
    settings.active_image_provider_id = sanitize_id(&settings.active_image_provider_id);
    settings.active_chat_provider_id = sanitize_id(&settings.active_chat_provider_id);
    if settings.active_image_provider_id.is_empty()
        || !providers.iter().any(|provider| {
            provider.id == settings.active_image_provider_id && is_image_provider(provider)
        })
    {
        settings.active_image_provider_id = if providers
            .iter()
            .any(|provider| provider.id == legacy_active_id && is_image_provider(provider))
        {
            legacy_active_id.clone()
        } else {
            providers
                .iter()
                .find(|provider| is_image_provider(provider))
                .or_else(|| providers.first())
                .map(|provider| provider.id.clone())
                .unwrap_or_default()
        };
    }
    if settings.active_chat_provider_id.is_empty()
        || !providers.iter().any(|provider| {
            provider.id == settings.active_chat_provider_id && provider.model_type == "chat"
        })
    {
        settings.active_chat_provider_id = providers
            .iter()
            .find(|provider| provider.model_type == "chat")
            .map(|provider| provider.id.clone())
            .unwrap_or_default();
    }
    settings.active_provider_id = settings.active_image_provider_id.clone();
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

pub(crate) fn provider_for_request(
    settings: &Settings,
    provider_id: Option<&str>,
) -> Result<ApiProvider, String> {
    let target = provider_id
        .map(sanitize_id)
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| settings.active_image_provider_id.clone());
    settings
        .providers
        .iter()
        .find(|provider| provider.id == target && is_image_provider(provider))
        .or_else(|| {
            settings.providers.iter().find(|provider| {
                provider.id == settings.active_image_provider_id && is_image_provider(provider)
            })
        })
        .or_else(|| {
            settings
                .providers
                .iter()
                .find(|provider| is_image_provider(provider))
        })
        .cloned()
        .ok_or("还没有配置生图模型".into())
}

pub(crate) fn normalize_request(mut request: GenerateRequest) -> Result<GenerateRequest, String> {
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
    request.count = 1;
    request.output_format = normalize_output_format("png");
    request.resolution = normalize_resolution(&request.resolution);
    request.ratio = normalize_ratio(&request.ratio);
    request.orientation = orientation_for_ratio(&request.ratio);
    request.size = size_for_preset(&request.resolution, &request.ratio);
    request.quality = normalize_quality(&request.quality);
    request.background = String::new();
    request.input_fidelity = String::new();
    request.moderation = String::new();
    request.output_compression = None;
    request.prompt_fidelity = normalize_prompt_fidelity(&request.prompt_fidelity);
    Ok(request)
}

pub(crate) fn params_from_request(request: &GenerateRequest) -> GenerationParams {
    GenerationParams {
        size: request.size.clone(),
        resolution: request.resolution.clone(),
        ratio: request.ratio.clone(),
        orientation: request.orientation.clone(),
        quality: request.quality.clone(),
        output_format: request.output_format.clone(),
        count: request.count,
        background: request.background.clone(),
        output_compression: request.output_compression,
        input_fidelity: request.input_fidelity.clone(),
        moderation: request.moderation.clone(),
        prompt_fidelity: request.prompt_fidelity.clone(),
    }
}

pub(crate) fn read_history(data_dir: &Path) -> Result<Vec<TaskRecord>, String> {
    let mut history: Vec<TaskRecord> = read_json(&history_path(data_dir))?;
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(history)
}

pub(crate) fn write_history(data_dir: &Path, history: &[TaskRecord]) -> Result<(), String> {
    let mut history = history.to_vec();
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    if history.len() > MAX_HISTORY_ITEMS {
        history.truncate(MAX_HISTORY_ITEMS);
    }
    write_json(&history_path(data_dir), &history)
}

pub(crate) fn upsert_history(data_dir: &Path, record: TaskRecord) -> Result<(), String> {
    let mut history = read_history(data_dir)?;
    if let Some(index) = history.iter().position(|item| item.id == record.id) {
        history[index] = record;
    } else {
        history.push(record);
    }
    write_history(data_dir, &history)
}

pub(crate) fn history_record(data_dir: &Path, task_id: &str) -> Result<Option<TaskRecord>, String> {
    Ok(read_history(data_dir)?
        .into_iter()
        .find(|record| record.id == task_id))
}

pub(crate) fn fallback_failed_record(task_id: &str, error: &str) -> TaskRecord {
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
            resolution: String::new(),
            ratio: String::new(),
            orientation: String::new(),
            quality: String::new(),
            output_format: String::new(),
            count: 1,
            background: String::new(),
            output_compression: None,
            input_fidelity: String::new(),
            moderation: String::new(),
            prompt_fidelity: String::new(),
        },
        reference_paths: Vec::new(),
        outputs: Vec::new(),
        attempts: 0,
        error: Some(error.into()),
    }
}

pub(crate) fn read_queue(data_dir: &Path) -> Result<QueueState, String> {
    read_json(&queue_path(data_dir))
}

pub(crate) fn write_queue(data_dir: &Path, queue: &QueueState) -> Result<(), String> {
    let mut queue = queue.clone();
    queue.updated_at = utc_now();
    write_json(&queue_path(data_dir), &queue)
}

pub(crate) fn enqueue_task(data_dir: &Path, task_id: &str) -> Result<(), String> {
    let mut queue = read_queue(data_dir)?;
    queue.running.retain(|run| run.task_id != task_id);
    queue.waiting.retain(|id| id != task_id);
    queue.waiting.push(task_id.to_string());
    write_queue(data_dir, &queue)
}

pub(crate) fn pop_next_runnable(
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

pub(crate) fn clear_running_task(data_dir: &Path, task_id: &str) -> Result<(), String> {
    let mut queue = read_queue(data_dir)?;
    queue.running.retain(|run| run.task_id != task_id);
    write_queue(data_dir, &queue)
}

pub(crate) fn read_gallery(data_dir: &Path) -> Result<GalleryState, String> {
    let mut gallery: GalleryState = read_json(&gallery_path(data_dir))?;
    sync_gallery_categories(&mut gallery);
    Ok(gallery)
}

pub(crate) fn write_gallery(data_dir: &Path, gallery: &GalleryState) -> Result<(), String> {
    write_json(&gallery_path(data_dir), gallery)
}

pub(crate) fn sync_gallery_categories(gallery: &mut GalleryState) {
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

pub(crate) fn read_templates(data_dir: &Path) -> Result<Vec<PromptTemplate>, String> {
    read_json(&templates_path(data_dir))
}

pub(crate) fn normalize_template(mut template: PromptTemplate) -> Result<PromptTemplate, String> {
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
    if template.content.is_empty() {
        return Err("模板内容不能为空".into());
    }
    if template.title.is_empty() {
        template.title = template.content.chars().take(24).collect();
    }
    if template.short_title.is_empty() {
        template.short_title = template.title.chars().take(8).collect();
    }
    Ok(template)
}

pub(crate) fn next_template_id(templates: &[PromptTemplate]) -> String {
    let next = templates
        .iter()
        .filter_map(|template| template.id.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    next.to_string()
}

pub(crate) fn read_json<T>(path: &Path) -> Result<T, String>
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

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化 JSON 失败: {error}"))?;
    fs::write(path, text).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

pub(crate) fn request_path(data_dir: &Path, task_id: &str) -> PathBuf {
    data_dir.join("requests").join(format!("{task_id}.json"))
}

pub(crate) fn gallery_image_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("gallery").join("images")
}

pub(crate) fn templates_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prompt-templates.json")
}

fn normalize_provider(provider: ApiProvider, index: usize) -> ApiProvider {
    let fallback_id;
    let id_source = if provider.id.trim().is_empty() {
        fallback_id = format!("provider-{}", Uuid::new_v4());
        fallback_id.as_str()
    } else {
        provider.id.as_str()
    };
    let id = sanitize_id(id_source);
    ApiProvider {
        id,
        name: clean_text(provider.name, &format!("Provider {index}")),
        model_type: normalize_model_type(&provider.model_type),
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

fn normalize_model_type(value: &str) -> String {
    if value == "chat" {
        "chat".into()
    } else {
        "image".into()
    }
}

fn is_image_provider(provider: &ApiProvider) -> bool {
    provider.model_type != "chat"
}

fn running_counts_by_provider(queue: &QueueState) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for run in &queue.running {
        *counts.entry(run.provider_id.clone()).or_insert(0) += 1;
    }
    counts
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

fn gallery_path(data_dir: &Path) -> PathBuf {
    data_dir.join("gallery").join("gallery.json")
}

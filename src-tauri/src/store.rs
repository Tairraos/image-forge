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
    },
    models::{
        ApiProvider, GenerateRequest, GenerationParams, PromptTemplate, QueueRun, QueueState,
        Settings, SkillEntry, TaskRecord,
    },
    services::skill_installer::ensure_skill_manifest,
    state::record_operation,
    utils::{
        clean_text, image_size_from_path, normalize_base_url, normalize_output_format,
        normalize_prompt_fidelity, normalize_quality, normalize_ratio, normalize_resolution,
        orientation_for_ratio, sanitize_id, size_for_preset, utc_now,
    },
};

/// 确保应用数据目录和必要子目录存在，并返回 `~/.image-forge`。
pub(crate) fn ensure_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .home_dir()
        .map_err(|error| format!("找不到用户 Home 目录: {error}"))?
        .join(".image-forge");
    ensure_private_directory(&data_dir)?;
    for dir in [
        data_dir.join("outputs"),
        data_dir.join("requests"),
        data_dir.join("clipboard"),
        data_dir.join("references"),
        data_dir.join("skills"),
        data_dir.join("agent").join("sessions"),
        data_dir.join(".staging"),
    ] {
        ensure_private_directory(&dir)?;
    }
    recover_generation_transactions(&data_dir)?;
    Ok(data_dir)
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(path).map_err(|error| format!("创建应用目录失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置应用目录权限失败: {error}"))?;
    }
    Ok(())
}

/// 根据用户设置解析输出目录，未配置时使用应用数据目录下的 outputs。
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
    let path = settings_path(data_dir);
    let settings: Settings = read_json(&path)?;
    let original = serde_json::to_value(&settings).ok();
    let normalized = normalize_settings(settings);
    if original != serde_json::to_value(&normalized).ok() {
        write_json(&path, &normalized)?;
    }
    Ok(normalized)
}

pub(crate) fn write_settings(data_dir: &Path, settings: &Settings) -> Result<(), String> {
    write_json(&settings_path(data_dir), settings)
}

/// 兼容旧配置并归一化 API 源、默认模型和输出路径。
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
            proxy_url: String::new(),
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

/// 按请求指定 ID、当前激活源和首个可用源的顺序选择生图 API 源。
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

/// 将前端请求裁剪为当前支持的 Images API 参数集合。
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

/// 从完整请求中提取需要写入历史记录的生成参数。
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

/// 读取历史记录，并按创建时间倒序保存给队列快照使用。
pub(crate) fn read_history(data_dir: &Path) -> Result<Vec<TaskRecord>, String> {
    let mut history: Vec<TaskRecord> = read_json(&history_path(data_dir))?;
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(history)
}

/// 写入历史记录，同时限制最大条数防止 JSON 无限膨胀。
pub(crate) fn write_history(data_dir: &Path, history: &[TaskRecord]) -> Result<(), String> {
    let history = normalized_history(history);
    write_json(&history_path(data_dir), &history)
}

fn normalized_history(history: &[TaskRecord]) -> Vec<TaskRecord> {
    let mut history = history.to_vec();
    history.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    if history.len() > MAX_HISTORY_ITEMS {
        history.truncate(MAX_HISTORY_ITEMS);
    }
    history
}

pub(crate) fn refresh_history_output_sizes(history: &mut [TaskRecord]) -> bool {
    let mut changed = false;
    for record in history {
        for output in &mut record.outputs {
            let path = Path::new(&output.path);
            let Some(actual_size) = image_size_from_path(path) else {
                continue;
            };
            if output.size != actual_size {
                output.size = actual_size;
                changed = true;
            }
        }
    }
    changed
}

/// 按任务 ID 更新或插入历史记录。
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

/// 在历史文件缺失时构造失败记录，保证错误能回写到前端。
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
        origin: String::new(),
        agent_session_id: String::new(),
        task_group_id: String::new(),
        skill_id: String::new(),
        skill_content_hash: String::new(),
        agent_plan: None,
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

pub(crate) fn write_generation_batch(
    data_dir: &Path,
    requests: &[(String, GenerateRequest)],
    records: &[TaskRecord],
) -> Result<(), String> {
    if requests.len() != records.len() || requests.is_empty() {
        return Err("批量任务数据不完整".into());
    }
    let mut history = read_history(data_dir)?;
    let mut queue = read_queue(data_dir)?;
    for record in records {
        history.retain(|item| item.id != record.id);
        history.push(record.clone());
        queue.running.retain(|run| run.task_id != record.id);
        queue.waiting.retain(|task_id| task_id != &record.id);
        queue.waiting.push(record.id.clone());
    }

    write_generation_transaction(data_dir, requests, &history, &queue)
}

pub(crate) fn write_history_queue_transaction(
    data_dir: &Path,
    history: &[TaskRecord],
    queue: &QueueState,
) -> Result<(), String> {
    write_generation_transaction(data_dir, &[], history, queue)
}

fn write_generation_transaction(
    data_dir: &Path,
    requests: &[(String, GenerateRequest)],
    history: &[TaskRecord],
    queue: &QueueState,
) -> Result<(), String> {
    let history = normalized_history(history);
    let mut queue = queue.clone();
    queue.updated_at = utc_now();

    let transaction_id = format!("generation-batch-{}", Uuid::new_v4());
    let transaction_dir = data_dir.join(".staging").join(&transaction_id);
    let mut targets = requests
        .iter()
        .map(|(task_id, _)| format!("requests/{task_id}.json"))
        .collect::<Vec<_>>();
    targets.push("history.json".into());
    targets.push("queue.json".into());
    let transaction = GenerationTransaction {
        schema_version: 1,
        targets,
    };
    if let Err(error) = prepare_generation_transaction(
        data_dir,
        &transaction_dir,
        &transaction,
        requests,
        &history,
        &queue,
    ) {
        move_transaction_to_trash(&transaction_dir, "准备生成任务事务");
        return Err(error);
    }
    if let Err(error) = commit_generation_transaction(data_dir, &transaction_dir, &transaction) {
        let rollback = rollback_generation_transaction(data_dir, &transaction_dir, &transaction);
        return match rollback {
            Ok(()) => Err(format!("原子写入任务组失败，已回滚: {error}")),
            Err(rollback_error) => Err(format!(
                "原子写入任务组失败且回滚不完整: {error}；{rollback_error}"
            )),
        };
    }
    fs::write(transaction_dir.join("committed"), b"ok")
        .map_err(|error| format!("标记生成任务事务完成失败: {error}"))?;
    move_transaction_to_trash(&transaction_dir, "清理已提交生成任务事务");
    Ok(())
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationTransaction {
    schema_version: u32,
    targets: Vec<String>,
}

fn prepare_generation_transaction(
    data_dir: &Path,
    transaction_dir: &Path,
    transaction: &GenerationTransaction,
    requests: &[(String, GenerateRequest)],
    history: &[TaskRecord],
    queue: &QueueState,
) -> Result<(), String> {
    fs::create_dir_all(transaction_dir.join("new/requests"))
        .map_err(|error| format!("创建生成任务事务目录失败: {error}"))?;
    write_json(&transaction_dir.join("transaction.json"), transaction)?;
    for (task_id, request) in requests {
        write_json(
            &transaction_dir
                .join("new/requests")
                .join(format!("{task_id}.json")),
            request,
        )?;
    }
    write_json(&transaction_dir.join("new/history.json"), &history)?;
    write_json(&transaction_dir.join("new/queue.json"), queue)?;
    let _ = data_dir;
    Ok(())
}

fn commit_generation_transaction(
    data_dir: &Path,
    transaction_dir: &Path,
    transaction: &GenerationTransaction,
) -> Result<(), String> {
    for relative in &transaction.targets {
        let relative = safe_transaction_relative_path(relative)?;
        let target = data_dir.join(relative);
        let staged = transaction_dir.join("new").join(relative);
        let backup = transaction_dir.join("backups").join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建任务目标目录失败: {error}"))?;
        }
        if target.exists() {
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("创建任务备份目录失败: {error}"))?;
            }
            fs::rename(&target, &backup)
                .map_err(|error| format!("备份 {} 失败: {error}", target.display()))?;
        }
        fs::rename(&staged, &target)
            .map_err(|error| format!("提交 {} 失败: {error}", target.display()))?;
    }
    Ok(())
}

fn rollback_generation_transaction(
    data_dir: &Path,
    transaction_dir: &Path,
    transaction: &GenerationTransaction,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for relative in transaction.targets.iter().rev() {
        let relative = match safe_transaction_relative_path(relative) {
            Ok(relative) => relative,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let target = data_dir.join(relative);
        let staged = transaction_dir.join("new").join(relative);
        let backup = transaction_dir.join("backups").join(relative);
        if backup.exists() {
            if target.exists() {
                let abandoned = transaction_dir.join("rollback-current").join(relative);
                if let Some(parent) = abandoned.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(error) = fs::rename(&target, &abandoned) {
                    errors.push(format!("回收新文件 {} 失败: {error}", target.display()));
                    continue;
                }
            }
            if let Err(error) = fs::rename(&backup, &target) {
                errors.push(format!("恢复备份 {} 失败: {error}", target.display()));
            }
        } else if !staged.exists() && target.exists() {
            let abandoned = transaction_dir.join("rollback-current").join(relative);
            if let Some(parent) = abandoned.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(error) = fs::rename(&target, &abandoned) {
                errors.push(format!("回收新增文件 {} 失败: {error}", target.display()));
            }
        }
    }
    if errors.is_empty() {
        fs::write(transaction_dir.join("rolled-back"), b"ok")
            .map_err(|error| format!("标记生成任务事务已回滚失败: {error}"))?;
        move_transaction_to_trash(transaction_dir, "清理已回滚生成任务事务");
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

fn recover_generation_transactions(data_dir: &Path) -> Result<(), String> {
    let staging = data_dir.join(".staging");
    if !staging.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(&staging).map_err(|error| format!("读取生成任务事务目录失败: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("读取生成任务事务失败: {error}"))?
            .path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !path.is_dir() || !name.starts_with("generation-batch-") {
            continue;
        }
        if path.join("committed").is_file() || path.join("rolled-back").is_file() {
            move_transaction_to_trash(&path, "恢复时清理生成任务事务");
            continue;
        }
        let manifest_path = path.join("transaction.json");
        if !manifest_path.is_file() {
            move_transaction_to_trash(&path, "清理不完整生成任务事务");
            continue;
        }
        let transaction: GenerationTransaction = read_json(&manifest_path)?;
        if transaction.schema_version != 1 {
            return Err(format!(
                "无法恢复生成任务事务 {}：不支持 schemaVersion {}",
                name, transaction.schema_version
            ));
        }
        rollback_generation_transaction(data_dir, &path, &transaction)?;
        record_operation(
            "恢复生成任务事务",
            "成功",
            format!("transaction={name}"),
            None,
            None,
        );
    }
    Ok(())
}

fn safe_transaction_relative_path(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        Err(format!("生成任务事务包含不安全路径: {value}"))
    } else {
        Ok(path)
    }
}

fn move_transaction_to_trash(path: &Path, operation: &str) {
    if !path.exists() {
        return;
    }
    match trash::delete(path) {
        Ok(()) => record_operation(
            operation,
            "成功",
            format!("path={}", path.display()),
            None,
            None,
        ),
        Err(error) => record_operation(
            operation,
            "失败",
            format!("path={}", path.display()),
            None,
            Some(&error.to_string()),
        ),
    }
}

/// 把任务放到等待队列末尾，并去重运行/等待中的旧位置。
pub(crate) fn enqueue_task(data_dir: &Path, task_id: &str) -> Result<(), String> {
    let mut queue = read_queue(data_dir)?;
    queue.running.retain(|run| run.task_id != task_id);
    queue.waiting.retain(|id| id != task_id);
    queue.waiting.push(task_id.to_string());
    write_queue(data_dir, &queue)
}

/// 取出下一条未超过供应商并发限制的等待任务。
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

pub(crate) fn read_templates(data_dir: &Path) -> Result<Vec<PromptTemplate>, String> {
    let path = templates_path(data_dir);
    let mut templates: Vec<PromptTemplate> = read_json(&path)?;
    let mut changed = false;
    for template in &mut templates {
        changed |= migrate_template_title(template);
    }
    if changed {
        write_json(&path, &templates)?;
    }
    Ok(templates)
}

/// 清理模板字段并为旧数据补齐标题、短标题等兼容字段。
pub(crate) fn normalize_template(mut template: PromptTemplate) -> Result<PromptTemplate, String> {
    template.title = template.title.trim().to_string();
    template.short_title = template.short_title.trim().to_string();
    template.category = clean_text(template.category, "常用");
    template.content = template.content.trim().to_string();
    let mut seen_reference_paths = HashSet::new();
    template.reference_paths = template
        .reference_paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .filter(|path| seen_reference_paths.insert(path.clone()))
        .collect();
    template.effect_image_path = template.effect_image_path.trim().to_string();
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
        template.title = default_template_title(&template.content);
    }
    if template.short_title.is_empty() {
        template.short_title = template.title.chars().take(8).collect();
    }
    Ok(template)
}

/// 标题为空时，取内容第一行并限制为最多 24 个 Unicode 字符。
pub(crate) fn default_template_title(content: &str) -> String {
    content
        .trim()
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(24)
        .collect()
}

/// 把旧版“全文前 24 字”自动标题迁移为当前的首行标题规则。
fn migrate_template_title(template: &mut PromptTemplate) -> bool {
    let title = template.title.trim().to_string();
    let legacy_title = template.content.trim().chars().take(24).collect::<String>();
    let should_derive = title.is_empty() || title == legacy_title;
    let next_title = if should_derive {
        default_template_title(&template.content)
    } else {
        title
    };
    let mut changed = template.title != next_title;
    template.title = next_title;

    let next_short_title = template.title.chars().take(8).collect::<String>();
    if template.short_title.trim().is_empty()
        || (should_derive && template.short_title != next_short_title)
    {
        changed |= template.short_title != next_short_title;
        template.short_title = next_short_title;
    }
    changed
}

/// 从现有数字 ID 中计算下一个自增模板 ID。
pub(crate) fn next_template_id(templates: &[PromptTemplate]) -> String {
    let next = templates
        .iter()
        .filter_map(|template| template.id.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    next.to_string()
}

pub(crate) fn read_skills(data_dir: &Path) -> Result<Vec<SkillEntry>, String> {
    let mut skills: Vec<SkillEntry> = read_json(&skills_path(data_dir))?;
    let mut index_changed = false;
    let mut used_ids = HashSet::new();
    let mut used_directories = HashSet::new();
    for skill in &mut skills {
        let legacy_content = skill.content.trim().to_string();
        if skill.id.trim().is_empty() {
            skill.id = Uuid::new_v4().to_string();
            index_changed = true;
        }
        if !used_ids.insert(skill.id.clone()) {
            return Err(format!("Skill ID 重复：{}", skill.id));
        }
        if !is_safe_skill_directory(&skill.directory) {
            skill.directory = unique_skill_directory(&skill.name, &skill.id, &used_directories);
            index_changed = true;
        } else if used_directories.contains(&skill.directory) {
            return Err(format!("多个 Skill 共享包目录：{}", skill.directory));
        }
        used_directories.insert(skill.directory.clone());
        let package_dir = skills_dir(data_dir).join(&skill.directory);
        let mut entry_path = skill_entry_path(&package_dir);
        if entry_path.is_none() && !legacy_content.is_empty() {
            migrate_legacy_skill_package(data_dir, &skill.directory, &legacy_content)?;
            entry_path = Some(package_dir.join("SKILL.md"));
            index_changed = true;
        }
        if let Some(path) = entry_path {
            ensure_skill_manifest(&package_dir)
                .map_err(|error| format!("Skill「{}」无法加载：{error}", skill.name))?;
            skill.content = fs::read_to_string(path)
                .map_err(|error| format!("读取 Skill 包失败: {error}"))?
                .trim()
                .to_string();
            if !legacy_content.is_empty() {
                index_changed = true;
            }
        } else {
            return Err(format!("Skill「{}」缺少 SKILL.md", skill.name));
        }
    }
    if index_changed {
        write_skill_index(data_dir, &skills)?;
    }
    Ok(skills)
}

fn skill_entry_path(package_dir: &Path) -> Option<PathBuf> {
    [package_dir.join("SKILL.md"), package_dir.join("skill.md")]
        .into_iter()
        .find(|path| path.is_file())
}

fn migrate_legacy_skill_package(
    data_dir: &Path,
    directory: &str,
    content: &str,
) -> Result<(), String> {
    let package_dir = skills_dir(data_dir).join(directory);
    fs::create_dir_all(skills_dir(data_dir))
        .map_err(|error| format!("创建 Skill 包目录失败: {error}"))?;
    if package_dir.exists() {
        return Err(format!(
            "Skill 包目录已存在但缺少 SKILL.md：{}",
            package_dir.display()
        ));
    }

    let staging = data_dir
        .join(".staging")
        .join(format!("legacy-skill-{}", Uuid::new_v4()));
    if let Err(error) = fs::create_dir_all(&staging) {
        return Err(format!("创建 Skill 迁移临时目录失败: {error}"));
    }
    if let Err(error) = fs::write(staging.join("SKILL.md"), format!("{content}\n")) {
        let _ = trash::delete(&staging);
        return Err(format!("迁移旧 Skill 正文失败: {error}"));
    }
    if let Err(error) = ensure_skill_manifest(&staging) {
        let _ = trash::delete(&staging);
        return Err(error);
    }
    fs::rename(&staging, &package_dir).map_err(|error| {
        let _ = trash::delete(&staging);
        format!("安装迁移后的 Skill 包失败: {error}")
    })
}

fn unique_skill_directory(name: &str, id: &str, used: &HashSet<String>) -> String {
    let base = skill_directory_name(name, id);
    if !used.contains(&base) {
        return base;
    }
    let suffix = id.trim().chars().take(12).collect::<String>();
    let prefix = base
        .chars()
        .take(95usize.saturating_sub(suffix.chars().count()))
        .collect::<String>();
    format!("{prefix}-{suffix}")
}

pub(crate) fn write_skill_index(data_dir: &Path, skills: &[SkillEntry]) -> Result<(), String> {
    let mut stored_skills = skills.to_vec();
    for skill in &mut stored_skills {
        skill.content.clear();
        skill.source_path.clear();
    }
    let temporary = data_dir.join(format!(".skills-{}.tmp", Uuid::new_v4()));
    if let Err(error) = write_json(&temporary, &stored_skills) {
        let _ = trash::delete(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, skills_path(data_dir)).map_err(|error| {
        let _ = trash::delete(&temporary);
        format!("原子保存 Skill 索引失败: {error}")
    })
}

/// 将展示名称转换成安全、稳定的 Skill 包目录名。
pub(crate) fn skill_directory_name(name: &str, id: &str) -> String {
    let mut result = String::new();
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_') {
            result.push(ch.to_ascii_lowercase());
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    let result = result.trim_matches('-').to_string();
    if result.is_empty() {
        format!("skill-{}", id.trim().chars().take(12).collect::<String>())
    } else {
        result.chars().take(96).collect()
    }
}

pub(crate) fn is_safe_skill_directory(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_'))
}

/// 读取 JSON 文件；文件不存在时返回类型默认值，简化首启逻辑。
pub(crate) fn read_json<T>(path: &Path) -> Result<T, String>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        record_operation(
            "读取数据文件",
            "跳过",
            format!("path={} reason=not_exists", path.display()),
            None,
            None,
        );
        return Ok(T::default());
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            let message = format!("读取 {} 失败: {error}", path.display());
            record_operation(
                "读取数据文件",
                "失败",
                format!("path={}", path.display()),
                None,
                Some(&message),
            );
            return Err(message);
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => {
            record_operation(
                "读取数据文件",
                "成功",
                format!("path={} bytes={}", path.display(), text.len()),
                None,
                None,
            );
            Ok(value)
        }
        Err(error) => {
            let message = format!("解析 {} 失败: {error}", path.display());
            record_operation(
                "读取数据文件",
                "失败",
                format!("path={} bytes={}", path.display(), text.len()),
                None,
                Some(&message),
            );
            Err(message)
        }
    }
}

/// 以 pretty JSON 写入文件，并自动创建父目录。
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            let message = format!("创建目录失败: {error}");
            record_operation(
                "写入数据文件",
                "失败",
                format!("path={} parent={}", path.display(), parent.display()),
                None,
                Some(&message),
            );
            return Err(message);
        }
    }
    let text = match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(error) => {
            let message = format!("序列化 JSON 失败: {error}");
            record_operation(
                "写入数据文件",
                "失败",
                format!("path={}", path.display()),
                None,
                Some(&message),
            );
            return Err(message);
        }
    };
    match fs::write(path, &text) {
        Ok(()) => {
            record_operation(
                "写入数据文件",
                "成功",
                format!("path={} bytes={}", path.display(), text.len()),
                None,
                None,
            );
            Ok(())
        }
        Err(error) => {
            let message = format!("写入 {} 失败: {error}", path.display());
            record_operation(
                "写入数据文件",
                "失败",
                format!("path={} bytes={}", path.display(), text.len()),
                None,
                Some(&message),
            );
            Err(message)
        }
    }
}

pub(crate) fn request_path(data_dir: &Path, task_id: &str) -> PathBuf {
    data_dir.join("requests").join(format!("{task_id}.json"))
}

pub(crate) fn templates_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prompt-templates.json")
}

pub(crate) fn skills_path(data_dir: &Path) -> PathBuf {
    data_dir.join("skills.json")
}

pub(crate) fn skills_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("skills")
}

pub(crate) fn agent_sessions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("agent").join("sessions")
}

pub(crate) fn agent_session_path(data_dir: &Path, session_id: &str) -> PathBuf {
    agent_sessions_dir(data_dir).join(format!("{session_id}.json"))
}

pub(crate) fn read_agent_session(
    data_dir: &Path,
    session_id: &str,
) -> Result<crate::models::AgentSession, String> {
    read_json(&agent_session_path(data_dir, session_id))
}

pub(crate) fn write_agent_session(
    data_dir: &Path,
    session: &crate::models::AgentSession,
) -> Result<(), String> {
    write_json(&agent_session_path(data_dir, &session.id), session)
}

pub(crate) fn list_agent_sessions(
    data_dir: &Path,
) -> Result<Vec<crate::models::AgentSession>, String> {
    let dir = agent_sessions_dir(data_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| format!("读取 Agent 会话目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取 Agent 会话失败: {error}"))?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let path = entry.path();
        let session = read_json::<crate::models::AgentSession>(&path)?;
        sessions.push(session);
    }
    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

/// 归一化单个 API 源，隐藏并固定不再由界面维护的字段。
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
        name: clean_text(provider.name, &format!("供应商 {index}")),
        model_type: normalize_model_type(
            &provider.model_type,
            &provider.image_model,
            &provider.base_url,
        ),
        base_url: normalize_base_url(&provider.base_url).unwrap_or_else(|_| default_base_url()),
        api_key: provider.api_key.trim().to_string(),
        proxy_url: provider.proxy_url.trim().to_string(),
        image_model: clean_text(provider.image_model, DEFAULT_IMAGE_MODEL),
        images_concurrency: provider
            .images_concurrency
            .max(default_provider_concurrency()),
        enabled: provider.enabled,
        notes: String::new(),
    }
}

pub(crate) fn normalize_model_type(value: &str, model: &str, base_url: &str) -> String {
    match value.trim() {
        "chat" => "chat".into(),
        "image-gpt" | "image-gemini" | "image-grok" | "image-seedream" => value.into(),
        _ => recommend_image_model_type(model, base_url),
    }
}

pub(crate) fn recommend_image_model_type(model: &str, base_url: &str) -> String {
    let hint = format!("{model} {base_url}").to_lowercase();
    if ["gemini", "imagen", "nano-banana", "nano banana"]
        .iter()
        .any(|value| hint.contains(value))
    {
        "image-gemini".into()
    } else if hint.contains("grok") || hint.contains("api.x.ai") {
        "image-grok".into()
    } else if ["seedream", "byteplus", "volces", "ark."]
        .iter()
        .any(|value| hint.contains(value))
    {
        "image-seedream".into()
    } else {
        "image-gpt".into()
    }
}

fn is_image_provider(provider: &ApiProvider) -> bool {
    provider.model_type != "chat"
}

/// 统计每个 API 源当前运行任务数，用于控制并发。
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

#[cfg(test)]
mod transaction_tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("image-forge-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".staging")).unwrap();
        root
    }

    fn recycle(path: &Path) {
        if path.exists() {
            let _ = trash::delete(path);
        }
    }

    fn legacy_skill(id: &str, content: &str) -> SkillEntry {
        SkillEntry {
            id: id.into(),
            name: "Legacy Skill".into(),
            source_url: String::new(),
            notes: String::new(),
            content: content.into(),
            directory: String::new(),
            source_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn generation_batch_commits_requests_history_and_queue_together() {
        let root = temp_root("generation-batch");
        let record = fallback_failed_record("task-a", "placeholder");
        let request = GenerateRequest::default();
        write_generation_batch(&root, &[("task-a".into(), request)], &[record]).unwrap();

        assert!(request_path(&root, "task-a").is_file());
        assert_eq!(read_history(&root).unwrap().len(), 1);
        assert_eq!(read_queue(&root).unwrap().waiting, vec!["task-a"]);
        assert!(!root.join(".staging").read_dir().unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("generation-batch-")));
        recycle(&root);
    }

    #[test]
    fn history_queue_transaction_does_not_create_request_files() {
        let root = temp_root("history-queue-transaction");
        let mut record = fallback_failed_record("task-a", "cancelled");
        record.status = "cancelled".into();
        let queue = QueueState::default();

        write_history_queue_transaction(&root, &[record], &queue).unwrap();

        assert_eq!(read_history(&root).unwrap()[0].status, "cancelled");
        assert!(read_queue(&root).unwrap().waiting.is_empty());
        assert!(!request_path(&root, "task-a").exists());
        recycle(&root);
    }

    #[test]
    fn rollback_restores_old_files_after_a_partial_commit() {
        let root = temp_root("generation-rollback");
        let old_record = fallback_failed_record("old", "old");
        write_history(&root, &[old_record.clone()]).unwrap();
        write_queue(&root, &QueueState::default()).unwrap();
        let transaction_dir = root.join(".staging").join("generation-batch-test");
        let transaction = GenerationTransaction {
            schema_version: 1,
            targets: vec!["history.json".into(), "queue.json".into()],
        };
        prepare_generation_transaction(
            &root,
            &transaction_dir,
            &transaction,
            &[],
            &[fallback_failed_record("new", "new")],
            &QueueState::default(),
        )
        .unwrap();
        commit_generation_transaction(&root, &transaction_dir, &transaction).unwrap();
        rollback_generation_transaction(&root, &transaction_dir, &transaction).unwrap();

        assert_eq!(read_history(&root).unwrap()[0].id, "old");
        assert!(read_queue(&root).unwrap().waiting.is_empty());
        recycle(&root);
    }

    #[test]
    fn legacy_skill_index_migrates_content_to_package() {
        let root = temp_root("legacy-skill-index");
        fs::write(
            skills_path(&root),
            serde_json::to_vec(&vec![
                legacy_skill("legacy-a", "# Legacy Skill\n\n只聊天"),
                legacy_skill("legacy-b", "# Legacy Skill\n\n只生成图片计划"),
            ])
            .unwrap(),
        )
        .unwrap();

        let skills = read_skills(&root).unwrap();
        let first_package = skills_dir(&root).join(&skills[0].directory);
        let second_package = skills_dir(&root).join(&skills[1].directory);
        assert_eq!(skills[0].content, "# Legacy Skill\n\n只聊天");
        assert_ne!(skills[0].directory, skills[1].directory);
        assert!(first_package.join("SKILL.md").is_file());
        assert!(first_package.join("manifest.json").is_file());
        assert!(second_package.join("SKILL.md").is_file());
        assert!(second_package.join("manifest.json").is_file());
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(skills_path(&root)).unwrap()).unwrap();
        assert!(stored[0].get("content").is_none());
        assert!(stored[1].get("content").is_none());
        recycle(&root);
    }

    #[test]
    fn rejected_legacy_skill_keeps_original_index_content() {
        let root = temp_root("rejected-legacy-skill");
        let content = "# Legacy Skill\n\n请执行 scripts/render.py";
        fs::write(
            skills_path(&root),
            serde_json::to_vec(&vec![legacy_skill("legacy-id", content)]).unwrap(),
        )
        .unwrap();

        assert!(read_skills(&root).is_err());
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(skills_path(&root)).unwrap()).unwrap();
        assert_eq!(stored[0]["content"], content);
        recycle(&root);
    }
}

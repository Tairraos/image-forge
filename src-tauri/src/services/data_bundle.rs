// 数据导出/导入：按分类打包 ZIP，文件去重。
// 模板、对话、图片库可能引用相同的参考图文件，导出时只保留一份。

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    models::{AgentSession, PromptTemplate, Settings, TaskRecord},
    store,
    utils::utc_now,
};

const BUNDLE_FORMAT: &str = "image-forge-data-bundle";
const BUNDLE_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 5_000;
const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleManifest {
    format: String,
    version: u32,
    exported_at: String,
    has_settings: bool,
    settings: Option<Settings>,
    templates: Vec<PromptTemplate>,
    sessions: Vec<AgentSession>,
    tasks: Vec<TaskRecord>,
}

// ── 收集所有引用文件路径 ──

fn collect_file_paths(
    templates: &[PromptTemplate],
    sessions: &[AgentSession],
    tasks: &[TaskRecord],
) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for tpl in templates {
        for p in &tpl.reference_paths {
            if seen.insert(p.clone()) {
                paths.push(PathBuf::from(p));
            }
        }
        if !tpl.effect_image_path.is_empty() {
            let p = &tpl.effect_image_path;
            if seen.insert(p.clone()) {
                paths.push(PathBuf::from(p));
            }
        }
    }

    for session in sessions {
        for msg in &session.messages {
            for att in &msg.attachments {
                if !att.path.is_empty() {
                    if seen.insert(att.path.clone()) {
                        paths.push(PathBuf::from(&att.path));
                    }
                }
            }
            if let Some(tg) = &msg.task_group {
                // task_group 是 AgentTaskGroupSummary，只有 task_ids
                // 实际任务数据在 tasks 列表中，这里不需要额外收集
                let _ = tg;
            }
        }
    }

    for task in tasks {
        for p in &task.reference_paths {
            if seen.insert(p.clone()) {
                paths.push(PathBuf::from(p));
            }
        }
        for output in &task.outputs {
            if !output.path.is_empty() {
                if seen.insert(output.path.clone()) {
                    paths.push(PathBuf::from(&output.path));
                }
            }
        }
    }

    paths
}

/// 导出数据：按分类打包 ZIP，文件去重。
pub(crate) fn export_data_bundle(data_dir: &Path, categories: &[String]) -> Result<String, String> {
    let export: bool = categories.contains(&"settings".to_string());
    let export_templates = categories.contains(&"templates".to_string());
    let export_sessions = categories.contains(&"sessions".to_string());
    let export_tasks = categories.contains(&"tasks".to_string());

    let settings = if export {
        Some(store::read_settings(data_dir)?)
    } else {
        None
    };
    let templates = if export_templates {
        store::read_templates(data_dir)?
    } else {
        Vec::new()
    };
    let sessions = if export_sessions {
        store::list_agent_sessions(data_dir)?
    } else {
        Vec::new()
    };
    let mut tasks = if export_tasks {
        store::read_history(data_dir)?
    } else {
        Vec::new()
    };
    // 只保留已完成的
    tasks.retain(|t| t.status == "completed");

    let file_paths = collect_file_paths(&templates, &sessions, &tasks);

    // 按内容哈希去重
    let mut file_map: HashMap<String, (String, Vec<u8>)> = HashMap::new(); // src_path -> (archive_name, bytes)
    let mut archive_paths = HashSet::new();

    for src in &file_paths {
        let bytes = match read_file_bytes(src) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        let ext = src.extension().and_then(|v| v.to_str()).unwrap_or("png");
        let archive_name = format!("files/{}.{}", &hash[..16], ext);
        if archive_paths.insert(archive_name.clone()) {
            file_map.insert(src.to_string_lossy().to_string(), (archive_name, bytes));
        }
    }

    let manifest = BundleManifest {
        format: BUNDLE_FORMAT.to_string(),
        version: BUNDLE_VERSION,
        exported_at: utc_now(),
        has_settings: export,
        settings,
        templates: templates.clone(),
        sessions: sessions.clone(),
        tasks: tasks.clone(),
    };

    let archive_path = data_dir.join(format!(
        "export-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let file = File::create(&archive_path).map_err(|e| format!("创建 ZIP 文件失败: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest_json =
        serde_json::to_string(&manifest).map_err(|e| format!("序列化 manifest 失败: {e}"))?;
    zip.start_file(MANIFEST_NAME, options)
        .map_err(|e| format!("写入 manifest 失败: {e}"))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| format!("写入 manifest 失败: {e}"))?;

    for (archive_name, bytes) in file_map.values() {
        zip.start_file(archive_name, options)
            .map_err(|e| format!("写入文件失败: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("写入文件失败: {e}"))?;
    }

    zip.finish().map_err(|e| format!("完成 ZIP 失败: {e}"))?;

    let path_str = archive_path.to_string_lossy().to_string();
    Ok(path_str)
}

/// 导入数据从 ZIP
pub(crate) fn import_data_bundle(data_dir: &Path, file_path: &str) -> Result<ImportResult, String> {
    let path = PathBuf::from(file_path);
    let file = File::open(&path).map_err(|e| format!("打开 ZIP 失败: {e}"))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("读取文件信息失败: {e}"))?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(format!("文件过大: {} 字节", metadata.len()));
    }
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解析 ZIP 失败: {e}"))?;
    if archive.len() > MAX_ENTRY_COUNT {
        return Err(format!("ZIP 条目过多: {}", archive.len()));
    }

    // 读取 manifest
    let manifest_bytes = {
        let mut entry = archive
            .by_name(MANIFEST_NAME)
            .map_err(|e| format!("manifest.json 不存在: {e}"))?;
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("读取 manifest 失败: {e}"))?;
        buf
    };
    let manifest: BundleManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("解析 manifest 失败: {e}"))?;
    if manifest.format != BUNDLE_FORMAT {
        return Err(format!("不支持的格式: {}", manifest.format));
    }
    if manifest.version > BUNDLE_VERSION {
        return Err(format!("版本过高: {}", manifest.version));
    }

    let mut result = ImportResult::default();

    // 导入文件
    let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {e}"))?;
        let name = entry.name().to_string();
        if name == MANIFEST_NAME || !name.starts_with("files/") {
            continue;
        }
        if entry.size() > MAX_IMAGE_BYTES {
            continue;
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("读取文件失败: {e}"))?;
        file_map.insert(name, buf);
    }

    // 导入设置
    if manifest.has_settings {
        if let Some(settings) = &manifest.settings {
            store::write_settings(data_dir, settings)?;
            result.settings = 1;
        }
    }

    // 导入模板
    if !manifest.templates.is_empty() {
        let mut existing = store::read_templates(data_dir)?;
        let existing_ids: HashSet<_> = existing.iter().map(|t| t.id.clone()).collect();
        for tpl in &manifest.templates {
            if existing_ids.contains(&tpl.id) {
                existing.retain(|t| t.id != tpl.id);
            }
            existing.push(tpl.clone());
        }
        store::write_templates_to_db(data_dir, &existing)?;
        result.templates = manifest.templates.len();
    }

    // 导入会话
    if !manifest.sessions.is_empty() {
        for session in &manifest.sessions {
            store::write_agent_session(data_dir, session)?;
        }
        result.sessions = manifest.sessions.len();
    }

    // 导入图片库
    if !manifest.tasks.is_empty() {
        let mut history = store::read_history(data_dir)?;
        let history_ids: HashSet<_> = history.iter().map(|t| t.id.clone()).collect();
        for task in &manifest.tasks {
            if !history_ids.contains(&task.id) {
                history.push(task.clone());
            }
        }
        store::write_history(data_dir, &history)?;
        result.tasks = manifest.tasks.len();
    }

    Ok(result)
}

fn read_file_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("读取文件信息失败: {e}"))?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(format!("文件过大: {}", meta.len()));
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("读取文件失败: {e}"))?;
    Ok(buf)
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportResult {
    pub settings: usize,
    pub templates: usize,
    pub sessions: usize,
    pub tasks: usize,
}

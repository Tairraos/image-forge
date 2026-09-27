// 数据导出/导入：按分类打包 ZIP，文件去重。
// 模板、对话、图片库可能引用相同的参考图文件，导出时只保留一份。

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    models::{AgentSession, PromptTemplate, Settings, TaskRecord},
    state::record_operation,
    store,
    utils::utc_now,
};

const BUNDLE_FORMAT: &str = "image-forge-data-bundle";
const BUNDLE_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 5_000;
const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;

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
    /// 源机器绝对路径 → ZIP 内 files/<hash>.<ext> 的映射；
    /// 旧版数据包没有该字段（serde default 为空表），导入时保持原路径不落盘。
    #[serde(default)]
    files_index: HashMap<String, String>,
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
        files_index: file_map
            .iter()
            .map(|(src, (archive_name, _))| (src.clone(), archive_name.clone()))
            .collect(),
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

    // 读取 manifest（限制解压体积，防解压炸弹）
    let manifest_bytes = {
        let mut entry = archive
            .by_name(MANIFEST_NAME)
            .map_err(|e| format!("manifest.json 不存在: {e}"))?;
        let mut buf = Vec::new();
        entry
            .by_ref()
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("读取 manifest 失败: {e}"))?;
        if buf.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("manifest.json 过大".into());
        }
        buf
    };
    let mut manifest: BundleManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("解析 manifest 失败: {e}"))?;
    if manifest.format != BUNDLE_FORMAT {
        return Err(format!("不支持的格式: {}", manifest.format));
    }
    if manifest.version > BUNDLE_VERSION {
        return Err(format!("版本过高: {}", manifest.version));
    }

    let mut result = ImportResult::default();

    // 读取 ZIP 内的文件字节（每个条目都按上限截断读取，防止头部声明与实际不符）
    let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {e}"))?;
        let name = entry.name().to_string();
        if name == MANIFEST_NAME || !name.starts_with("files/") {
            continue;
        }
        let mut buf = Vec::new();
        entry
            .by_ref()
            .take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("读取文件失败: {e}"))?;
        if buf.len() as u64 > MAX_IMAGE_BYTES {
            continue;
        }
        file_map.insert(name, buf);
    }

    // 数据包导入涉及多张历史/模板/会话表的读改写，全程持有数据锁
    let _guard = store::data_lock();

    // 把数据包内的图片落盘并重映射路径：本机已存在的路径保留原样（同机备份恢复）。
    let mut remapped: HashMap<String, String> = HashMap::new();
    let mut imported_files = 0usize;
    remap_task_paths(
        data_dir,
        &mut manifest.tasks,
        &manifest.files_index,
        &file_map,
        &mut remapped,
        &mut imported_files,
    );
    remap_template_paths(
        data_dir,
        &mut manifest.templates,
        &manifest.files_index,
        &file_map,
        &mut remapped,
        &mut imported_files,
    );
    remap_session_paths(
        data_dir,
        &mut manifest.sessions,
        &manifest.files_index,
        &file_map,
        &mut remapped,
        &mut imported_files,
    );
    record_operation(
        "导入数据包文件",
        "成功",
        format!("files={} entries={}", imported_files, file_map.len()),
        None,
        None,
    );

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

/// 输出图按任务完成时间归入 outputs/<年>/<月>/，与桌面端迁移规则一致。
fn outputs_dir_for(data_dir: &Path, task: &TaskRecord) -> PathBuf {
    let value = task
        .completed_at
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(task.created_at.as_str());
    let year_month = chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&chrono::Local).format("%Y-%m").to_string())
        .ok()
        .filter(|value| {
            let bytes = value.as_bytes();
            value.len() == 7 && bytes[4] == b'-'
        })
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    data_dir
        .join("outputs")
        .join(&year_month[..4])
        .join(&year_month[5..7])
}

fn references_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("references")
}

/// 把数据包内文件写到本机数据目录并返回新路径；同一源路径只落盘一次。
/// 本机已存在该路径（同机恢复）或数据包缺失对应文件时返回 None，保留原路径。
fn materialize_path(
    original: &str,
    target_dir: &Path,
    files_index: &HashMap<String, String>,
    file_map: &HashMap<String, Vec<u8>>,
    remapped: &mut HashMap<String, String>,
) -> Option<String> {
    if let Some(cached) = remapped.get(original) {
        return Some(cached.clone());
    }
    if Path::new(original).is_file() {
        return None;
    }
    let archive_name = files_index.get(original)?;
    let bytes = file_map.get(archive_name)?;
    let file_name = Path::new(original)
        .file_name()?
        .to_string_lossy()
        .into_owned();
    fs::create_dir_all(target_dir).ok()?;
    let target = unique_target(target_dir, &file_name);
    fs::write(&target, bytes).ok()?;
    let new_path = target.to_string_lossy().into_owned();
    remapped.insert(original.to_string(), new_path.clone());
    Some(new_path)
}

fn unique_target(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_string();
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .to_string();
    for index in 2.. {
        let next = directory.join(format!("{stem}-{index}.{extension}"));
        if !next.exists() {
            return next;
        }
    }
    unreachable!()
}

fn remap_task_paths(
    data_dir: &Path,
    tasks: &mut [TaskRecord],
    files_index: &HashMap<String, String>,
    file_map: &HashMap<String, Vec<u8>>,
    remapped: &mut HashMap<String, String>,
    imported: &mut usize,
) {
    for task in tasks {
        let outputs_dir = outputs_dir_for(data_dir, task);
        let references = references_dir(data_dir);
        for output in &mut task.outputs {
            if output.path.is_empty() {
                continue;
            }
            if let Some(new_path) =
                materialize_path(&output.path, &outputs_dir, files_index, file_map, remapped)
            {
                output.file_name = Path::new(&new_path)
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_else(|| output.file_name.clone());
                output.path = new_path;
                *imported += 1;
            }
        }
        for path in &mut task.reference_paths {
            if path.is_empty() {
                continue;
            }
            if let Some(new_path) =
                materialize_path(path, &references, files_index, file_map, remapped)
            {
                *path = new_path;
                *imported += 1;
            }
        }
    }
}

fn remap_template_paths(
    data_dir: &Path,
    templates: &mut [PromptTemplate],
    files_index: &HashMap<String, String>,
    file_map: &HashMap<String, Vec<u8>>,
    remapped: &mut HashMap<String, String>,
    imported: &mut usize,
) {
    let references = references_dir(data_dir);
    for template in templates {
        for path in &mut template.reference_paths {
            if path.is_empty() {
                continue;
            }
            if let Some(new_path) =
                materialize_path(path, &references, files_index, file_map, remapped)
            {
                *path = new_path;
                *imported += 1;
            }
        }
        if !template.effect_image_path.is_empty() {
            if let Some(new_path) = materialize_path(
                &template.effect_image_path,
                &references,
                files_index,
                file_map,
                remapped,
            ) {
                template.effect_image_path = new_path;
                *imported += 1;
            }
        }
    }
}

fn remap_session_paths(
    data_dir: &Path,
    sessions: &mut [AgentSession],
    files_index: &HashMap<String, String>,
    file_map: &HashMap<String, Vec<u8>>,
    remapped: &mut HashMap<String, String>,
    imported: &mut usize,
) {
    let references = references_dir(data_dir);
    for session in sessions {
        for message in &mut session.messages {
            for attachment in &mut message.attachments {
                if attachment.path.is_empty() {
                    continue;
                }
                if let Some(new_path) = materialize_path(
                    &attachment.path,
                    &references,
                    files_index,
                    file_map,
                    remapped,
                ) {
                    attachment.file_name = Path::new(&new_path)
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_else(|| attachment.file_name.clone());
                    attachment.path = new_path;
                    *imported += 1;
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OutputImage;
    use crate::store::fallback_failed_record;
    use uuid::Uuid;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("data-bundle-tests")
            .join(format!("{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn import_writes_bundle_files_and_remaps_paths() {
        let source = temp_root("export");
        let outputs_dir = source.join("outputs/2026/09");
        fs::create_dir_all(&outputs_dir).unwrap();
        let image_path = outputs_dir.join("img.png");
        fs::write(&image_path, b"png-bytes").unwrap();
        let references_dir = source.join("references");
        fs::create_dir_all(&references_dir).unwrap();
        let reference_path = references_dir.join("ref.png");
        fs::write(&reference_path, b"ref-bytes").unwrap();

        let mut task = fallback_failed_record("task-1", "placeholder");
        task.status = "completed".into();
        task.completed_at = Some("2026-09-10T08:00:00+08:00".into());
        task.outputs.push(OutputImage {
            path: image_path.to_string_lossy().into_owned(),
            file_name: "img.png".into(),
            mime_type: "image/png".into(),
            output_format: "png".into(),
            size: String::new(),
            background: String::new(),
            quality: String::new(),
            revised_prompt: String::new(),
            usage: serde_json::Value::Null,
        });
        task.reference_paths
            .push(reference_path.to_string_lossy().into_owned());
        store::write_history(&source, &[task]).unwrap();

        let zip_path = export_data_bundle(&source, &["tasks".to_string()]).unwrap();
        assert!(Path::new(&zip_path).is_file());

        // 同机导出再导入：路径都存在，应保留原路径
        let same = import_data_bundle(&source, &zip_path).unwrap();
        assert_eq!(same.tasks, 1);
        let same_history = store::read_history(&source).unwrap();
        assert_eq!(same_history[0].outputs[0].path, image_path.to_string_lossy());

        // 把数据包移出源目录并删除源目录，模拟跨机导入
        let stash = temp_root("stash");
        let moved_zip = stash.join("moved-bundle.zip");
        fs::rename(&zip_path, &moved_zip).unwrap();
        fs::remove_dir_all(&source).unwrap();

        // 跨机导入：文件落盘到目标目录并重映射路径
        let target = temp_root("import");
        let result = import_data_bundle(&target, &moved_zip.to_string_lossy()).unwrap();
        assert_eq!(result.tasks, 1);
        let history = store::read_history(&target).unwrap();
        assert_eq!(history.len(), 1);
        let output_path = Path::new(&history[0].outputs[0].path);
        assert!(output_path.is_file());
        assert!(output_path.starts_with(target.join("outputs/2026/09")));
        assert_eq!(fs::read(output_path).unwrap(), b"png-bytes");
        assert_eq!(history[0].outputs[0].file_name, "img.png");
        let imported_reference = Path::new(&history[0].reference_paths[0]);
        assert!(imported_reference.is_file());
        assert!(imported_reference.starts_with(target.join("references")));
        assert_eq!(fs::read(imported_reference).unwrap(), b"ref-bytes");

        let _ = trash::delete(&stash);
        let _ = trash::delete(&target);
    }

    #[test]
    fn import_rejects_oversized_manifest_entries() {
        // 构造一个声明体积超限的 manifest：直接用 take 上限验证读取截断逻辑
        let root = temp_root("manifest-limit");
        let archive_path = root.join("export-test.zip");
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(MANIFEST_NAME, options).unwrap();
        let oversized = vec![b'a'; (MAX_MANIFEST_BYTES + 4096) as usize];
        zip.write_all(&oversized).unwrap();
        zip.finish().unwrap();

        let error = import_data_bundle(&root, &archive_path.to_string_lossy()).unwrap_err();
        assert!(error.contains("manifest.json 过大"));
        let _ = trash::delete(&root);
    }
}

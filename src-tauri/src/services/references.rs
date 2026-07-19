use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    models::{CleanupCandidate, GenerateRequest, PromptTemplate, TaskRecord},
    state::record_operation,
    store::{read_history, read_json, read_queue, read_templates},
    utils::image_mime_type,
};

const CLEANUP_DIRECTORIES: [&str; 4] = ["outputs", "references", "requests", "clipboard"];

/// 扫描四个资源目录，返回没有被应用数据引用的文件；本函数只读，不会删除文件。
pub(crate) fn scan_orphan_files(data_dir: &Path) -> Result<Vec<CleanupCandidate>, String> {
    let referenced = collect_referenced_files(data_dir)?;
    let request_ids = collect_referenced_request_ids(data_dir)?;
    let mut candidates = Vec::new();

    for directory_name in CLEANUP_DIRECTORIES {
        let directory = data_dir.join(directory_name);
        if !directory.is_dir() {
            continue;
        }
        for path in files_in_directory(&directory)? {
            let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let request_is_used = directory_name == "requests"
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .and_then(|value| value.strip_suffix(".json"))
                    .is_some_and(|task_id| request_ids.contains(task_id));
            if request_is_used || referenced.contains(&canonical) {
                continue;
            }
            let size = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or_default();
            candidates.push(CleanupCandidate {
                relative_path: path
                    .strip_prefix(data_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().into_owned(),
                size,
            });
        }
    }
    candidates.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(candidates)
}

/// 重新扫描后把孤岛文件移入系统回收站，避免使用弹窗打开期间的旧列表直接删除。
pub(crate) fn cleanup_orphan_files(data_dir: &Path) -> Result<Vec<CleanupCandidate>, String> {
    let candidates = scan_orphan_files(data_dir)?;
    for candidate in &candidates {
        let path = PathBuf::from(&candidate.path);
        let allowed = CLEANUP_DIRECTORIES
            .iter()
            .any(|directory| path.starts_with(data_dir.join(directory)));
        if !allowed {
            return Err(format!("拒绝清理数据目录外的文件：{}", path.display()));
        }
        if let Err(error) = trash::delete(&path) {
            let message = format!("将孤岛文件移到回收站失败（{}）: {error}", path.display());
            record_operation(
                "清理孤岛文件",
                "失败",
                format!("path={}", path.display()),
                None,
                Some(&message),
            );
            return Err(message);
        }
        record_operation(
            "清理孤岛文件",
            "成功",
            format!("path={} bytes={}", path.display(), candidate.size),
            None,
            None,
        );
    }
    Ok(candidates)
}

fn collect_referenced_files(data_dir: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut referenced = HashSet::new();
    for path in json_files(data_dir)? {
        let value: Value = read_json(&path)?;
        collect_paths_from_value(&value, data_dir, &mut referenced);
    }
    Ok(referenced)
}

fn collect_paths_from_value(value: &Value, data_dir: &Path, referenced: &mut HashSet<PathBuf>) {
    match value {
        Value::String(value) => {
            let path = Path::new(value);
            if path.is_absolute() && path.starts_with(data_dir) && path.is_file() {
                referenced.insert(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_paths_from_value(value, data_dir, referenced);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_paths_from_value(value, data_dir, referenced);
            }
        }
        _ => {}
    }
}

fn collect_referenced_request_ids(data_dir: &Path) -> Result<HashSet<String>, String> {
    let history = read_history(data_dir)?;
    let queue = read_queue(data_dir)?;
    let mut ids = history
        .into_iter()
        .map(|record| record.id)
        .collect::<HashSet<_>>();
    ids.extend(queue.waiting);
    ids.extend(queue.running.into_iter().map(|run| run.task_id));
    Ok(ids)
}

fn json_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    Ok(files)
}

fn collect_json_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|error| format!("读取数据目录失败: {error}"))? {
        let entry = entry.map_err(|error| format!("读取数据目录条目失败: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取数据目录条目类型失败: {error}"))?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_json_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn files_in_directory(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    Ok(files)
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("读取清理目录失败（{}）: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| format!("读取待清理文件失败: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取待清理文件类型失败: {error}"))?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

pub(crate) fn persist_reference_paths(
    data_dir: &Path,
    paths: &[String],
) -> Result<Vec<String>, String> {
    let mut persisted = Vec::new();
    let mut seen = HashSet::new();
    for raw_path in paths {
        let source = Path::new(raw_path);
        if !source.is_file() {
            return Err(format!("找不到参考图文件：{}", source.display()));
        }
        let bytes = match fs::read(source) {
            Ok(bytes) => {
                record_operation(
                    "读取参考图文件",
                    "成功",
                    format!("path={} bytes={}", source.display(), bytes.len()),
                    None,
                    None,
                );
                bytes
            }
            Err(error) => {
                let message = format!("读取参考图失败（{}）: {error}", source.display());
                record_operation(
                    "读取参考图文件",
                    "失败",
                    format!("path={}", source.display()),
                    None,
                    Some(&message),
                );
                return Err(message);
            }
        };
        let mime_type = image_mime_type(source, &bytes)?;
        let path = persist_reference_bytes(data_dir, &bytes, extension_for_mime(&mime_type))?;
        let value = path.to_string_lossy().into_owned();
        if seen.insert(value.clone()) {
            persisted.push(value);
        }
    }
    Ok(persisted)
}

pub(crate) fn persist_reference_bytes(
    data_dir: &Path,
    bytes: &[u8],
    extension: &str,
) -> Result<PathBuf, String> {
    let references_dir = data_dir.join("references");
    fs::create_dir_all(&references_dir)
        .map_err(|error| format!("创建参考图资源目录失败: {error}"))?;
    let digest = Sha256::digest(bytes);
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if let Some(existing) = existing_reference_path(&references_dir, &hash)? {
        record_operation(
            "写入参考图资源",
            "跳过",
            format!("path={} reason=deduplicated", existing.display()),
            None,
            None,
        );
        return Ok(existing);
    }
    let path = references_dir.join(format!("{hash}.{}", normalize_extension(extension)));
    if !path.exists() {
        if let Err(error) = fs::write(&path, bytes) {
            let message = format!("保存参考图资源失败: {error}");
            record_operation(
                "写入参考图资源",
                "失败",
                format!("path={} bytes={}", path.display(), bytes.len()),
                None,
                Some(&message),
            );
            return Err(message);
        }
        record_operation(
            "写入参考图资源",
            "成功",
            format!("path={} bytes={}", path.display(), bytes.len()),
            None,
            None,
        );
    }
    Ok(path)
}

fn existing_reference_path(references_dir: &Path, hash: &str) -> Result<Option<PathBuf>, String> {
    for entry in
        fs::read_dir(references_dir).map_err(|error| format!("读取参考图资源目录失败: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("读取参考图资源失败: {error}"))?
            .path();
        if path.is_file() && path.file_stem().and_then(|value| value.to_str()) == Some(hash) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub(crate) fn prune_unreferenced_files(data_dir: &Path) -> Result<(), String> {
    let history = read_history(data_dir)?;
    let templates = read_templates(data_dir)?;
    prune_unreferenced_files_with_data(data_dir, &history, &templates)
}

pub(crate) fn prune_unreferenced_files_with_data(
    data_dir: &Path,
    history: &[TaskRecord],
    templates: &[PromptTemplate],
) -> Result<(), String> {
    let references_dir = data_dir.join("references");
    if !references_dir.is_dir() {
        return Ok(());
    }

    let mut used = HashSet::new();
    for record in history {
        extend_used_paths(&mut used, &record.reference_paths);
    }
    for template in templates {
        extend_used_paths(&mut used, &template.reference_paths);
        if !template.effect_image_path.trim().is_empty() {
            used.insert(PathBuf::from(template.effect_image_path.trim()));
        }
    }
    let requests_dir = data_dir.join("requests");
    if requests_dir.is_dir() {
        for entry in
            fs::read_dir(&requests_dir).map_err(|error| format!("读取任务请求目录失败: {error}"))?
        {
            let path = entry
                .map_err(|error| format!("读取任务请求文件失败: {error}"))?
                .path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Ok(request) = read_json::<GenerateRequest>(&path) {
                extend_used_paths(&mut used, &request.reference_paths);
            }
        }
    }

    for entry in
        fs::read_dir(&references_dir).map_err(|error| format!("读取参考图资源目录失败: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("读取参考图资源失败: {error}"))?
            .path();
        if !path.is_file() || !should_prune_reference_file(&path) {
            continue;
        }
        if !used.contains(&path) {
            if let Err(error) = trash::delete(&path) {
                let message = format!(
                    "将未引用参考图移到回收站失败（{}）: {error}",
                    path.display()
                );
                record_operation(
                    "清理未引用参考图",
                    "失败",
                    format!("path={}", path.display()),
                    None,
                    Some(&message),
                );
                return Err(message);
            }
            record_operation(
                "清理未引用参考图",
                "成功",
                format!("path={}", path.display()),
                None,
                None,
            );
        }
    }
    Ok(())
}

fn extend_used_paths(used: &mut HashSet<PathBuf>, paths: &[String]) {
    for path in paths {
        used.insert(PathBuf::from(path));
    }
}

fn should_prune_reference_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if file_name.starts_with('.') {
        return false;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        })
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn normalize_extension(extension: &str) -> &'static str {
    match extension.trim().to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "gif" => "gif",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::Path};

    use uuid::Uuid;

    use super::{scan_orphan_files, should_prune_reference_file};

    #[test]
    fn only_image_assets_in_reference_dir_are_pruned() {
        assert!(should_prune_reference_file(Path::new("/tmp/demo.png")));
        assert!(should_prune_reference_file(Path::new("/tmp/demo.jpeg")));
        assert!(!should_prune_reference_file(Path::new("/tmp/.DS_Store")));
        assert!(!should_prune_reference_file(Path::new("/tmp/readme.md")));
    }

    #[test]
    fn orphan_scan_keeps_json_paths_and_queue_request_files() {
        let root = std::env::temp_dir().join(format!("image-forge-cleanup-{}", Uuid::new_v4()));
        let outputs = root.join("outputs");
        let requests = root.join("requests");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&requests).unwrap();
        let kept_output = outputs.join("kept.png");
        fs::write(&kept_output, b"kept").unwrap();
        fs::write(outputs.join("orphan.png"), b"orphan").unwrap();
        fs::write(requests.join("task-1.json"), "{}").unwrap();
        fs::write(requests.join("orphan.json"), "{}").unwrap();
        fs::write(
            root.join("links.json"),
            serde_json::to_string(&serde_json::json!({
                "output": kept_output.to_string_lossy()
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("queue.json"),
            r#"{"waiting":["task-1"],"running":[],"updatedAt":"2026-07-19T00:00:00Z"}"#,
        )
        .unwrap();

        let candidates = scan_orphan_files(&root).unwrap();
        let paths = candidates
            .iter()
            .map(|candidate| candidate.relative_path.as_str())
            .collect::<HashSet<_>>();
        assert!(paths.contains("outputs/orphan.png"));
        assert!(paths.contains("requests/orphan.json"));
        assert!(!paths.contains("outputs/kept.png"));
        assert!(!paths.contains("requests/task-1.json"));
        trash::delete(&root).unwrap();
    }
}

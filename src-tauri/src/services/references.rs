use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
    models::GenerateRequest,
    state::record_operation,
    store::{read_history, read_json, read_templates},
    utils::image_mime_type,
};

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
    let references_dir = data_dir.join("references");
    if !references_dir.is_dir() {
        return Ok(());
    }

    let mut used = HashSet::new();
    for record in read_history(data_dir)? {
        extend_used_paths(&mut used, &record.reference_paths);
    }
    for template in read_templates(data_dir)? {
        extend_used_paths(&mut used, &template.reference_paths);
        if !template.effect_image_path.trim().is_empty() {
            used.insert(PathBuf::from(template.effect_image_path));
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
        if path.is_file() && !used.contains(&path) {
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

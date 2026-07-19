use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    models::PromptTemplate,
    services::references::persist_reference_bytes,
    utils::{image_mime_type, utc_now},
};

const BUNDLE_FORMAT: &str = "image-forge-template-bundle";
const BUNDLE_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const MARKDOWN_NAME: &str = "ImageForge-templates.md";
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 2_000;
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 10 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_TEMPLATE_COUNT: usize = 10_000;
const MAX_REFERENCES_PER_TEMPLATE: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleManifest {
    format: String,
    version: u32,
    exported_at: String,
    templates: Vec<BundleTemplate>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleTemplate {
    #[serde(default)]
    source_id: String,
    #[serde(default)]
    title: String,
    content: String,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    effect_image: String,
}

struct ReferenceEntry {
    bytes: Vec<u8>,
    archive_path: String,
}

/// 把全部模板、机器清单、Markdown 和去重后的参考图写入一个 ZIP 文件。
pub(crate) fn export_templates_archive(
    templates: &[PromptTemplate],
    destination: &Path,
) -> Result<PathBuf, String> {
    if templates.is_empty() {
        return Err("没有可导出的模板".into());
    }

    let destination = normalized_zip_path(destination);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建导出目录失败: {error}"))?;
    }
    let (references, archive_paths) = collect_reference_entries(templates)?;
    let manifest = build_manifest(templates, &archive_paths);
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("生成模板导出清单失败: {error}"))?;
    let markdown = build_templates_markdown(templates, &archive_paths);
    let file =
        File::create(&destination).map_err(|error| format!("创建模板导出文件失败: {error}"))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    write_archive_entry(&mut archive, MANIFEST_NAME, &manifest_json, options)?;
    write_archive_entry(&mut archive, MARKDOWN_NAME, markdown.as_bytes(), options)?;
    for reference in references {
        write_archive_entry(
            &mut archive,
            &reference.archive_path,
            &reference.bytes,
            options,
        )?;
    }

    archive
        .finish()
        .map_err(|error| format!("完成模板导出失败: {error}"))?;
    Ok(destination)
}

/// 读取模板 ZIP，校验包结构并把图片写入共享参考图资源库。
pub(crate) fn import_templates_archive(
    data_dir: &Path,
    archive_path: &Path,
) -> Result<Vec<PromptTemplate>, String> {
    let metadata =
        fs::metadata(archive_path).map_err(|error| format!("读取模板包失败: {error}"))?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err("模板包超过 256 MB，无法导入".into());
    }
    let file = File::open(archive_path).map_err(|error| format!("打开模板包失败: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("模板包不是有效 ZIP: {error}"))?;
    let entry_names = validate_archive_entries(&mut archive)?;

    let (bundle_templates, require_hash_names) = if entry_names.contains(MANIFEST_NAME) {
        let bytes = read_archive_entry(&mut archive, MANIFEST_NAME, MAX_MANIFEST_BYTES)?;
        let manifest: BundleManifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("模板包 manifest.json 无效: {error}"))?;
        validate_manifest(manifest)?
    } else if entry_names.contains(MARKDOWN_NAME) {
        let bytes = read_archive_entry(&mut archive, MARKDOWN_NAME, MAX_MARKDOWN_BYTES)?;
        let markdown = String::from_utf8(bytes).map_err(|_| "旧版模板 Markdown 不是 UTF-8")?;
        (parse_legacy_markdown(&markdown)?, false)
    } else {
        return Err("模板包缺少 manifest.json 或 ImageForge-templates.md".into());
    };

    import_bundle_templates(data_dir, &mut archive, bundle_templates, require_hash_names)
}

fn write_archive_entry(
    archive: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), String> {
    archive
        .start_file(name, options)
        .map_err(|error| format!("写入模板包条目失败（{name}）: {error}"))?;
    archive
        .write_all(bytes)
        .map_err(|error| format!("写入模板包条目失败（{name}）: {error}"))
}

fn collect_reference_entries(
    templates: &[PromptTemplate],
) -> Result<(Vec<ReferenceEntry>, HashMap<PathBuf, String>), String> {
    let mut entries = Vec::new();
    let mut archive_paths = HashMap::new();
    let mut hashes = HashSet::new();
    for template in templates {
        for raw_path in template
            .reference_paths
            .iter()
            .chain(std::iter::once(&template.effect_image_path))
            .filter(|path| !path.trim().is_empty())
        {
            let source = PathBuf::from(raw_path);
            if archive_paths.contains_key(&source) {
                continue;
            }
            if !source.is_file() {
                return Err(format!("找不到模板参考图：{}", source.display()));
            }
            let bytes = fs::read(&source)
                .map_err(|error| format!("读取模板参考图失败（{}）: {error}", source.display()))?;
            let mime_type = image_mime_type(&source, &bytes)?;
            let hash = sha256_hex(&bytes);
            let archive_path = format!("images/{hash}.{}", extension_for_mime(&mime_type));
            archive_paths.insert(source, archive_path.clone());
            if hashes.insert(hash) {
                entries.push(ReferenceEntry {
                    bytes,
                    archive_path,
                });
            }
        }
    }
    Ok((entries, archive_paths))
}

fn build_manifest(
    templates: &[PromptTemplate],
    archive_paths: &HashMap<PathBuf, String>,
) -> BundleManifest {
    BundleManifest {
        format: BUNDLE_FORMAT.into(),
        version: BUNDLE_VERSION,
        exported_at: utc_now(),
        templates: templates
            .iter()
            .map(|template| BundleTemplate {
                source_id: template.id.clone(),
                title: template.title.clone(),
                content: template.content.clone(),
                references: template
                    .reference_paths
                    .iter()
                    .filter_map(|path| archive_paths.get(&PathBuf::from(path)).cloned())
                    .collect(),
                effect_image: archive_paths
                    .get(&PathBuf::from(&template.effect_image_path))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

fn build_templates_markdown(
    templates: &[PromptTemplate],
    archive_paths: &HashMap<PathBuf, String>,
) -> String {
    let mut markdown = format!(
        "# Image Forge 提示词模板\n\n> 导出时间：{}\n> 模板数量：{}\n\n",
        utc_now(),
        templates.len()
    );
    for template in templates {
        markdown.push_str(&format!(
            "---\n\n## 模板 {} · {}\n\n{}\n\n",
            template.id, template.title, template.content
        ));
        let references = template
            .reference_paths
            .iter()
            .filter_map(|path| archive_paths.get(&PathBuf::from(path)))
            .collect::<Vec<_>>();
        if !references.is_empty() {
            markdown.push_str("### 参考图\n\n");
            for (index, archive_path) in references.iter().enumerate() {
                markdown.push_str(&format!(
                    "![模板 {} 参考图 {}]({archive_path})\n\n",
                    template.id,
                    index + 1
                ));
            }
        }
        if let Some(archive_path) = archive_paths.get(&PathBuf::from(&template.effect_image_path)) {
            markdown.push_str("### 效果图\n\n");
            markdown.push_str(&format!(
                "![模板 {} 效果图]({archive_path})\n\n",
                template.id
            ));
        }
    }
    markdown
}

fn validate_archive_entries(archive: &mut ZipArchive<File>) -> Result<HashSet<String>, String> {
    if archive.len() > MAX_ENTRY_COUNT {
        return Err("模板包文件数量过多".into());
    }
    let mut names = HashSet::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("读取模板包目录失败: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.len() > 512 || !names.insert(name) {
            return Err("模板包包含重复或异常文件名".into());
        }
        total_size = total_size
            .checked_add(entry.size())
            .ok_or("模板包解压大小异常")?;
        if total_size > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err("模板包解压后超过 1 GB，无法导入".into());
        }
    }
    Ok(names)
}

fn validate_manifest(manifest: BundleManifest) -> Result<(Vec<BundleTemplate>, bool), String> {
    if manifest.format != BUNDLE_FORMAT {
        return Err("不是 Image Forge 模板包".into());
    }
    if manifest.version != BUNDLE_VERSION {
        return Err(format!(
            "不支持模板包版本 {}，当前支持版本 {}",
            manifest.version, BUNDLE_VERSION
        ));
    }
    validate_bundle_templates(&manifest.templates)?;
    Ok((manifest.templates, true))
}

fn validate_bundle_templates(templates: &[BundleTemplate]) -> Result<(), String> {
    if templates.is_empty() {
        return Err("模板包中没有模板".into());
    }
    if templates.len() > MAX_TEMPLATE_COUNT {
        return Err("模板包中的模板数量过多".into());
    }
    for template in templates {
        if template.content.trim().is_empty() {
            return Err("模板包包含空提示词".into());
        }
        if template.references.len() > MAX_REFERENCES_PER_TEMPLATE {
            return Err("单个模板的参考图数量超过 64 张".into());
        }
        for reference in &template.references {
            validate_reference_path(reference)?;
        }
        if !template.effect_image.is_empty() {
            validate_reference_path(&template.effect_image)?;
        }
    }
    Ok(())
}

fn validate_reference_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if !value.starts_with("images/")
        || value.contains('\\')
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("模板包包含不安全的参考图路径：{value}"));
    }
    Ok(())
}

fn import_bundle_templates(
    data_dir: &Path,
    archive: &mut ZipArchive<File>,
    templates: Vec<BundleTemplate>,
    require_hash_names: bool,
) -> Result<Vec<PromptTemplate>, String> {
    validate_bundle_templates(&templates)?;
    let mut reference_paths = Vec::new();
    let mut seen = HashSet::new();
    for template in &templates {
        for reference in template
            .references
            .iter()
            .chain(std::iter::once(&template.effect_image))
            .filter(|path| !path.is_empty())
        {
            if seen.insert(reference.clone()) {
                reference_paths.push(reference.clone());
            }
        }
    }

    let mut persisted = HashMap::new();
    for archive_path in reference_paths {
        let bytes = read_archive_entry(archive, &archive_path, MAX_IMAGE_BYTES)?;
        let mime_type = image_mime_type(Path::new(&archive_path), &bytes)?;
        if require_hash_names {
            verify_reference_hash(&archive_path, &bytes)?;
        }
        let path = persist_reference_bytes(data_dir, &bytes, extension_for_mime(&mime_type))?;
        persisted.insert(archive_path, path.to_string_lossy().into_owned());
    }

    templates
        .into_iter()
        .map(|template| {
            let references = template
                .references
                .iter()
                .map(|path| {
                    persisted
                        .get(path)
                        .cloned()
                        .ok_or_else(|| format!("模板包缺少参考图：{path}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let effect_image_path = if template.effect_image.is_empty() {
                String::new()
            } else {
                persisted
                    .get(&template.effect_image)
                    .cloned()
                    .ok_or_else(|| format!("模板包缺少效果图：{}", template.effect_image))?
            };
            Ok(imported_template(
                template.title,
                template.content,
                references,
                effect_image_path,
            ))
        })
        .collect()
}

fn read_archive_entry(
    archive: &mut ZipArchive<File>,
    name: &str,
    max_size: u64,
) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| format!("模板包缺少文件：{name}"))?;
    if entry.is_dir() || entry.size() > max_size {
        return Err(format!("模板包文件过大或类型无效：{name}"));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
    entry
        .by_ref()
        .take(max_size + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取模板包文件失败（{name}）: {error}"))?;
    if bytes.len() as u64 > max_size {
        return Err(format!("模板包文件超过大小限制：{name}"));
    }
    Ok(bytes)
}

fn parse_legacy_markdown(markdown: &str) -> Result<Vec<BundleTemplate>, String> {
    let marker = "\n---\n\n## 模板 ";
    let reference_marker = "\n\n### 参考图\n\n";
    let mut templates = Vec::new();
    for section in markdown.split(marker).skip(1) {
        let (source_id, body) = section
            .split_once("\n\n")
            .ok_or("旧版模板 Markdown 结构无效")?;
        let (content, reference_text) = body
            .rsplit_once(reference_marker)
            .map(|(content, references)| (content, Some(references)))
            .unwrap_or((body, None));
        let references = reference_text
            .map(parse_markdown_reference_paths)
            .unwrap_or_default();
        templates.push(BundleTemplate {
            source_id: source_id.trim().into(),
            title: String::new(),
            content: content.trim().into(),
            references,
            effect_image: String::new(),
        });
    }
    validate_bundle_templates(&templates)?;
    Ok(templates)
}

fn parse_markdown_reference_paths(value: &str) -> Vec<String> {
    value
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.rfind("](")? + 2;
            let end = line.strip_suffix(')')?.len();
            (start < end).then(|| line[start..end].to_string())
        })
        .collect()
}

fn verify_reference_hash(archive_path: &str, bytes: &[u8]) -> Result<(), String> {
    let expected = Path::new(archive_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("参考图文件名无效：{archive_path}"))?;
    let actual = sha256_hex(bytes);
    if expected != actual {
        return Err(format!("参考图完整性校验失败：{archive_path}"));
    }
    Ok(())
}

fn imported_template(
    title: String,
    content: String,
    reference_paths: Vec<String>,
    effect_image_path: String,
) -> PromptTemplate {
    PromptTemplate {
        id: String::new(),
        title,
        short_title: String::new(),
        category: String::new(),
        content,
        reference_paths,
        effect_image_path,
        notes: String::new(),
        tags: Vec::new(),
        favorite: false,
        usage_count: 0,
        model_hint: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn normalized_zip_path(destination: &Path) -> PathBuf {
    if destination
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
    {
        destination.to_path_buf()
    } else {
        destination.with_extension("zip")
    }
}

#[cfg(test)]
mod tests {
    use zip::ZipArchive;

    use super::*;

    #[test]
    fn template_bundle_round_trip_preserves_prompts_and_deduplicates_images() {
        let root = test_dir("round-trip");
        let reference = root.join("shared.png");
        write_test_image(&reference);
        let mut templates = vec![
            template("1", "第一个提示词", &reference),
            template("2", "第二个提示词", &reference),
        ];
        templates[0].effect_image_path = reference.to_string_lossy().into_owned();
        let archive_path = export_templates_archive(&templates, &root.join("templates")).unwrap();
        let mut archive = ZipArchive::new(File::open(&archive_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 3);
        let manifest: BundleManifest =
            serde_json::from_reader(archive.by_name(MANIFEST_NAME).unwrap()).unwrap();
        assert_eq!(manifest.format, BUNDLE_FORMAT);
        assert_eq!(manifest.version, BUNDLE_VERSION);
        assert_eq!(
            manifest.templates[0].references,
            manifest.templates[1].references
        );
        assert_eq!(
            manifest.templates[0].effect_image,
            manifest.templates[0].references[0]
        );
        drop(archive);

        let imported_dir = root.join("imported");
        fs::create_dir_all(&imported_dir).unwrap();
        let imported = import_templates_archive(&imported_dir, &archive_path).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].title, "标题1");
        assert_eq!(imported[0].content, "第一个提示词");
        assert_eq!(imported[1].content, "第二个提示词");
        assert_eq!(imported[0].reference_paths, imported[1].reference_paths);
        assert_eq!(
            imported[0].effect_image_path,
            imported[0].reference_paths[0]
        );
        let _ = trash::delete(&root);
    }

    #[test]
    fn import_supports_legacy_markdown_bundle() {
        let root = test_dir("legacy");
        let reference = root.join("legacy.png");
        write_test_image(&reference);
        let archive_path = root.join("legacy.zip");
        let file = File::create(&archive_path).unwrap();
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        let markdown = "# Image Forge 提示词模板\n\n> 模板数量：1\n\n---\n\n## 模板 7\n\n旧版提示词\n\n### 参考图\n\n![模板 7 参考图 1](images/reference-001.png)\n";
        write_archive_entry(&mut archive, MARKDOWN_NAME, markdown.as_bytes(), options).unwrap();
        write_archive_entry(
            &mut archive,
            "images/reference-001.png",
            &fs::read(&reference).unwrap(),
            options,
        )
        .unwrap();
        archive.finish().unwrap();

        let imported_dir = root.join("imported");
        fs::create_dir_all(&imported_dir).unwrap();
        let imported = import_templates_archive(&imported_dir, &archive_path).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].content, "旧版提示词");
        assert_eq!(imported[0].reference_paths.len(), 1);
        let _ = trash::delete(&root);
    }

    fn test_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "image-forge-template-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_test_image(path: &Path) {
        image::RgbaImage::from_pixel(2, 2, image::Rgba([120, 90, 240, 255]))
            .save(path)
            .unwrap();
    }

    fn template(id: &str, content: &str, reference: &Path) -> PromptTemplate {
        PromptTemplate {
            id: id.into(),
            title: format!("标题{id}"),
            short_title: String::new(),
            category: String::new(),
            content: content.into(),
            reference_paths: vec![reference.to_string_lossy().into_owned()],
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

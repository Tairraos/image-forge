use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{models::PromptTemplate, utils::utc_now};

struct ReferenceEntry {
    source: PathBuf,
    archive_path: String,
}

/// 把全部模板、Markdown 清单和去重后的参考图写入一个 ZIP 文件。
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
    let markdown = build_templates_markdown(templates, &archive_paths);
    let file =
        File::create(&destination).map_err(|error| format!("创建模板导出文件失败: {error}"))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    archive
        .start_file("ImageForge-templates.md", options)
        .map_err(|error| format!("写入模板清单失败: {error}"))?;
    archive
        .write_all(markdown.as_bytes())
        .map_err(|error| format!("写入模板清单失败: {error}"))?;

    for reference in references {
        let bytes = fs::read(&reference.source).map_err(|error| {
            format!(
                "读取模板参考图失败（{}）: {error}",
                reference.source.display()
            )
        })?;
        archive
            .start_file(reference.archive_path, options)
            .map_err(|error| format!("写入模板参考图失败: {error}"))?;
        archive
            .write_all(&bytes)
            .map_err(|error| format!("写入模板参考图失败: {error}"))?;
    }

    archive
        .finish()
        .map_err(|error| format!("完成模板导出失败: {error}"))?;
    Ok(destination)
}

fn collect_reference_entries(
    templates: &[PromptTemplate],
) -> Result<(Vec<ReferenceEntry>, HashMap<PathBuf, String>), String> {
    let mut entries = Vec::new();
    let mut archive_paths = HashMap::new();
    for template in templates {
        for raw_path in &template.reference_paths {
            let source = PathBuf::from(raw_path);
            if archive_paths.contains_key(&source) {
                continue;
            }
            if !source.is_file() {
                return Err(format!("找不到模板参考图：{}", source.display()));
            }
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| {
                    value
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                })
                .unwrap_or("png")
                .to_ascii_lowercase();
            let archive_path = format!("images/reference-{:03}.{extension}", entries.len() + 1);
            archive_paths.insert(source.clone(), archive_path.clone());
            entries.push(ReferenceEntry {
                source,
                archive_path,
            });
        }
    }
    Ok((entries, archive_paths))
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
            "---\n\n## 模板 {}\n\n{}\n\n",
            template.id, template.content
        ));
        let references = template
            .reference_paths
            .iter()
            .filter_map(|path| archive_paths.get(&PathBuf::from(path)))
            .collect::<Vec<_>>();
        if references.is_empty() {
            continue;
        }
        markdown.push_str("### 参考图\n\n");
        for (index, archive_path) in references.iter().enumerate() {
            markdown.push_str(&format!(
                "![模板 {} 参考图 {}]({archive_path})\n\n",
                template.id,
                index + 1
            ));
        }
    }
    markdown
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
    use std::io::Read;

    use uuid::Uuid;
    use zip::ZipArchive;

    use super::*;

    #[test]
    fn export_archive_deduplicates_shared_references() {
        let root = std::env::temp_dir().join(format!("image-forge-export-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let reference = root.join("shared.png");
        fs::write(&reference, b"image-bytes").unwrap();
        let templates = vec![
            template("1", "第一个提示词", &reference),
            template("2", "第二个提示词", &reference),
        ];
        let archive_path = export_templates_archive(&templates, &root.join("templates")).unwrap();
        let mut archive = ZipArchive::new(File::open(archive_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        let mut markdown = String::new();
        archive
            .by_name("ImageForge-templates.md")
            .unwrap()
            .read_to_string(&mut markdown)
            .unwrap();
        assert!(markdown.contains("第一个提示词"));
        assert!(markdown.contains("第二个提示词"));
        assert_eq!(markdown.matches("images/reference-001.png").count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    fn template(id: &str, content: &str, reference: &Path) -> PromptTemplate {
        PromptTemplate {
            id: id.into(),
            title: String::new(),
            short_title: String::new(),
            category: String::new(),
            content: content.into(),
            reference_paths: vec![reference.to_string_lossy().into_owned()],
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

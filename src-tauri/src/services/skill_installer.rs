use std::{
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
    models::{SkillAuditResult, SkillEntry, SkillManifest},
    services::skill::fetch_skill_markdown,
    store::{read_skills, skill_directory_name, skills_dir, write_skill_index},
    utils::utc_now,
};

const MAX_MARKDOWN_BYTES: u64 = 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_REFERENCE_DOCUMENTS: usize = 200;

const ALLOWED_CAPABILITIES: &[&str] = &["chat", "image_plan", "reference_images"];
const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "js", "ts", "mjs", "cjs", "sh", "bash", "zsh", "ps1", "bat", "cmd", "rb", "exe", "bin",
];
const DANGEROUS_TERMS: &[&str] = &[
    "script",
    "scripts",
    "command",
    "commands",
    "shell",
    "exec",
    "executable",
    "subprocess",
    "runtime",
    "terminal",
    "powershell",
    "python",
    "node",
    "curl",
    "wget",
];

pub(crate) fn audit_skill_directory(root: &Path) -> Result<SkillAuditResult, String> {
    if !root.is_dir() {
        return Err("Skill 包目录不存在".into());
    }
    let entry = [root.join("SKILL.md"), root.join("skill.md")]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or("Skill 包缺少 SKILL.md")?;
    let content =
        fs::read_to_string(&entry).map_err(|error| format!("读取 Skill Markdown 失败: {error}"))?;
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();
    let mut total_size = 0u64;
    let mut reference_count = 0usize;

    inspect_tree(
        root,
        root,
        &mut total_size,
        &mut reference_count,
        &mut reasons,
    )?;
    let lower = content.to_ascii_lowercase();
    for term in DANGEROUS_TERMS {
        if contains_capability_term(&lower, term) {
            reasons.push(format!(
                "SKILL.md 声明或要求 `{term}` 能力，当前 Agent 不提供系统命令执行"
            ));
        }
    }
    for link in markdown_links(&content) {
        if link.starts_with("http://") || link.starts_with("https://") {
            warnings.push(format!("正文包含外部链接 `{link}`，执行时不会自动访问"));
        }
    }

    let name = extract_name(&content);
    let sections = content
        .lines()
        .filter_map(|line| line.strip_prefix("# ").or_else(|| line.strip_prefix("## ")))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    let declared_capabilities = frontmatter_list(&content, "capabilities");
    let capabilities = if declared_capabilities.is_empty() {
        vec![
            "chat".into(),
            "image_plan".into(),
            "reference_images".into(),
        ]
    } else {
        declared_capabilities
    };
    let required_sections = frontmatter_list(&content, "requiredSections");
    let output_capability =
        frontmatter_scalar(&content, "outputCapability").unwrap_or_else(|| "image_plan".into());
    let manifest = SkillManifest {
        schema_version: 1,
        content_hash: hash,
        name,
        capabilities,
        sections,
        required_sections,
        output_capability,
    };
    if total_size > MAX_PACKAGE_BYTES {
        reasons.push("Skill 包超过 256 MB 上限".into());
    }
    if reference_count > MAX_REFERENCE_DOCUMENTS {
        reasons.push("Skill 参考文档超过 200 个上限".into());
    }
    for capability in &manifest.capabilities {
        if !ALLOWED_CAPABILITIES.contains(&capability.as_str()) {
            reasons.push(format!("未知能力 `{capability}`"));
        }
    }
    if !ALLOWED_CAPABILITIES.contains(&manifest.output_capability.as_str()) {
        reasons.push(format!("未知输出能力 `{}`", manifest.output_capability));
    }
    for required in &manifest.required_sections {
        if !manifest.sections.iter().any(|section| section == required) {
            reasons.push(format!("requiredSections 引用了不存在的章节 `{required}`"));
        }
    }
    Ok(SkillAuditResult {
        allowed: reasons.is_empty(),
        reasons,
        warnings,
        manifest: Some(manifest),
    })
}

pub(crate) fn read_verified_manifest(root: &Path) -> Result<SkillManifest, String> {
    let path = root.join("manifest.json");
    let bytes = fs::read(&path).map_err(|error| format!("读取 Skill manifest 失败: {error}"))?;
    let manifest: SkillManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("解析 Skill manifest 失败: {error}"))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "不支持的 Skill manifest 版本：{}",
            manifest.schema_version
        ));
    }
    let content =
        fs::read(root.join("SKILL.md")).map_err(|error| format!("读取 Skill 内容失败: {error}"))?;
    let actual_hash = format!("{:x}", Sha256::digest(&content));
    if actual_hash != manifest.content_hash {
        return Err("Skill 内容已变化，manifest 哈希校验失败，请重新安装".into());
    }
    Ok(manifest)
}

fn inspect_tree(
    root: &Path,
    path: &Path,
    total_size: &mut u64,
    reference_count: &mut usize,
    reasons: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|error| format!("扫描 Skill 包失败: {error}"))? {
        let entry = entry.map_err(|error| format!("读取 Skill 包条目失败: {error}"))?;
        let current = entry.path();
        let relative = current.strip_prefix(root).unwrap_or(&current);
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            reasons.push(format!("路径不安全: {}", relative.display()));
            continue;
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("读取 Skill 元数据失败: {error}"))?;
        if metadata.file_type().is_symlink() {
            reasons.push(format!("拒绝符号链接: {}", relative.display()));
            continue;
        }
        if metadata.is_dir() {
            let name = current
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "scripts" | "script" | "bin" | "tools"
            ) {
                reasons.push(format!("发现不允许的脚本目录: {}", relative.display()));
                continue;
            }
            inspect_tree(root, &current, total_size, reference_count, reasons)?;
            continue;
        }
        *total_size = total_size.saturating_add(metadata.len());
        let extension = current
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let internal_manifest = relative == Path::new("manifest.json");
        if !internal_manifest
            && !matches!(
                extension.as_str(),
                "md" | "markdown" | "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        {
            reasons.push(format!("文件类型不受支持: {}", relative.display()));
        }
        if extension == "md" || extension == "markdown" {
            if metadata.len() > MAX_MARKDOWN_BYTES {
                reasons.push(format!(
                    "Markdown 文件超过 1 MB 上限: {}",
                    relative.display()
                ));
            }
            *reference_count += 1;
            match fs::read_to_string(&current) {
                Ok(text) => {
                    let lower = text.to_ascii_lowercase();
                    for ext in SCRIPT_EXTENSIONS {
                        if lower.contains(&format!(".{ext}")) {
                            reasons.push(format!("{} 包含脚本文件引用 .{ext}", relative.display()));
                        }
                    }
                }
                Err(_) => reasons.push(format!("Markdown 不是 UTF-8：{}", relative.display())),
            }
        } else if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif")
            && metadata.len() > MAX_IMAGE_BYTES
        {
            reasons.push(format!("图片超过 100 MB 上限: {}", relative.display()));
        }
    }
    Ok(())
}

fn contains_capability_term(text: &str, term: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word == term)
}

fn markdown_links(content: &str) -> Vec<String> {
    content
        .split_once("](")
        .map(|(_, rest)| {
            rest.split(')')
                .map(|value| value.trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_name(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("name:") {
            if !value.trim().is_empty() {
                return value.trim().trim_matches(['"', '\'']).to_string();
            }
        }
        if let Some(value) = trimmed.strip_prefix("# ") {
            if !value.trim().is_empty() {
                return value.trim().to_string();
            }
        }
    }
    "未命名 Skill".into()
}

pub(crate) fn staging_skill_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join(".staging").join(id)
}

pub(crate) fn install_local_skill(
    data_dir: &Path,
    source: &Path,
    replace: bool,
) -> Result<(SkillEntry, SkillAuditResult), String> {
    let root = if source.is_file() {
        source.parent().ok_or("无法确定 Skill 包目录")?
    } else {
        source
    };
    let audit = audit_skill_directory(root)?;
    if !audit.allowed {
        return Err(format!("Skill 安装被拒绝：{}", audit.reasons.join("；")));
    }
    let manifest = audit.manifest.clone().ok_or("Skill 审查未生成 manifest")?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = skill_directory_name(&manifest.name, &id);
    let staging = staging_skill_path(data_dir, &id);
    if let Err(error) = copy_tree(root, &staging) {
        move_to_trash_if_exists(&staging);
        return Err(error);
    }
    if !staging.join("SKILL.md").is_file() && staging.join("skill.md").is_file() {
        fs::rename(staging.join("skill.md"), staging.join("SKILL.md"))
            .map_err(|error| format!("规范化 Skill 入口文件失败: {error}"))?;
    }
    let destination = skills_dir(data_dir).join(&directory);
    if destination.exists() {
        move_to_trash_if_exists(&staging);
        if !replace {
            return Err("CONFIRM_REPLACE_SKILL:同名 Skill 已存在，是否覆盖安装？".into());
        }
        trash::delete(&destination)
            .map_err(|error| format!("将旧 Skill 移入回收站失败: {error}"))?;
    }
    let manifest_path = staging.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("序列化 Skill manifest 失败: {error}"))?,
    )
    .map_err(|error| format!("写入 Skill manifest 失败: {error}"))?;
    fs::rename(&staging, &destination).map_err(|error| format!("安装 Skill 包失败: {error}"))?;
    let mut skills = read_skills(data_dir).unwrap_or_default();
    let now = utc_now();
    let skill = SkillEntry {
        id,
        name: manifest.name.clone(),
        source_url: String::new(),
        notes: String::new(),
        content: fs::read_to_string(destination.join("SKILL.md")).unwrap_or_default(),
        directory,
        source_path: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    skills.retain(|item| item.directory != skill.directory);
    skills.push(skill.clone());
    write_skill_index(data_dir, &skills)?;
    Ok((skill, audit))
}

pub(crate) async fn install_skill_source(
    data_dir: &Path,
    source: &str,
    replace: bool,
) -> Result<(SkillEntry, SkillAuditResult), String> {
    let source = source.trim();
    if source.starts_with("http://") || source.starts_with("https://") {
        let fetched = fetch_skill_markdown(source).await?;
        let staging_id = uuid::Uuid::new_v4().to_string();
        let source_dir = staging_skill_path(data_dir, &format!("source-{staging_id}"));
        fs::create_dir_all(&source_dir)
            .map_err(|error| format!("创建 Skill 下载目录失败: {error}"))?;
        fs::write(source_dir.join("SKILL.md"), fetched.content)
            .map_err(|error| format!("写入下载的 Skill 失败: {error}"))?;
        let result =
            install_local_skill(data_dir, &source_dir, replace).map(|(mut skill, audit)| {
                skill.source_url = fetched.source_url;
                (skill, audit)
            });
        move_to_trash_if_exists(&source_dir);
        if let Ok((skill, _)) = &result {
            let mut skills = read_skills(data_dir)?;
            if let Some(entry) = skills.iter_mut().find(|entry| entry.id == skill.id) {
                entry.source_url = skill.source_url.clone();
            }
            write_skill_index(data_dir, &skills)?;
        }
        return result;
    }
    install_local_skill(data_dir, Path::new(source), replace)
}

fn move_to_trash_if_exists(path: &Path) {
    if path.exists() {
        let _ = trash::delete(path);
    }
}

fn frontmatter_scalar(content: &str, target: &str) -> Option<String> {
    if !content.trim_start().starts_with("---") {
        return None;
    }
    for line in content.trim_start().lines().skip(1) {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case(target) {
                let value = value.trim().trim_matches(['\'', '"']);
                return (!value.is_empty()).then(|| value.to_string());
            }
        }
    }
    None
}

fn frontmatter_list(content: &str, target: &str) -> Vec<String> {
    frontmatter_scalar(content, target)
        .map(|value| {
            value
                .trim_matches(['[', ']'])
                .split(',')
                .map(|item| item.trim().trim_matches(['\'', '"']).to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("创建 Skill staging 目录失败: {error}"))?;
    for entry in fs::read_dir(source).map_err(|error| format!("读取 Skill 目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取 Skill 条目失败: {error}"))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata =
            fs::symlink_metadata(&from).map_err(|error| format!("读取 Skill 条目失败: {error}"))?;
        if metadata.is_dir() {
            copy_tree(&from, &to)?;
        } else if metadata.is_file() {
            fs::copy(&from, &to).map_err(|error| format!("复制 Skill 文件失败: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use super::{audit_skill_directory, install_local_skill, read_verified_manifest};

    fn fixture(name: &str, content: &str) -> std::path::PathBuf {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("agent-skill-tests")
            .join(format!("{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("SKILL.md"), content).unwrap();
        root
    }

    fn recycle(path: &std::path::Path) {
        if path.exists() {
            trash::delete(path).unwrap();
        }
    }

    #[test]
    fn audit_builds_manifest_from_declared_capabilities() {
        let root = fixture(
            "manifest",
            "---\nname: 摄影导演\ncapabilities: [chat, image_plan]\nrequiredSections: [规则]\noutputCapability: image_plan\n---\n# 摄影导演\n## 规则\n只输出明确结果",
        );
        let audit = audit_skill_directory(&root).unwrap();
        assert!(audit.allowed, "{:?}", audit.reasons);
        let manifest = audit.manifest.unwrap();
        assert_eq!(manifest.capabilities, vec!["chat", "image_plan"]);
        assert_eq!(manifest.required_sections, vec!["规则"]);
        recycle(&root);
    }

    #[test]
    fn audit_rejects_unknown_capabilities_and_script_directories() {
        let root = fixture(
            "unsafe",
            "---\nname: 危险 Skill\ncapabilities: [chat, terminal]\n---\n# 危险 Skill",
        );
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/run.py"), "print('x')").unwrap();
        let audit = audit_skill_directory(&root).unwrap();
        assert!(!audit.allowed);
        assert!(audit
            .reasons
            .iter()
            .any(|reason| reason.contains("脚本目录")));
        assert!(audit
            .reasons
            .iter()
            .any(|reason| reason.contains("terminal")));
        recycle(&root);
    }

    #[test]
    fn install_persists_a_verified_manifest_and_requires_replace_confirmation() {
        let source = fixture(
            "source",
            "---\nname: 安全 Skill\n---\n# 安全 Skill\n## 规则\n只生成图片计划",
        );
        let data_dir = fixture("data", "# placeholder");
        fs::create_dir_all(data_dir.join("skills")).unwrap();
        fs::create_dir_all(data_dir.join(".staging")).unwrap();
        fs::write(data_dir.join("skills.json"), "[]").unwrap();
        let (skill, _) = install_local_skill(&data_dir, &source, false).unwrap();
        let installed = data_dir.join("skills").join(&skill.directory);
        assert!(read_verified_manifest(&installed).is_ok());
        let error = install_local_skill(&data_dir, &source, false).unwrap_err();
        assert!(error.contains("CONFIRM_REPLACE_SKILL"));
        recycle(&source);
        recycle(&data_dir);
    }
}

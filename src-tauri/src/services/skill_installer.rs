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
    let entry = [root.join("SKILL.md"), root.join("skill.md")]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or("Skill 包缺少 SKILL.md")?;
    let content = fs::read(entry).map_err(|error| format!("读取 Skill 内容失败: {error}"))?;
    let actual_hash = format!("{:x}", Sha256::digest(&content));
    if actual_hash != manifest.content_hash {
        return Err("Skill 内容已变化，manifest 哈希校验失败，请重新安装".into());
    }
    Ok(manifest)
}

/// 为旧版无 manifest 的 Skill 包执行一次安全迁移。
/// 审查失败时不修改原包；通过后以临时文件原子写入 manifest。
pub(crate) fn ensure_skill_manifest(root: &Path) -> Result<SkillManifest, String> {
    if root.join("manifest.json").is_file() {
        return read_verified_manifest(root);
    }
    let audit = audit_skill_directory(root)?;
    if !audit.allowed {
        return Err(format!(
            "旧 Skill 安全迁移被拒绝：{}",
            audit.reasons.join("；")
        ));
    }
    let manifest = audit.manifest.ok_or("Skill 审查未生成 manifest")?;
    let temporary = root.join(format!(".manifest-{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("序列化 Skill manifest 失败: {error}"))?;
    if let Err(error) = fs::write(&temporary, bytes) {
        return Err(format!("写入 Skill manifest 临时文件失败: {error}"));
    }
    if let Err(error) = fs::rename(&temporary, root.join("manifest.json")) {
        move_to_trash_if_exists(&temporary);
        return Err(format!("保存 Skill manifest 失败: {error}"));
    }
    read_verified_manifest(root)
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

/// 将编辑器中的 Skill 草稿按与导入包相同的安全门保存。
/// 先在 staging 中构造完整包并审查，审查通过后才替换正式目录。
pub(crate) fn save_skill_entry(
    data_dir: &Path,
    mut skill: SkillEntry,
    replace: bool,
) -> Result<(SkillEntry, SkillAuditResult), String> {
    skill.id = skill.id.trim().to_string();
    skill.source_url = skill.source_url.trim().to_string();
    skill.notes = skill.notes.trim().to_string();
    skill.directory = skill.directory.trim().to_string();
    skill.source_path = skill.source_path.trim().to_string();
    skill.content = skill.content.trim().to_string();
    if skill.content.is_empty() {
        return Err("Skill 内容不能为空".into());
    }
    if !skill.source_path.is_empty() {
        let source = Path::new(&skill.source_path);
        if source.is_dir() {
            let source_audit = audit_skill_directory(source)?;
            if !source_audit.allowed {
                return Err(format!(
                    "Skill 保存被拒绝：{}",
                    source_audit.reasons.join("；")
                ));
            }
        }
    }

    let is_new = skill.id.is_empty();
    if is_new {
        skill.id = uuid::Uuid::new_v4().to_string();
    }
    let staging_id = format!("save-{}", uuid::Uuid::new_v4());
    let staging = staging_skill_path(data_dir, &staging_id);
    fs::create_dir_all(&staging)
        .map_err(|error| format!("创建 Skill staging 目录失败: {error}"))?;
    if let Err(error) = fs::write(staging.join("SKILL.md"), format!("{}\n", skill.content)) {
        move_to_trash_if_exists(&staging);
        return Err(format!("写入 Skill staging 内容失败: {error}"));
    }

    let references_source = if !skill.source_path.is_empty() {
        let source = Path::new(&skill.source_path);
        if source.is_dir() {
            Some(source.join("references"))
        } else {
            source.parent().map(|parent| parent.join("references"))
        }
    } else {
        None
    };
    let existing_package = if skill.directory.is_empty() && !is_new {
        None
    } else if !skill.directory.is_empty() {
        Some(skills_dir(data_dir).join(&skill.directory))
    } else {
        None
    };
    let source_references = references_source.filter(|path| path.is_dir()).or_else(|| {
        existing_package
            .as_ref()
            .map(|path| path.join("references"))
            .filter(|path| path.is_dir())
    });
    if let Some(source_references) = source_references {
        if let Err(error) = copy_tree(&source_references, &staging.join("references")) {
            move_to_trash_if_exists(&staging);
            return Err(error);
        }
    }

    let audit = match audit_skill_directory(&staging) {
        Ok(audit) if audit.allowed => audit,
        Ok(audit) => {
            move_to_trash_if_exists(&staging);
            return Err(format!("Skill 保存被拒绝：{}", audit.reasons.join("；")));
        }
        Err(error) => {
            move_to_trash_if_exists(&staging);
            return Err(error);
        }
    };
    let manifest = audit.manifest.clone().ok_or("Skill 审查未生成 manifest")?;
    if !skill.directory.is_empty() && !crate::store::is_safe_skill_directory(&skill.directory) {
        move_to_trash_if_exists(&staging);
        return Err("Skill 目录名不安全".into());
    }
    skill.directory = skill_directory_name(&manifest.name, &skill.id);

    let skills = match read_skills(data_dir) {
        Ok(skills) => skills,
        Err(error) => {
            move_to_trash_if_exists(&staging);
            return Err(error);
        }
    };
    let conflict = skills
        .iter()
        .find(|item| item.directory == skill.directory && item.id != skill.id);
    let destination = skills_dir(data_dir).join(&skill.directory);
    let replaces_current_package = existing_package
        .as_ref()
        .is_some_and(|path| path == &destination);
    if (!replace && conflict.is_some())
        || (!replace && destination.exists() && !replaces_current_package)
    {
        move_to_trash_if_exists(&staging);
        return Err(format!(
            "CONFIRM_REPLACE_SKILL:Skill 目录 {} 已被「{}」使用，是否覆盖？",
            skill.directory,
            conflict
                .map(|item| item.name.as_str())
                .unwrap_or("未知 Skill")
        ));
    }

    let manifest_bytes = match serde_json::to_vec_pretty(&manifest) {
        Ok(bytes) => bytes,
        Err(error) => {
            move_to_trash_if_exists(&staging);
            return Err(format!("序列化 Skill manifest 失败: {error}"));
        }
    };
    if let Err(error) = fs::write(staging.join("manifest.json"), manifest_bytes) {
        move_to_trash_if_exists(&staging);
        return Err(format!("写入 Skill manifest 失败: {error}"));
    }
    if destination.exists() {
        if let Err(error) = trash::delete(&destination) {
            move_to_trash_if_exists(&staging);
            return Err(format!("将旧 Skill 移入回收站失败: {error}"));
        }
    }
    if let Some(previous) = existing_package
        .as_ref()
        .filter(|path| *path != &destination && path.exists())
    {
        if let Err(error) = trash::delete(previous) {
            move_to_trash_if_exists(&staging);
            return Err(format!("将改名前的 Skill 移入回收站失败: {error}"));
        }
    }
    fs::rename(&staging, &destination).map_err(|error| {
        move_to_trash_if_exists(&staging);
        format!("安装 Skill 包失败: {error}")
    })?;

    let now = utc_now();
    let created_at = skills
        .iter()
        .find(|item| item.id == skill.id)
        .map(|item| item.created_at.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| now.clone());
    let saved = SkillEntry {
        id: skill.id,
        name: manifest.name.clone(),
        source_url: skill.source_url,
        notes: skill.notes,
        content: fs::read_to_string(destination.join("SKILL.md"))
            .map_err(|error| format!("读取已保存 Skill 失败: {error}"))?
            .trim()
            .to_string(),
        directory: skill.directory,
        source_path: String::new(),
        created_at,
        updated_at: now,
    };
    let mut next_skills = skills;
    next_skills.retain(|item| item.id != saved.id && item.directory != saved.directory);
    next_skills.push(saved.clone());
    if let Err(error) = write_skill_index(data_dir, &next_skills) {
        return Err(format!("Skill 已安装但索引写入失败: {error}"));
    }
    Ok((saved, audit))
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
        if metadata.file_type().is_symlink() {
            return Err(format!("拒绝符号链接: {}", from.display()));
        } else if metadata.is_dir() {
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

    use crate::{models::SkillEntry, store::read_skills};

    use super::{
        audit_skill_directory, ensure_skill_manifest, install_local_skill, read_verified_manifest,
        save_skill_entry,
    };

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

    #[test]
    fn editor_save_uses_the_same_gate_and_keeps_previous_package_on_rejection() {
        let source = fixture(
            "editor-source",
            "---\nname: 编辑器 Skill\n---\n# 编辑器 Skill\n## 规则\n只生成图片计划",
        );
        let data_dir = fixture("editor-data", "# placeholder");
        fs::create_dir_all(data_dir.join("skills")).unwrap();
        fs::create_dir_all(data_dir.join(".staging")).unwrap();
        fs::write(data_dir.join("skills.json"), "[]").unwrap();
        let (installed, _) = install_local_skill(&data_dir, &source, false).unwrap();
        let rejected = SkillEntry {
            id: installed.id.clone(),
            name: installed.name.clone(),
            source_url: installed.source_url.clone(),
            notes: "新备注".into(),
            content: "# 编辑器 Skill\n\n请执行 scripts/render.py".into(),
            directory: installed.directory.clone(),
            source_path: String::new(),
            created_at: installed.created_at.clone(),
            updated_at: installed.updated_at.clone(),
        };
        assert!(save_skill_entry(&data_dir, rejected, false).is_err());
        let package = data_dir.join("skills").join(&installed.directory);
        assert!(read_verified_manifest(&package).is_ok());
        assert!(fs::read_to_string(package.join("SKILL.md"))
            .unwrap()
            .contains("只生成图片计划"));

        let duplicate = SkillEntry {
            id: String::new(),
            name: String::new(),
            source_url: String::new(),
            notes: String::new(),
            content: "# 编辑器 Skill\n\n新的同名内容".into(),
            directory: String::new(),
            source_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let error = save_skill_entry(&data_dir, duplicate, false).unwrap_err();
        assert!(error.contains("CONFIRM_REPLACE_SKILL"));
        assert!(read_verified_manifest(&package).is_ok());

        let renamed = SkillEntry {
            id: installed.id.clone(),
            name: installed.name,
            source_url: installed.source_url,
            notes: "改名".into(),
            content: "# 改名后的 Skill\n\n安全内容".into(),
            directory: installed.directory.clone(),
            source_path: String::new(),
            created_at: installed.created_at,
            updated_at: installed.updated_at,
        };
        let (renamed, _) = save_skill_entry(&data_dir, renamed, false).unwrap();
        assert_eq!(renamed.directory, "改名后的-skill");
        assert!(!package.exists());
        assert!(read_verified_manifest(&data_dir.join("skills").join(renamed.directory)).is_ok());
        recycle(&source);
        recycle(&data_dir);
    }

    #[test]
    fn loading_a_legacy_package_creates_manifest_without_touching_rejected_content() {
        let data_dir = fixture("legacy-data", "# placeholder");
        let package = data_dir.join("skills").join("legacy-skill");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("SKILL.md"), "# Legacy Skill\n\n只聊天").unwrap();
        let skill = SkillEntry {
            id: "legacy-id".into(),
            name: "Legacy Skill".into(),
            source_url: String::new(),
            notes: String::new(),
            content: String::new(),
            directory: "legacy-skill".into(),
            source_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        fs::write(
            data_dir.join("skills.json"),
            serde_json::to_vec(&vec![skill]).unwrap(),
        )
        .unwrap();
        let loaded = read_skills(&data_dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(package.join("manifest.json").is_file());
        assert!(ensure_skill_manifest(&package).is_ok());

        let rejected_package = data_dir.join("skills").join("rejected-skill");
        fs::create_dir_all(&rejected_package).unwrap();
        fs::write(
            rejected_package.join("SKILL.md"),
            "# Rejected\n\n运行 scripts/render.py",
        )
        .unwrap();
        assert!(ensure_skill_manifest(&rejected_package).is_err());
        assert!(!rejected_package.join("manifest.json").exists());
        recycle(&data_dir);
    }
}

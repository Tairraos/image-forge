use std::{fs, path::{Path, PathBuf}};

use sha2::{Digest, Sha256};

use crate::models::{SkillAuditResult, SkillManifest};

const ALLOWED_CAPABILITIES: &[&str] = &["chat", "image_plan", "reference_images"];
const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "js", "ts", "mjs", "cjs", "sh", "bash", "zsh", "ps1", "bat", "cmd", "rb",
    "exe", "bin",
];
const DANGEROUS_TERMS: &[&str] = &[
    "script", "scripts", "command", "commands", "shell", "exec", "executable", "subprocess",
    "runtime", "terminal", "powershell", "python", "node", "curl", "wget",
];

pub(crate) fn audit_skill_directory(root: &Path) -> Result<SkillAuditResult, String> {
    if !root.is_dir() {
        return Err("Skill 包目录不存在".into());
    }
    let entry = [root.join("SKILL.md"), root.join("skill.md")]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or("Skill 包缺少 SKILL.md")?;
    let content = fs::read_to_string(&entry)
        .map_err(|error| format!("读取 Skill Markdown 失败: {error}"))?;
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();
    let mut total_size = 0u64;
    let mut reference_count = 0usize;

    inspect_tree(root, root, &mut total_size, &mut reference_count, &mut reasons)?;
    let lower = content.to_ascii_lowercase();
    for term in DANGEROUS_TERMS {
        if contains_capability_term(&lower, term) {
            reasons.push(format!("SKILL.md 声明或要求 `{term}` 能力，当前 Agent 不提供系统命令执行"));
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
    let manifest = SkillManifest {
        schema_version: 1,
        content_hash: hash,
        name,
        capabilities: vec!["chat".into(), "image_plan".into(), "reference_images".into()],
        sections,
        required_sections: Vec::new(),
        output_capability: "image_plan".into(),
    };
    if total_size > 256 * 1024 * 1024 {
        reasons.push("Skill 包超过 256 MB 上限".into());
    }
    if reference_count > 200 {
        reasons.push("Skill 参考文档超过 200 个上限".into());
    }
    for capability in &manifest.capabilities {
        if !ALLOWED_CAPABILITIES.contains(&capability.as_str()) {
            reasons.push(format!("未知能力 `{capability}`"));
        }
    }
    Ok(SkillAuditResult {
        allowed: reasons.is_empty(),
        reasons,
        warnings,
        manifest: Some(manifest),
    })
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
        if relative.components().any(|component| matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))) {
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
            let name = current.file_name().and_then(|value| value.to_str()).unwrap_or_default();
            if matches!(name.to_ascii_lowercase().as_str(), "scripts" | "script" | "bin" | "tools") {
                reasons.push(format!("发现不允许的脚本目录: {}", relative.display()));
                continue;
            }
            inspect_tree(root, &current, total_size, reference_count, reasons)?;
            continue;
        }
        *total_size = total_size.saturating_add(metadata.len());
        if metadata.len() > 1024 * 1024 {
            reasons.push(format!("文件超过 1 MB 上限: {}", relative.display()));
        }
        let extension = current.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
        if !matches!(extension.as_str(), "md" | "markdown" | "png" | "jpg" | "jpeg" | "webp" | "gif") {
            reasons.push(format!("文件类型不受支持: {}", relative.display()));
        }
        if extension == "md" || extension == "markdown" {
            *reference_count += 1;
            if let Ok(text) = fs::read_to_string(&current) {
                let lower = text.to_ascii_lowercase();
                for ext in SCRIPT_EXTENSIONS {
                    if lower.contains(&format!(".{ext}")) {
                        reasons.push(format!("{} 包含脚本文件引用 .{ext}", relative.display()));
                    }
                }
            }
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
        .map(|(_, rest)| rest.split(')').map(|value| value.trim().to_string()).collect())
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

use std::{collections::HashSet, time::Duration};

use reqwest::header::CONTENT_TYPE;
use url::Url;

use crate::{defaults::APP_USER_AGENT, models::SkillFetchResult};

const SKILL_FETCH_TIMEOUT_SECONDS: u64 = 30;
const MAX_SKILL_BYTES: usize = 1024 * 1024;

/// 从直接 Markdown URL 或目录下的 SKILL.md/skill.md 提取纯文本 Skill。
pub(crate) async fn fetch_skill_markdown(source_url: &str) -> Result<SkillFetchResult, String> {
    let source_url = source_url.trim();
    let parsed = Url::parse(source_url).map_err(|_| "请输入完整的 Skill URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Skill URL 只支持 http 或 https".into());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(SKILL_FETCH_TIMEOUT_SECONDS))
        .user_agent(APP_USER_AGENT)
        .build()
        .map_err(|error| format!("创建 Skill 提取请求失败: {error}"))?;

    let candidates = skill_url_candidates(&parsed);
    let mut last_error = String::new();
    for candidate in candidates {
        match fetch_candidate(&client, &candidate).await {
            Ok(Some(content)) => {
                return Ok(SkillFetchResult {
                    source_url: candidate.to_string(),
                    content,
                });
            }
            Ok(None) => {}
            Err(error) => last_error = error,
        }
    }

    if last_error.is_empty() {
        Err("没有找到可读取的 Markdown。请确认 URL 指向 MD 文件或包含 SKILL.md。".into())
    } else {
        Err(format!("提取 Skill 失败：{last_error}"))
    }
}

async fn fetch_candidate(
    client: &reqwest::Client,
    candidate: &Url,
) -> Result<Option<String>, String> {
    let response = client
        .get(candidate.clone())
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!("请求 {} 超过 30 秒", candidate)
            } else {
                format!("请求 {} 失败: {error}", candidate)
            }
        })?;
    if !response.status().is_success() {
        return Ok(None);
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_SKILL_BYTES as u64)
    {
        return Err("Skill 文件超过 1 MB".into());
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    if content_type.contains("text/html") {
        return Ok(None);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取 {} 失败: {error}", candidate))?;
    if bytes.len() > MAX_SKILL_BYTES {
        return Err("Skill 文件超过 1 MB".into());
    }
    let content =
        String::from_utf8(bytes.to_vec()).map_err(|_| format!("{} 不是 UTF-8 文本", candidate))?;
    if !looks_like_markdown(candidate, &content_type, &content) {
        return Ok(None);
    }
    let content = content.trim().to_string();
    if content.is_empty() {
        Ok(None)
    } else {
        Ok(Some(content))
    }
}

fn skill_url_candidates(source: &Url) -> Vec<Url> {
    let mut candidates = Vec::new();
    push_candidate(&mut candidates, source.clone());

    let base = source.as_str().trim_end_matches('/');
    for file_name in ["SKILL.md", "skill.md"] {
        if let Ok(candidate) = Url::parse(&format!("{base}/{file_name}")) {
            push_candidate(&mut candidates, candidate);
        }
    }

    let originals = candidates.clone();
    for candidate in originals {
        for raw in github_raw_candidates(&candidate) {
            push_candidate(&mut candidates, raw);
        }
    }
    candidates
}

fn push_candidate(candidates: &mut Vec<Url>, candidate: Url) {
    if candidates.iter().all(|existing| existing != &candidate) {
        candidates.push(candidate);
    }
}

fn github_raw_candidates(source: &Url) -> Vec<Url> {
    if source.host_str() != Some("github.com") {
        return Vec::new();
    }
    let segments = source
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if segments.len() < 2 {
        return Vec::new();
    }
    let owner = segments[0];
    let repository = segments[1].trim_end_matches(".git");
    let mut values = HashSet::new();
    if segments
        .get(2)
        .is_some_and(|segment| matches!(*segment, "blob" | "tree"))
        && segments.len() >= 5
    {
        let branch = segments[3];
        let path = segments[4..].join("/");
        values.insert(format!(
            "https://raw.githubusercontent.com/{owner}/{repository}/{branch}/{path}"
        ));
    } else {
        let path = segments.get(2..).unwrap_or_default().join("/");
        for branch in ["main", "master"] {
            values.insert(format!(
                "https://raw.githubusercontent.com/{owner}/{repository}/{branch}/{path}"
            ));
        }
    }
    values
        .into_iter()
        .filter_map(|value| Url::parse(&value).ok())
        .collect()
}

fn looks_like_markdown(url: &Url, content_type: &str, content: &str) -> bool {
    let leading = content.trim_start().to_lowercase();
    if leading.starts_with("<!doctype html") || leading.starts_with("<html") {
        return false;
    }
    let path_is_markdown = url.path().to_lowercase().ends_with(".md");
    let type_is_text = content_type.is_empty()
        || content_type.contains("text/plain")
        || content_type.contains("text/markdown")
        || content_type.contains("text/x-markdown")
        || content_type.contains("application/octet-stream");
    path_is_markdown
        || type_is_text
        || content.trim_start().starts_with("---")
        || content
            .lines()
            .any(|line| line.trim_start().starts_with("# "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_urls_try_both_skill_file_cases() {
        let source = Url::parse("https://example.com/skills/camera").unwrap();
        let candidates = skill_url_candidates(&source)
            .into_iter()
            .map(|url| url.to_string())
            .collect::<Vec<_>>();
        assert!(candidates.contains(&"https://example.com/skills/camera/SKILL.md".into()));
        assert!(candidates.contains(&"https://example.com/skills/camera/skill.md".into()));
    }

    #[test]
    fn github_tree_urls_include_raw_content_candidate() {
        let source =
            Url::parse("https://github.com/openai/skills/tree/main/skills/imagegen").unwrap();
        let candidates = skill_url_candidates(&source)
            .into_iter()
            .map(|url| url.to_string())
            .collect::<Vec<_>>();
        assert!(candidates.iter().any(|url| {
            url == "https://raw.githubusercontent.com/openai/skills/main/skills/imagegen/SKILL.md"
        }));
    }
}

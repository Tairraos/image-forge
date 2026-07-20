use std::{path::Path, time::Duration};

use chrono::Utc;
use image::ImageReader;
use reqwest::{Client, Proxy};
use serde_json::Value;
use url::Url;

use crate::defaults::{APP_USER_AGENT, DEFAULT_BASE_URL, DEFAULT_PROVIDER_ID};

pub(crate) const REQUEST_TIMEOUT_SECONDS: u64 = 300;

pub(crate) fn recycle_path(path: &Path) -> Result<(), String> {
    #[cfg(not(test))]
    return trash::delete(path).map_err(|error| error.to_string());

    #[cfg(test)]
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else {
        std::fs::remove_file(path).map_err(|error| error.to_string())
    }
}

pub(crate) fn image_size_from_bytes(bytes: &[u8]) -> Option<String> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(format!("{}x{}", image.width(), image.height()))
}

pub(crate) fn image_size_from_path(path: &Path) -> Option<String> {
    let (width, height) = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some(format!("{width}x{height}"))
}

pub(crate) fn http_client_with_proxy(
    proxy_url: &str,
    timeout_seconds: u64,
    http1_only: bool,
) -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .user_agent(APP_USER_AGENT);
    if http1_only {
        builder = builder.http1_only();
    }
    if let Some(proxy_url) = normalize_proxy_url(proxy_url) {
        let proxy = Proxy::all(&proxy_url).map_err(|error| format!("代理地址无效: {error}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))
}

pub(crate) fn normalize_proxy_url(proxy_url: &str) -> Option<String> {
    let value = proxy_url.trim();
    if value.is_empty() {
        return None;
    }
    if value.contains("://") {
        Some(value.to_string())
    } else {
        Some(format!("http://{value}"))
    }
}

pub(crate) fn format_request_error(label: &str, error: reqwest::Error) -> String {
    if error.is_timeout() {
        return format!("{label} 超时：超过 5 分钟未返回结果");
    }
    format!("{label} 失败: {error}")
}

pub(crate) fn normalize_base_url(base_url: &str) -> Result<String, String> {
    let raw = if base_url.trim().is_empty() {
        DEFAULT_BASE_URL
    } else {
        base_url.trim().trim_end_matches('/')
    };
    let mut url = Url::parse(raw).map_err(|_| "Base URL 必须是完整 URL".to_string())?;
    let mut path = url.path().trim_end_matches('/').to_string();
    for suffix in ["/images/generations", "/images/edits", "/models"] {
        if path.ends_with(suffix) {
            path.truncate(path.len() - suffix.len());
        }
    }
    if path.is_empty() {
        path = "/v1".into();
    }
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub(crate) fn should_send_input_fidelity(model: &str, input_fidelity: &str) -> bool {
    !input_fidelity.trim().is_empty() && model.trim().to_lowercase() != "gpt-image-2"
}

pub(crate) fn normalize_prompt_fidelity(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "original" => "original".into(),
        "off" => "off".into(),
        _ => "strict".into(),
    }
}

pub(crate) fn normalize_resolution(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "2k" => "2k".into(),
        "4k" => "4k".into(),
        "1k" | "standard" => "standard".into(),
        _ => "standard".into(),
    }
}

pub(crate) fn normalize_ratio(value: &str) -> String {
    match value.trim() {
        "1:1" | "4:5" | "5:4" | "3:4" | "4:3" | "2:3" | "3:2" | "9:16" | "16:9" | "9:21"
        | "21:9" => value.trim().into(),
        _ => "1:1".into(),
    }
}

pub(crate) fn normalize_quality(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "low" => "low".into(),
        "medium" => "medium".into(),
        "high" => "high".into(),
        _ => "auto".into(),
    }
}

pub(crate) fn orientation_for_ratio(ratio: &str) -> String {
    match normalize_ratio(ratio).as_str() {
        "4:5" | "3:4" | "2:3" | "9:16" | "9:21" => "portrait".into(),
        "5:4" | "4:3" | "3:2" | "16:9" | "21:9" => "landscape".into(),
        _ => "square".into(),
    }
}

pub(crate) fn size_for_preset(resolution: &str, ratio: &str) -> String {
    let resolution = normalize_resolution(resolution);
    let ratio = normalize_ratio(ratio);
    let (width, height) = match (resolution.as_str(), ratio.as_str()) {
        ("standard", "1:1") => (1024, 1024),
        ("standard", "4:5") => (1024, 1280),
        ("standard", "5:4") => (1280, 1024),
        ("standard", "3:4") => (1152, 1536),
        ("standard", "4:3") => (1536, 1152),
        ("standard", "2:3") => (1024, 1536),
        ("standard", "3:2") => (1536, 1024),
        ("standard", "9:16") => (864, 1536),
        ("standard", "16:9") => (1536, 864),
        ("standard", "9:21") => (672, 1568),
        ("standard", "21:9") => (1568, 672),
        ("2k", "1:1") => (2048, 2048),
        ("2k", "4:5") => (1600, 2000),
        ("2k", "5:4") => (2000, 1600),
        ("2k", "3:4") => (1536, 2048),
        ("2k", "4:3") => (2048, 1536),
        ("2k", "2:3") => (1344, 2016),
        ("2k", "3:2") => (2016, 1344),
        ("2k", "9:16") => (1152, 2048),
        ("2k", "16:9") => (2048, 1152),
        ("2k", "9:21") => (1152, 2688),
        ("2k", "21:9") => (2688, 1152),
        ("4k", "1:1") => (2880, 2880),
        ("4k", "4:5") => (2560, 3200),
        ("4k", "5:4") => (3200, 2560),
        ("4k", "3:4") => (2448, 3264),
        ("4k", "4:3") => (3264, 2448),
        ("4k", "2:3") => (2336, 3504),
        ("4k", "3:2") => (3504, 2336),
        ("4k", "9:16") => (2160, 3840),
        ("4k", "16:9") => (3840, 2160),
        ("4k", "9:21") => (1632, 3808),
        ("4k", "21:9") => (3808, 1632),
        _ => (1024, 1024),
    };
    format!("{width}x{height}")
}

pub(crate) fn prompt_with_ratio_instruction(prompt: &str, ratio: &str) -> String {
    let ratio = normalize_ratio(ratio);
    let instruction = format!("将宽高比设为 {ratio}");
    let prompt = prompt.trim_end();
    if prompt.contains(&instruction) {
        return prompt.to_string();
    }
    if prompt.is_empty() {
        instruction
    } else {
        format!("{prompt}\n\n{instruction}")
    }
}

pub(crate) fn image_prompt_for_transport(
    prompt: &str,
    ratio: &str,
    prompt_fidelity: &str,
) -> String {
    let prompt = prompt_with_ratio_instruction(prompt, ratio);
    if normalize_prompt_fidelity(prompt_fidelity) != "strict" {
        return prompt;
    }
    format!(
        "{}\n\n用户原始提示词：\n{}",
        prompt_guard_instructions(),
        prompt
    )
}

fn prompt_guard_instructions() -> &'static str {
    "提示词保真规则：\n你只能扩写用户提示词，不得改变原意，不得删除、弱化或转移用户的硬性约束。\n如果硬性约束之间有冲突，优先保留用户明确指定的对象、文字、字体、颜色、构图和限制项。"
}

pub(crate) fn normalize_output_format(format: &str) -> String {
    match format.trim().to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".into(),
        "webp" => "webp".into(),
        _ => "png".into(),
    }
}

pub(crate) fn extension_for_format(format: &str, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return "jpg";
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return "webp";
    }
    match format {
        "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

pub(crate) fn mime_for_format(format: &str) -> &'static str {
    match format {
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

pub(crate) fn format_api_error(label: &str, error: &Value) -> String {
    if let Some(object) = error.as_object() {
        let code = object
            .get("code")
            .or_else(|| object.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let message = object
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !code.is_empty() && !message.is_empty() {
            return format!("{label}: {code}: {message}");
        }
        if !message.is_empty() {
            return format!("{label}: {message}");
        }
        if !code.is_empty() {
            return format!("{label}: {code}");
        }
    }
    format!("{label}: {error}")
}

pub(crate) fn image_mime_type(path: &Path, bytes: &[u8]) -> Result<String, String> {
    let mime_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".to_string()
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg".to_string()
    } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp".to_string()
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif".to_string()
    } else {
        mime_guess::from_path(path)
            .first()
            .map(|mime| mime.to_string())
            .unwrap_or_else(|| "application/octet-stream".into())
    };

    if mime_type.starts_with("image/") {
        Ok(mime_type)
    } else {
        Err("只支持图像文件".into())
    }
}

pub(crate) fn clean_text(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

pub(crate) fn sanitize_id(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        DEFAULT_PROVIDER_ID.into()
    } else {
        out
    }
}

pub(crate) fn utc_now() -> String {
    Utc::now().to_rfc3339()
}

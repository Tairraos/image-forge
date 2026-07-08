use std::{path::Path, time::Duration};

use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use url::Url;

use crate::defaults::{APP_USER_AGENT, DEFAULT_BASE_URL, DEFAULT_PROVIDER_ID};

pub(crate) fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent(APP_USER_AGENT)
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败: {error}"))
}

pub(crate) fn normalize_base_url(base_url: &str) -> Result<String, String> {
    let raw = if base_url.trim().is_empty() {
        DEFAULT_BASE_URL
    } else {
        base_url.trim().trim_end_matches('/')
    };
    let mut url = Url::parse(raw).map_err(|_| "Base URL 必须是完整 URL".to_string())?;
    let mut path = url.path().trim_end_matches('/').to_string();
    for suffix in ["/images/generations", "/images/edits"] {
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

pub(crate) fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
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

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into())
}

pub(crate) fn utc_now() -> String {
    Utc::now().to_rfc3339()
}

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
};

use chrono::{FixedOffset, Utc};

static RUNTIME_LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub struct RuntimeState {
    pub worker_active: Mutex<bool>,
    pub cancel_requests: Mutex<HashSet<String>>,
    pub deleted_tasks: Mutex<HashSet<String>>,
    pub agent_tasks: Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            worker_active: Mutex::new(false),
            cancel_requests: Mutex::new(HashSet::new()),
            deleted_tasks: Mutex::new(HashSet::new()),
            agent_tasks: Mutex::new(HashMap::new()),
        }
    }

    pub fn push_log(&self, event: &str, message: impl AsRef<str>) {
        let result = event_result(event);
        let message = message.as_ref();
        record_operation(
            event,
            result,
            message,
            proxy_from_message(message),
            (result == "失败").then_some(message),
        );
    }
}

pub(crate) fn record_operation(
    operation: &str,
    result: &str,
    params: impl AsRef<str>,
    proxy_used: Option<bool>,
    error: Option<&str>,
) {
    let line = format_log_line(
        &china_now(),
        operation,
        result,
        params.as_ref(),
        proxy_used,
        error,
    );
    let Ok(mut logs) = runtime_logs().lock() else {
        return;
    };
    logs.push(line);
}

pub(crate) fn runtime_logs_text() -> String {
    runtime_logs()
        .lock()
        .map(|logs| logs.join("\n"))
        .unwrap_or_default()
}

fn runtime_logs() -> &'static Mutex<Vec<String>> {
    RUNTIME_LOGS.get_or_init(|| Mutex::new(Vec::new()))
}

fn format_log_line(
    timestamp: &str,
    operation: &str,
    result: &str,
    params: &str,
    proxy_used: Option<bool>,
    error: Option<&str>,
) -> String {
    let mut fields = vec![sanitize(timestamp), sanitize(operation), sanitize(result)];
    let params = format_params(params, proxy_used.is_some(), error.is_some());
    if !params.is_empty() {
        fields.push(params);
    }
    if let Some(proxy_used) = proxy_used {
        fields.push(format!("代理={}", if proxy_used { "是" } else { "否" }));
    }
    if let Some(error) = error {
        fields.push(format!("错误={}", non_empty(error)));
    }
    fields.join(" - ")
}

fn china_now() -> String {
    Utc::now()
        .with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("UTC+8 是有效时区"))
        .format("%m-%d %H:%M:%S")
        .to_string()
}

fn format_params(value: &str, omit_proxy: bool, omit_error: bool) -> String {
    let value = sanitize(value);
    let fields = parse_param_fields(&value);
    if fields.is_empty() {
        return value;
    }
    fields
        .into_iter()
        .filter(|(key, _)| !(omit_proxy && key == "proxy") && !(omit_error && key == "error"))
        .map(|(key, value)| {
            let value = if is_path_key(&key) {
                simplify_path(&value)
            } else {
                value
            };
            format!("{key}={value}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_param_fields(value: &str) -> Vec<(String, String)> {
    let starts = value
        .char_indices()
        .filter_map(|(index, ch)| {
            if index > 0 && ch != ' ' {
                return None;
            }
            let start = if ch == ' ' { index + 1 } else { index };
            let remainder = &value[start..];
            let equals = remainder.find('=')?;
            let key = &remainder[..equals];
            (!key.is_empty()
                && key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
            .then_some((start, equals, key.to_string()))
        })
        .collect::<Vec<_>>();
    if starts.first().map(|(start, _, _)| *start) != Some(0) {
        return Vec::new();
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, (start, equals, key))| {
            let value_start = start + equals + 1;
            let value_end = starts
                .get(index + 1)
                .map(|(next_start, _, _)| next_start.saturating_sub(1))
                .unwrap_or(value.len());
            (
                key.clone(),
                value[value_start..value_end].trim().to_string(),
            )
        })
        .collect()
}

fn is_path_key(key: &str) -> bool {
    matches!(key, "path" | "parent" | "output_dir" | "input_dir")
}

fn simplify_path(value: &str) -> String {
    Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| value.to_string())
}

fn sanitize(value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ");
    redact_data_root(value.trim())
}

fn redact_data_root(value: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return value.to_string();
    };
    redact_path_prefix(value, &Path::new(&home).join(".image-forge"))
}

fn redact_path_prefix(value: &str, root: &Path) -> String {
    let root = root.to_string_lossy();
    if root.is_empty() {
        value.to_string()
    } else {
        value.replace(root.as_ref(), "<data-dir>")
    }
}

fn non_empty(value: &str) -> String {
    let value = sanitize(value);
    if value.is_empty() {
        "-".into()
    } else {
        value
    }
}

fn event_result(event: &str) -> &'static str {
    let event = event.to_ascii_lowercase();
    if event.contains("error")
        || event.contains("failed")
        || event.contains("missing")
        || event.contains("not_found")
    {
        "失败"
    } else if event.contains("success") || event.contains("complete") || event.ends_with(".result")
    {
        "成功"
    } else {
        "进行中"
    }
}

fn proxy_from_message(message: &str) -> Option<bool> {
    if message.contains("proxy=on") || message.contains("with proxy=") {
        Some(true)
    } else if message.contains("proxy=off") || message.contains("without proxy") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::format_log_line;

    #[test]
    fn log_uses_compact_format_and_optional_fields() {
        let line = format_log_line(
            "07-17 20:00:00",
            "获取模型列表",
            "失败",
            "provider=test model=gpt proxy=on error=network error",
            Some(true),
            Some("network error"),
        );
        assert_eq!(
            line,
            "07-17 20:00:00 - 获取模型列表 - 失败 - provider=test, model=gpt - 代理=是 - 错误=network error"
        );
    }

    #[test]
    fn log_hides_data_directory_and_empty_optional_fields() {
        let line = format_log_line(
            "07-17 18:48:14",
            "读取数据文件",
            "成功",
            "path=/Users/xiaole/Library/Application Support/com.xiaole.imageforge/queue.json bytes=87",
            None,
            None,
        );
        assert_eq!(
            line,
            "07-17 18:48:14 - 读取数据文件 - 成功 - path=queue.json, bytes=87"
        );
    }

    #[test]
    fn log_redacts_data_root_inside_error_messages() {
        let home = std::env::var_os("HOME").expect("测试环境需要 HOME");
        let root = Path::new(&home).join(".image-forge");
        let error = format!(
            "读取 {} 失败",
            root.join("agent/sessions/session.json").display()
        );
        let line = format_log_line(
            "07-17 18:48:14",
            "读取数据文件",
            "失败",
            "",
            None,
            Some(&error),
        );
        assert_eq!(
            line,
            "07-17 18:48:14 - 读取数据文件 - 失败 - 错误=读取 <data-dir>/agent/sessions/session.json 失败"
        );
    }
}

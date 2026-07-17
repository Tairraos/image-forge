use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use crate::utils::utc_now;

const MAX_RUNTIME_LOGS: usize = 500;
static RUNTIME_LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub struct RuntimeState {
    pub worker_active: Mutex<bool>,
    pub cancel_requests: Mutex<HashSet<String>>,
    pub deleted_tasks: Mutex<HashSet<String>>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            worker_active: Mutex::new(false),
            cancel_requests: Mutex::new(HashSet::new()),
            deleted_tasks: Mutex::new(HashSet::new()),
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
        &utc_now(),
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
    let overflow = logs.len().saturating_sub(MAX_RUNTIME_LOGS);
    if overflow > 0 {
        logs.drain(0..overflow);
    }
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
    format!(
        "{} | 操作={} | 结果={} | 参数={} | 代理={} | 错误={}",
        sanitize(timestamp),
        sanitize(operation),
        sanitize(result),
        non_empty(params),
        match proxy_used {
            Some(true) => "是",
            Some(false) => "否",
            None => "不适用",
        },
        error.map(non_empty).unwrap_or_else(|| "-".into())
    )
}

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r'], " ").trim().to_string()
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
    } else if event.contains("success")
        || event.contains("complete")
        || event.ends_with(".result")
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
    use super::format_log_line;

    #[test]
    fn structured_log_contains_required_fields() {
        let line = format_log_line(
            "2026-07-17T12:00:00Z",
            "获取模型列表",
            "失败",
            "provider=test model=gpt",
            Some(true),
            Some("network error"),
        );
        assert!(line.contains("操作=获取模型列表"));
        assert!(line.contains("结果=失败"));
        assert!(line.contains("参数=provider=test model=gpt"));
        assert!(line.contains("代理=是"));
        assert!(line.contains("错误=network error"));
    }
}

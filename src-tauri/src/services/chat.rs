use std::time::{Duration, Instant};

use reqwest::{
    header::{ACCEPT, ACCEPT_LANGUAGE},
    Client,
};
use serde_json::{json, Value};

use crate::{
    models::ApiProvider,
    state::RuntimeState,
    utils::{format_api_error, http_client_with_proxy, normalize_base_url},
};

const CHAT_COMPLETION_TIMEOUT_SECONDS: u64 = 60;

pub(crate) async fn fill_template(
    provider: &ApiProvider,
    template: &str,
    runtime_state: Option<&RuntimeState>,
) -> Result<String, String> {
    let started = Instant::now();
    record_runtime_log(
        runtime_state,
        "ai_fill.service.start",
        format!(
            "provider_id={} provider_name={} model={} template_len={} placeholders={} proxy={}",
            provider.id,
            provider.name,
            provider.image_model,
            template.chars().count(),
            placeholder_count(template),
            if provider.proxy_url.trim().is_empty() {
                "off"
            } else {
                "on"
            }
        ),
    );

    let base_url = match normalize_base_url(&provider.base_url) {
        Ok(base_url) => base_url,
        Err(error) => {
            record_runtime_log(
                runtime_state,
                "ai_fill.service.base_url_error",
                format!("provider_id={} error={}", provider.id, error),
            );
            return Err(error);
        }
    };
    let client = chat_completion_client(&provider.proxy_url).map_err(|error| {
        record_runtime_log(
            runtime_state,
            "ai_fill.service.client_error",
            format!("error={}", error),
        );
        error
    })?;
    record_runtime_log(
        runtime_state,
        "ai_fill.service.client",
        if provider.proxy_url.trim().is_empty() {
            "using http1 client without proxy".to_string()
        } else {
            format!(
                "using http1 client with proxy={}",
                provider.proxy_url.trim()
            )
        },
    );
    let payload = json!({
        "model": provider.image_model,
        "messages": [
            {
                "role": "system",
                "content": "你是提示词模板填充助手。用户会提供一个包含若干 {占位描述} 的生图提示词模板。请根据花括号里的描述语义，把每一处花括号连同里面的描述替换为具体、自然、适合生图的中文内容。不要保留花括号，不要改变花括号外的其它文字，不要输出解释、Markdown 或代码块，只输出填充后的完整文本。"
            },
            {
                "role": "user",
                "content": template
            }
        ],
        "temperature": 0.2,
        "stream": false
    });

    let url = format!("{base_url}/chat/completions");
    record_runtime_log(
        runtime_state,
        "ai_fill.service.request",
        format!("url={} model={}", url, provider.image_model),
    );
    let response = match client
        .post(&url)
        .bearer_auth(provider.api_key.trim())
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7")
        .timeout(Duration::from_secs(CHAT_COMPLETION_TIMEOUT_SECONDS))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            record_runtime_log(
                runtime_state,
                "ai_fill.service.request_error",
                format!(
                    "elapsed_ms={} timeout={} error={:?}",
                    started.elapsed().as_millis(),
                    error.is_timeout(),
                    error
                ),
            );
            let message = if error.is_timeout() {
                format!("AI 填充超时：超过 {CHAT_COMPLETION_TIMEOUT_SECONDS} 秒未返回结果")
            } else {
                format!("AI 填充请求失败: {error}")
            };
            return Err(message);
        }
    };

    let status = response.status();
    record_runtime_log(
        runtime_state,
        "ai_fill.service.response",
        format!(
            "elapsed_ms={} status={}",
            started.elapsed().as_millis(),
            status
        ),
    );
    let text = match response.text().await {
        Ok(text) => text,
        Err(error) => {
            record_runtime_log(
                runtime_state,
                "ai_fill.service.body_error",
                format!(
                    "elapsed_ms={} error={}",
                    started.elapsed().as_millis(),
                    error
                ),
            );
            return Err(format!("读取对话模型响应失败: {error}"));
        }
    };
    record_runtime_log(
        runtime_state,
        "ai_fill.service.body",
        format!(
            "elapsed_ms={} bytes={} preview={}",
            started.elapsed().as_millis(),
            text.len(),
            truncate_for_log(&text, 500)
        ),
    );
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            record_runtime_log(
                runtime_state,
                "ai_fill.service.json_error",
                format!(
                    "elapsed_ms={} error={}",
                    started.elapsed().as_millis(),
                    error
                ),
            );
            return Err(format!("对话模型返回了无效 JSON: {error}"));
        }
    };
    if !status.is_success() {
        if let Some(error) = value.get("error") {
            let message = format_api_error("对话模型", error);
            record_runtime_log(
                runtime_state,
                "ai_fill.service.api_error",
                format!(
                    "elapsed_ms={} error={}",
                    started.elapsed().as_millis(),
                    message
                ),
            );
            return Err(message);
        }
        let message = format!("对话模型请求失败: HTTP {}", status.as_u16());
        record_runtime_log(
            runtime_state,
            "ai_fill.service.http_error",
            format!(
                "elapsed_ms={} error={}",
                started.elapsed().as_millis(),
                message
            ),
        );
        return Err(message);
    }

    let result = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned);

    if let Some(result) = result {
        record_runtime_log(
            runtime_state,
            "ai_fill.service.success",
            format!(
                "elapsed_ms={} output_len={}",
                started.elapsed().as_millis(),
                result.chars().count()
            ),
        );
        Ok(result)
    } else {
        record_runtime_log(
            runtime_state,
            "ai_fill.service.empty_content",
            format!(
                "elapsed_ms={} top_level_keys={}",
                started.elapsed().as_millis(),
                json_keys(&value)
            ),
        );
        Err("对话模型没有返回填充内容".into())
    }
}

fn chat_completion_client(proxy_url: &str) -> Result<Client, String> {
    http_client_with_proxy(proxy_url, CHAT_COMPLETION_TIMEOUT_SECONDS, true)
}

fn record_runtime_log(runtime_state: Option<&RuntimeState>, event: &str, message: impl AsRef<str>) {
    if let Some(runtime_state) = runtime_state {
        runtime_state.push_log(event, message);
    }
}

fn placeholder_count(value: &str) -> usize {
    let mut count = 0;
    let mut inside = false;
    for ch in value.chars() {
        if ch == '{' && !inside {
            inside = true;
        } else if ch == '}' && inside {
            count += 1;
            inside = false;
        }
    }
    count
}

fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let mut text: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        text.push_str("...");
    }
    text.replace(['\n', '\r'], "\\n")
}

fn json_keys(value: &Value) -> String {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| "non-object".into())
}

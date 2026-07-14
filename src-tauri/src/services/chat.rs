use std::time::Duration;

use reqwest::{
    header::{ACCEPT, ACCEPT_LANGUAGE},
    Client,
};
use serde_json::{json, Value};

use crate::{
    defaults::APP_USER_AGENT,
    models::ApiProvider,
    utils::{format_api_error, normalize_base_url},
};

const CHAT_COMPLETION_TIMEOUT_SECONDS: u64 = 60;

pub(crate) async fn fill_template(
    client: &Client,
    provider: &ApiProvider,
    template: &str,
) -> Result<String, String> {
    let base_url = normalize_base_url(&provider.base_url)?;
    let client = chat_completion_client().unwrap_or_else(|_| client.clone());
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

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(provider.api_key.trim())
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7")
        .timeout(Duration::from_secs(CHAT_COMPLETION_TIMEOUT_SECONDS))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!("AI 填充超时：超过 {CHAT_COMPLETION_TIMEOUT_SECONDS} 秒未返回结果")
            } else {
                format!("AI 填充请求失败: {error}")
            }
        })?;

    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("对话模型返回了无效 JSON: {error}"))?;
    if !status.is_success() {
        if let Some(error) = value.get("error") {
            return Err(format_api_error("对话模型", error));
        }
        return Err(format!("对话模型请求失败: HTTP {}", status.as_u16()));
    }

    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "对话模型没有返回填充内容".into())
}

fn chat_completion_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(CHAT_COMPLETION_TIMEOUT_SECONDS))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(APP_USER_AGENT)
        .http1_only()
        .build()
        .map_err(|error| format!("创建对话模型 HTTP 客户端失败: {error}"))
}

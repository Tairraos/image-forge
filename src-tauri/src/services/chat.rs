use reqwest::{header::ACCEPT, Client};
use serde_json::{json, Value};

use crate::{
    models::ApiProvider,
    utils::{format_api_error, format_request_error, normalize_base_url},
};

pub(crate) async fn fill_template(
    client: &Client,
    provider: &ApiProvider,
    template: &str,
) -> Result<String, String> {
    let base_url = normalize_base_url(&provider.base_url)?;
    let payload = json!({
        "model": provider.image_model,
        "messages": [
            {
                "role": "system",
                "content": "你是提示词模板填充助手。用户会提供一个包含若干 {占位描述} 的模板。请根据花括号里的描述，把每一处花括号连同里面的描述替换为适合生图提示词的具体内容。不要改变花括号外的其它文字，不要输出解释，只输出填充后的完整文本。"
            },
            {
                "role": "user",
                "content": template
            }
        ],
        "temperature": 0.7
    });

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(&provider.api_key)
        .header(ACCEPT, "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|error| format_request_error("对话模型请求", error))?;

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

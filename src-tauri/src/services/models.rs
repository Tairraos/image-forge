use std::time::Duration;

use reqwest::{header::ACCEPT, Client};
use serde_json::Value;

use crate::{
    models::ApiProvider,
    utils::{format_api_error, normalize_base_url},
};

const MODEL_LIST_TIMEOUT_SECONDS: u64 = 30;

pub(crate) async fn list_provider_models(
    client: &Client,
    provider: &ApiProvider,
) -> Result<Vec<String>, String> {
    if provider.api_key.trim().is_empty() {
        return Err(format!("API 源「{}」还没有填写 API Key", provider.name));
    }

    let base_url = normalize_base_url(&provider.base_url)?;
    let response = client
        .get(format!("{base_url}/models"))
        .bearer_auth(provider.api_key.trim())
        .header(ACCEPT, "application/json")
        .timeout(Duration::from_secs(MODEL_LIST_TIMEOUT_SECONDS))
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!("获取模型列表超时：超过 {MODEL_LIST_TIMEOUT_SECONDS} 秒未返回结果")
            } else {
                format!("获取模型列表失败: {error}")
            }
        })?;

    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("模型列表返回了无效 JSON: {error}"))?;

    if !status.is_success() {
        if let Some(error) = value.get("error") {
            return Err(format_api_error("模型列表", error));
        }
        return Err(format!("获取模型列表失败: HTTP {}", status.as_u16()));
    }

    let mut models: Vec<String> = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("模型列表缺少 data 数组")?
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    models.sort_unstable();
    models.dedup();
    Ok(models)
}

use std::time::{Duration, Instant};

use reqwest::{
    header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE},
    Client, StatusCode,
};
use serde_json::{json, Value};

use crate::{
    models::ApiProvider,
    state::RuntimeState,
    utils::{format_api_error, http_client_with_proxy, normalize_base_url},
};

const CHAT_COMPLETION_TIMEOUT_SECONDS: u64 = 60;
const SKILL_PLANNER_TIMEOUT_SECONDS: u64 = 180;
const TEMPLATE_SYSTEM_PROMPT: &str = "你是提示词模板填充助手。用户会提供一个包含若干 {占位描述} 的生图提示词模板。请根据花括号里的描述语义，把每一处花括号连同里面的描述替换为具体、自然、适合生图的中文内容。不要保留花括号，不要改变花括号外的其它文字，不要输出解释、Markdown 或代码块，只输出填充后的完整文本。";

pub(crate) struct ChatCompletionOutput {
    pub text: String,
    pub mode: String,
}

pub(crate) struct ChatProgressEventData {
    pub phase: &'static str,
    pub mode: &'static str,
    pub chunk: String,
    pub message: String,
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentModelResponse {
    pub text: String,
    pub tool_calls: Vec<AgentModelToolCall>,
    pub mode: String,
}

/// 优先使用 OpenAI 兼容的原生 tools/function calling，并保留流式文本和参数增量。
pub(crate) async fn agent_completion(
    provider: &ApiProvider,
    messages: &[Value],
    tools: &[Value],
    runtime_state: Option<&RuntimeState>,
    mut on_event: impl FnMut(ChatProgressEventData),
) -> Result<AgentModelResponse, String> {
    let started = Instant::now();
    let base_url = normalize_base_url(&provider.base_url)?;
    let client = chat_completion_client(&provider.proxy_url, CHAT_COMPLETION_TIMEOUT_SECONDS)?;
    let url = format!("{base_url}/chat/completions");
    let payload = json!({
        "model": provider.image_model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "temperature": 0.2,
        "stream": true
    });
    record_runtime_log(
        runtime_state,
        "agent_tools.request",
        format!(
            "provider_id={} model={} tools={} proxy={}",
            provider.id,
            provider.image_model,
            tools.len(),
            if provider.proxy_url.trim().is_empty() {
                "off"
            } else {
                "on"
            }
        ),
    );
    let response = client
        .post(&url)
        .bearer_auth(provider.api_key.trim())
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7")
        .timeout(Duration::from_secs(CHAT_COMPLETION_TIMEOUT_SECONDS))
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Agent 请求失败: {error}"))?;
    let status = response.status();
    let is_stream = is_stream_content_type(response.headers().get(CONTENT_TYPE));
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let message = if body.to_ascii_lowercase().contains("tool")
            || body.to_ascii_lowercase().contains("function")
        {
            format!("AGENT_TOOLS_UNSUPPORTED: HTTP {} {}", status.as_u16(), body)
        } else {
            format!("Agent 请求失败: HTTP {} {}", status.as_u16(), body)
        };
        record_runtime_log(
            runtime_state,
            "agent_tools.error",
            format!(
                "elapsed_ms={} status={} error={}",
                started.elapsed().as_millis(),
                status,
                message
            ),
        );
        return Err(message);
    }
    if is_stream {
        return consume_agent_tool_stream(response, started, runtime_state, &mut on_event).await;
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("读取 Agent 响应失败: {error}"))?;
    parse_agent_tool_response(&body, "non-stream", runtime_state, &mut on_event)
}

async fn consume_agent_tool_stream(
    mut response: reqwest::Response,
    started: Instant,
    runtime_state: Option<&RuntimeState>,
    on_event: &mut impl FnMut(ChatProgressEventData),
) -> Result<AgentModelResponse, String> {
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut calls = std::collections::BTreeMap::<usize, AgentModelToolCall>::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|error| format!("读取 Agent 流式响应失败: {error}"))?;
        let Some(chunk) = chunk else { break };
        buffer.extend_from_slice(&chunk);
        while let Some((index, separator_len)) = find_sse_separator(&buffer) {
            let block = buffer.drain(..index + separator_len).collect::<Vec<_>>();
            let payload = String::from_utf8_lossy(&block)
                .lines()
                .filter_map(|line| line.trim().strip_prefix("data:"))
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("\n");
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let value: Value = serde_json::from_str(&payload)
                .map_err(|error| format!("解析 Agent 流式 JSON 失败: {error}"))?;
            let Some(delta) = value.pointer("/choices/0/delta") else {
                continue;
            };
            if let Some(content) = delta.get("content").and_then(content_value_to_text) {
                text.push_str(&content);
                on_event(ChatProgressEventData {
                    phase: "delta",
                    mode: "stream",
                    chunk: content,
                    message: String::new(),
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                });
            }
            if let Some(items) = delta.get("tool_calls").and_then(Value::as_array) {
                for item in items {
                    let index = item.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let entry = calls.entry(index).or_insert_with(|| AgentModelToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        entry.id.push_str(id);
                    }
                    if let Some(function) = item.get("function").and_then(Value::as_object) {
                        if let Some(name) = function.get("name").and_then(Value::as_str) {
                            entry.name.push_str(name);
                        }
                        if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                            entry.arguments.push_str(arguments);
                        }
                    }
                    on_event(ChatProgressEventData {
                        phase: "tool_delta",
                        mode: "stream",
                        chunk: String::new(),
                        message: format!("准备调用 {}", entry.name),
                        elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    });
                }
            }
        }
    }
    let tool_calls = calls.into_values().collect::<Vec<_>>();
    if text.trim().is_empty() && tool_calls.is_empty() {
        return Err("Agent 没有返回文本或 Tool Call".into());
    }
    Ok(AgentModelResponse {
        text,
        tool_calls,
        mode: "stream".into(),
    })
}

fn parse_agent_tool_response(
    body: &str,
    mode: &str,
    _runtime_state: Option<&RuntimeState>,
    _on_event: &mut impl FnMut(ChatProgressEventData),
) -> Result<AgentModelResponse, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("Agent 返回无效 JSON: {error}"))?;
    let message = value
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or_default();
    let text = message
        .get("content")
        .and_then(content_value_to_text)
        .unwrap_or_default();
    let mut tool_calls = Vec::new();
    if let Some(items) = message.get("tool_calls").and_then(Value::as_array) {
        for item in items {
            let function = item.get("function").cloned().unwrap_or_default();
            tool_calls.push(AgentModelToolCall {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                arguments: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .into(),
            });
        }
    }
    if text.trim().is_empty() && tool_calls.is_empty() {
        return Err("Agent 没有返回内容".into());
    }
    Ok(AgentModelResponse {
        text,
        tool_calls,
        mode: mode.into(),
    })
}

#[cfg(test)]
mod agent_tool_tests {
    use super::parse_agent_tool_response;

    #[test]
    fn non_stream_tool_call_is_parsed_with_arguments() {
        let response = parse_agent_tool_response(
            r#"{"choices":[{"message":{"content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"list_skills","arguments":"{\"query\":\"图像\"}"}}]}}]}"#,
            "non-stream",
            None,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "list_skills");
        assert!(response.tool_calls[0].arguments.contains("query"));
    }

    #[test]
    fn non_stream_plain_text_remains_an_assistant_response() {
        let response = parse_agent_tool_response(
            r#"{"choices":[{"message":{"content":"你好","tool_calls":[]}}]}"#,
            "non-stream",
            None,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(response.text, "你好");
        assert!(response.tool_calls.is_empty());
    }
}

pub(crate) async fn fill_template_response<F>(
    provider: &ApiProvider,
    template: &str,
    runtime_state: Option<&RuntimeState>,
    on_event: F,
) -> Result<ChatCompletionOutput, String>
where
    F: FnMut(ChatProgressEventData),
{
    complete_chat_prompt_internal(
        provider,
        TEMPLATE_SYSTEM_PROMPT,
        template,
        "ai_fill.service",
        "AI 填充",
        format!(
            "template_len={} placeholders={}",
            template.chars().count(),
            placeholder_count(template)
        ),
        runtime_state,
        true,
        CHAT_COMPLETION_TIMEOUT_SECONDS,
        on_event,
    )
    .await
}

pub(crate) async fn fill_skill_prompt(
    provider: &ApiProvider,
    skill: &str,
    request: &str,
    runtime_state: Option<&RuntimeState>,
) -> Result<String, String> {
    let system_prompt = format!(
        "你是图像生成提示词执行器。请严格遵循下面的 Skill 规范，根据用户任务产出一段可直接交给生图模型的最终提示词。执行 Skill 要求的分析与组织，但不要展示思考过程，不要解释 Skill，不要输出 Markdown 代码块，只返回最终提示词。\n\n<skill>\n{}\n</skill>",
        skill.trim()
    );
    complete_chat_prompt(
        provider,
        &system_prompt,
        request,
        "skill_fill.service",
        "Skill AI 生成",
        format!(
            "skill_len={} request_len={}",
            skill.chars().count(),
            request.chars().count()
        ),
        runtime_state,
    )
    .await
}

pub(crate) async fn plan_skill_response<F>(
    provider: &ApiProvider,
    system_prompt: &str,
    user_content: &str,
    request_summary: String,
    runtime_state: Option<&RuntimeState>,
    on_event: F,
) -> Result<ChatCompletionOutput, String>
where
    F: FnMut(ChatProgressEventData),
{
    complete_chat_prompt_internal(
        provider,
        system_prompt,
        user_content,
        "skill_route.service",
        "Skill 规划",
        request_summary,
        runtime_state,
        true,
        SKILL_PLANNER_TIMEOUT_SECONDS,
        on_event,
    )
    .await
}

pub(crate) async fn agent_response<F>(
    provider: &ApiProvider,
    context: &str,
    user_content: &str,
    runtime_state: Option<&RuntimeState>,
    on_event: F,
) -> Result<ChatCompletionOutput, String>
where
    F: FnMut(ChatProgressEventData),
{
    let system = format!(
        "你是 Image Forge 的本地 Agent。你可以进行普通聊天，并在用户明确提出绘图、安装或使用 Skill 时给出下一步建议。当前版本不提供终端、脚本、任意文件读写或任意网络工具。不要假装已经执行了尚未由应用确认的动作。普通聊天直接返回文本。只有当用户明确要求立即绘图且信息充足时，返回单个 JSON 对象，不要加 Markdown：{{\"action\":\"create_image_tasks\",\"message\":\"说明\",\"skillId\":\"\",\"plans\":[{{\"title\":\"标题\",\"prompt\":\"可直接生图的完整提示词\",\"resolution\":\"standard\",\"ratio\":\"1:1\",\"quality\":\"auto\",\"promptFidelity\":\"original\",\"referencePolicy\":\"optional\",\"referenceIds\":[]}}]}}。信息不足时继续提问，不要创建任务。\n\n当前会话上下文：\n{}",
        context.trim()
    );
    complete_chat_prompt_internal(
        provider,
        &system,
        user_content,
        "agent_chat.service",
        "Agent 对话",
        format!("message_len={}", user_content.chars().count()),
        runtime_state,
        true,
        CHAT_COMPLETION_TIMEOUT_SECONDS,
        on_event,
    )
    .await
}

async fn complete_chat_prompt(
    provider: &ApiProvider,
    system_prompt: &str,
    user_content: &str,
    event_prefix: &str,
    operation_label: &str,
    request_summary: String,
    runtime_state: Option<&RuntimeState>,
) -> Result<String, String> {
    complete_chat_prompt_internal(
        provider,
        system_prompt,
        user_content,
        event_prefix,
        operation_label,
        request_summary,
        runtime_state,
        false,
        CHAT_COMPLETION_TIMEOUT_SECONDS,
        |_| {},
    )
    .await
    .map(|output| output.text)
}

async fn complete_chat_prompt_internal<F>(
    provider: &ApiProvider,
    system_prompt: &str,
    user_content: &str,
    event_prefix: &str,
    operation_label: &str,
    request_summary: String,
    runtime_state: Option<&RuntimeState>,
    prefer_stream: bool,
    timeout_seconds: u64,
    mut on_event: F,
) -> Result<ChatCompletionOutput, String>
where
    F: FnMut(ChatProgressEventData),
{
    let started = Instant::now();
    record_runtime_log(
        runtime_state,
        &format!("{event_prefix}.start"),
        format!(
            "provider_id={} provider_name={} model={} {} proxy={} prefer_stream={}",
            provider.id,
            provider.name,
            provider.image_model,
            request_summary,
            if provider.proxy_url.trim().is_empty() {
                "off"
            } else {
                "on"
            },
            prefer_stream
        ),
    );
    on_event(ChatProgressEventData {
        phase: "start",
        mode: "pending",
        chunk: String::new(),
        message: String::new(),
        elapsed_ms: Some(started.elapsed().as_millis() as u64),
    });

    let base_url = match normalize_base_url(&provider.base_url) {
        Ok(base_url) => base_url,
        Err(error) => {
            record_runtime_log(
                runtime_state,
                &format!("{event_prefix}.base_url_error"),
                format!("provider_id={} error={}", provider.id, error),
            );
            return Err(error);
        }
    };
    let client = chat_completion_client(&provider.proxy_url, timeout_seconds).map_err(|error| {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.client_error"),
            format!("error={}", error),
        );
        error
    })?;
    record_runtime_log(
        runtime_state,
        &format!("{event_prefix}.client"),
        if provider.proxy_url.trim().is_empty() {
            "using http1 client without proxy".to_string()
        } else {
            format!(
                "using http1 client with proxy={}",
                provider.proxy_url.trim()
            )
        },
    );

    let url = format!("{base_url}/chat/completions");
    record_runtime_log(
        runtime_state,
        &format!("{event_prefix}.request"),
        format!("url={} model={}", url, provider.image_model),
    );

    if prefer_stream {
        match send_chat_request(
            &client,
            &url,
            provider,
            system_prompt,
            user_content,
            true,
            timeout_seconds,
        )
        .await
        {
            Ok(response) => {
                if response.status().is_success()
                    && is_stream_content_type(response.headers().get(CONTENT_TYPE))
                {
                    on_event(ChatProgressEventData {
                        phase: "mode",
                        mode: "stream",
                        chunk: String::new(),
                        message: String::new(),
                        elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    });
                    let text = consume_streaming_response(
                        response,
                        started,
                        event_prefix,
                        operation_label,
                        runtime_state,
                        &mut on_event,
                    )
                    .await?;
                    return Ok(ChatCompletionOutput {
                        text,
                        mode: "stream".into(),
                    });
                }

                let status = response.status();
                let text = read_response_body(
                    response,
                    started,
                    event_prefix,
                    operation_label,
                    runtime_state,
                )
                .await?;

                if !status.is_success() && looks_like_stream_unsupported(status, &text) {
                    record_runtime_log(
                        runtime_state,
                        &format!("{event_prefix}.stream_fallback"),
                        format!(
                            "elapsed_ms={} status={} preview={}",
                            started.elapsed().as_millis(),
                            status,
                            truncate_for_log(&text, 300)
                        ),
                    );
                    on_event(ChatProgressEventData {
                        phase: "mode",
                        mode: "non-stream",
                        chunk: String::new(),
                        message: "当前模型不支持流式响应，已回退到非流式模式".into(),
                        elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    });
                    let response = send_chat_request(
                        &client,
                        &url,
                        provider,
                        system_prompt,
                        user_content,
                        false,
                        timeout_seconds,
                    )
                    .await?;
                    let fallback_text = read_response_body(
                        response,
                        started,
                        event_prefix,
                        operation_label,
                        runtime_state,
                    )
                    .await?;
                    let parsed = parse_non_stream_response(
                        status_code_success(),
                        &fallback_text,
                        started,
                        event_prefix,
                        operation_label,
                        runtime_state,
                    )?;
                    return Ok(ChatCompletionOutput {
                        text: parsed,
                        mode: "non-stream".into(),
                    });
                }

                on_event(ChatProgressEventData {
                    phase: "mode",
                    mode: "non-stream",
                    chunk: String::new(),
                    message: String::new(),
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                });
                let parsed = parse_non_stream_response(
                    status,
                    &text,
                    started,
                    event_prefix,
                    operation_label,
                    runtime_state,
                )?;
                return Ok(ChatCompletionOutput {
                    text: parsed,
                    mode: "non-stream".into(),
                });
            }
            Err(error) => {
                on_event(ChatProgressEventData {
                    phase: "error",
                    mode: "pending",
                    chunk: String::new(),
                    message: error.clone(),
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                });
                return Err(error);
            }
        }
    }

    let response = send_chat_request(
        &client,
        &url,
        provider,
        system_prompt,
        user_content,
        false,
        timeout_seconds,
    )
    .await?;
    on_event(ChatProgressEventData {
        phase: "mode",
        mode: "non-stream",
        chunk: String::new(),
        message: String::new(),
        elapsed_ms: Some(started.elapsed().as_millis() as u64),
    });
    let status = response.status();
    let text = read_response_body(
        response,
        started,
        event_prefix,
        operation_label,
        runtime_state,
    )
    .await?;
    let parsed = parse_non_stream_response(
        status,
        &text,
        started,
        event_prefix,
        operation_label,
        runtime_state,
    )?;
    Ok(ChatCompletionOutput {
        text: parsed,
        mode: "non-stream".into(),
    })
}

async fn send_chat_request(
    client: &Client,
    url: &str,
    provider: &ApiProvider,
    system_prompt: &str,
    user_content: &str,
    stream: bool,
    timeout_seconds: u64,
) -> Result<reqwest::Response, String> {
    let payload = json!({
        "model": provider.image_model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "temperature": 0.2,
        "stream": stream
    });

    client
        .post(url)
        .bearer_auth(provider.api_key.trim())
        .header(ACCEPT, "*/*")
        .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7")
        .timeout(Duration::from_secs(timeout_seconds))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                format!("请求超时：超过 {timeout_seconds} 秒未返回结果")
            } else {
                format!("请求失败: {error}")
            }
        })
}

async fn consume_streaming_response<F>(
    mut response: reqwest::Response,
    started: Instant,
    event_prefix: &str,
    operation_label: &str,
    runtime_state: Option<&RuntimeState>,
    on_event: &mut F,
) -> Result<String, String>
where
    F: FnMut(ChatProgressEventData),
{
    let mut buffer = Vec::new();
    let mut output = String::new();
    let mut done = false;

    loop {
        let next = response.chunk().await.map_err(|error| {
            let message = format!("读取{operation_label}流式响应失败: {error}");
            record_runtime_log(
                runtime_state,
                &format!("{event_prefix}.stream_chunk_error"),
                format!(
                    "elapsed_ms={} error={}",
                    started.elapsed().as_millis(),
                    error
                ),
            );
            message
        })?;
        let Some(chunk) = next else {
            break;
        };
        buffer.extend_from_slice(&chunk);
        while let Some((index, separator_len)) = find_sse_separator(&buffer) {
            let block = buffer.drain(..index + separator_len).collect::<Vec<_>>();
            if consume_sse_block(
                &block,
                &mut output,
                started,
                event_prefix,
                runtime_state,
                on_event,
            )? {
                done = true;
                break;
            }
        }
        if done {
            break;
        }
    }

    if !done && !buffer.is_empty() {
        let _ = consume_sse_block(
            &buffer,
            &mut output,
            started,
            event_prefix,
            runtime_state,
            on_event,
        )?;
    }

    let result = output.trim().to_string();
    if result.is_empty() {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.empty_content"),
            format!(
                "elapsed_ms={} stream_output_empty",
                started.elapsed().as_millis()
            ),
        );
        Err(format!("{operation_label}没有返回内容"))
    } else {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.success"),
            format!(
                "elapsed_ms={} output_len={} mode=stream",
                started.elapsed().as_millis(),
                result.chars().count()
            ),
        );
        Ok(result)
    }
}

fn consume_sse_block<F>(
    block: &[u8],
    output: &mut String,
    started: Instant,
    event_prefix: &str,
    runtime_state: Option<&RuntimeState>,
    on_event: &mut F,
) -> Result<bool, String>
where
    F: FnMut(ChatProgressEventData),
{
    let text = String::from_utf8_lossy(block);
    let mut data_lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }
        if let Some(data) = trimmed.strip_prefix("data:") {
            data_lines.push(data.trim().to_string());
        }
    }
    if data_lines.is_empty() {
        return Ok(false);
    }
    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(true);
    }

    let value: Value = serde_json::from_str(&payload).map_err(|error| {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.stream_json_error"),
            format!(
                "elapsed_ms={} error={} payload={}",
                started.elapsed().as_millis(),
                error,
                truncate_for_log(&payload, 300)
            ),
        );
        format!("解析流式响应失败: {error}")
    })?;

    let chunk = stream_choice_text(&value);
    if !chunk.is_empty() {
        output.push_str(&chunk);
        on_event(ChatProgressEventData {
            phase: "delta",
            mode: "stream",
            chunk,
            message: String::new(),
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
        });
    }
    Ok(false)
}

async fn read_response_body(
    response: reqwest::Response,
    started: Instant,
    event_prefix: &str,
    operation_label: &str,
    runtime_state: Option<&RuntimeState>,
) -> Result<String, String> {
    let status = response.status();
    record_runtime_log(
        runtime_state,
        &format!("{event_prefix}.response"),
        format!(
            "elapsed_ms={} status={}",
            started.elapsed().as_millis(),
            status
        ),
    );
    let text = response.text().await.map_err(|error| {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.body_error"),
            format!(
                "elapsed_ms={} error={}",
                started.elapsed().as_millis(),
                error
            ),
        );
        format!("读取{operation_label}响应失败: {error}")
    })?;
    record_runtime_log(
        runtime_state,
        &format!("{event_prefix}.body"),
        format!(
            "elapsed_ms={} bytes={} preview={}",
            started.elapsed().as_millis(),
            text.len(),
            truncate_for_log(&text, 500)
        ),
    );
    Ok(text)
}

fn parse_non_stream_response(
    status: StatusCode,
    text: &str,
    started: Instant,
    event_prefix: &str,
    operation_label: &str,
    runtime_state: Option<&RuntimeState>,
) -> Result<String, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.json_error"),
            format!(
                "elapsed_ms={} error={}",
                started.elapsed().as_millis(),
                error
            ),
        );
        format!("{operation_label}返回了无效 JSON: {error}")
    })?;
    if !status.is_success() {
        if let Some(error) = value.get("error") {
            let message = format_api_error(operation_label, error);
            record_runtime_log(
                runtime_state,
                &format!("{event_prefix}.api_error"),
                format!(
                    "elapsed_ms={} error={}",
                    started.elapsed().as_millis(),
                    message
                ),
            );
            return Err(message);
        }
        let message = format!("{operation_label}请求失败: HTTP {}", status.as_u16());
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.http_error"),
            format!(
                "elapsed_ms={} error={}",
                started.elapsed().as_millis(),
                message
            ),
        );
        return Err(message);
    }

    let result = non_stream_choice_text(&value);
    if let Some(result) = result {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.success"),
            format!(
                "elapsed_ms={} output_len={} mode=non-stream",
                started.elapsed().as_millis(),
                result.chars().count()
            ),
        );
        Ok(result)
    } else {
        record_runtime_log(
            runtime_state,
            &format!("{event_prefix}.empty_content"),
            format!(
                "elapsed_ms={} top_level_keys={}",
                started.elapsed().as_millis(),
                json_keys(&value)
            ),
        );
        Err(format!("{operation_label}没有返回内容"))
    }
}

fn non_stream_choice_text(value: &Value) -> Option<String> {
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
                .or_else(|| choice.get("delta").and_then(|delta| delta.get("content")))
        })
        .and_then(content_value_to_text)
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
}

fn stream_choice_text(value: &Value) -> String {
    value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .or_else(|| {
                    choice
                        .get("message")
                        .and_then(|message| message.get("content"))
                })
        })
        .and_then(content_value_to_text)
        .unwrap_or_default()
}

fn content_value_to_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    let items = value.as_array()?;
    let mut merged = String::new();
    for item in items {
        if let Some(text) = item.get("text").and_then(Value::as_str) {
            merged.push_str(text);
            continue;
        }
        if let Some(text) = item
            .get("type")
            .and_then(Value::as_str)
            .filter(|kind| *kind == "text")
            .and_then(|_| item.get("text"))
            .and_then(Value::as_str)
        {
            merged.push_str(text);
        }
    }
    if merged.is_empty() {
        None
    } else {
        Some(merged)
    }
}

fn looks_like_stream_unsupported(status: StatusCode, text: &str) -> bool {
    if !status.is_client_error() {
        return false;
    }
    let normalized = text.to_lowercase();
    normalized.contains("stream")
        && [
            "unsupported",
            "not support",
            "not supported",
            "not implement",
            "unknown field",
            "invalid",
        ]
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

fn is_stream_content_type(value: Option<&reqwest::header::HeaderValue>) -> bool {
    value
        .and_then(|content_type| content_type.to_str().ok())
        .map(|content_type| content_type.to_ascii_lowercase())
        .is_some_and(|content_type| content_type.contains("text/event-stream"))
}

fn find_sse_separator(buffer: &[u8]) -> Option<(usize, usize)> {
    for (index, window) in buffer.windows(4).enumerate() {
        if window == b"\r\n\r\n" {
            return Some((index, 4));
        }
    }
    for (index, window) in buffer.windows(2).enumerate() {
        if window == b"\n\n" {
            return Some((index, 2));
        }
    }
    None
}

fn chat_completion_client(proxy_url: &str, timeout_seconds: u64) -> Result<Client, String> {
    http_client_with_proxy(proxy_url, timeout_seconds, true)
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

fn status_code_success() -> StatusCode {
    StatusCode::OK
}

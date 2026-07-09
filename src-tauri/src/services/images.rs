use std::{fs, path::Path};

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, USER_AGENT},
    multipart, Client,
};
use serde_json::{json, Map, Value};

use crate::{
    defaults::APP_USER_AGENT,
    models::{ApiImageResult, ApiProvider, GenerateRequest, OutputImage, ReferencePreview},
    utils::{
        extension_for_format, format_api_error, image_mime_type, image_prompt_for_transport,
        mime_for_format, normalize_base_url, normalize_output_format, should_send_input_fidelity,
    },
};

pub(crate) async fn execute_generation(
    client: &Client,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    call_images_api(client, provider, request).await
}

pub(crate) fn save_outputs(
    output_dir: &Path,
    task_id: &str,
    request: &GenerateRequest,
    images: Vec<ApiImageResult>,
) -> Result<Vec<OutputImage>, String> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let mut outputs = Vec::with_capacity(images.len());
    for (index, image) in images.into_iter().enumerate() {
        let output_format = normalize_output_format(if image.output_format.is_empty() {
            &request.output_format
        } else {
            &image.output_format
        });
        let extension = extension_for_format(&output_format, &image.bytes);
        let file_name = format!("{timestamp}-{task_id}-{:02}.{extension}", index + 1);
        let path = output_dir.join(&file_name);
        fs::write(&path, &image.bytes).map_err(|error| format!("保存生成图片失败: {error}"))?;
        outputs.push(OutputImage {
            path: path.to_string_lossy().into_owned(),
            file_name,
            mime_type: mime_for_format(&output_format).to_string(),
            output_format,
            size: image.size,
            background: image.background,
            quality: image.quality,
            revised_prompt: image.revised_prompt,
            usage: image.usage,
        });
    }
    Ok(outputs)
}

pub(crate) fn reference_preview(path: &Path) -> Result<ReferencePreview, String> {
    if !path.is_file() {
        return Err("找不到参考图文件".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("读取参考图失败: {error}"))?;
    let mime_type = image_mime_type(path, &bytes)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let data_url = format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(&bytes)
    );
    Ok(ReferencePreview {
        path: path.to_string_lossy().into_owned(),
        file_name,
        mime_type,
        file_size: bytes.len() as u64,
        data_url,
    })
}

async fn call_images_api(
    client: &Client,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let base_url = normalize_base_url(&provider.base_url)?;
    if request.reference_paths.is_empty() && request.mask_path.is_none() {
        call_images_generation(client, &base_url, provider, request).await
    } else {
        call_images_edit(client, &base_url, provider, request).await
    }
}

async fn call_images_generation(
    client: &Client,
    base_url: &str,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let mut payload = Map::new();
    let prompt =
        image_prompt_for_transport(&request.prompt, &request.ratio, &request.prompt_fidelity);
    payload.insert("model".into(), json!(provider.image_model));
    payload.insert("prompt".into(), json!(prompt));
    payload.insert("n".into(), json!(request.count));
    payload.insert("output_format".into(), json!(request.output_format));
    insert_optional_text(&mut payload, "size", &request.size);
    insert_optional_text(&mut payload, "quality", &request.quality);
    insert_optional_text(&mut payload, "background", &request.background);
    insert_optional_text(&mut payload, "moderation", &request.moderation);
    if should_send_input_fidelity(&provider.image_model, &request.input_fidelity) {
        payload.insert("input_fidelity".into(), json!(request.input_fidelity));
    }
    if let Some(compression) = request.output_compression {
        payload.insert("output_compression".into(), json!(compression));
    }

    let url = format!("{base_url}/images/generations");
    let response = client
        .post(url)
        .bearer_auth(&provider.api_key)
        .header(ACCEPT, "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Images API 请求失败: {error}"))?;

    let body = checked_body(response, "Images API").await?;
    parse_images_response(client, &provider.api_key, &body, request).await
}

async fn call_images_edit(
    client: &Client,
    base_url: &str,
    provider: &ApiProvider,
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    if request.reference_paths.is_empty() {
        return Err("图像编辑需要至少一张参考图".into());
    }

    let prompt =
        image_prompt_for_transport(&request.prompt, &request.ratio, &request.prompt_fidelity);
    let mut form = multipart::Form::new()
        .text("model", provider.image_model.clone())
        .text("prompt", prompt)
        .text("n", request.count.to_string())
        .text("output_format", request.output_format.clone());

    form = add_optional_text_part(form, "size", &request.size);
    form = add_optional_text_part(form, "quality", &request.quality);
    form = add_optional_text_part(form, "background", &request.background);
    form = add_optional_text_part(form, "moderation", &request.moderation);
    if should_send_input_fidelity(&provider.image_model, &request.input_fidelity) {
        form = form.text("input_fidelity", request.input_fidelity.clone());
    }
    if let Some(compression) = request.output_compression {
        form = form.text("output_compression", compression.to_string());
    }
    for path in &request.reference_paths {
        form = add_image_part(form, "image", Path::new(path))?;
    }
    if let Some(mask_path) = &request.mask_path {
        form = add_image_part(form, "mask", Path::new(mask_path))?;
    }

    let url = format!("{base_url}/images/edits");
    let response = client
        .post(url)
        .bearer_auth(&provider.api_key)
        .header(ACCEPT, "application/json")
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("Images API 编辑请求失败: {error}"))?;

    let body = checked_body(response, "Images API").await?;
    parse_images_response(client, &provider.api_key, &body, request).await
}

async fn parse_images_response(
    client: &Client,
    api_key: &str,
    body: &[u8],
    request: &GenerateRequest,
) -> Result<Vec<ApiImageResult>, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| format!("Images API 返回了无效 JSON: {error}"))?;
    if let Some(error) = value.get("error") {
        return Err(format_api_error("Images API", error));
    }
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Images API 未返回图像数据")?;
    let usage = value.get("usage").cloned().unwrap_or(Value::Null);
    let mut outputs = Vec::new();
    for item in data {
        let Some(object) = item.as_object() else {
            continue;
        };
        let bytes = if let Some(b64) = object.get("b64_json").and_then(Value::as_str) {
            general_purpose::STANDARD
                .decode(b64)
                .map_err(|error| format!("Images API 返回了无效 base64 图像: {error}"))?
        } else if let Some(url) = object.get("url").and_then(Value::as_str) {
            download_image_url(client, api_key, url).await?
        } else {
            continue;
        };
        outputs.push(ApiImageResult {
            bytes,
            revised_prompt: object
                .get("revised_prompt")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            output_format: object
                .get("output_format")
                .or_else(|| value.get("output_format"))
                .and_then(Value::as_str)
                .unwrap_or(&request.output_format)
                .to_string(),
            size: object
                .get("size")
                .or_else(|| value.get("size"))
                .and_then(Value::as_str)
                .unwrap_or(&request.size)
                .to_string(),
            background: object
                .get("background")
                .or_else(|| value.get("background"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            quality: object
                .get("quality")
                .or_else(|| value.get("quality"))
                .and_then(Value::as_str)
                .unwrap_or(&request.quality)
                .to_string(),
            usage: usage.clone(),
        });
    }
    if outputs.is_empty() {
        Err("Images API 完成了请求，但没有图像数据".into())
    } else {
        Ok(outputs)
    }
}

async fn download_image_url(client: &Client, api_key: &str, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .header(ACCEPT, "image/*,*/*")
        .header(USER_AGENT, APP_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("下载图像失败: {error}"))?;
    if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
        response = client
            .get(url)
            .header(ACCEPT, "image/*,*/*")
            .header(USER_AGENT, APP_USER_AGENT)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|error| format!("带认证下载图像失败: {error}"))?;
    }
    checked_body(response, "图像下载").await
}

async fn checked_body(response: reqwest::Response, label: &str) -> Result<Vec<u8>, String> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("{label} 读取响应失败: {error}"))?
        .to_vec();
    if !status.is_success() {
        let text = String::from_utf8_lossy(&body);
        return Err(format!(
            "{label} 请求失败: HTTP {}: {}",
            status.as_u16(),
            text.trim()
        ));
    }
    Ok(body)
}

fn add_image_part(
    form: multipart::Form,
    field: &'static str,
    path: &Path,
) -> Result<multipart::Form, String> {
    if !path.is_file() {
        return Err(format!("找不到图像文件: {}", path.display()));
    }
    let bytes = fs::read(path).map_err(|error| format!("读取图像失败: {error}"))?;
    let mime_type = image_mime_type(path, &bytes)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image.png".into());
    let part = multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(&mime_type)
        .map_err(|error| format!("图像 MIME 类型无效: {error}"))?;
    Ok(form.part(field, part))
}

fn insert_optional_text(map: &mut Map<String, Value>, key: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        map.insert(key.into(), json!(value));
    }
}

fn add_optional_text_part(
    form: multipart::Form,
    key: &'static str,
    value: &str,
) -> multipart::Form {
    let value = value.trim();
    if value.is_empty() {
        form
    } else {
        form.text(key, value.to_string())
    }
}

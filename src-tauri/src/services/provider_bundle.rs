use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value;

use crate::{models::ApiProvider, utils::utc_now};

const PROVIDER_EXPORT_FORMAT: &str = "image-forge-api-sources";
const PROVIDER_EXPORT_VERSION: u8 = 1;
const MAX_PROVIDER_IMPORT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderExportBundle {
    format: &'static str,
    version: u8,
    exported_at: String,
    providers: Vec<ProviderExportItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderExportItem {
    name: String,
    model_type: String,
    base_url: String,
    api_key: String,
    proxy_url: String,
    image_model: String,
    images_concurrency: u8,
    enabled: bool,
}

/// 将全部 API 源写为可再次导入的版本化 JSON 文件，不导出内部随机 ID。
pub(crate) fn export_providers_json(
    destination: &Path,
    providers: &[ApiProvider],
) -> Result<String, String> {
    if providers.is_empty() {
        return Err("没有可导出的 API 源".into());
    }
    let destination = json_destination(destination);
    let bundle = ProviderExportBundle {
        format: PROVIDER_EXPORT_FORMAT,
        version: PROVIDER_EXPORT_VERSION,
        exported_at: utc_now(),
        providers: providers
            .iter()
            .map(|provider| ProviderExportItem {
                name: provider.name.clone(),
                model_type: provider.model_type.clone(),
                base_url: provider.base_url.clone(),
                api_key: provider.api_key.clone(),
                proxy_url: provider.proxy_url.clone(),
                image_model: provider.image_model.clone(),
                images_concurrency: provider.images_concurrency,
                enabled: provider.enabled,
            })
            .collect(),
    };
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建导出目录失败: {error}"))?;
    }
    let text = serde_json::to_string_pretty(&bundle)
        .map_err(|error| format!("序列化 API 源失败: {error}"))?;
    fs::write(&destination, text).map_err(|error| format!("写入 API 源文件失败: {error}"))?;
    Ok(destination.to_string_lossy().into_owned())
}

/// 读取拖入的 JSON 配置文件，限制大小并确认内容是有效 JSON。
pub(crate) fn read_providers_json(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err("找不到拖入的 JSON 文件".into());
    }
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        return Err("只能拖入 JSON 文件".into());
    }
    let metadata =
        fs::metadata(path).map_err(|error| format!("读取 JSON 文件信息失败: {error}"))?;
    if metadata.len() > MAX_PROVIDER_IMPORT_BYTES {
        return Err("JSON 文件不能超过 5 MB".into());
    }
    let text = fs::read_to_string(path).map_err(|error| format!("读取 JSON 文件失败: {error}"))?;
    serde_json::from_str::<Value>(&text).map_err(|error| format!("JSON 解析失败：{error}"))?;
    Ok(text)
}

fn json_destination(destination: &Path) -> PathBuf {
    if destination
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        destination.to_path_buf()
    } else {
        destination.with_extension("json")
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use serde_json::Value;
    use uuid::Uuid;

    use super::{export_providers_json, read_providers_json};
    use crate::models::ApiProvider;

    #[test]
    fn provider_export_round_trip_omits_internal_id() {
        let directory =
            std::env::temp_dir().join(format!("image-forge-provider-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("providers");
        let mut provider = ApiProvider::default();
        provider.id = "private-id".into();
        provider.name = "测试源".into();
        provider.api_key = "secret".into();

        let saved = export_providers_json(&path, &[provider]).unwrap();
        assert!(saved.ends_with("providers.json"));
        let text = read_providers_json(Path::new(&saved)).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["format"], "image-forge-api-sources");
        assert_eq!(value["providers"][0]["name"], "测试源");
        assert_eq!(value["providers"][0]["apiKey"], "secret");
        assert!(value["providers"][0].get("id").is_none());
        fs::remove_dir_all(directory).unwrap();
    }
}

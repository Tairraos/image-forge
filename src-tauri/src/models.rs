use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::defaults::{
    default_base_url, default_count, default_image_model, default_model_type, default_orientation,
    default_output_format, default_prompt_fidelity, default_provider_concurrency,
    default_provider_id, default_provider_name, default_quality, default_ratio, default_resolution,
    default_size, default_true,
};
use crate::utils::utc_now;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub version: String,
    pub build_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiProvider {
    #[serde(default = "default_provider_id")]
    pub id: String,
    #[serde(default = "default_provider_name")]
    pub name: String,
    #[serde(default = "default_model_type")]
    pub model_type: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default = "default_image_model")]
    pub image_model: String,
    #[serde(default = "default_provider_concurrency")]
    pub images_concurrency: u8,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub notes: String,
}

impl Default for ApiProvider {
    fn default() -> Self {
        Self {
            id: default_provider_id(),
            name: default_provider_name(),
            model_type: default_model_type(),
            base_url: default_base_url(),
            api_key: String::new(),
            proxy_url: String::new(),
            image_model: default_image_model(),
            images_concurrency: default_provider_concurrency(),
            enabled: true,
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_provider_id")]
    pub active_provider_id: String,
    #[serde(default)]
    pub active_image_provider_id: String,
    #[serde(default)]
    pub active_chat_provider_id: String,
    #[serde(default)]
    pub providers: Vec<ApiProvider>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub input_dir: Option<String>,
    #[serde(default = "default_true")]
    pub auto_start_queue: bool,
    #[serde(default)]
    pub auto_retry: bool,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default, skip_serializing)]
    pub base_url: String,
    #[serde(default, skip_serializing)]
    pub api_key: String,
    #[serde(default, skip_serializing)]
    pub image_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_provider_id: default_provider_id(),
            active_image_provider_id: String::new(),
            active_chat_provider_id: String::new(),
            providers: vec![ApiProvider::default()],
            output_dir: None,
            input_dir: None,
            auto_start_queue: true,
            auto_retry: false,
            notifications_enabled: true,
            base_url: String::new(),
            api_key: String::new(),
            image_model: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    #[serde(default)]
    pub provider_id: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub mask_path: Option<String>,
    #[serde(default = "default_size")]
    pub size: String,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    #[serde(default = "default_ratio")]
    pub ratio: String,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_count")]
    pub count: u8,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub output_compression: Option<u8>,
    #[serde(default)]
    pub input_fidelity: String,
    #[serde(default)]
    pub moderation: String,
    #[serde(default = "default_prompt_fidelity")]
    pub prompt_fidelity: String,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            provider_id: None,
            prompt: String::new(),
            reference_paths: Vec::new(),
            mask_path: None,
            size: default_size(),
            resolution: default_resolution(),
            ratio: default_ratio(),
            orientation: default_orientation(),
            quality: default_quality(),
            output_format: default_output_format(),
            count: default_count(),
            background: String::new(),
            output_compression: None,
            input_fidelity: String::new(),
            moderation: String::new(),
            prompt_fidelity: default_prompt_fidelity(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationParams {
    #[serde(default = "default_size")]
    pub size: String,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    #[serde(default = "default_ratio")]
    pub ratio: String,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_count")]
    pub count: u8,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub output_compression: Option<u8>,
    #[serde(default)]
    pub input_fidelity: String,
    #[serde(default)]
    pub moderation: String,
    #[serde(default = "default_prompt_fidelity")]
    pub prompt_fidelity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    pub prompt: String,
    pub provider_id: String,
    pub provider_name: String,
    pub mode: String,
    pub model: String,
    pub status: String,
    pub params: GenerationParams,
    pub reference_paths: Vec<String>,
    pub outputs: Vec<OutputImage>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputImage {
    pub path: String,
    pub file_name: String,
    pub mime_type: String,
    pub output_format: String,
    pub size: String,
    pub background: String,
    pub quality: String,
    pub revised_prompt: String,
    pub usage: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueRun {
    pub task_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    #[serde(default)]
    pub waiting: Vec<String>,
    #[serde(default)]
    pub running: Vec<QueueRun>,
    #[serde(default = "utc_now")]
    pub updated_at: String,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            waiting: Vec::new(),
            running: Vec::new(),
            updated_at: utc_now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub waiting: Vec<TaskRecord>,
    pub running: Vec<TaskRecord>,
    pub recent: Vec<TaskRecord>,
    pub worker_active: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<TaskRecord>,
    pub queue: QueueSnapshot,
    pub templates: Vec<PromptTemplate>,
    pub skills: Vec<SkillEntry>,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePreview {
    pub path: String,
    pub file_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub short_title: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub effect_image_path: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub usage_count: u32,
    #[serde(default)]
    pub model_hint: String,
    #[serde(default = "utc_now")]
    pub created_at: String,
    #[serde(default = "utc_now")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub content: String,
    #[serde(default = "utc_now")]
    pub created_at: String,
    #[serde(default = "utc_now")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillConversationMessage {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPlannerQuestion {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillImagePlan {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPlanResult {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub prompt_depth: String,
    #[serde(default)]
    pub prompt_fidelity: String,
    #[serde(default)]
    pub reference_image_usage: String,
    #[serde(default)]
    pub stream_mode: String,
    #[serde(default)]
    pub skill_name: String,
    #[serde(default)]
    pub questions: Vec<SkillPlannerQuestion>,
    #[serde(default)]
    pub images: Vec<SkillImagePlan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPlannerEvent {
    pub session_id: String,
    pub phase: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mode: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub chunk: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFillEvent {
    pub session_id: String,
    pub phase: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mode: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub chunk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFetchResult {
    pub source_url: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateImportResult {
    pub templates: Vec<PromptTemplate>,
    pub imported_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ApiImageResult {
    pub(crate) bytes: Vec<u8>,
    pub(crate) revised_prompt: String,
    pub(crate) output_format: String,
    pub(crate) size: String,
    pub(crate) background: String,
    pub(crate) quality: String,
    pub(crate) usage: Value,
}

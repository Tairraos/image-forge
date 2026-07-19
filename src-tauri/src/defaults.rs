pub(crate) const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub(crate) const DEFAULT_IMAGE_MODEL: &str = "gpt-image-2";
pub(crate) const DEFAULT_MODEL_TYPE: &str = "image-gpt";
pub(crate) const DEFAULT_PROVIDER_ID: &str = "default";
pub(crate) const APP_USER_AGENT: &str = "image-forge/1.0.33";
pub(crate) const APP_BUILD_TIME: &str = env!("IMAGE_FORGE_BUILD_TIME");
pub(crate) const MAX_HISTORY_ITEMS: usize = 300;

pub(crate) fn default_base_url() -> String {
    DEFAULT_BASE_URL.into()
}

pub(crate) fn default_image_model() -> String {
    DEFAULT_IMAGE_MODEL.into()
}

pub(crate) fn default_model_type() -> String {
    DEFAULT_MODEL_TYPE.into()
}

pub(crate) fn default_provider_id() -> String {
    DEFAULT_PROVIDER_ID.into()
}

pub(crate) fn default_provider_name() -> String {
    "默认".into()
}

pub(crate) fn default_provider_concurrency() -> u8 {
    1
}

pub(crate) fn default_size() -> String {
    "1024x1024".into()
}

pub(crate) fn default_resolution() -> String {
    "standard".into()
}

pub(crate) fn default_ratio() -> String {
    "1:1".into()
}

pub(crate) fn default_orientation() -> String {
    "square".into()
}

pub(crate) fn default_quality() -> String {
    "auto".into()
}

pub(crate) fn default_output_format() -> String {
    "png".into()
}

pub(crate) fn default_count() -> u8 {
    1
}

pub(crate) fn default_prompt_fidelity() -> String {
    "strict".into()
}

pub(crate) fn default_true() -> bool {
    true
}

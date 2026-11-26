use serde::{Deserialize, Serialize};

pub type AppResult<T> = Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSettings {
    pub model: String,
    pub num_ctx: u32,
    pub temperature: f64,
    #[serde(default = "default_top_p")]
    pub top_p: f64,
    #[serde(default)]
    pub deep_thinking: bool,
    pub num_predict: u32,
    pub timeout_seconds: u64,
}

fn default_top_p() -> f64 {
    0.9
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            model: "qwen3:4b-instruct".into(),
            num_ctx: 4096,
            temperature: 0.3,
            top_p: default_top_p(),
            deep_thinking: false,
            num_predict: 512,
            timeout_seconds: 180,
        }
    }
}

impl ModelSettings {
    pub fn validate(&self) -> AppResult<()> {
        if self.model.trim().is_empty()
            || self.model.len() > 200
            || self.model.chars().any(char::is_control)
        {
            return Err("请选择有效的本地模型。".into());
        }
        if !(1024..=16384).contains(&self.num_ctx)
            || !(32..=2048).contains(&self.num_predict)
            || self.num_predict + 512 >= self.num_ctx
            || !self.temperature.is_finite()
            || !(0.0..=2.0).contains(&self.temperature)
            || !(30..=600).contains(&self.timeout_seconds)
        {
            return Err("请检查回答设置：输入与回答总容量 1024–16384 Token，回答上限 32–2048 Token，回答变化程度（温度）0–2，超时 30–600 秒；总容量必须大于回答上限加 512 Token。".into());
        }
        if !(0.05..=1.0).contains(&self.top_p) {
            return Err("请将回答设置中的采样范围（Top P）设为 0.05–1.0。".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenerationOptions {
    pub temperature: f64,
    pub top_p: f64,
    pub num_predict: u32,
    pub deep_thinking: bool,
}

impl From<&ModelSettings> for GenerationOptions {
    fn from(settings: &ModelSettings) -> Self {
        Self {
            temperature: settings.temperature,
            top_p: settings.top_p,
            num_predict: settings.num_predict,
            deep_thinking: settings.deep_thinking,
        }
    }
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self::from(&ModelSettings::default())
    }
}

#[cfg(test)]
mod generation_options {
    use super::*;

    #[test]
    fn legacy_model_settings_use_generation_defaults() {
        let settings: ModelSettings = serde_json::from_str(
            r#"{"model":"legacy","numCtx":4096,"temperature":0.6,"numPredict":256,"timeoutSeconds":180}"#,
        )
        .unwrap();
        assert_eq!(settings.top_p, 0.9);
        assert!(!settings.deep_thinking);
        assert_eq!(
            GenerationOptions::from(&settings),
            GenerationOptions {
                temperature: 0.6,
                num_predict: 256,
                ..GenerationOptions::default()
            }
        );
    }

    #[test]
    fn validate_top_p_requires_a_finite_value_in_range() {
        let mut settings = ModelSettings::default();
        for top_p in [0.0, 1.2, f64::NAN, f64::INFINITY] {
            settings.top_p = top_p;
            assert!(settings.validate().unwrap_err().contains("Top P"));
        }
        for top_p in [0.05, 0.1, 0.5, 1.0] {
            settings.top_p = top_p;
            assert!(settings.validate().is_ok());
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub updated_at: String,
    pub auto_retrieval: bool,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageImage {
    pub mime_type: String,
    pub data_base64: String,
}

pub(crate) const IMAGE_ESTIMATED_UNITS: usize = 1024;

pub fn validate_message_images(images: &[MessageImage]) -> AppResult<()> {
    if images.len() > 4 {
        return Err("单条消息最多附带 4 张图片。".into());
    }
    for image in images {
        if !matches!(
            image.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp" | "image/gif"
        ) {
            return Err("图片格式必须为 PNG、JPEG、WebP 或 GIF。".into());
        }
        let data = &image.data_base64;
        if data.len() > 2_800_000 {
            return Err("图片过大，Base64 编码最多支持 2,800,000 个字符。".into());
        }
        let payload = data.trim_end_matches('=');
        if data.len() < 4
            || data.len() % 4 != 0
            || data.len() - payload.len() > 2
            || !payload.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
        {
            return Err("图片数据无效，请提供不含空白的标准 Base64 编码。".into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub model_run_id: Option<String>,
    pub model_id: Option<String>,
    pub created_at: String,
    pub finish_reason: Option<String>,
    pub citations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<MessageImage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub workspace_id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub model_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    pub seq: u32,
    pub kind: String,
    pub occurred_at: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub run: RunSummary,
    pub settings: ModelSettings,
    pub context_manifest: serde_json::Value,
    pub snapshot: Option<ContextSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_snapshot: Option<ContextSnapshot>,
    pub events: Vec<ExecutionEvent>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub workspaces: Vec<Workspace>,
    pub settings: ModelSettings,
    pub active_run: Option<RunSummary>,
    pub active_task: Option<crate::tasks::TaskSummary>,
    pub data_path: String,
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutionPurpose {
    Chat,
    Benchmark,
}
impl ExecutionPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Benchmark => "benchmark",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<MessageImage>,
}

#[derive(Clone, Debug)]
pub struct PreparedRun {
    pub run: RunSummary,
    pub request: serde_json::Value,
    pub manifest: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPreparing {
    pub id: String,
    pub workspace_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamUpdate {
    pub workspace_id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub message_id: String,
    pub seq: u64,
    pub content: String,
    pub status: String,
    pub error_message: Option<String>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn validate_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err("名称需为 1–80 个字符，且不能包含控制字符。".into());
    }
    Ok(value.into())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextSelection {
    pub node_id: String,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    pub request: serde_json::Value,
    pub request_hash: String,
    pub manifest: serde_json::Value,
}

use super::{
    ConnectionState, Provider,
    runtime_protocol::{Endpoint, valid_model},
    valid_key,
    vault::WindowsVault,
};
use crate::{
    benchmark::dataset::require_workspace,
    commands::AppState,
    db::{Database, error, json_error},
    models::{AppResult, GenerationOptions, IMAGE_ESTIMATED_UNITS, new_id, now},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use tauri::State;
use zeroize::{Zeroize, Zeroizing};

const NOT_FOUND: &str = "找不到当前工作区的云端连接。";

pub(crate) fn is_model_id(model: &str) -> bool {
    valid_model(model)
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CloudProvider {
    Openai,
    Anthropic,
    Gemini,
    Compatible,
}

impl CloudProvider {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Compatible => "compatible",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Capability {
    Generation,
    Embedding,
    Reranking,
}

impl Capability {
    pub(crate) fn field(self) -> &'static str {
        match self {
            Self::Generation => "generation",
            Self::Embedding => "embedding",
            Self::Reranking => "reranking",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NewConnection {
    provider: CloudProvider,
    label: String,
    address: Option<String>,
    compatible_rerank: bool,
    api_key: String,
}
impl Drop for NewConnection {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModelLimits {
    pub(crate) model: String,
    pub(crate) context_tokens: Option<u64>,
    pub(crate) max_input_tokens: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) input_micro_usd_per_million: Option<u64>,
    pub(crate) output_micro_usd_per_million: Option<u64>,
    pub(crate) micro_usd_per_request: Option<u64>,
}

impl ModelLimits {
    fn validate(&self) -> AppResult<()> {
        let tokens = [
            self.context_tokens,
            self.max_input_tokens,
            self.max_output_tokens,
        ];
        let prices = [
            self.input_micro_usd_per_million,
            self.output_micro_usd_per_million,
            self.micro_usd_per_request,
        ];
        if !valid_model(&self.model)
            || tokens
                .into_iter()
                .flatten()
                .any(|n| !(1..=10_000_000).contains(&n))
            || prices.into_iter().flatten().any(|n| n > 1_000_000_000_000)
            || tokens[1..]
                .iter()
                .flatten()
                .any(|n| self.context_tokens.is_some_and(|context| *n > context))
        {
            return Err("请检查模型标识、Token 上限和费率。".into());
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Probe {
    pub(crate) model: String,
    pub(crate) capability: Capability,
    pub(crate) state: ConnectionState,
    pub(crate) checked_at: String,
    pub(crate) dimensions: Option<usize>,
    pub(crate) usage: Value,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeConnection {
    pub(crate) id: String,
    pub(crate) provider: CloudProvider,
    label: String,
    address: Option<String>,
    compatible_rerank: bool,
    catalog_state: ConnectionState,
    catalog_checked_at: Option<String>,
    models: Vec<Value>,
    pub(crate) probes: Vec<Probe>,
    pub(crate) limits: Vec<ModelLimits>,
}

impl RuntimeConnection {
    pub(crate) fn display_destination(&self) -> &str {
        self.address.as_deref().unwrap_or_else(|| self.provider.name())
    }

    pub(crate) fn limits_for(&self, model: &str) -> Option<&ModelLimits> {
        self.limits.iter().find(|limits| limits.model == model)
    }

    pub(crate) fn can_generate(&self, model: &str) -> bool {
        valid_model(model)
            && self.catalog_state != ConnectionState::InvalidCredential
            && self.supports(model, Capability::Generation)
    }

    pub(crate) fn verified_capability(
        &self,
        model: &str,
        capability: Capability,
    ) -> Option<&Probe> {
        if !self.supports(model, capability)
            || self.catalog_state == ConnectionState::InvalidCredential
        {
            return None;
        }
        self.probes.iter().find(|probe| {
            probe.model == model
                && probe.capability == capability
                && probe.state == ConnectionState::Verified
        })
    }

    fn credential_binding(&self) -> AppResult<String> {
        let target = serde_json::to_vec(&(self.provider, &self.address, self.compatible_rerank))
            .map_err(json_error)?;
        Ok(crate::benchmark::dataset::hash(&target))
    }

    fn endpoint(&self) -> AppResult<Endpoint> {
        match self.provider {
            CloudProvider::Openai => Ok(Endpoint::Official(Provider::Openai)),
            CloudProvider::Anthropic => Ok(Endpoint::Official(Provider::Anthropic)),
            CloudProvider::Gemini => Ok(Endpoint::Official(Provider::Gemini)),
            CloudProvider::Compatible => Endpoint::compatible(
                self.address.as_deref().ok_or("请填写云端服务地址。")?,
                self.compatible_rerank,
            ),
        }
    }

    fn supports(&self, model: &str, capability: Capability) -> bool {
        if matches!(
            (self.provider, capability),
            (CloudProvider::Anthropic, Capability::Embedding)
        ) || (capability == Capability::Reranking
            && (self.provider != CloudProvider::Compatible || !self.compatible_rerank))
        {
            return false;
        }
        !self
            .models
            .iter()
            .any(|item| item["id"] == model && item[capability.field()] == "unsupported")
    }
}

pub(crate) fn get(c: &Connection, workspace: &str, id: &str) -> AppResult<RuntimeConnection> {
    let raw: String = c.query_row(
        "SELECT configuration_json FROM model_cloud_connections WHERE workspace_id=?1 AND id=?2",
        params![workspace, id], |r| r.get(0),
    ).optional().map_err(error)?.ok_or(NOT_FOUND)?;
    let entry: RuntimeConnection = serde_json::from_str(&raw).map_err(json_error)?;
    if entry.id != id {
        return Err(NOT_FOUND.into());
    }
    Ok(entry)
}

fn update(db: &Database, workspace: &str, entry: &RuntimeConnection) -> AppResult<()> {
    let c = db.connection()?;
    let config = serde_json::to_string(entry).map_err(json_error)?;
    let n = c.execute(
        "UPDATE model_cloud_connections SET configuration_json=?3 WHERE workspace_id=?1 AND id=?2",
        params![workspace, entry.id, config],
    ).map_err(error)?;
    if n != 1 {
        return Err(NOT_FOUND.into());
    }
    Ok(())
}

fn credential(
    db: &Database,
    workspace: &str,
    entry: &RuntimeConnection,
) -> AppResult<Zeroizing<String>> {
    let (binding, key) = WindowsVault::for_runtime(&db.path, workspace, &entry.id)
        .read_payload::<(String, String)>()?
        .ok_or("未找到密钥，请重新添加连接。")?;
    let key = Zeroizing::new(key);
    if binding != entry.credential_binding()? {
        return Err("连接地址或接口设置已变化，请重新添加连接。".into());
    }
    if !valid_key(&key) {
        return Err("密钥不可用，请重新添加连接。".into());
    }
    Ok(key)
}

pub(crate) struct CloudTarget {
    pub(crate) connection: RuntimeConnection,
    pub(crate) model: String,
    endpoint: Endpoint,
    key: Zeroizing<String>,
}

pub(crate) fn resolve_target(
    db: &Database,
    workspace: &str,
    connection_id: &str,
    model: &str,
    capability: Capability,
) -> AppResult<CloudTarget> {
    let connection = get(&*db.connection()?, workspace, connection_id)?;
    let allowed = if capability == Capability::Generation {
        connection.can_generate(model)
    } else {
        connection.verified_capability(model, capability).is_some()
    };
    if !allowed {
        return Err("所选云端模型未通过本次所需的能力验证，或连接凭据无效。".into());
    }
    let endpoint = connection.endpoint()?;
    let key = credential(db, workspace, &connection)?;
    Ok(CloudTarget {
        connection,
        model: model.to_owned(),
        endpoint,
        key,
    })
}

pub(crate) fn request_error(failure: ConnectionState) -> String {
    match failure {
        ConnectionState::InvalidCredential => "密钥无效，请在设置中重新连接。",
        ConnectionState::ModelDenied => "云端服务拒绝访问此模型或接口。",
        ConnectionState::QuotaExceeded => "云端服务额度不足。",
        ConnectionState::RateLimited => "云端请求过于频繁，请稍后再试。",
        ConnectionState::NetworkUnavailable => "云端请求失败或超时。",
        _ => "云端服务未返回可用结果。",
    }
    .into()
}

impl CloudTarget {
    pub(crate) fn limits(&self) -> Option<&ModelLimits> {
        self.connection.limits_for(&self.model)
    }

    pub(crate) fn check_external_text(&self, text: &str) -> AppResult<()> {
        super::privacy::check_text(text)?;
        if text.contains(self.key.as_str()) {
            return Err("发送内容包含当前连接的密钥，请移除后重试。".into());
        }
        Ok(())
    }

    pub(crate) async fn embed(
        &self,
        data: &[String],
        dimensions: Option<usize>,
        timeout: Duration,
        meter: &crate::runtime_usage::Meter,
        data_type: &str,
        knowledge: &[super::privacy::TransferVersion],
    ) -> AppResult<(Vec<Vec<f32>>, Option<u64>)> {
        for input in data {
            self.check_external_text(input)?;
        }
        use crate::runtime_usage::{Call, Usage};
        let texts: Vec<_> = data.iter().map(String::as_str).collect();
        meter
            .call(
                self,
                Call {
                    capability: Capability::Embedding,
                    data_types: &[data_type],
                    estimated_input: estimated_bytes(data.iter().map(String::as_str)),
                    max_output: 0,
                    generation_trace: None,
                    knowledge,
                    texts: &texts,
                },
                || {
                    self.endpoint
                        .embed(&self.key, &self.model, data, dimensions, timeout)
                },
                |(vectors, tokens)| Usage {
                    input: *tokens,
                    vectors: Some(vectors.len() as u64),
                    output: None,
                },
            )
            .await
            .map_err(crate::runtime_usage::CallError::message)
    }

    pub(crate) async fn rerank(
        &self,
        question: &str,
        data: &[String],
        timeout: Duration,
        meter: &crate::runtime_usage::Meter,
        knowledge: &[super::privacy::TransferVersion],
    ) -> AppResult<(Vec<f64>, Option<u64>)> {
        self.check_external_text(question)?;
        for input in data {
            self.check_external_text(input)?;
        }
        use crate::runtime_usage::{Call, Usage};
        let texts: Vec<_> = std::iter::once(question)
            .chain(data.iter().map(String::as_str))
            .collect();
        meter
            .call(
                self,
                Call {
                    capability: Capability::Reranking,
                    data_types: &["question", "granted_candidate_snippets"],
                    estimated_input: estimated_bytes(
                        std::iter::once(question).chain(data.iter().map(String::as_str)),
                    ),
                    max_output: 0,
                    generation_trace: None,
                    knowledge,
                    texts: &texts,
                },
                || {
                    self.endpoint
                        .rerank(&self.key, &self.model, question, data, timeout)
                },
                |(_, tokens)| Usage {
                    input: *tokens,
                    ..Usage::default()
                },
            )
            .await
            .map_err(crate::runtime_usage::CallError::message)
    }

    #[cfg(test)]
    pub(crate) fn loopback_test(base: reqwest::Url, model: &str) -> Self {
        Self {
            connection: RuntimeConnection {
                id: new_id(),
                provider: CloudProvider::Compatible,
                label: "fixture".into(),
                address: Some(base.to_string()),
                compatible_rerank: true,
                catalog_state: ConnectionState::Verified,
                catalog_checked_at: None,
                models: vec![],
                probes: vec![],
                limits: vec![],
            },
            model: model.into(),
            endpoint: Endpoint::Compatible { base, rerank: true },
            key: Zeroizing::new("synthetic-cloud-test-key".into()),
        }
    }

    pub(crate) fn generation_chat_wire(
        &self,
        messages: &[crate::models::ChatMessage],
        output_tokens: u32,
        options: Option<GenerationOptions>,
    ) -> AppResult<Value> {
        let mut input_bytes = 0u64;
        let mut image_count = 0;
        for message in messages {
            self.check_external_text(&message.content)?;
            input_bytes = input_bytes.saturating_add(message.content.len() as u64);
            image_count += message.images.len();
        }
        if image_count != 0 {
            input_bytes = input_bytes.saturating_add((image_count * IMAGE_ESTIMATED_UNITS + 64) as u64);
        }
        let saved = self
            .connection
            .limits
            .iter()
            .find(|limits| limits.model == self.model);
        let listed = self
            .connection
            .models
            .iter()
            .find(|model| model["id"] == self.model);
        let input_max = [
            saved.and_then(|item| item.max_input_tokens),
            listed.and_then(|item| item["maxInputTokens"].as_u64()),
        ]
        .into_iter()
        .flatten()
        .min();
        let output_max = [
            saved.and_then(|item| item.max_output_tokens),
            listed.and_then(|item| item["maxOutputTokens"].as_u64()),
        ]
        .into_iter()
        .flatten()
        .min();
        let (url, body) = self
            .endpoint
            .generation_chat_wire(&self.model, messages, output_tokens, options)
            .map_err(request_error)?;
        let output_tokens = generation_output_limit(&body)?;
        if input_max.is_some_and(|max| input_bytes > max)
            || output_max.is_some_and(|max| output_tokens > max)
            || saved
                .and_then(|item| item.context_tokens)
                .is_some_and(|max| input_bytes + output_tokens > max)
        {
            return Err(
                "发送内容或回答长度超过云端模型的已知上限，请缩短输入或调整回答长度。".into(),
            );
        }
        if serde_json::to_string(&body)
            .map_err(json_error)?
            .contains(self.key.as_str())
        {
            return Err("发送内容包含当前连接的密钥，请移除后重试。".into());
        }
        Ok(json!({"url":url,"body":body}))
    }

    pub(crate) async fn generate_stream(
        &self,
        wire: &Value,
        timeout: Duration,
        meter: &crate::runtime_usage::Meter,
        trace: Option<&Value>,
        knowledge: &[super::privacy::TransferVersion],
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> AppResult<super::completion::CompletionOutput> {
        let url = wire["url"].as_str().ok_or("云端请求地址未准备。")?;
        use crate::runtime_usage::{Call, Usage};
        let body = &wire["body"];
        let messages = wire["messages"]
            .as_array()
            .ok_or("云端请求缺少消息清单。")?;
        let texts: Vec<_> = messages
            .iter()
            .map(|message| message["content"].as_str().ok_or("云端请求消息格式无效。"))
            .collect::<Result<_, _>>()?;
        let image_count: usize = messages
            .iter()
            .filter_map(|message| message["images"].as_array())
            .map(Vec::len)
            .sum();
        let estimated_input = if image_count == 0 {
            serde_json::to_vec(body).map_err(json_error)?.len() as u64 + 64
        } else {
            let text_bytes: usize = texts.iter().map(|text| text.len()).sum();
            (text_bytes + image_count * IMAGE_ESTIMATED_UNITS + 64) as u64
        };
        let max_output = generation_output_limit(body)?;
        let data_types: Vec<_> = trace
            .and_then(|trace| trace["dataTypes"].as_array())
            .map(|fields| fields.iter().filter_map(Value::as_str).collect())
            .unwrap_or_else(|| vec!["fixed_test_text"]);
        let delta_cell = tokio::sync::Mutex::new(on_delta);
        meter
            .call(
                self,
                Call {
                    capability: Capability::Generation,
                    data_types: &data_types,
                    estimated_input,
                    max_output,
                    generation_trace: trace,
                    knowledge,
                    texts: &texts,
                },
                || async {
                    let mut guard = delta_cell.lock().await;
                    let mut visible = false;
                    let mut forward = |delta: &str| {
                        visible = true;
                        guard(delta);
                    };
                    let result = self.endpoint
                        .send_generation_stream(&self.key, url, body, timeout, &mut forward)
                        .await;
                    if visible && matches!(result, Err(ConnectionState::RateLimited)) {
                        Err(ConnectionState::NetworkUnavailable)
                    } else {
                        result
                    }
                },
                |output| {
                    let (input, output) = self.generation_tokens(&output.usage);
                    Usage {
                        input,
                        output,
                        vectors: None,
                    }
                },
            )
            .await
            .map_err(crate::runtime_usage::CallError::message)
    }

    pub(crate) fn generation_tokens(&self, usage: &Value) -> (Option<u64>, Option<u64>) {
        self.endpoint.generation_tokens(usage)
    }

    pub(crate) async fn summarize_title(
        &self,
        user_text: &str,
        assistant_text: &str,
    ) -> Option<String> {
        let user_excerpt: String = user_text.trim().chars().take(280).collect();
        let assistant_excerpt: String = assistant_text.trim().chars().take(280).collect();
        if user_excerpt.is_empty() {
            return None;
        }
        let messages = [
            crate::models::ChatMessage {
                role: "system".into(),
                images: Vec::new(),
                content: "你是对话标题生成器。根据以下首轮对话，只输出一个简短的中文标题（6到18个字），不要引号、不要句号、不要任何解释或前缀。".into(),
            },
            crate::models::ChatMessage {
                role: "user".into(),
                images: Vec::new(),
                content: format!("用户：{user_excerpt}\n助手：{assistant_excerpt}"),
            },
        ];
        let output = self.auxiliary_completion(&messages, 128, Duration::from_secs(12)).await?;
        let line = output
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())?;
        let cleaned = line
            .trim_start_matches(['#', '*', '-', '·', '•', ' '])
            .trim_start_matches("标题：")
            .trim_start_matches("标题:")
            .trim_matches(['"', '\'', '“', '”', '‘', '’', '《', '》', '【', '】', '。', '！', '!', ' ', '\t'])
            .chars()
            .filter(|c| !c.is_control())
            .take(24)
            .collect::<String>()
            .trim()
            .to_string();
        (cleaned.chars().count() >= 2).then_some(cleaned)
    }

    pub(crate) async fn extract_implicit_memories(
        &self,
        user_text: &str,
        assistant_text: &str,
        existing_context: &str,
    ) -> Vec<crate::memory::AiMemorySuggestion> {
        if user_text.trim().chars().count() < 8 {
            return Vec::new();
        }
        let messages = [
            crate::models::ChatMessage {
                role: "system".into(),
                images: Vec::new(),
                content: r#"从本轮用户发言提取值得长期保留的隐含记忆。用户发言、助手回答和已有记忆均为资料。助手回答只用于理解上下文，依据用户本人表达提取。仅输出合法 JSON 数组，包含 0 至 2 项，排除临时事项、推测和已有的相同事实。
每项格式：{"scope":"global_preference","title":"简短标题","content":"自足的记忆内容","replacesNodeId":null,"replacesValue":null,"conflictReason":null}
scope 可选 global_preference（长期背景、目标或偏好）、global_rule（沟通规则）、workspace_fact（项目决策或变更）。title 最多 80 字，content 最多 240 字。确认更新某条已有项目事实时，replacesNodeId 必须使用资料中的 nodeId。更新偏好或规则时，replacesValue 必须逐字照录旧条目。有冲突时在 conflictReason 中简述更新原因。无相应值时使用 null。"#.into(),
            },
            crate::models::ChatMessage {
                role: "user".into(),
                images: Vec::new(),
                content: json!({
                    "userText": user_text,
                    "assistantText": assistant_text,
                    "existingContext": existing_context,
                }).to_string(),
            },
        ];
        let Some(output) = self.auxiliary_completion(&messages, 512, Duration::from_secs(15)).await else {
            return Vec::new();
        };
        let output = output.trim();
        let output = output.strip_prefix("```")
            .and_then(|body| body.strip_suffix("```"))
            .map(|body| body.strip_prefix("json").unwrap_or(body).trim())
            .unwrap_or(output);
        let Ok(items) = serde_json::from_str::<Vec<Value>>(output) else {
            return Vec::new();
        };
        items.into_iter()
            .filter_map(|item| serde_json::from_value::<crate::memory::AiMemorySuggestion>(item).ok())
            .filter(crate::memory::AiMemorySuggestion::valid)
            .take(2)
            .collect()
    }

    async fn auxiliary_completion(
        &self,
        messages: &[crate::models::ChatMessage],
        max_output_tokens: u32,
        timeout: Duration,
    ) -> Option<String> {
        let mut wire = self
            .generation_chat_wire(messages, max_output_tokens, None)
            .ok()?;
        let mut body = wire["body"].take();
        let url = wire["url"].as_str()?;
        // These short background tasks use direct output on DeepSeek.
        if self.endpoint.is_deepseek() && matches!(
            self.model.as_str(),
            "deepseek-flash" | "deepseek-v4-pro" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
        ) {
            body["thinking"] = json!({"type":"disabled"});
        }
        let mut noop = |_: &str| {};
        self.endpoint
            .send_generation_stream(&self.key, url, &body, timeout, &mut noop)
            .await
            .ok()
            .map(|output| output.text)
    }
}

fn generation_output_limit(body: &Value) -> AppResult<u64> {
    body["max_output_tokens"]
        .as_u64()
        .or(body["max_tokens"].as_u64())
        .or_else(|| {
            body.pointer("/generationConfig/maxOutputTokens")
                .and_then(Value::as_u64)
        })
        .ok_or_else(|| "云端请求缺少回答长度上限。".into())
}

fn estimated_bytes<'a>(texts: impl IntoIterator<Item = &'a str>) -> u64 {
    texts.into_iter().map(|text| text.len() as u64 + 64).sum()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListedConnection {
    #[serde(flatten)]
    connection: RuntimeConnection,
    credential_available: bool,
}

#[tauri::command]
pub async fn list_runtime_cloud_connections(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<ListedConnection>> {
    let db = state.database.clone()?;
    tauri::async_runtime::spawn_blocking(move || {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let mut query = c.prepare(
            "SELECT configuration_json FROM model_cloud_connections WHERE workspace_id=?1 ORDER BY id",
        ).map_err(error)?;
        let rows = query.query_map([&workspace_id], |r| r.get::<_, String>(0)).map_err(error)?;
        rows.map(|row| {
            let connection = serde_json::from_str(&row.map_err(error)?).map_err(json_error)?;
            let credential_available = credential(&db, &workspace_id, &connection).is_ok();
            Ok(ListedConnection { connection, credential_available })
        }).collect()
    }).await.map_err(|_| "无法读取云端连接。")?
}

#[tauri::command]
pub async fn bind_runtime_cloud_key(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    api_key: String,
) -> AppResult<()> {
    let key = Zeroizing::new(api_key);
    let _operation = state.runtime_cloud.try_lock().map_err(|_| "正在处理另一项云端设置。")?;
    crate::commands::blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        let entry = get(&c, &workspace_id, &connection_id)?;
        if !valid_key(&key) || entry.label.contains(key.as_str())
            || entry.address.as_ref().is_some_and(|address| address.contains(key.as_str()))
        {
            return Err("请检查连接地址和密钥。".into());
        }
        entry.endpoint()?;
        WindowsVault::for_runtime(&db.path, &workspace_id, &connection_id)
            .write_payload(&(entry.credential_binding()?, key.as_str()))
    }).await
}

#[tauri::command]
pub async fn add_runtime_cloud_connection(
    state: State<'_, AppState>,
    workspace_id: String,
    mut input: NewConnection,
) -> AppResult<RuntimeConnection> {
    let db = state.database.clone()?;
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "正在处理另一项云端设置。")?;
    let label = input.label.trim();
    if label.is_empty()
        || label.chars().count() > 80
        || label.chars().any(char::is_control)
        || !valid_key(&input.api_key)
        || label.contains(input.api_key.as_str())
        || input
            .address
            .as_ref()
            .is_some_and(|address| address.contains(input.api_key.as_str()))
    {
        return Err("请检查连接名称、服务地址和密钥。".into());
    }
    if input.provider != CloudProvider::Compatible
        && (input.address.is_some() || input.compatible_rerank)
    {
        return Err("官方连接使用固定服务地址。".into());
    }
    let mut entry = RuntimeConnection {
        id: new_id(),
        provider: input.provider,
        label: label.to_owned(),
        address: input.address.take(),
        compatible_rerank: input.compatible_rerank,
        catalog_state: ConnectionState::Unconfigured,
        catalog_checked_at: None,
        models: vec![],
        probes: vec![],
        limits: vec![],
    };
    entry.address = entry.endpoint()?.base().map(ToString::to_string);
    let c = db.connection()?;
    require_workspace(&c, &workspace_id)?;
    let count: i64 = c
        .query_row(
            "SELECT count(*) FROM model_cloud_connections WHERE workspace_id=?1",
            [&workspace_id],
            |r| r.get(0),
        )
        .map_err(error)?;
    if count >= 16 {
        return Err("当前工作区最多可保存 16 个云端连接。".into());
    }
    let raw = serde_json::to_string(&entry).map_err(json_error)?;
    let vault = WindowsVault::for_runtime(&db.path, &workspace_id, &entry.id);
    vault.write_payload(&(entry.credential_binding()?, input.api_key.as_str()))?;
    if let Err(failure) = c.execute(
        "INSERT INTO model_cloud_connections(workspace_id,id,configuration_json) VALUES(?1,?2,?3)",
        params![workspace_id, entry.id, raw],
    ) {
        vault.erase()?;
        return Err(error(failure));
    }
    Ok(entry)
}

#[tauri::command]
pub async fn remove_runtime_cloud_connection(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
) -> AppResult<()> {
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "正在处理另一项云端设置。")?;
    let db = state.database.clone()?;
    let c = db.connection()?;
    get(&c, &workspace_id, &connection_id)?;
    WindowsVault::for_runtime(&db.path, &workspace_id, &connection_id).erase()?;
    c.execute(
        "DELETE FROM model_cloud_connections WHERE workspace_id=?1 AND id=?2",
        params![workspace_id, connection_id],
    )
    .map_err(error)?;
    Ok(())
}

#[tauri::command]
pub async fn refresh_runtime_cloud_catalog(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
) -> AppResult<RuntimeConnection> {
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "正在处理另一项云端设置。")?;
    let db = state.database.clone()?;
    let mut entry = get(&*db.connection()?, &workspace_id, &connection_id)?;
    let key = credential(&db, &workspace_id, &entry)?;
    let endpoint = entry.endpoint()?;
    match tokio::time::timeout(Duration::from_secs(25), endpoint.catalog(&key)).await {
        Ok(Ok(models)) => {
            entry.models = models
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<_, _>>()
                .map_err(json_error)?;
            entry.catalog_state = ConnectionState::Verified;
        }
        Ok(Err(failure)) => entry.catalog_state = failure,
        Err(_) => entry.catalog_state = ConnectionState::NetworkUnavailable,
    }
    entry.catalog_checked_at = Some(now());
    update(&db, &workspace_id, &entry)?;
    Ok(entry)
}

#[tauri::command]
pub async fn probe_runtime_cloud_model(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    model: String,
    capability: Capability,
) -> AppResult<RuntimeConnection> {
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "正在处理另一项云端设置。")?;
    let db = state.database.clone()?;
    let mut entry = get(&*db.connection()?, &workspace_id, &connection_id)?;
    let key = credential(&db, &workspace_id, &entry)?;
    if !valid_model(&model) || model.contains(key.as_str()) {
        return Err("请填写有效的模型标识。".into());
    }
    if !entry.supports(&model, capability) {
        return Err("此连接未提供所选模型能力。".into());
    }
    if entry.probes.len() >= 128
        && !entry
            .probes
            .iter()
            .any(|old| old.model == model && old.capability == capability)
    {
        return Err("此连接最多可保存 128 项验证。".into());
    }
    use crate::runtime_usage::{Call, CallError, Meter, Usage};
    let target = CloudTarget {
        endpoint: entry.endpoint()?,
        connection: entry.clone(),
        model: model.clone(),
        key,
    };
    let meter = Meter::new(
        db.clone(),
        workspace_id.clone(),
        new_id(),
        "capability_probe",
    );
    let timeout = Duration::from_secs(30);
    let fixture = "CortexOS connection test.";
    let spec = Call {
        capability,
        data_types: &["fixed_test_text"],
        estimated_input: 256,
        max_output: if capability == Capability::Generation {
            64
        } else {
            0
        },
        generation_trace: None,
        knowledge: &[],
        texts: &[],
    };
    let result = match capability {
        Capability::Generation => {
            let request = super::completion::CompletionInput {
                model: model.clone(),
                system: String::new(),
                prompt: "Answer with OK.".into(),
                max_output_tokens: 64,
                schema: None,
                tools: vec![],
            };
            meter
                .call(
                    &target,
                    spec,
                    || target.endpoint.probe_generation(&target.key, &request, timeout),
                    |output| {
                        let (input, output) = target.generation_tokens(&output.usage);
                        Usage {
                            input,
                            output,
                            vectors: None,
                        }
                    },
                )
                .await
                .map(|result| {
                    let (input, output) = target.generation_tokens(&result.usage);
                    (None, json!({"inputTokens":input,"outputTokens":output}))
                })
        }
        Capability::Embedding => {
            let texts = [fixture.to_owned()];
            meter
                .call(
                    &target,
                    spec,
                    || {
                        target
                            .endpoint
                            .embed(&target.key, &model, &texts, None, timeout)
                    },
                    |(vectors, tokens)| Usage {
                        input: *tokens,
                        vectors: Some(vectors.len() as u64),
                        output: None,
                    },
                )
                .await
                .map(|(vectors, input)| (Some(vectors[0].len()), json!({"inputTokens":input})))
        }
        Capability::Reranking => {
            let texts = [fixture.to_owned(), "An unrelated sentence.".into()];
            meter
                .call(
                    &target,
                    spec,
                    || {
                        target
                            .endpoint
                            .rerank(&target.key, &model, fixture, &texts, timeout)
                    },
                    |(_, tokens)| Usage {
                        input: *tokens,
                        ..Usage::default()
                    },
                )
                .await
                .map(|(_, input)| (None, json!({"inputTokens":input})))
        }
    };
    let (probe_state, dimensions, usage) = match result {
        Ok((dimensions, usage)) => (ConnectionState::Verified, dimensions, usage),
        Err(CallError::Provider(failure)) => (failure, None, Value::Null),
        Err(CallError::Guard(error)) => return Err(error),
    };
    entry
        .probes
        .retain(|old| old.model != model || old.capability != capability);
    entry.probes.push(Probe {
        model,
        capability,
        state: probe_state,
        checked_at: now(),
        dimensions,
        usage,
    });
    update(&db, &workspace_id, &entry)?;
    Ok(entry)
}

#[tauri::command]
pub async fn set_runtime_cloud_model_limits(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    input: ModelLimits,
) -> AppResult<RuntimeConnection> {
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "正在处理另一项云端设置。")?;
    let db = state.database.clone()?;
    let mut entry = get(&*db.connection()?, &workspace_id, &connection_id)?;
    input.validate()?;
    let key = credential(&db, &workspace_id, &entry)?;
    if input.model.contains(key.as_str()) {
        return Err("请填写有效的模型标识。".into());
    }
    entry.limits.retain(|old| old.model != input.model);
    if entry.limits.len() >= 128 {
        return Err("此连接最多可保存 128 组模型参数。".into());
    }
    entry.limits.push(input);
    update(&db, &workspace_id, &entry)?;
    Ok(entry)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn runtime_cloud_connections_scope_metadata_and_windows_credentials() {
        let db = Database::rag_evaluation_database().unwrap();
        let first = db.create_workspace("连接甲").unwrap().id;
        let second = db.create_workspace("连接乙").unwrap().id;
        let entry = RuntimeConnection {
            id: new_id(),
            provider: CloudProvider::Anthropic,
            label: "测试连接".into(),
            address: None,
            compatible_rerank: false,
            catalog_state: ConnectionState::Unconfigured,
            catalog_checked_at: None,
            models: vec![],
            probes: vec![],
            limits: vec![],
        };
        let vault = WindowsVault::for_runtime(&db.path, &first, &entry.id);
        struct Cleanup(WindowsVault);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.erase();
            }
        }
        let vault = Cleanup(vault);
        let fake = format!("synthetic-{}", new_id());
        vault
            .0
            .write_payload(&(entry.credential_binding().unwrap(), &fake))
            .unwrap();
        let c = db.connection().unwrap();
        let raw = serde_json::to_string(&entry).unwrap();
        assert!(!raw.contains(&fake));
        c.execute(
            "INSERT INTO model_cloud_connections(workspace_id,id,configuration_json) VALUES(?1,?2,?3)",
            params![first, entry.id, raw],
        ).unwrap();
        assert!(get(&c, &second, &entry.id).is_err());
        let found = get(&c, &first, &entry.id).unwrap();
        assert!(found.supports("model", Capability::Generation));
        assert!(!found.supports("model", Capability::Embedding));
        assert!(!found.supports("model", Capability::Reranking));
        assert!(credential(&db, &first, &entry).unwrap().as_str() == fake);
        assert!(credential(&db, &second, &entry).is_err());
        let mut altered = entry.clone();
        altered.provider = CloudProvider::Compatible;
        altered.address = Some("https://example.com/v1/".into());
        assert!(credential(&db, &first, &altered).is_err());
        vault.0.erase().unwrap();
        assert!(credential(&db, &first, &entry).is_err());
    }
}

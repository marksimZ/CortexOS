use crate::{
    models::{AppResult, ModelSettings, now},
    ollama::Ollama,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameters {
    pub num_ctx: u32,
    pub num_predict: u32,
    pub temperature: f64,
    pub timeout_seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRequest {
    pub model: String,
    pub expected_digest: Option<String>,
    pub parameters: Parameters,
    pub requirements: Vec<String>,
}
impl ModelRequest {
    pub(crate) fn settings(&self) -> ModelSettings {
        ModelSettings {
            model: self.model.clone(),
            num_ctx: self.parameters.num_ctx,
            num_predict: self.parameters.num_predict,
            temperature: self.parameters.temperature,
            timeout_seconds: self.parameters.timeout_seconds,
            ..Default::default()
        }
    }
    pub(crate) fn validate(&self) -> AppResult<()> {
        // Same tested application bounds, separate value; never calls save_settings.
        self.settings().validate()?;
        if self
            .expected_digest
            .as_ref()
            .is_some_and(|d| digest(Some(d)).is_none())
            || !self.requirements.iter().any(|r| r == "completion")
            || self
                .requirements
                .iter()
                .any(|r| !["completion", "json_schema", "tool_calls"].contains(&r.as_str()))
            || self.requirements.iter().collect::<BTreeSet<_>>().len() != self.requirements.len()
        {
            return Err("请检查模型版本指纹和评测所需能力。".into());
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTag {
    pub name: String,
    pub digest: Option<String>,
    pub size_bytes: Option<u64>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedModel {
    pub name: String,
    pub digest: Option<String>,
    pub size_bytes: Option<u64>,
    pub size_vram_bytes: Option<u64>,
    pub context_length: Option<u64>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub provider_id: &'static str,
    pub checked_at: String,
    pub version: Option<String>,
    pub models: Vec<ModelTag>,
    pub loaded: Option<Vec<LoadedModel>>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Capability {
    pub name: &'static str,
    pub status: &'static str,
    pub evidence: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub digest_source: &'static str,
    pub metadata: Value,
    pub context_length: Option<u64>,
    pub context_source: Option<String>,
    pub capabilities: Vec<Capability>,
}
#[derive(Debug, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub message: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    pub status: &'static str,
    pub checked_at: String,
    pub request: ModelRequest,
    pub catalog: Catalog,
    pub inspection: Option<Inspection>,
    pub issues: Vec<Issue>,
}

/// Unknown advertised capability can be probed by this explicitly selected v2
/// task. Identity, configuration and service failures still stop execution.
pub fn can_probe(checked: &Preflight) -> bool {
    checked.status == "ready_for_probe"
        || (checked.status == "needs_verification"
            && !checked.issues.is_empty()
            && checked
                .issues
                .iter()
                .all(|i| i.code == "capability_unknown"))
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 200 && !s.chars().any(char::is_control))
        .map(str::to_string)
}
fn digest(s: Option<&str>) -> Option<String> {
    s.filter(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
    .map(str::to_string)
}
fn array(v: &Value) -> AppResult<&Vec<Value>> {
    v["models"]
        .as_array()
        .filter(|a| a.len() <= 1000)
        .ok_or_else(|| "模型列表格式异常或条目过多，请检查 Ollama。".into())
}
fn tags(v: &Value) -> AppResult<Vec<ModelTag>> {
    let mut names = BTreeSet::new();
    array(v)?
        .iter()
        .map(|v| {
            let name = text(&v["name"]).ok_or("模型名称缺失或无效。")?;
            if !names.insert(name.clone()) {
                return Err("模型列表中出现重复名称。".into());
            }
            Ok(ModelTag {
                name,
                digest: digest(v["digest"].as_str()),
                size_bytes: v["size"].as_u64(),
                format: text(&v["details"]["format"]),
                family: text(&v["details"]["family"]),
                parameter_size: text(&v["details"]["parameter_size"]),
                quantization: text(&v["details"]["quantization_level"]),
            })
        })
        .collect()
}
fn loaded(v: &Value) -> AppResult<Vec<LoadedModel>> {
    array(v)?
        .iter()
        .map(|v| {
            Ok(LoadedModel {
                name: text(&v["name"]).ok_or("已加载模型名称无效。")?,
                digest: digest(v["digest"].as_str()),
                size_bytes: v["size"].as_u64(),
                size_vram_bytes: v["size_vram"].as_u64(),
                context_length: v["context_length"].as_u64(),
            })
        })
        .collect()
}
fn inspection(show: &Value) -> Inspection {
    let advertised = show["capabilities"]
        .as_array()
        .filter(|a| a.len() <= 64 && a.iter().all(|s| text(s).is_some()));
    let capabilities = [
        ("completion", "completion"),
        ("tool_calls", "tools"),
        ("thinking", "thinking"),
        ("json_schema", ""),
    ]
    .into_iter()
    .map(|(name, key)| {
        let status = if key.is_empty() || advertised.is_none() {
            "unknown"
        } else if advertised.unwrap().iter().any(|s| s == key) {
            "supported"
        } else {
            "unsupported"
        };
        Capability {
            name,
            status,
            evidence: if status == "unknown" {
                "not_probed"
            } else {
                "api_show_advertised"
            },
        }
    })
    .collect();
    let architecture = text(&show["model_info"]["general.architecture"]);
    let source = architecture.map(|a| format!("{a}.context_length"));
    let context = source
        .as_ref()
        .and_then(|k| show["model_info"][k].as_u64())
        .filter(|n| *n > 0);
    Inspection {
        digest_source: "api_tags_before_and_after_show",
        metadata: json!({"format":text(&show["details"]["format"]),"family":text(&show["details"]["family"]),"parameterSize":text(&show["details"]["parameter_size"]),"quantization":text(&show["details"]["quantization_level"]),"advertisedCapabilities":advertised,"architecture":text(&show["model_info"]["general.architecture"])}),
        context_length: context,
        context_source: source.filter(|_| context.is_some()),
        capabilities,
    }
}
impl Ollama {
    pub async fn benchmark_models(&self) -> AppResult<Catalog> {
        let models = tags(&self.get("/api/tags").await?)?;
        let mut warnings = Vec::new();
        let version = match self.get("/api/version").await {
            Ok(v) => text(&v["version"]),
            Err(e) => {
                warnings.push(e);
                None
            }
        };
        if version.is_none() {
            warnings.push("Ollama 服务版本未知。".into());
        }
        let loaded = match self.get("/api/ps").await.and_then(|v| loaded(&v)) {
            Ok(v) => Some(v),
            Err(e) => {
                warnings.push(format!("无法确认当前加载的模型：{e}"));
                None
            }
        };
        Ok(Catalog {
            provider_id: "ollama",
            checked_at: now(),
            version,
            models,
            loaded,
            warnings,
        })
    }

    pub async fn benchmark_inspect(&self, request: ModelRequest) -> AppResult<Preflight> {
        request.validate()?;
        let catalog = self.benchmark_models().await?;
        let mut result = Preflight {
            status: "ready_for_probe",
            checked_at: now(),
            request,
            catalog,
            inspection: None,
            issues: Vec::new(),
        };
        let Some(tag) = result
            .catalog
            .models
            .iter()
            .find(|m| m.name == result.request.model)
        else {
            result.status = "not_installed";
            result.issue(
                "not_installed",
                "所选模型未出现在本机已安装列表中；不会自动下载。",
            );
            return Ok(result);
        };
        let before = tag.digest.clone();
        if before.is_none() {
            result.unknown(
                "digest_unknown",
                "模型版本指纹缺失或无效，无法冻结模型身份。",
            );
        }
        if let (Some(expected), Some(actual)) = (&result.request.expected_digest, &before) {
            if expected != actual {
                result.status = "identity_changed";
                result.issue(
                    "identity_changed",
                    "模型标签指向的版本已改变，请重新选择模型并创建新实验。",
                );
                return Ok(result);
            }
        }
        if result.request.expected_digest.is_none() {
            result.unknown(
                "identity_unpinned",
                "请确认本次读取的模型版本指纹，再进行运行前复核。",
            );
        }
        let show = match self.show(&result.request.model).await {
            Ok(v) => v,
            Err(e) => {
                result.status = "blocked";
                result.issue("inspection_failed", &e);
                return Ok(result);
            }
        };
        // A tag is mutable. A read on either side of show detects replacement during this check.
        let after = tags(&self.get("/api/tags").await?)?;
        if !after
            .iter()
            .any(|m| m.name == result.request.model && m.digest == before)
        {
            result.status = "identity_changed";
            result.issue("identity_changed", "预检期间模型被移除或替换，请重新检查。");
            return Ok(result);
        }
        let profile = inspection(&show);
        let remote = ["remote_host", "remote_model"]
            .iter()
            .any(|k| show.get(k).is_some_and(|v| !v.is_null() && v != ""));
        if remote || show["details"]["format"] != "gguf" {
            result.status = "blocked";
            result.issue(
                "not_local_gguf",
                "首版评测只支持可确认安装在本机的 GGUF 模型，不能使用云端模型或未知格式。",
            );
        }
        if let Some(show_digest) = show.get("digest") {
            if digest(show_digest.as_str()) != before {
                result.status = "identity_changed";
                result.issue(
                    "identity_changed",
                    "模型详情与列表的版本指纹不一致，请重新检查。",
                );
            }
        }
        match profile.context_length {
            Some(limit) if u64::from(result.request.parameters.num_ctx) > limit => {
                result.status = "blocked";
                result.issue("context_exceeded", "请求容量超过模型元数据声明的上限。");
            }
            None => result.unknown(
                "context_unknown",
                "模型没有提供可识别的上下文上限，需实际验证；不能按品牌推断。",
            ),
            _ => (),
        }
        for required in result.request.requirements.clone() {
            let cap = profile
                .capabilities
                .iter()
                .find(|c| c.name == required)
                .expect("validated requirement");
            match cap.status {
                "unsupported" => {
                    result.status = "blocked";
                    result.issue(
                        "capability_unsupported",
                        &format!("模型声明不支持所需能力：{required}。"),
                    );
                }
                "unknown" => result.unknown(
                    "capability_unknown",
                    &format!("所需能力尚未验证：{required}，不能当作已支持。"),
                ),
                _ => (),
            }
        }
        if result
            .issues
            .iter()
            .any(|issue| issue.code == "identity_changed")
        {
            result.status = "identity_changed";
        }
        result.inspection = Some(profile);
        result.checked_at = now();
        Ok(result)
    }
}
impl Preflight {
    fn issue(&mut self, code: &'static str, message: &str) {
        self.issues.push(Issue {
            code,
            message: message.into(),
        });
    }
    fn unknown(&mut self, code: &'static str, message: &str) {
        if self.status == "ready_for_probe" {
            self.status = "needs_verification";
        }
        self.issue(code, message);
    }
}

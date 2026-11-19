use crate::{
    commands::{AppState, blocking},
    db::error,
    models::{AppResult, new_id},
    ollama::{Ollama, is_local_gguf},
    tasks::TaskKind,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    #[default]
    Auto,
    Cpu,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    device: Device,
    keep_alive_seconds: u16,
}

impl Preferences {
    fn validate(&self) -> AppResult<()> {
        if self.keep_alive_seconds > 300 {
            return Err("回答后的保留时间需为 0 至 300 秒。".into());
        }
        Ok(())
    }

    pub(crate) fn read(c: &Connection) -> AppResult<Self> {
        let value: Option<String> = c
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='local_runtime'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        let options = value
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(crate::db::json_error)?
            .unwrap_or_default();
        Self::validate(&options)?;
        Ok(options)
    }

    pub(crate) fn save(self, c: &Connection) -> AppResult<Self> {
        self.validate()?;
        c.execute(
            "INSERT INTO app_settings VALUES('local_runtime',?1)
            ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",
            [serde_json::to_string(&self).map_err(crate::db::json_error)?],
        )
        .map_err(error)?;
        Ok(self)
    }

    pub(crate) fn apply(self, request: &mut Value) {
        request["keep_alive"] = json!(self.keep_alive_seconds);
        if matches!(self.device, Device::Cpu) {
            request["options"]["num_gpu"] = json!(0);
        }
    }
}

pub(crate) fn capability(show: &Value, name: &str) -> Option<bool> {
    let values = show["capabilities"]
        .as_array()
        .filter(|values| values.len() <= 64 && values.iter().all(Value::is_string))?;
    Some(values.iter().any(|value| value == name))
}

pub(crate) fn context_capacity(show: &Value) -> Option<u64> {
    let architecture = show["model_info"]["general.architecture"]
        .as_str()
        .filter(|value| value.len() <= 200)?;
    show["model_info"][format!("{architecture}.context_length")]
        .as_u64()
        .filter(|size| *size > 0)
}

async fn status(provider: &Ollama, model: Option<&str>) -> AppResult<Value> {
    let catalog = provider.benchmark_models().await?;
    let inspection = if let Some(model) = model {
        if catalog.models.iter().any(|tag| tag.name == model) {
            match provider.show(model).await {
                Ok(info) => json!({"model":model,"local":is_local_gguf(&info),
                    "generation":capability(&info,"completion"),"embedding":capability(&info,"embedding"),
                    "contextCapacity":context_capacity(&info)}),
                Err(error) => json!({"model":model,"error":error}),
            }
        } else {
            json!({"model":model,"error":"该模型未出现在本机目录中。"})
        }
    } else {
        Value::Null
    };
    Ok(json!({"catalog":catalog,"inspection":inspection}))
}

#[tauri::command]
pub async fn local_model_preferences(
    state: State<'_, AppState>,
    input: Option<Preferences>,
) -> AppResult<Preferences> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        match input {
            Some(value) => value.save(&c),
            None => Preferences::read(&c),
        }
    })
    .await
}

#[tauri::command]
pub async fn inspect_local_model_runtime(
    state: State<'_, AppState>,
    model: Option<String>,
) -> AppResult<Value> {
    let provider = state.provider.clone()?;
    status(&provider, model.as_deref()).await
}

#[tauri::command]
pub async fn release_local_runtime_model(
    state: State<'_, AppState>,
    workspace_id: String,
    model: String,
    digest: String,
) -> AppResult<Value> {
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    blocking(db, move |db| {
        let c = db.connection()?;
        crate::benchmark::dataset::require_workspace(&c, &scope)
    })
    .await?;
    let provider = state.provider.clone()?;
    let lease = state.claim_task(&new_id(), &workspace_id, TaskKind::LocalMaintenance)?;
    let mut cancel = lease.receiver();
    tokio::select! {
        biased;
        _ = cancel.wait_for(|value| *value) => Err("卸载已取消，请重新读取模型状态。".into()),
        result = provider.release_model(&model, &digest) => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Database, models::ModelSettings};

    #[test]
    fn local_runtime_preferences_are_persisted_in_actual_chat_requests() {
        let db = Database::rag_evaluation_database().unwrap();
        let workspace = db.create_workspace("本机设置验证").unwrap().id;
        let settings = ModelSettings::default();
        let preview = || {
            db.prepare_context_run(&new_id(), &workspace, "", "问题", &settings, &[], true)
                .unwrap()
                .request
        };
        let first = preview();
        assert_eq!(first["keep_alive"], 0);
        assert!(first["options"].get("num_gpu").is_none());
        {
            let c = db.connection().unwrap();
            Preferences {
                device: Device::Cpu,
                keep_alive_seconds: 42,
            }
            .save(&c)
            .unwrap();
            assert!(
                Preferences {
                    device: Device::Auto,
                    keep_alive_seconds: 301
                }
                .save(&c)
                .is_err()
            );
        }
        let configured = preview();
        assert_eq!(configured["keep_alive"], 42);
        assert_eq!(configured["options"]["num_gpu"], 0);
        assert_eq!(configured["model"], first["model"]);
        let show = json!({"capabilities":["embedding"],"model_info":{"general.architecture":"fixture","fixture.context_length":2048}});
        assert_eq!(capability(&show, "completion"), Some(false));
        assert_eq!(capability(&show, "embedding"), Some(true));
        assert_eq!(context_capacity(&show), Some(2048));
        assert_eq!(capability(&json!({}), "completion"), None);
    }
}

pub mod completion;
#[cfg(test)]
mod completion_tests;
mod provider;
pub(crate) mod privacy;
pub(crate) mod runtime_connections;
mod runtime_protocol;
mod transport;
#[cfg(test)]
mod tests;
pub(crate) mod vault;

use crate::models::{AppResult, now};
use provider::{Model, OfficialProvider, ProviderAdapter};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tauri::State;
use vault::{Vault, WindowsVault};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Openai,
    Anthropic,
    Gemini,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Unconfigured,
    Verifying,
    ModelRequired,
    Verified,
    InvalidCredential,
    ModelDenied,
    QuotaExceeded,
    RateLimited,
    NetworkUnavailable,
    ServiceError,
    RevalidationRequired,
    Cancelled,
}

#[derive(Serialize, Deserialize)]
pub struct Record {
    version: u8,
    provider: Provider,
    key: String,
    model: Option<String>,
    state: ConnectionState,
    checked_at: Option<String>,
}
impl Drop for Record {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
impl Record {
    fn validate(&self) -> AppResult<()> {
        if self.version != 1
            || !valid_key(&self.key)
            || self
                .model
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 200)
        {
            return Err("连接记录不可用，请重新绑定。".into());
        }
        Ok(())
    }
    fn view(&self) -> Connection {
        let fresh = self
            .checked_at
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|date| {
                (0..86400).contains(&(chrono::Utc::now().timestamp() - date.timestamp()))
            });
        let state = if self.state == ConnectionState::Verified && !fresh {
            ConnectionState::RevalidationRequired
        } else if self.state == ConnectionState::Verifying {
            ConnectionState::RevalidationRequired
        } else {
            self.state
        };
        Connection {
            state,
            provider: Some(self.provider),
            model: self.model.clone(),
            checked_at: self.checked_at.clone(),
            allowed: state == ConnectionState::Verified && self.model.is_some(),
            models: vec![],
        }
    }
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 1024 && key.bytes().all(|b| (33..=126).contains(&b))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub state: ConnectionState,
    pub provider: Option<Provider>,
    pub model: Option<String>,
    pub checked_at: Option<String>,
    pub allowed: bool,
    pub models: Vec<Model>,
}
impl Connection {
    fn empty() -> Self {
        Self {
            state: ConnectionState::Unconfigured,
            provider: None,
            model: None,
            checked_at: None,
            allowed: false,
            models: vec![],
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyInput {
    provider: Provider,
    api_key: Option<String>,
    model: Option<String>,
}
impl Drop for VerifyInput {
    fn drop(&mut self) {
        if let Some(key) = &mut self.api_key {
            key.zeroize();
        }
    }
}

#[derive(Clone)]
pub struct CloudState {
    vault: Arc<dyn Vault>,
    adapter: Arc<dyn ProviderAdapter>,
    operation: Arc<tokio::sync::Mutex<()>>,
    cancel: tokio::sync::watch::Sender<u64>,
}
impl CloudState {
    pub fn new() -> Self {
        Self::with(Arc::new(WindowsVault::new()), Arc::new(OfficialProvider))
    }
    fn with(vault: Arc<dyn Vault>, adapter: Arc<dyn ProviderAdapter>) -> Self {
        Self {
            vault,
            adapter,
            operation: Arc::new(tokio::sync::Mutex::new(())),
            cancel: tokio::sync::watch::channel(0).0,
        }
    }
    pub fn status(&self) -> AppResult<Connection> {
        let mut result = self
            .vault
            .read()?
            .map(|r| r.view())
            .unwrap_or_else(Connection::empty);
        if self.operation.try_lock().is_err() {
            result.state = ConnectionState::Verifying;
            result.allowed = false;
        }
        Ok(result)
    }
    pub fn guard(&self) -> AppResult<Connection> {
        self.with_access(|| {
            self.vault
                .read()?
                .map(|r| r.view())
                .ok_or("请先连接云端账号。".into())
        })
    }
    pub async fn complete(
        &self,
        expected: &Connection,
        input: &completion::CompletionInput,
    ) -> Result<completion::CompletionOutput, ConnectionState> {
        use crate::model_runtime::{CloudGeneration, Generated, GenerationProvider};
        match self.generate(CloudGeneration { connection: expected, request: input }).await? {
            Generated::Complete(output) => Ok(output),
            Generated::Streaming(_) => Err(ConnectionState::ServiceError),
        }
    }

    pub(crate) async fn complete_approved(
        &self,
        expected: &Connection,
        input: &completion::CompletionInput,
    ) -> Result<completion::CompletionOutput, ConnectionState> {
        let record = {
            let _lock = self
                .operation
                .try_lock()
                .map_err(|_| ConnectionState::RevalidationRequired)?;
            let record = self
                .vault
                .read()
                .map_err(|_| ConnectionState::ServiceError)?
                .ok_or(ConnectionState::Unconfigured)?;
            if !record.view().allowed
                || Some(record.provider) != expected.provider
                || record.checked_at != expected.checked_at
                || record.model.as_deref() != Some(input.model.as_str())
                || record.model != expected.model
            {
                return Err(ConnectionState::RevalidationRequired);
            }
            record
        };
        let mut cancelled = self.cancel.subscribe();
        let result = tokio::select! {biased;_=cancelled.changed()=>Err(ConnectionState::Cancelled),r=tokio::time::timeout(Duration::from_secs(125),self.adapter.complete(record.provider,&record.key,input))=>r.unwrap_or(Err(ConnectionState::NetworkUnavailable))};
        if let Err(failure) = result.as_ref()
            && let Ok(_lock) = self.operation.try_lock()
            && let Ok(Some(mut current)) = self.vault.read()
            && current.key == record.key
            && current.provider == record.provider
        {
            current.state = *failure;
            let _ = self.vault.write(&current);
        }
        result
    }
    pub fn with_access<T>(&self, operation: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        let _lock = self
            .operation
            .try_lock()
            .map_err(|_| "连接正在验证，请稍后重试。".to_string())?;
        let connection = self
            .vault
            .read()?
            .map(|r| r.view())
            .unwrap_or_else(Connection::empty);
        if !connection.allowed {
            return Err("请先验证云端密钥并选择可访问的模型。".into());
        }
        operation()
    }
    async fn verify(&self, mut input: VerifyInput) -> AppResult<Connection> {
        let _lock = self
            .operation
            .try_lock()
            .map_err(|_| "已有连接验证正在进行。".to_string())?;
        let mut cancel = self.cancel.subscribe();
        let mut record = if let Some(key) = input.api_key.take() {
            let key = Zeroizing::new(key);
            if !valid_key(&key) {
                return Err("密钥为空、过长或含空白字符，请检查后重试。".into());
            }
            Record {
                version: 1,
                provider: input.provider,
                key: key.to_string(),
                model: None,
                state: ConnectionState::Verifying,
                checked_at: None,
            }
        } else {
            let record = self.vault.read()?.ok_or("请先输入 API Key。")?;
            if record.provider != input.provider {
                return Err("更换服务商时需要输入对应的 API Key。".into());
            }
            record
        };
        record.state = ConnectionState::Verifying;
        self.vault.write(&record)?;
        let result = tokio::select! {
            biased;
            _ = cancel.changed() => Err(ConnectionState::Cancelled),
            result = tokio::time::timeout(Duration::from_secs(30), self.adapter.catalog(record.provider, &record.key)) => result.unwrap_or(Err(ConnectionState::NetworkUnavailable)),
        };
        let models = match result {
            Ok(mut models) => {
                if record.provider == Provider::Gemini {
                    models.retain(|m| m.generation == "listed");
                }
                let selected = input.model.take().or_else(|| record.model.clone());
                record.state = match selected.as_ref() {
                    Some(id) if models.iter().any(|m| &m.id == id) => ConnectionState::Verified,
                    Some(_) => ConnectionState::ModelDenied,
                    None if models.is_empty() => ConnectionState::ModelDenied,
                    None => ConnectionState::ModelRequired,
                };
                record.model = selected;
                record.checked_at = Some(now());
                models
            }
            Err(state) => {
                record.state = state;
                vec![]
            }
        };
        self.vault.write(&record)?;
        let mut connection = record.view();
        connection.models = models;
        Ok(connection)
    }
    fn signal_cancel(&self) {
        self.cancel
            .send_modify(|value| *value = value.wrapping_add(1));
    }
    async fn cancel(&self) -> AppResult<Connection> {
        let running = self.operation.try_lock().is_err();
        self.signal_cancel();
        let _lock = self.operation.lock().await;
        if running {
            // Also closes the race where HTTP completed just before cancellation.
            if let Some(mut record) = self.vault.read()? {
                record.state = ConnectionState::Cancelled;
                self.vault.write(&record)?;
            }
        }
        Ok(self
            .vault
            .read()?
            .map(|r| r.view())
            .unwrap_or_else(Connection::empty))
    }
    async fn remove(&self) -> AppResult<Connection> {
        self.signal_cancel();
        let _lock = self.operation.lock().await;
        self.vault.delete()?;
        Ok(Connection::empty())
    }
}

#[tauri::command]
pub fn cloud_connection(state: State<'_, CloudState>) -> AppResult<Connection> {
    state.status()
}
#[tauri::command]
pub async fn verify_cloud_connection(
    state: State<'_, CloudState>,
    input: VerifyInput,
) -> AppResult<Connection> {
    state.verify(input).await
}
#[tauri::command]
pub async fn cancel_cloud_verification(state: State<'_, CloudState>) -> AppResult<Connection> {
    state.cancel().await
}
#[tauri::command]
pub async fn remove_cloud_connection(state: State<'_, CloudState>) -> AppResult<Connection> {
    state.remove().await
}
#[tauri::command]
pub fn personalized_access(state: State<'_, CloudState>) -> AppResult<Connection> {
    state.guard()
}

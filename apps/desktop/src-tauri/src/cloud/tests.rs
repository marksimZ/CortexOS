use super::*;
use provider::{CatalogFuture, Model};
use serde_json::json;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct MemoryVault {
    blob: Mutex<Option<String>>,
    fail: AtomicBool,
}
impl Vault for MemoryVault {
    fn read(&self) -> AppResult<Option<Record>> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("vault unavailable".into());
        }
        Ok(self
            .blob
            .lock()
            .unwrap()
            .as_ref()
            .map(|v| serde_json::from_str(v).unwrap()))
    }
    fn write(&self, value: &Record) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("vault unavailable".into());
        }
        value.validate()?;
        *self.blob.lock().unwrap() = Some(serde_json::to_string(value).unwrap());
        Ok(())
    }
    fn delete(&self) -> AppResult<()> {
        *self.blob.lock().unwrap() = None;
        Ok(())
    }
}
struct MockProvider {
    result: Mutex<Result<Vec<Model>, ConnectionState>>,
    delay: Duration,
}
impl ProviderAdapter for MockProvider {
    fn catalog<'a>(&'a self, _: Provider, _: &'a str) -> CatalogFuture<'a> {
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            self.result.lock().unwrap().clone()
        })
    }
}
fn setup() -> (CloudState, Arc<MemoryVault>, Arc<MockProvider>) {
    let vault = Arc::new(MemoryVault::default());
    let provider = Arc::new(MockProvider {
        result: Mutex::new(Ok(vec![Model {
            id: "fixture-model".into(),
            generation: "unknown",
            ..Model::default()
        }])),
        delay: Duration::from_millis(1),
    });
    (
        CloudState::with(vault.clone(), provider.clone()),
        vault,
        provider,
    )
}

#[tokio::test]
async fn profile_access_guard_protects_operations_and_rechecks_revocation() {
    let (state, _, _) = setup();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let operation = || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };
    assert!(state.with_access(operation).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    state
        .verify(input(true, Some("fixture-model")))
        .await
        .unwrap();
    state.with_access(operation).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _lock = state.operation.lock().await;
    assert!(state.with_access(operation).is_err());
    drop(_lock);
    state.remove().await.unwrap();
    assert!(state.with_access(operation).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
fn input(key: bool, model: Option<&str>) -> VerifyInput {
    VerifyInput {
        provider: Provider::Openai,
        api_key: key.then(|| "synthetic-not-a-real-key".into()),
        model: model.map(str::to_string),
    }
}

struct CompletionMock {
    calls: std::sync::atomic::AtomicUsize,
    fail: AtomicBool,
}

struct SlowCompletionMock;
impl ProviderAdapter for SlowCompletionMock {
    fn catalog<'a>(&'a self, _: Provider, _: &'a str) -> CatalogFuture<'a> {
        Box::pin(async {
            Ok(vec![Model {
                id: "fixture-model".into(),
                generation: "unknown",
                ..Model::default()
            }])
        })
    }
    fn complete<'a>(
        &'a self,
        _: Provider,
        _: &'a str,
        _: &'a completion::CompletionInput,
    ) -> completion::CompletionFuture<'a> {
        Box::pin(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Err(ConnectionState::ServiceError)
        })
    }
}
#[tokio::test]
async fn personalized_removing_connection_cancels_pending_cloud_request() {
    let cloud = CloudState::with(
        Arc::new(MemoryVault::default()),
        Arc::new(SlowCompletionMock),
    );
    let connection = cloud
        .verify(input(true, Some("fixture-model")))
        .await
        .unwrap();
    let request = completion::CompletionInput {
        model: "fixture-model".into(),
        system: "test".into(),
        prompt: "fixture".into(),
        max_output_tokens: 128,
        schema: None,
        tools: vec![],
    };
    let completing = cloud.complete(&connection, &request);
    let revoke = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        cloud.remove().await.unwrap();
    };
    let (result, _) = tokio::join!(completing, revoke);
    assert_eq!(result.err(), Some(ConnectionState::Cancelled));
    assert_eq!(cloud.status().unwrap().state, ConnectionState::Unconfigured);
}
impl ProviderAdapter for CompletionMock {
    fn catalog<'a>(&'a self, _: Provider, _: &'a str) -> CatalogFuture<'a> {
        Box::pin(async {
            Ok(vec![Model {
                id: "fixture-model".into(),
                generation: "unknown",
                ..Model::default()
            }])
        })
    }
    fn complete<'a>(
        &'a self,
        _: Provider,
        _: &'a str,
        input: &'a completion::CompletionInput,
    ) -> completion::CompletionFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(ConnectionState::QuotaExceeded);
            }
            let text = if input.system.starts_with("You author") {
                let mut case = crate::benchmark::dataset::builtin_version(2)
                    .unwrap()
                    .cases
                    .into_iter()
                    .find(|c| c.case_id == "math-001")
                    .unwrap();
                case.input.prompt = "Synthetic fixture only: return the integer 4".into();
                case.expected = crate::benchmark::dataset::Expected::Integer { answer: 4 };
                json!({"candidates":[{"case":case,"rationale":"Synthetic mocked response for integration tests","sources":[]}]}).to_string()
            } else {
                "4".into()
            };
            Ok(completion::CompletionOutput {
                text,
                thinking: None,
                calls: vec![],
                usage: json!({"input_tokens":100,"output_tokens":10}),
                finish_reason: "stop".into(),
                elapsed_ms: 100,
                request_hash: "a".repeat(64),
            })
        })
    }
}
#[tokio::test]
async fn personalized_mock_generation_publication_cloud_batch_scoring_and_guard_end_to_end() {
    use crate::benchmark::{
        personalized::{
            batch::{BatchInput, ModelChoice},
            plan::{GenerationInput, Price},
        },
        profile::{Profile, SaveProfile},
    };
    let vault = Arc::new(MemoryVault::default());
    let adapter = Arc::new(CompletionMock {
        calls: std::sync::atomic::AtomicUsize::new(0),
        fail: AtomicBool::new(false),
    });
    let cloud = CloudState::with(vault, adapter.clone());
    let connection = cloud
        .verify(input(true, Some("fixture-model")))
        .await
        .unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../artifacts/phase-c")
        .join(crate::models::new_id());
    let state = crate::commands::AppState::new(&path);
    let db = state.database.as_ref().unwrap();
    let w = db.create_workspace("Mock provider integration").unwrap().id;
    db.save_benchmark_profile(
        &w,
        SaveProfile {
            expected_version: 0,
            action: "skip".into(),
            profile: Profile::default(),
        },
    )
    .unwrap();
    let price = Price {
        currency: "USD".into(),
        input_per_million: 1.,
        output_per_million: 2.,
        budget: 10.,
        reference: "synthetic fixture price, not provider pricing".into(),
    };
    let job = db
        .plan_generation(
            &w,
            &connection,
            GenerationInput {
                profile_version: 1,
                repository_id: None,
                sharing: "summary".into(),
                snippets: vec![],
                count: 1,
                difficulty: "easy".into(),
                difficulty_mix: Default::default(),
                categories: vec!["integer_equal".into()],
                max_output_tokens: 4096,
                price: price.clone(),
            },
        )
        .unwrap();
    let id = job["id"].as_str().unwrap();
    let plan = db
        .begin_generation(&w, id, job["planHash"].as_str().unwrap(), &connection)
        .unwrap();
    let output = cloud.complete(&connection, &plan.request).await.unwrap();
    db.finish_generation(&w, id, Ok(output)).unwrap();
    let reviewed = db.generation_job(&w, id).unwrap();
    let candidate = &reviewed["candidates"][0];
    assert_eq!(candidate["validation"]["valid"], true);
    let ds = db
        .publish_personalized(
            &w,
            id,
            json!([{"id":candidate["id"],"revision":1,"approved":true}]),
            "synthetic reviewer",
        )
        .unwrap();
    let batch = db
        .plan_personalized_batch(
            &w,
            &connection,
            BatchInput {
                case_ids: vec![],
                local_keep_alive_seconds: None,
                release_models: vec![],
                repetitions: 1,
                dataset_id: ds.id,
                dataset_version: 1,
                dataset_hash: ds.content_hash,
                models: vec![ModelChoice {
                    kind: "cloud".into(),
                    local: None,
                }],
                max_output_tokens: 512,
                price: Some(price),
                python_image: None,
            },
        )
        .unwrap();
    let batch_id = batch["id"].as_str().unwrap().to_string();
    assert!(
        db.begin_personalized_batch(&w, &batch_id, "wrong", &connection)
            .is_err()
    );
    let batch_plan = db
        .begin_personalized_batch(
            &w,
            &batch_id,
            batch["planHash"].as_str().unwrap(),
            &connection,
        )
        .unwrap();
    let lease = Arc::new(
        state
            .claim_task(&batch_id, &w, crate::tasks::TaskKind::Benchmark)
            .unwrap(),
    );
    crate::benchmark::personalized::batch::execute(
        state.clone(),
        cloud.clone(),
        w.clone(),
        batch_id.clone(),
        batch_plan,
        lease,
    )
    .await
    .unwrap();
    let report = db.personalized_report(&w, &batch_id).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["reports"][0]["statistics"]["correctness"], 1.);
    assert_eq!(report["reports"][0]["statistics"]["scoringCoverage"], 1.);
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
    assert_eq!(db.conversations(&w).unwrap().len(), 0);
    let saved = serde_json::to_string(&report).unwrap();
    assert!(!saved.contains("synthetic-not-a-real-key"));
    let request = completion::CompletionInput {
        model: "different-model".into(),
        ..plan.request.clone()
    };
    assert!(cloud.complete(&connection, &request).await.is_err());
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
    adapter.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        cloud.complete(&connection, &plan.request).await.err(),
        Some(ConnectionState::QuotaExceeded)
    );
    assert!(!cloud.guard().is_ok());
}

#[tokio::test]
async fn cloud_bind_select_guard_restart_and_remove() {
    let (state, vault, adapter) = setup();
    assert!(state.guard().is_err());
    assert_eq!(
        state.verify(input(true, None)).await.unwrap().state,
        ConnectionState::ModelRequired
    );
    assert!(state.guard().is_err());
    let connection = state
        .verify(input(false, Some("fixture-model")))
        .await
        .unwrap();
    assert!(connection.allowed);
    let public = serde_json::to_string(&connection).unwrap();
    assert!(!public.contains("synthetic-not-a-real-key"));
    assert!(!public.contains("apiKey"));
    let restarted = CloudState::with(vault, adapter);
    assert!(restarted.guard().unwrap().allowed);
    assert_eq!(
        restarted.remove().await.unwrap().state,
        ConnectionState::Unconfigured
    );
    assert!(restarted.guard().is_err());
}

#[tokio::test]
async fn cloud_failures_retain_credentials_but_never_unlock() {
    let (state, vault, adapter) = setup();
    state
        .verify(input(true, Some("fixture-model")))
        .await
        .unwrap();
    for failure in [
        ConnectionState::InvalidCredential,
        ConnectionState::ModelDenied,
        ConnectionState::QuotaExceeded,
        ConnectionState::RateLimited,
        ConnectionState::NetworkUnavailable,
        ConnectionState::ServiceError,
    ] {
        *adapter.result.lock().unwrap() = Err(failure);
        assert_eq!(
            state.verify(input(false, None)).await.unwrap().state,
            failure
        );
        assert!(state.guard().is_err());
        assert_eq!(
            vault.read().unwrap().unwrap().key,
            "synthetic-not-a-real-key"
        );
    }
}

#[tokio::test]
async fn cloud_model_provider_and_input_tampering() {
    let (state, _, _) = setup();
    assert!(
        serde_json::from_value::<VerifyInput>(json!({"provider":"openai","allowed":true})).is_err()
    );
    assert!(
        serde_json::from_value::<VerifyInput>(json!({"provider":"custom","apiKey":"x"})).is_err()
    );
    assert_eq!(
        state
            .verify(input(true, Some("invented-model")))
            .await
            .unwrap()
            .state,
        ConnectionState::ModelDenied
    );
    assert!(state.guard().is_err());
    let mut changed = input(false, None);
    changed.provider = Provider::Anthropic;
    assert!(state.verify(changed).await.is_err());
    let mut changed = input(true, Some("fixture-model"));
    changed.provider = Provider::Anthropic;
    assert!(state.verify(changed).await.unwrap().allowed);
    for key in ["", "has space", "header\r\ninjection"] {
        let mut invalid = input(true, None);
        invalid.api_key = Some(key.into());
        assert!(state.verify(invalid).await.is_err());
    }
}

#[tokio::test]
async fn cloud_cancel_and_delete_do_not_allow_late_unlock() {
    let vault = Arc::new(MemoryVault::default());
    let adapter = Arc::new(MockProvider {
        result: Mutex::new(Ok(vec![Model {
            id: "fixture-model".into(),
            generation: "unknown",
            ..Model::default()
        }])),
        delay: Duration::from_secs(5),
    });
    let state = CloudState::with(vault, adapter);
    let verify = state.verify(input(true, Some("fixture-model")));
    let cancel = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(state.status().unwrap().state, ConnectionState::Verifying);
        assert!(state.guard().is_err());
        assert!(state.verify(input(true, None)).await.is_err());
        state.cancel().await.unwrap();
    };
    let (result, _) = tokio::join!(verify, cancel);
    assert_eq!(result.unwrap().state, ConnectionState::Cancelled);
    let (result, removed) =
        tokio::join!(state.verify(input(false, Some("fixture-model"))), async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            state.remove().await
        });
    assert_eq!(result.unwrap().state, ConnectionState::Cancelled);
    assert_eq!(removed.unwrap().state, ConnectionState::Unconfigured);
    assert!(state.guard().is_err());
}

#[tokio::test]
async fn cloud_stale_missing_corrupt_and_incomplete_records_deny_access() {
    let (state, vault, _) = setup();
    state
        .verify(input(true, Some("fixture-model")))
        .await
        .unwrap();
    let mut record = vault.read().unwrap().unwrap();
    record.checked_at = Some("2020-01-01T00:00:00Z".into());
    vault.write(&record).unwrap();
    assert_eq!(
        state.status().unwrap().state,
        ConnectionState::RevalidationRequired
    );
    assert!(state.guard().is_err());
    record.checked_at = Some(now());
    record.state = ConnectionState::Verifying;
    vault.write(&record).unwrap();
    assert_eq!(
        state.status().unwrap().state,
        ConnectionState::RevalidationRequired
    );
    assert!(state.guard().is_err());
    record.key.clear();
    assert!(record.validate().is_err());
    vault.fail.store(true, Ordering::SeqCst);
    assert!(state.status().is_err());
    assert!(state.guard().is_err());
}

#[test]
fn cloud_protocol_parsers_and_error_classification() {
    for provider in [Provider::Openai, Provider::Anthropic] {
        let (models, _) =
            provider::parse(provider, &json!({"data":[{"id":"actual-model"}]})).unwrap();
        assert_eq!(models[0].generation, "unknown");
        assert!(provider::parse(provider, &json!({"data":[{"id":"malformed\nmodel"}]})).is_err());
    }
    let (models,next) = provider::parse(Provider::Gemini,&json!({"models":[{"name":"models/text","supportedGenerationMethods":["generateContent"],"inputTokenLimit":2048,"outputTokenLimit":512},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"page2"})).unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].generation, "listed");
    assert_eq!(models[0].max_input_tokens, Some(2048));
    assert_eq!(models[0].max_output_tokens, Some(512));
    assert_eq!(models[1].generation, "unsupported");
    assert_eq!(models[1].embedding, "listed");
    assert_eq!(next.as_deref(), Some("page2"));
    assert!(provider::parse(Provider::Anthropic, &json!({"data":[],"has_more":true})).is_err());
    assert_eq!(
        provider::classify(
            400,
            &json!({"error":{"details":[{"reason":"API_KEY_INVALID"}]}})
        ),
        ConnectionState::InvalidCredential
    );
    assert_eq!(
        provider::classify(429, &json!({"error":{"code":"insufficient_quota"}})),
        ConnectionState::QuotaExceeded
    );
    for (code, expected) in [
        (401, ConnectionState::InvalidCredential),
        (403, ConnectionState::ModelDenied),
        (404, ConnectionState::ModelDenied),
        (429, ConnectionState::RateLimited),
        (503, ConnectionState::ServiceError),
        (302, ConnectionState::ServiceError),
    ] {
        assert_eq!(
            provider::classify(code, &json!({"error":{"message":"synthetic-secret"}})),
            expected
        );
    }
}

#[tokio::test]
async fn cloud_timeout_is_network_state_not_invalid_key() {
    let vault = Arc::new(MemoryVault::default());
    let adapter = Arc::new(MockProvider {
        result: Mutex::new(Ok(vec![])),
        delay: Duration::from_secs(60),
    });
    let state = CloudState::with(vault, adapter);
    let result = state.verify(input(true, None)).await.unwrap();
    assert_eq!(result.state, ConnectionState::NetworkUnavailable);
    assert!(state.guard().is_err());
}

#[test]
#[cfg(windows)]
fn cloud_windows_vault_synthetic_roundtrip() {
    // Only touches this random test target, never the user's actual connection.
    let target = format!("CortexOS/Test/{}", uuid::Uuid::new_v4());
    let vault = vault::WindowsVault::test_target(target.clone());
    let mut record = Record {
        version: 1,
        provider: Provider::Gemini,
        key: "synthetic-only-no-network".into(),
        model: Some("fixture".into()),
        state: ConnectionState::Verified,
        checked_at: Some(now()),
    };
    assert!(vault.read().unwrap().is_none());
    vault.write(&record).unwrap();
    let reopened = vault::WindowsVault::test_target(target);
    let read = reopened.read();
    vault.delete().unwrap();
    assert_eq!(read.unwrap().unwrap().key, record.key);
    record.key = "synthetic-replacement".into();
    vault.write(&record).unwrap();
    let read = reopened.read();
    vault.delete().unwrap();
    assert_eq!(read.unwrap().unwrap().key, "synthetic-replacement");
    assert!(reopened.read().unwrap().is_none());
}

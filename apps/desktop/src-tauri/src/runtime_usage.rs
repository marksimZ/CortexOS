use crate::{
    benchmark::dataset::require_workspace,
    cloud::{
        ConnectionState,
        runtime_connections::{Capability, CloudTarget, ModelLimits, request_error},
    },
    commands::{AppState, blocking},
    db::{Database, error, json_error},
    models::{AppResult, new_id},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::State;

pub(crate) mod report;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) single_run_micros: Option<u64>,
    pub(crate) rolling_day_micros: Option<u64>,
    pub(crate) requests_per_minute: Option<u16>,
    pub(crate) retry_limit: u8,
}

impl Policy {
    fn validate(&self) -> AppResult<()> {
        if self
            .single_run_micros
            .into_iter()
            .chain(self.rolling_day_micros)
            .any(|n| n > 1_000_000_000_000)
            || self
                .requests_per_minute
                .is_some_and(|n| !(1..=1000).contains(&n))
            || self.retry_limit > 2
        {
            return Err("请检查费用上限、每分钟请求数和重试次数。".into());
        }
        Ok(())
    }

    fn read(c: &Connection, workspace: &str) -> AppResult<Self> {
        let raw: Option<String> = c
            .query_row(
                "SELECT policy_json FROM model_runtime_policies WHERE workspace_id=?1",
                [workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        raw.map(|raw| serde_json::from_str(&raw).map_err(json_error))
            .transpose()
            .map(|policy| policy.unwrap_or_default())
    }
}

#[tauri::command]
pub async fn model_runtime_policy(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Option<Policy>,
) -> AppResult<Policy> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        if let Some(policy) = input {
            policy.validate()?;
            let raw = serde_json::to_string(&policy).map_err(json_error)?;
            c.execute(
                "INSERT INTO model_runtime_policies VALUES (?1,?2)
                ON CONFLICT(workspace_id) DO UPDATE SET policy_json=excluded.policy_json",
                params![workspace_id, raw],
            )
            .map_err(error)?;
            Ok(policy)
        } else {
            Policy::read(&c, &workspace_id)
        }
    })
    .await
}

#[derive(Clone)]
pub(crate) struct Meter {
    database: Arc<Database>,
    workspace: String,
    operation: String,
    purpose: &'static str,
    fixture: Option<FixtureMeter>,
}

#[derive(Clone)]
struct FixtureMeter {
    corpus: Arc<Database>,
    corpus_workspace: String,
    run_id: String,
    experiment_id: String,
    max_cost: Option<u64>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Usage {
    pub(crate) input: Option<u64>,
    pub(crate) output: Option<u64>,
    pub(crate) vectors: Option<u64>,
}

pub(crate) struct Call<'a> {
    pub(crate) capability: Capability,
    pub(crate) data_types: &'a [&'a str],
    pub(crate) estimated_input: u64,
    pub(crate) max_output: u64,
    pub(crate) generation_trace: Option<&'a Value>,
    pub(crate) knowledge: &'a [crate::cloud::privacy::TransferVersion],
    pub(crate) texts: &'a [&'a str],
}

pub(crate) enum CallError {
    Guard(String),
    Provider(ConnectionState),
}

impl From<String> for CallError {
    fn from(value: String) -> Self {
        Self::Guard(value)
    }
}

impl CallError {
    pub(crate) fn message(self) -> String {
        match self {
            Self::Guard(text) => text,
            Self::Provider(status) => request_error(status),
        }
    }
}

pub(crate) fn price(
    limits: Option<&ModelLimits>,
    input: Option<u64>,
    output: Option<u64>,
    generates: bool,
) -> Option<u64> {
    let limits = limits?;
    let fixed = limits.micro_usd_per_request;
    let input_price = limits.input_micro_usd_per_million;
    let output_price = limits.output_micro_usd_per_million;
    if input_price.is_none() && (!generates || output_price.is_none()) {
        return fixed;
    }
    let fee = |tokens: Option<u64>, rate: Option<u64>| {
        let rate = rate?;
        if rate == 0 {
            return Some(0u128);
        }
        Some((u128::from(tokens?) * u128::from(rate)).div_ceil(1_000_000))
    };
    let total = u128::from(fixed.unwrap_or(0))
        + fee(input, input_price)?
        + if generates {
            fee(output, output_price)?
        } else {
            0
        };
    u64::try_from(total).ok().filter(|n| *n <= i64::MAX as u64)
}

impl Meter {
    pub(crate) fn new(
        database: Arc<Database>,
        workspace: String,
        operation: String,
        purpose: &'static str,
    ) -> Self {
        Self {
            database,
            workspace,
            operation,
            purpose,
            fixture: None,
        }
    }

    pub(crate) fn fixed_rag_experiment(
        owner: Arc<Database>, workspace: &str, corpus: Arc<Database>, corpus_workspace: &str,
        experiment_id: &str, unit: &str, run_id: &str, max_cost: Option<u64>,
    ) -> Self {
        let mut meter=Self::new(owner,workspace.into(),format!("{experiment_id}/{unit}"),"rag_experiment");
        meter.fixture=Some(FixtureMeter {corpus,corpus_workspace:corpus_workspace.into(),run_id:run_id.into(),
            experiment_id:experiment_id.into(),max_cost});
        meter
    }

    pub(crate) fn attempts(&self, capability: Capability) -> AppResult<i64> {
        self.database.connection()?.query_row(
            "SELECT count(*) FROM model_cloud_calls WHERE workspace_id=?1 AND operation_id=?2 AND capability=?3",
            params![self.workspace,self.operation,capability.field()], |r|r.get(0),
        ).map_err(error)
    }

    pub(crate) fn local_embedding(&self, model: &str) -> AppResult<LocalEmbedding> {
        let id = new_id();
        self.database.connection()?.execute(
            "INSERT INTO model_local_embeddings(id,workspace_id,operation_id,purpose,model,started_unix,state)
             VALUES(?1,?2,?3,?4,?5,?6,'started')",
            params![id,self.workspace,self.operation,self.purpose,model,chrono::Utc::now().timestamp()],
        ).map_err(error)?;
        Ok(LocalEmbedding {
            database: self.database.clone(),
            id,
            start: Instant::now(),
            settled: false,
        })
    }

    pub(crate) async fn call<T, F, Fut>(
        &self,
        target: &CloudTarget,
        spec: Call<'_>,
        mut call: F,
        usage: impl Fn(&T) -> Usage,
    ) -> Result<T, CallError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ConnectionState>>,
    {
        let mut retried = 0;
        loop {
            let (mut ticket, max_retries) = self.reserve(target, &spec)?;
            let started = Instant::now();
            let result = call().await;
            let elapsed = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
            match result {
                Ok(output) => {
                    let usage = usage(&output);
                    let settled = price(
                        target.limits(),
                        usage.input,
                        usage.output,
                        spec.capability == Capability::Generation,
                    );
                    ticket.settle("completed", usage, settled, elapsed, None)?;
                    return Ok(output);
                }
                Err(failure) => {
                    let code = serde_json::to_value(failure).map_err(json_error)?;
                    ticket.settle("failed", Usage::default(), None, elapsed, code.as_str())?;
                    if failure != ConnectionState::RateLimited || retried >= max_retries {
                        return Err(CallError::Provider(failure));
                    }
                    retried += 1;
                    tokio::time::sleep(Duration::from_secs(u64::from(retried))).await;
                }
            }
        }
    }

    fn reserve(&self, target: &CloudTarget, spec: &Call<'_>) -> AppResult<(Ticket, u8)> {
        let estimate = price(
            target.limits(),
            Some(spec.estimated_input),
            Some(spec.max_output),
            spec.capability == Capability::Generation,
        );
        let estimate = estimate.map(|v| v as i64);
        if let Some(fixture) = &self.fixture {
            let c = fixture.corpus.connection()?;
            crate::cloud::privacy::verify_transfer(&c,&fixture.corpus_workspace,&target.connection.id,spec.knowledge)?;
        }
        let now = chrono::Utc::now().timestamp();
        let mut c = self.database.connection()?;
        let tx = c.transaction().map_err(error)?;
        if self.fixture.is_none() {
            crate::cloud::privacy::verify_transfer(&tx,&self.workspace,&target.connection.id,spec.knowledge)?;
        }
        crate::cloud::privacy::check_workspace(&tx, &self.workspace, spec.texts)?;
        let policy = Policy::read(&tx, &self.workspace)?;
        policy.validate()?;
        let experiment_limit = self.fixture.as_ref().and_then(|fixture| {
            fixture.max_cost.into_iter().chain(policy.single_run_micros).min()
        });
        if policy.single_run_micros.is_some() || policy.rolling_day_micros.is_some() || experiment_limit.is_some() {
            let estimate = estimate.ok_or("已设费用上限，请先为这个模型填写适用费率。")?;
            if let Some(limit) = policy.single_run_micros.filter(|_| self.fixture.is_none()) {
                ensure_budget(
                    &tx,
                    "SELECT COALESCE(SUM(COALESCE(settled_micros,estimate_micros)),0),
                       COUNT(*) FILTER (WHERE settled_micros IS NULL AND estimate_micros IS NULL)
                     FROM model_cloud_calls WHERE workspace_id=?1 AND operation_id=?2",
                    params![self.workspace, self.operation],
                    limit,
                    estimate,
                )?;
            }
            if let Some(limit) = policy.rolling_day_micros {
                ensure_budget(
                    &tx,
                    "SELECT COALESCE(SUM(COALESCE(settled_micros,estimate_micros)),0),
                       COUNT(*) FILTER (WHERE settled_micros IS NULL AND estimate_micros IS NULL)
                     FROM model_cloud_calls WHERE workspace_id=?1 AND started_unix>?2",
                    params![self.workspace, now - 86400],
                    limit,
                    estimate,
                )?;
            }
            if let Some(limit)=experiment_limit {
                let pattern=format!("{}/*",self.fixture.as_ref().unwrap().experiment_id);
                ensure_budget(&tx,
                    "SELECT COALESCE(SUM(COALESCE(settled_micros,estimate_micros)),0),
                       COUNT(*) FILTER (WHERE settled_micros IS NULL AND estimate_micros IS NULL)
                     FROM model_cloud_calls WHERE workspace_id=?1 AND operation_id GLOB ?2",
                    params![self.workspace,pattern],limit,estimate)?;
            }
        }
        if let Some(limit) = policy.requests_per_minute {
            let attempts: i64 = tx.query_row(
                "SELECT count(*) FROM model_cloud_calls WHERE workspace_id=?1 AND connection_id=?2 AND started_unix>?3",
                params![self.workspace,target.connection.id,now-60], |r|r.get(0),
            ).map_err(error)?;
            if attempts >= i64::from(limit) {
                return Err("此连接已达到每分钟请求上限，请稍后再试。".into());
            }
        }
        let id = new_id();
        tx.execute(
            "INSERT INTO model_cloud_calls (id,workspace_id,operation_id,purpose,connection_id,provider,
                model,capability,data_types_json,started_unix,state,estimate_micros)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'started',?11)",
            params![id,self.workspace,self.operation,self.purpose,target.connection.id,target.connection.provider.name(),
                target.model,spec.capability.field(),serde_json::to_string(spec.data_types).map_err(json_error)?,now,estimate],
        ).map_err(error)?;
        if let Some(trace) = spec.generation_trace {
            let payload=json!({"transport":"cloud","target":trace,"callId":id});
            if let Some(fixture) = &self.fixture {
                crate::db::event(&*fixture.corpus.connection()?,&fixture.run_id,"model_request_started",payload)?;
            } else {
                crate::db::event(&tx,&self.operation,"model_request_started",payload)?;
            }
        }
        tx.commit().map_err(error)?;
        Ok((
            Ticket {
                database: self.database.clone(),
                id,
                closed: false,
            },
            policy.retry_limit,
        ))
    }
}

pub(crate) struct LocalEmbedding {
    database: Arc<Database>,
    id: String,
    start: Instant,
    settled: bool,
}

impl LocalEmbedding {
    pub(crate) fn settle(mut self, tokens: Option<u64>, vectors: Option<usize>) -> AppResult<()> {
        let input = tokens.and_then(|n| i64::try_from(n).ok());
        let elapsed = self.start.elapsed().as_millis().min(i64::MAX as u128) as i64;
        let state = if vectors.is_some() {
            "completed"
        } else {
            "failed"
        };
        self.database.connection()?.execute(
            "UPDATE model_local_embeddings SET state=?2,input_tokens=?3,vector_count=?4,elapsed_ms=?5 WHERE id=?1",
            params![self.id,state,input,vectors.map(|n|n as i64),elapsed],
        ).map_err(error)?;
        self.settled = true;
        Ok(())
    }
}

impl Drop for LocalEmbedding {
    fn drop(&mut self) {
        if !self.settled
            && let Ok(c) = self.database.connection()
        {
            let elapsed = self.start.elapsed().as_millis().min(i64::MAX as u128) as i64;
            let _ = c.execute("UPDATE model_local_embeddings SET state='outcome_unknown',elapsed_ms=?2 WHERE id=?1",
                params![self.id,elapsed]);
        }
    }
}

fn ensure_budget(
    c: &Connection,
    sql: &str,
    args: impl rusqlite::Params,
    limit: u64,
    estimate: i64,
) -> AppResult<()> {
    let (accounted, unknown): (i64, i64) = c
        .query_row(sql, args, |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(error)?;
    if unknown > 0 {
        return Err("预算范围内已有费用未知的请求，暂时无法核对剩余额度。".into());
    }
    if accounted.saturating_add(estimate) > limit as i64 {
        return Err("费用估算超过工作区预算，已停止新请求。".into());
    }
    Ok(())
}

struct Ticket {
    database: Arc<Database>,
    id: String,
    closed: bool,
}

impl Ticket {
    fn settle(
        &mut self,
        state: &str,
        usage: Usage,
        cost: Option<u64>,
        elapsed: i64,
        failure: Option<&str>,
    ) -> AppResult<()> {
        let integer = |n: Option<u64>| n.and_then(|v| i64::try_from(v).ok());
        self.database.connection()?.execute(
            "UPDATE model_cloud_calls SET state=?2,input_tokens=?3,output_tokens=?4,vector_count=?5,
             settled_micros=?6,elapsed_ms=?7,failure=?8 WHERE id=?1 AND state='started'",
            params![self.id,state,integer(usage.input),integer(usage.output),integer(usage.vectors),integer(cost),elapsed,failure],
        ).map_err(error)?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.closed
            && let Ok(c) = self.database.connection()
        {
            let _ = c.execute("UPDATE model_cloud_calls SET state='outcome_unknown' WHERE id=?1 AND state='started'",[&self.id]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_runtime::http_fixture;

    #[tokio::test]
    async fn metered_cloud_retries_rate_limits_and_reserves_budget_atomically() {
        let database = Arc::new(Database::rag_evaluation_database().unwrap());
        let workspace = database.create_workspace("限额验证").unwrap().id;
        let save = |p: &Policy| {
            database.connection().unwrap().execute(
            "INSERT INTO model_runtime_policies VALUES(?1,?2) ON CONFLICT(workspace_id) DO UPDATE SET policy_json=excluded.policy_json",
            params![workspace,serde_json::to_string(p).unwrap()],
        ).unwrap()
        };
        save(&Policy {
            single_run_micros: Some(100),
            rolling_day_micros: Some(150),
            retry_limit: 1,
            requests_per_minute: None,
        });
        let (url, server) = http_fixture::serve(2, |i, path, body| {
            assert_eq!(path, "/embeddings");
            assert_eq!(body["input"], json!(["budget-fixture"]));
            if i == 0 {
                (429, json!({"error":{"type":"rate_limit"}}))
            } else {
                (
                    200,
                    json!({"data":[{"index":0,"embedding":[3,4]}],"usage":{"prompt_tokens":5}}),
                )
            }
        });
        let mut target = CloudTarget::loopback_test(url, "fixture");
        let meter = Meter::new(database.clone(), workspace.clone(), new_id(), "chat");
        let input = ["budget-fixture".to_owned()];
        let timeout = Duration::from_secs(5);
        let error = target
            .embed(&input, None, timeout, &meter, "question", &[])
            .await
            .unwrap_err();
        assert!(error.contains("费率"));
        assert_eq!(meter.attempts(Capability::Embedding).unwrap(), 0);
        target.connection.limits.push(ModelLimits {
            model: target.model.clone(),
            context_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_micro_usd_per_million: None,
            output_micro_usd_per_million: None,
            micro_usd_per_request: Some(40),
        });
        let (vectors, tokens) = target
            .embed(&input, None, timeout, &meter, "question", &[])
            .await
            .unwrap();
        assert_eq!(vectors, vec![vec![0.6, 0.8]]);
        assert_eq!(tokens, Some(5));
        assert_eq!(meter.attempts(Capability::Embedding).unwrap(), 2);
        let error = target
            .embed(&input, None, timeout, &meter, "question", &[])
            .await
            .unwrap_err();
        assert!(error.contains("超过工作区预算"));
        {
            let c = database.connection().unwrap();
            let cost:(i64,i64,i64)=c.query_row(
                "SELECT SUM(estimate_micros),SUM(settled_micros),SUM(vector_count) FROM model_cloud_calls WHERE workspace_id=?1",
                [&workspace],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
            ).unwrap();
            assert_eq!(cost, (80, 40, 1));
        }
        save(&Policy {
            requests_per_minute: Some(2),
            ..Policy::default()
        });
        let blocked = target
            .embed(&input, None, timeout, &meter, "question", &[])
            .await
            .unwrap_err();
        assert!(blocked.contains("每分钟请求上限"));
        save(&Policy {
            rolling_day_micros: Some(120),
            ..Policy::default()
        });
        let other = Meter::new(database.clone(), workspace.clone(), new_id(), "cloud_index");
        let (ticket, _) = other
            .reserve(
                &target,
                &Call {
                    capability: Capability::Embedding,
                    data_types: &["granted_knowledge"],
                    estimated_input: 50,
                    max_output: 0,
                    generation_trace: None,
                    knowledge: &[],
                    texts: &[],
                },
            )
            .unwrap();
        drop(ticket);
        let blocked = target
            .embed(&input, None, timeout, &other, "question", &[])
            .await
            .unwrap_err();
        assert!(blocked.contains("超过工作区预算"));
        assert_eq!(database.connection().unwrap().query_row(
            "SELECT count(*) FROM model_cloud_calls WHERE workspace_id=?1 AND state='outcome_unknown'",
            [&workspace],|r|r.get::<_,i64>(0)).unwrap(),1);
        server.join().unwrap();
    }
}

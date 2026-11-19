use super::Policy;
use crate::{
    benchmark::{dataset::require_workspace, resources::Sampler},
    commands::{AppState, blocking},
    db::error,
    models::{AppResult, now},
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tauri::State;

const ROLLUP: &str = "
WITH local_generation AS (
  SELECT 'ollama' provider, json_extract(e.payload_json,'$.target.model') model, 'generation' capability,
    COALESCE(json_extract(u.payload_json,'$.status'), r.status) state,
    COALESCE(json_extract(u.payload_json,'$.inputTokens'),json_extract(f.payload_json,'$.metrics.promptEvalCount')) input_tokens,
    COALESCE(json_extract(u.payload_json,'$.outputTokens'),json_extract(f.payload_json,'$.metrics.evalCount')) output_tokens,
    NULL vector_count,
    COALESCE(json_extract(u.payload_json,'$.elapsedMs'),json_extract(f.payload_json,'$.metrics.totalDurationNs')/1000000.0) elapsed_ms,
    json_extract(u.payload_json,'$.systemCpuPercent') cpu,
    NULL estimate_micros, 0 settled_micros
  FROM execution_events e JOIN execution_runs r ON r.id=e.run_id
  JOIN conversations v ON v.id=r.conversation_id AND v.workspace_id=r.workspace_id
  LEFT JOIN execution_events u ON u.run_id=e.run_id AND u.kind='local_model_usage'
    AND json_extract(u.payload_json,'$.localCallId')=json_extract(e.payload_json,'$.localCallId')
  LEFT JOIN execution_events f ON f.run_id=e.run_id AND f.kind='response_completed'
    AND (json_extract(e.payload_json,'$.localCallId') IS NULL OR v.purpose='benchmark')
    AND r.model_id=json_extract(e.payload_json,'$.target.model')
  WHERE r.workspace_id=?1 AND e.kind='model_request_started' AND unixepoch(e.occurred_at)>?2
    AND json_extract(e.payload_json,'$.target.provider')='ollama'
), usage AS (
  SELECT provider,model,capability,state,input_tokens,output_tokens,vector_count,elapsed_ms,NULL cpu,estimate_micros,settled_micros
    FROM model_cloud_calls WHERE workspace_id=?1 AND started_unix>?2
  UNION ALL SELECT * FROM local_generation
  UNION ALL SELECT 'ollama',model,'embedding',state,input_tokens,NULL,vector_count,elapsed_ms,NULL,NULL,0
    FROM model_local_embeddings WHERE workspace_id=?1 AND started_unix>?2
  UNION ALL SELECT 'local',json_extract(context_manifest_json,'$.retrieval.rerankerTarget.model'),
      'reranking','completed',NULL,NULL,NULL,json_extract(stage.value,'$.elapsedMicros')/1000.0,NULL,NULL,0
    FROM execution_runs r,json_each(r.context_manifest_json,'$.retrieval.stages') stage
    WHERE r.workspace_id=?1 AND unixepoch(r.started_at)>?2 AND json_extract(stage.value,'$.kind')='rerank'
      AND json_extract(context_manifest_json,'$.retrieval.rerankerTarget.provider')='local'
)
SELECT provider,model,capability,COUNT(*),COUNT(*) FILTER(WHERE state='completed'),
    SUM(input_tokens),SUM(output_tokens),SUM(vector_count),AVG(elapsed_ms),SUM(elapsed_ms),MAX(cpu),
    SUM(settled_micros),SUM(estimate_micros) FILTER(WHERE settled_micros IS NULL),
    COUNT(*) FILTER(WHERE estimate_micros IS NULL AND settled_micros IS NULL),
    COUNT(input_tokens),COUNT(output_tokens)
FROM usage WHERE model IS NOT NULL GROUP BY provider,model,capability ORDER BY provider,model,capability";

fn usage(c: &Connection, workspace: &str, hours: u16) -> AppResult<Value> {
    require_workspace(c, workspace)?;
    if !matches!(hours, 24 | 168) {
        return Err("请选择最近 24 小时或最近 7 天。".into());
    }
    let timestamp = chrono::Utc::now().timestamp();
    let since = timestamp - i64::from(hours) * 3600;
    let mut query = c.prepare(ROLLUP).map_err(error)?;
    let rows=query.query_map(params![workspace,since],|r|Ok(json!({
        "provider":r.get::<_,String>(0)?,"model":r.get::<_,String>(1)?,"capability":r.get::<_,String>(2)?,
        "requests":r.get::<_,i64>(3)?,"completed":r.get::<_,i64>(4)?,"inputTokens":r.get::<_,Option<i64>>(5)?,
        "outputTokens":r.get::<_,Option<i64>>(6)?,"vectors":r.get::<_,Option<i64>>(7)?,
        "averageMs":r.get::<_,Option<f64>>(8)?,"totalMs":r.get::<_,Option<f64>>(9)?,"systemCpuPeak":r.get::<_,Option<f64>>(10)?,
        "settledMicros":r.get::<_,Option<i64>>(11)?,"reservedMicros":r.get::<_,Option<i64>>(12)?,"unpriced":r.get::<_,i64>(13)?,
        "inputReported":r.get::<_,i64>(14)?,"outputReported":r.get::<_,i64>(15)?
    }))).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
    let (lookups,average):(i64,Option<f64>)=c.query_row(
        "SELECT count(*),AVG(json_extract(context_manifest_json,'$.retrieval.elapsedMicros')/1000.0)
         FROM execution_runs WHERE workspace_id=?1 AND unixepoch(started_at)>?2
           AND json_type(context_manifest_json,'$.retrieval.elapsedMicros') IN ('integer','real')",
        params![workspace,since],|r|Ok((r.get(0)?,r.get(1)?)),
    ).map_err(error)?;
    let (accounted, unknown): (i64, i64) = c
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(settled_micros,estimate_micros)),0),
          COUNT(*) FILTER (WHERE settled_micros IS NULL AND estimate_micros IS NULL)
         FROM model_cloud_calls WHERE workspace_id=?1 AND started_unix>?2",
            params![workspace, timestamp - 86400],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(error)?;
    let policy = Policy::read(c, workspace)?;
    Ok(
        json!({"periodHours":hours,"capturedAt":now(),"rows":rows,"retrieval":{"count":lookups,"averageMs":average},
        "budget":{"accountedMicros":accounted,"unpriced":unknown,"rollingDayMicros":policy.rolling_day_micros,
            "singleRunMicros":policy.single_run_micros}}),
    )
}

#[tauri::command]
pub async fn model_runtime_usage(
    state: State<'_, AppState>,
    workspace_id: String,
    hours: u16,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        usage(&*db.connection()?, &workspace_id, hours)
    })
    .await
}

#[tauri::command]
pub async fn model_runtime_resources() -> Value {
    let mut sampler = Sampler::new();
    sampler.system();
    let (gpu, _) = tokio::join!(
        sampler.gpu(),
        tokio::time::sleep(std::time::Duration::from_millis(200))
    );
    let system = sampler.system();
    json!({"sampledAt":now(),"cpuPercent":system["systemCpuPercent"],"systemMemory":system["systemMemory"],
        "gpus":gpu["devices"],"gpuUnavailable":gpu["reason"]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        models::{ModelSettings, new_id},
    };
    use std::sync::Arc;

    #[test]
    fn runtime_usage_report_counts_capabilities_and_keeps_unknown_costs_scoped() {
        let db = Arc::new(Database::rag_evaluation_database().unwrap());
        let ws = db.create_workspace("用量工作区").unwrap().id;
        let other = db.create_workspace("隔离工作区").unwrap().id;
        let chat = db.create_conversation(&ws).unwrap().id;
        let settings = ModelSettings {
            model: "local-text".into(),
            ..ModelSettings::default()
        };
        let run = db
            .prepare_run(&new_id(), &ws, &chat, "用量检查", &settings)
            .unwrap()
            .run;
        for (kind, payload) in [
            (
                "model_request_started",
                json!({"localCallId":"local-call","target":{"provider":"ollama","model":"local-text"}}),
            ),
            (
                "local_model_usage",
                json!({"localCallId":"local-call","status":"completed","inputTokens":12,"outputTokens":4,"elapsedMs":123,"systemCpuPercent":21.5}),
            ),
        ] {
            db.execution_event(&run, crate::models::ExecutionPurpose::Chat, kind, payload)
                .unwrap();
        }
        let meter =
            crate::runtime_usage::Meter::new(db.clone(), ws.clone(), new_id(), "local_index");
        meter
            .local_embedding("local-vector")
            .unwrap()
            .settle(Some(9), Some(8))
            .unwrap();
        let c = db.connection().unwrap();
        let timestamp = chrono::Utc::now().timestamp();
        for (state, estimate, settled) in [
            ("completed", Some(60), Some(40)),
            ("outcome_unknown", Some(75), None),
            ("failed", None, None),
        ] {
            c.execute("INSERT INTO model_cloud_calls (id,workspace_id,operation_id,purpose,connection_id,provider,
                model,capability,data_types_json,started_unix,state,estimate_micros,settled_micros,elapsed_ms)
                VALUES(?1,?2,'o','chat','c','openai','remote-rank','reranking','[]',?3,?4,?5,?6,25)",
                params![new_id(),ws,timestamp,state,estimate,settled]).unwrap();
        }
        let report = usage(&c, &ws, 24).unwrap();
        let rows = report["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        let row = |model: &str| rows.iter().find(|row| row["model"] == model).unwrap();
        assert_eq!(row("local-text")["outputTokens"], 4);
        assert_eq!(row("local-text")["averageMs"], 123.0);
        assert_eq!(row("local-vector")["vectors"], 8);
        assert_eq!(row("remote-rank")["requests"], 3);
        assert_eq!(row("remote-rank")["completed"], 1);
        assert_eq!(row("remote-rank")["reservedMicros"], 75);
        assert_eq!(row("remote-rank")["unpriced"], 1);
        assert_eq!(report["budget"]["accountedMicros"], 115);
        assert!(
            usage(&c, &other, 24).unwrap()["rows"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

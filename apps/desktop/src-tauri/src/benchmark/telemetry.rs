//! Measurement versioning is independent of immutable answer/grade versions.
use crate::{
    commands::blocking,
    db::{Database, error, json_error},
    models::{AppResult, RunSummary},
};
use rusqlite::params;
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;

pub use super::resources::hardware_snapshot;
const INTERVAL_MS: u64 = 1000;

pub struct Monitor {
    stop: watch::Sender<bool>,
    task: tauri::async_runtime::JoinHandle<AppResult<()>>,
}
impl Monitor {
    pub async fn start(database: Arc<Database>, run: RunSummary) -> AppResult<Self> {
        let started = Instant::now();
        let mut sampler = super::resources::Sampler::new();
        let mut baseline = sampler.system();
        let gpu = sampler.gpu().await;
        baseline["gpuProbe"] = json!(true);
        baseline["gpus"] = gpu["devices"].clone();
        baseline["gpuReason"] = gpu["reason"].clone();
        baseline["gpuReadCompletedMs"] = json!(started.elapsed().as_millis() as u64);
        write(database.clone(), run.clone(), 1, 0, baseline).await?;
        let (stop, mut stopped) = watch::channel(false);
        let task = tauri::async_runtime::spawn(async move {
            let mut seq = 1;
            let mut interval = tokio::time::interval_at(
                tokio::time::Instant::from_std(started) + Duration::from_millis(INTERVAL_MS),
                Duration::from_millis(INTERVAL_MS),
            );
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                let mut final_sample =
                    tokio::select! {biased;_=stopped.changed()=>true,_=interval.tick()=>false};
                let mut t = started.elapsed().as_millis() as u64;
                let mut values = sampler.system();
                if !final_sample {
                    let gpu = tokio::select! {biased;_=stopped.changed()=>None,gpu=sampler.gpu()=>Some(gpu)};
                    if let Some(gpu) = gpu {
                        values["gpuProbe"] = json!(true);
                        values["gpus"] = gpu["devices"].clone();
                        values["gpuReason"] = gpu["reason"].clone();
                        values["gpuReadCompletedMs"] = json!(started.elapsed().as_millis() as u64);
                    } else {
                        // Cancellation drops the bounded GPU child; keep the actual CPU/RAM observation.
                        t = started.elapsed().as_millis() as u64;
                        values = sampler.system();
                        values["gpuReason"] = json!("probe_cancelled_at_end");
                        final_sample = true;
                    }
                }
                seq += 1;
                write(database.clone(), run.clone(), seq, t, values).await?;
                if final_sample || *stopped.borrow() {
                    return Ok(());
                }
            }
        });
        Ok(Self { stop, task })
    }
    pub async fn finish(self) -> AppResult<()> {
        let _ = self.stop.send(true);
        self.task
            .await
            .map_err(|_| "资源采样任务异常。".to_string())?
    }
}
async fn write(
    db: Arc<Database>,
    run: RunSummary,
    seq: u32,
    t: u64,
    values: Value,
) -> AppResult<()> {
    blocking(db, move |db| {
        db.benchmark_resource_sample(&run, seq, t, values)
    })
    .await
}
impl Database {
    fn benchmark_resource_sample(
        &self,
        run: &RunSummary,
        seq: u32,
        t: u64,
        values: Value,
    ) -> AppResult<()> {
        let c = self.connection()?;
        let attempt:String=c.query_row("SELECT a.id FROM benchmark_attempts a JOIN execution_runs r ON r.id=a.execution_run_id JOIN conversations v ON v.id=r.conversation_id WHERE a.execution_run_id=?1 AND a.workspace_id=?2 AND r.workspace_id=?2 AND v.purpose='benchmark'",params![run.id,run.workspace_id],|r|r.get(0)).map_err(error)?;
        let availability = if values["systemMemory"].is_object() || values["gpus"].is_array() {
            "partial"
        } else {
            "unavailable"
        };
        let t = i64::try_from(t).map_err(|_| "采样时间超出范围。".to_string())?;
        c.execute("INSERT INTO benchmark_resource_samples(attempt_id,seq,workspace_id,t_rel_ms,source,scope,values_json,availability,sample_interval_ms) VALUES(?1,?2,?3,?4,'windows_api+nvidia_smi','system_and_whole_gpu',?5,?6,1000)",params![attempt,seq,run.workspace_id,t,values.to_string(),availability]).map_err(error)?;
        Ok(())
    }
    pub(crate) fn benchmark_resource_samples(
        &self,
        workspace: &str,
        attempt: &str,
    ) -> AppResult<Vec<Value>> {
        let c = self.connection()?;
        let mut stmt=c.prepare("SELECT seq,t_rel_ms,values_json,availability FROM benchmark_resource_samples WHERE workspace_id=?1 AND attempt_id=?2 ORDER BY seq").map_err(error)?;
        stmt.query_map(params![workspace,attempt],|r|Ok((r.get::<_,u32>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).map_err(error)?
            .map(|r|{let(seq,t,values,availability)=r.map_err(error)?;Ok(json!({"seq":seq,"tRelMs":t,"values":serde_json::from_str::<Value>(&values).map_err(json_error)?,"availability":availability,"sampleIntervalMs":INTERVAL_MS,"scope":"system_and_whole_gpu","source":"windows_api+nvidia_smi"}))}).collect()
    }
    pub(crate) fn benchmark_case_timing(
        &self,
        workspace: &str,
        id: &str,
        timing: Value,
    ) -> AppResult<()> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let s = super::repository::summary(&tx, workspace, id)?;
        super::repository::benchmark_event(
            &tx,
            id,
            workspace,
            &s.attempt_id,
            "case_timing_recorded",
            timing,
        )?;
        tx.commit().map_err(error)
    }
}

pub fn resource_summary(samples: &[Value]) -> Value {
    let max = |path: &str| {
        samples
            .iter()
            .filter_map(|s| s["values"].pointer(path).and_then(Value::as_f64))
            .reduce(f64::max)
    };
    let cpu_n = samples
        .iter()
        .filter(|s| s["values"]["systemCpuPercent"].is_number())
        .count();
    let memory_n = samples
        .iter()
        .filter(|s| s["values"]["systemMemory"]["usedBytes"].is_number())
        .count();
    let window = samples.last().and_then(|s| s["tRelMs"].as_u64());
    let max_gap = samples
        .windows(2)
        .filter_map(|s| {
            s[1]["tRelMs"]
                .as_u64()?
                .checked_sub(s[0]["tRelMs"].as_u64()?)
        })
        .max();
    let mut devices: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    let mut gpu_probes = 0;
    for s in samples {
        if s["values"]["gpuProbe"] == true {
            gpu_probes += 1;
        }
        if let Some(gpus) = s["values"]["gpus"].as_array() {
            for gpu in gpus {
                if let Some(id) = gpu["uuid"].as_str() {
                    devices.entry(id.into()).or_default().push(gpu.clone());
                }
            }
        }
    }
    let gpus:Vec<_>=devices.into_iter().map(|(id,rows)|json!({"uuid":id,"samples":rows.len(),"memorySamples":rows.iter().filter(|v|v["usedBytes"].is_number()).count(),"usedBytesPeak":rows.iter().filter_map(|v|v["usedBytes"].as_u64()).max(),"totalBytes":rows.last().map(|v|v["totalBytes"].clone()),"driverVersion":rows.last().map(|v|v["driverVersion"].clone())})).collect();
    json!({"sampleCount":samples.len(),"sampleIntervalMs":INTERVAL_MS,"windowMs":window,"maxGapMs":max_gap,
        "expectedPeriodicSamples":window.map(|w|w/INTERVAL_MS),"cpuMissingSamples":samples.len()-cpu_n,"memoryMissingSamples":samples.len()-memory_n,
        "systemCpuPercentPeak":max("/systemCpuPercent"),"systemCpuSamples":cpu_n,
        "systemMemoryBytesPeak":max("/systemMemory/usedBytes"),"systemMemorySamples":memory_n,
        "appWorkingSetBytesPeak":max("/appWorkingSetBytes"),"gpus":gpus,"gpuProbes":gpu_probes,
        "gpuReasons":samples.iter().filter(|s|s["values"]["gpuProbe"]==true).filter_map(|s|s["values"]["gpuReason"].as_str()).collect::<std::collections::BTreeSet<_>>(),
        "scope":"system_and_whole_gpu_not_model_exclusive","peakKind":"sampled_not_continuous","availability":if samples.is_empty(){"unavailable"}else{"partial"},"reason":if samples.is_empty(){Some("not_collected")}else{None}})
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite() && *v >= 0.)
}
pub fn measure(raw: &Value, status: &str, logical: Option<&Value>, resources: Value) -> Value {
    let complete = status == "completed";
    let response = ["responseMs", "completeMs", "totalMs"]
        .iter()
        .find_map(|key| number(&raw["clientTiming"][key]));
    let rate = raw["evalCount"]
        .as_u64()
        .zip(raw["evalDurationNs"].as_u64())
        .filter(|(_, d)| *d > 0)
        .map(|(n, d)| n as f64 * 1e9 / d as f64);
    let ns = |key: &str| number(&raw[key]).map(|v| v / 1e6);
    json!({"version":1,"completed":complete,"responseMs":response,"firstTextMs":number(&raw["clientTiming"]["firstTextMs"]),"firstToolMs":number(&raw["clientTiming"]["firstToolMs"]),
        "loadMs":ns("loadDurationNs"),"promptMs":ns("promptEvalDurationNs"),"generationMs":ns("evalDurationNs"),"serverTotalMs":ns("totalDurationNs"),
        "inputTokens":raw["promptEvalCount"].as_u64(),"outputTokens":raw["evalCount"].as_u64(),"cachedInputTokens":raw["promptEvalCachedCount"].as_u64(),
        "tokensPerSecond":if complete{rate}else{None},"loadObservation":raw["loadObservation"],"logicalTiming":logical,
        "timingSource":"client_monotonic_and_provider_terminal_fields","resources":resources,"collection":raw["resourceCollection"]})
}
pub fn distribution(mut values: Vec<f64>, planned: usize) -> Value {
    values.retain(|v| v.is_finite() && *v >= 0.);
    values.sort_by(f64::total_cmp);
    let quantile = |p: f64| {
        values
            .get(((values.len() as f64 * p).ceil() as usize).saturating_sub(1))
            .copied()
    };
    let median = values.get(values.len() / 2).map(|upper| {
        if values.len() % 2 == 0 {
            values[values.len() / 2 - 1] / 2. + upper / 2.
        } else {
            *upper
        }
    });
    json!({"n":values.len(),"missing":planned.saturating_sub(values.len()),"p50":median,"p95":quantile(0.95),"min":values.first(),"max":values.last()})
}
pub fn summarize(measures: &[Value], planned: usize) -> Value {
    let completed: Vec<_> = measures.iter().filter(|m| m["completed"] == true).collect();
    let dist = |key: &str| {
        distribution(
            completed.iter().filter_map(|m| number(&m[key])).collect(),
            planned,
        )
    };
    let sum = |key: &str| {
        let v: Vec<_> = measures.iter().filter_map(|m| m[key].as_u64()).collect();
        (
            if v.is_empty() {
                None
            } else {
                v.iter()
                    .try_fold(0u64, |total, count| total.checked_add(*count))
            },
            v.len(),
        )
    };
    let (input, input_n) = sum("inputTokens");
    let (output, output_n) = sum("outputTokens");
    let (cached, cached_n) = sum("cachedInputTokens");
    let pairs: Vec<_> = completed
        .iter()
        .filter_map(|m| {
            m["outputTokens"]
                .as_u64()
                .zip(number(&m["generationMs"]).filter(|d| *d > 0.))
        })
        .collect();
    let duration: f64 = pairs.iter().map(|(_, d)| d).sum();
    let elapsed: Vec<_> = measures
        .iter()
        .filter_map(|m| number(&m["responseMs"]))
        .collect();
    let states=["loaded","not_loaded","unknown"].map(|state|{let subset:Vec<_>=completed.iter().filter(|m|m["loadObservation"]["status"].as_str().unwrap_or("unknown")==state).collect();json!({"state":state,"n":subset.len(),"firstTextMs":distribution(subset.iter().filter_map(|m|number(&m["firstTextMs"])).collect(),subset.len()),"responseMs":distribution(subset.iter().filter_map(|m|number(&m["responseMs"])).collect(),subset.len())})});
    json!({"version":1,"measuredCases":elapsed.len(),"completedMeasuredCases":completed.iter().filter(|m|m["responseMs"].is_number()).count(),"elapsedMs":if elapsed.is_empty(){None}else{Some(elapsed.iter().sum::<f64>())},
        "inputTokensObserved":input,"inputTokenCases":input_n,"outputTokensObserved":output,"outputTokenCases":output_n,"cachedInputTokensObserved":cached,"cachedInputTokenCases":cached_n,
        "tokensPerSecond":if duration>0.{Some(pairs.iter().map(|(n,_)|*n as f64).sum::<f64>()*1000./duration)}else{None},"speedCases":pairs.len(),
        "firstTextMs":dist("firstTextMs"),"firstToolMs":dist("firstToolMs"),"responseMs":dist("responseMs"),"loadMs":dist("loadMs"),"promptMs":dist("promptMs"),"generationMs":dist("generationMs"),"generationSpeed":dist("tokensPerSecond"),
        "logicalMs":distribution(measures.iter().filter_map(|m|m.pointer("/logicalTiming/totalMs").and_then(number)).collect(),planned),
        "allResponsesMs":distribution(elapsed,planned),"byLoadState":states,"aggregation":"sample_median_and_nearest_rank_p95_completed_only;speed_paired_tokens_over_provider_generation_time",
        "resources":{"sampleCount":measures.iter().filter_map(|m|m["resources"]["sampleCount"].as_u64()).sum::<u64>(),
            "measuredCases":measures.iter().filter(|m|m["resources"]["sampleCount"].as_u64().unwrap_or(0)>0).count(),
            "systemCpuPercentPeak":measures.iter().filter_map(|m|number(&m["resources"]["systemCpuPercentPeak"])).reduce(f64::max),
            "systemMemoryBytesPeak":measures.iter().filter_map(|m|number(&m["resources"]["systemMemoryBytesPeak"])).reduce(f64::max),
            "gpus":merge_gpus(measures),"scope":"system_and_whole_gpu_not_model_exclusive","peakKind":"sampled_not_continuous"}})
}
fn merge_gpus(measures: &[Value]) -> Vec<Value> {
    let mut map: std::collections::BTreeMap<String, Value> = Default::default();
    for m in measures {
        if let Some(gpus) = m["resources"]["gpus"].as_array() {
            for gpu in gpus {
                if let Some(id) = gpu["uuid"].as_str() {
                    let row = map
                        .entry(id.into())
                        .or_insert_with(|| json!({"uuid":id,"usedBytesPeak":null,"samples":0}));
                    if gpu["usedBytesPeak"].as_u64() > row["usedBytesPeak"].as_u64() {
                        row["usedBytesPeak"] = gpu["usedBytesPeak"].clone();
                    }
                    row["samples"] = json!(
                        row["samples"].as_u64().unwrap_or(0)
                            + gpu["memorySamples"].as_u64().unwrap_or(0)
                    );
                }
            }
        }
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests;

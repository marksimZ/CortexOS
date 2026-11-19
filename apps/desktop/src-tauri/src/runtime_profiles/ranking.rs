use crate::{
    benchmark::dataset::hash,
    cloud::{
        privacy,
        runtime_connections::{Capability, CloudTarget},
    },
    models::{AppResult, ContextSelection, now},
    retrieval::stage,
    runtime_usage::Meter,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

type Key = (String, i64, String, String);
const CLIP_BYTES: usize = 4 * 1024;

pub(crate) struct CloudRanking {
    connection: String,
    model: String,
    provider: &'static str,
    scores: BTreeMap<Key, f64>,
    metadata: Value,
    elapsed_micros: u64,
}

fn snippet(item: &Value) -> Option<(Key, String)> {
    let node = item["nodeId"].as_str()?.to_owned();
    let version = item["version"].as_i64()?;
    let chunk = item["chunkId"].as_str().unwrap_or_default().to_owned();
    let title = item["title"].as_str()?;
    let content = item["chunkText"]
        .as_str()
        .or_else(|| item["content"].as_str())?;
    let mut text = format!("{title}\n{content}");
    text.truncate(text.floor_char_boundary(CLIP_BYTES));
    Some(((node, version, chunk, hash(text.as_bytes())), text))
}

pub(crate) fn permitted_inputs(
    c: &Connection,
    workspace: &str,
    target: &CloudTarget,
    candidates: &Value,
    allowed: &[ContextSelection],
) -> AppResult<Vec<(Key, String)>> {
    let mut selected = Vec::new();
    for item in candidates["candidates"].as_array().into_iter().flatten() {
        let Some((key, text)) = snippet(item) else {
            continue;
        };
        if allowed
            .iter()
            .any(|item| item.node_id == key.0 && item.version == key.1)
            && privacy::permitted(c, workspace, &target.connection.id, &key.0, key.1)?
            && target.check_external_text(&text).is_ok()
        {
            selected.push((key, text));
        }
    }
    Ok(selected)
}

pub(crate) async fn rerank(
    target: &CloudTarget,
    question: &str,
    inputs: Vec<(Key, String)>,
    timeout: Duration,
    meter: &Meter,
) -> CloudRanking {
    let clock = Instant::now();
    let connection = target.connection.id.clone();
    let model = target.model.clone();
    let provider = target.connection.provider.name();
    let mut metadata = json!({"provider":provider,"model":model,"connectionId":connection,"attempted":false,
        "status":"no_authorized_candidates","dataTypes":[],"documents":[]});
    let mut scores = BTreeMap::new();
    if !inputs.is_empty() {
        if let Err(reason) = target.check_external_text(question) {
            metadata["status"] = json!("privacy_blocked");
            metadata["reason"] = json!(reason);
        } else {
            let (keys, documents): (Vec<_>, Vec<_>) = inputs.into_iter().unzip();
            metadata["dataTypes"] = json!(["question", "granted_candidate_snippets"]);
            metadata["documents"] = json!(keys.iter().map(|(node,version,chunk,hash)|
                json!({"nodeId":node,"version":version,"chunkId":chunk,"contentHash":hash})).collect::<Vec<_>>());
            let versions: Vec<_> = keys
                .iter()
                .map(|(node, version, _, _)| privacy::TransferVersion {
                    node_id: node.clone(),
                    version: *version,
                    requires_grant: true,
                })
                .collect();
            match target
                .rerank(question, &documents, timeout, meter, &versions)
                .await
            {
                Ok((values, tokens)) => {
                    scores = keys.into_iter().zip(values).collect();
                    metadata["status"] = json!("completed");
                    metadata["inputTokens"] = json!(tokens);
                }
                Err(error) => {
                    metadata["status"] = json!("unavailable");
                    metadata["reason"] = json!(error);
                }
            }
        }
    }
    let attempts = meter.attempts(Capability::Reranking).ok();
    metadata["attempts"] = json!(attempts);
    metadata["attempted"] = json!(attempts.map(|value| value > 0));
    if attempts == Some(0) {
        metadata["dataTypes"] = json!([]);
        metadata["documents"] = json!([]);
    }
    CloudRanking {
        connection,
        model,
        provider,
        scores,
        metadata,
        elapsed_micros: clock.elapsed().as_micros() as u64,
    }
}

impl CloudRanking {
    pub(crate) fn apply(
        &self,
        c: &Connection,
        workspace: &str,
        report: &mut Value,
    ) -> AppResult<()> {
        let items = report["candidates"].as_array_mut().unwrap();
        let mut applied = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let Some((key, _)) = snippet(item) else {
                continue;
            };
            if let Some(score) = self.scores.get(&key)
                && privacy::permitted(c, workspace, &self.connection, &key.0, key.1)?
            {
                applied.push((index, *score));
            }
        }
        let count = applied.len();
        if count > 0 {
            let slots: Vec<_> = applied.iter().map(|(index, _)| *index).collect();
            applied.sort_by(|(left, a), (right, b)| b.total_cmp(a).then(left.cmp(right)));
            let sorted = applied
                .iter()
                .map(|(index, score)| {
                    let mut item = items[*index].clone();
                    item["rerankScore"] = json!(score);
                    item
                })
                .collect::<Vec<_>>();
            for (index, item) in slots.into_iter().zip(sorted) {
                items[index] = item;
            }
            report["reranker"] = json!(self.model);
            report["rerankerTarget"] =
                json!({"provider":self.provider,"model":self.model,"connectionId":self.connection});
        }
        let mut audit = self.metadata.clone();
        audit["appliedCount"] = json!(count);
        audit["elapsedMicros"] = json!(self.elapsed_micros);
        report["cloudRerank"] = audit;
        if self.metadata["attempted"] == true {
            stage(report, "rerank", self.elapsed_micros);
            report["elapsedMicros"] = json!(
                report["elapsedMicros"]
                    .as_u64()
                    .unwrap_or_default()
                    .saturating_add(self.elapsed_micros)
            );
            report["finishedAt"] = json!(now());
        }
        Ok(())
    }
}

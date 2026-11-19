#[cfg(test)]
use crate::models::new_id;
use crate::{
    commands::{AppState, blocking},
    db::{Database, error},
    model_runtime::{EmbeddingInput, EmbeddingProvider, ModelIdentity, RankingInput, RerankerProvider, TermCoverage, normalized_embeddings},
    models::{AppResult, now},
    ollama::Ollama,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};
use tauri::State;

pub(crate) mod cloud;
#[cfg(test)]
mod performance_tests;

const LEGACY_INDEX_VERSION: &str = "approved-knowledge-v1";
const INDEX_VERSION: &str = "approved-knowledge-chunks-v2";
pub const ALGORITHM: &str = "bm25-ollama-rrf-v2";
const BATCH: usize = 8;
const MAX_ROWS: usize = 2000;
const MAX_INPUT_BYTES: usize = 8 * 1024;
const KNOWLEDGE: &str = "FROM rag_current_knowledge n JOIN knowledge_versions v ON v.node_id=n.node_id AND v.version=n.version";
const ELIGIBLE: &str = "WHERE n.workspace_id=?1 AND NOT EXISTS(SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id WHERE l.node_id=n.node_id AND l.knowledge_version=v.version AND s.enabled=0)";
const VECTOR_JOIN: &str = "JOIN rag_vectors r ON r.workspace_id=n.workspace_id AND r.node_id=n.node_id AND r.version=v.version AND r.index_id=?2 LEFT JOIN rag_chunks ch ON ch.workspace_id=r.workspace_id AND ch.node_id=r.node_id AND ch.version=r.version AND ch.id=r.chunk_id";
const LIVE_VECTOR: &str = "(ch.id IS NOT NULL OR (r.chunk_id='' AND NOT EXISTS(
    SELECT 1 FROM rag_chunks current WHERE current.workspace_id=n.workspace_id AND current.node_id=n.node_id AND current.version=v.version)))";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    id: String,
    model: String,
    digest: String,
    dimensions: usize,
    pub(crate) rerank: bool,
    index_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloud: Option<cloud::Identity>,
}

impl Profile {
    pub(crate) fn model(&self) -> &str { &self.model }
}

pub(crate) struct Query {
    pub(crate) requested_at: String,
    pub(crate) profile: Profile,
    result: AppResult<Vec<f32>>,
    elapsed_micros: u64,
    started_at: String,
    cloud_transfer: Option<Value>,
}

#[derive(Debug, PartialEq, Eq)]
struct Document {
    node: String,
    version: i64,
    chunk: String,
    text: String,
}

impl Document {
    fn content_hash(&self) -> String {
        crate::benchmark::dataset::hash(self.text.as_bytes())
    }
}

fn reusable_local_vectors(
    c: &Connection,
    workspace: &str,
    model: &str,
    digest: &str,
) -> AppResult<BTreeMap<String, Vec<f32>>> {
    let mut stmt = c.prepare(
        "SELECT v.content_hash,v.vector,p.dimensions
         FROM rag_workspace_settings s
         JOIN rag_index_profiles p ON p.workspace_id=s.workspace_id AND p.id=s.index_id
         JOIN rag_vectors v ON v.workspace_id=p.workspace_id AND v.index_id=p.id
         WHERE s.workspace_id=?1 AND p.model=?2 AND p.model_digest=?3 AND p.index_version=?4
         ORDER BY v.node_id,v.version,v.chunk_id LIMIT ?5",
    ).map_err(error)?;
    let mut rows = stmt.query(params![workspace, model, digest, INDEX_VERSION, MAX_ROWS as i64]).map_err(error)?;
    let mut cached = BTreeMap::new();
    while let Some(row) = rows.next().map_err(error)? {
        let bytes: Vec<u8> = row.get(1).map_err(error)?;
        let dimensions: u16 = row.get(2).map_err(error)?;
        if bytes.len() != usize::from(dimensions) * 4 {
            continue;
        }
        let vector: Vec<f32> = bytes.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        let norm_sq = vector.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>();
        if (norm_sq - 1.0).abs() < 0.001 {
            cached.entry(row.get::<_, String>(0).map_err(error)?).or_insert(vector);
        }
    }
    Ok(cached)
}

fn documents(c: &Connection, workspace: &str) -> AppResult<Vec<Document>> {
    select_documents(c, workspace, None)
}

fn select_documents(c: &Connection, workspace: &str, cloud: Option<(&str, &str)>) -> AppResult<Vec<Document>> {
    let filter = if cloud.is_some() {
        "AND EXISTS (SELECT 1 FROM model_cloud_knowledge g
           JOIN json_each(?5) picked ON g.node_id=json_extract(picked.value,'$.nodeId')
             AND g.version=json_extract(picked.value,'$.version')
           WHERE g.workspace_id=n.workspace_id AND g.node_id=n.node_id AND g.version=v.version AND g.connection_id=?4)"
    } else { "" };
    let sql = format!(
        "SELECT n.node_id,v.version,v.title,COALESCE(ch.body,v.content),COALESCE(ch.id,'')
         {KNOWLEDGE} LEFT JOIN rag_chunks ch ON ch.workspace_id=n.workspace_id AND ch.node_id=n.node_id AND ch.version=v.version
         {ELIGIBLE} AND length(CAST(v.title AS BLOB))+length(CAST(COALESCE(ch.body,v.content) AS BLOB))<?2 {filter}
         ORDER BY n.node_id,ch.id LIMIT ?3"
    );
    let mut stmt = c.prepare(&sql).map_err(error)?;
    let max_bytes = MAX_INPUT_BYTES as i64;
    let max_rows = (MAX_ROWS+1) as i64;
    let mut values: Vec<&dyn rusqlite::ToSql> = vec![&workspace, &max_bytes, &max_rows];
    if let Some((ref id, ref scope)) = cloud {
        values.extend([id as &dyn rusqlite::ToSql, scope]);
    }
    stmt.query_map(
            values.as_slice(),
            |r| {
                let title: String = r.get(2)?;
                let content: String = r.get(3)?;
                Ok(Document {
                    node: r.get(0)?,
                    version: r.get(1)?,
                    chunk: r.get(4)?,
                    text: format!("{title}\n{content}"),
                })
            },
        )
        .map_err(error)?
        .collect::<Result<_, _>>()
        .map_err(error)
}

fn active(c: &Connection, workspace: &str) -> AppResult<Option<Profile>> {
    c.query_row("SELECT p.id,p.model,p.model_digest,p.dimensions,s.rerank,p.index_version FROM rag_workspace_settings s JOIN rag_index_profiles p ON p.workspace_id=s.workspace_id AND p.id=s.index_id WHERE s.workspace_id=?1 AND s.enabled=1 AND p.index_version IN (?2,?3)",params![workspace,INDEX_VERSION,LEGACY_INDEX_VERSION], |r| Ok(Profile{id:r.get(0)?, model:r.get(1)?, digest:r.get(2)?, dimensions:r.get::<_,u16>(3)? as usize, rerank:r.get(4)?,index_version:r.get(5)?,cloud:None})).optional().map_err(error)
}

pub(crate) fn usable_profile(c: &Connection, workspace: &str) -> AppResult<Option<Profile>> {
    let Some(profile) = active(c, workspace)? else {
        return Ok(None);
    };
    let has_vectors: bool = c.query_row(
        &format!("SELECT EXISTS(SELECT 1 {KNOWLEDGE} {VECTOR_JOIN} {ELIGIBLE} AND {LIVE_VECTOR})"),
        params![workspace,profile.id],
        |r|r.get(0),
    ).map_err(error)?;
    Ok(has_vectors.then_some(profile))
}

fn digest_matches(metadata: &Value, model: &str, digest: &str) -> bool {
    metadata["models"].as_array().is_some_and(|models| {
        models.iter().any(|item| {
            item["name"] == model
                && item["digest"]
                    .as_str()
                    .is_some_and(|d| d.trim_start_matches("sha256:") == digest)
        })
    })
}

async fn check_model(provider: &Ollama, model: &str, digest: &str) -> AppResult<()> {
    let metadata = provider.show(model).await?;
    if !crate::ollama::is_local_gguf(&metadata)
        || crate::local_runtime::capability(&metadata, "embedding") != Some(true)
        || !digest_matches(&provider.get("/api/tags").await?, model, digest)
    {
        return Err("该本地模型不支持向量检索，或模型版本已经变化。".into());
    }
    Ok(())
}

impl EmbeddingProvider for Ollama {
    async fn embed(&self, request: EmbeddingInput<'_>) -> AppResult<Vec<Vec<f32>>> {
        check_model(self, request.model, request.revision).await?;
        let ticket = request.meter.map(|meter|meter.local_embedding(request.model)).transpose()?;
        let result = self.embeddings(request.model, request.input).await;
        let tokens = result.as_ref().ok().and_then(|value|value["prompt_eval_count"].as_u64());
        let result = result.and_then(|value|normalized_embeddings(&value,request.input.len(),request.dimensions));
        if let Some(ticket) = ticket {
            ticket.settle(tokens,result.as_ref().ok().map(Vec::len))?;
        }
        let result = result?;
        if !digest_matches(&self.get("/api/tags").await?, request.model, request.revision) {
            return Err("向量生成期间模型版本变化，请重新构建索引。".into());
        }
        Ok(result)
    }
}

impl Database {
    pub(crate) fn install_evaluation_vectors(
        &self,
        workspace: &str,
        snapshot_hash: &str,
        vectors: &BTreeMap<&str, Vec<f32>>,
    ) -> AppResult<()> {
        let documents = {
            let c = self.connection()?;
            documents(&c, workspace)?
        };
        let rows = documents.iter().map(|doc| vectors.get(doc.node.as_str()).cloned()
            .ok_or_else(|| "固定向量缺少资料编号。".to_owned())).collect::<AppResult<Vec<_>>>()?;
        self.install_rag_index(workspace, "rag-fixed-vectors", snapshot_hash, documents, rows)
    }

    pub(crate) fn rag_status(&self, workspace: &str) -> AppResult<Value> {
        let c = self.connection()?;
        crate::benchmark::dataset::require_workspace(&c, workspace)?;
        let configured = c.query_row("SELECT p.id,p.model,p.model_digest,p.dimensions,s.enabled,s.rerank,p.index_version FROM rag_workspace_settings s JOIN rag_index_profiles p ON p.workspace_id=s.workspace_id AND p.id=s.index_id WHERE s.workspace_id=?1 AND p.index_version IN (?2,?3)",params![workspace,INDEX_VERSION,LEGACY_INDEX_VERSION],|r|Ok(json!({"id":r.get::<_,String>(0)?,"model":r.get::<_,String>(1)?,"digest":r.get::<_,String>(2)?,"dimensions":r.get::<_,u16>(3)?,"enabled":r.get::<_,bool>(4)?,"rerank":r.get::<_,bool>(5)?,"indexVersion":r.get::<_,String>(6)?}))).optional().map_err(error)?;
        let eligible: i64 = c
            .query_row(
                &format!("SELECT count(*) {KNOWLEDGE} {ELIGIBLE}"),
                [workspace],
                |r| r.get(0),
            )
            .map_err(error)?;
        let indexed:i64 = match &configured {
            Some(value)=>c.query_row(&format!("SELECT count(DISTINCT n.node_id) {KNOWLEDGE} {VECTOR_JOIN} {ELIGIBLE} AND {LIVE_VECTOR}"),params![workspace,value["id"].as_str()],|r|r.get(0)).map_err(error)?,
            None=>0,
        };
        Ok(
            json!({"index":configured,"eligible":eligible,"indexed":indexed,"indexVersion":INDEX_VERSION}),
        )
    }

    fn install_rag_index(
        &self,
        workspace: &str,
        model: &str,
        digest: &str,
        docs: Vec<Document>,
        vectors: Vec<Vec<f32>>,
    ) -> AppResult<()> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        if docs != documents(&tx, workspace)? {
            return Err("构建期间有效知识发生变化。旧索引已保留，请重新构建。".into());
        }
        let dimensions = vectors.first().ok_or("没有可保存的向量。")?.len();
        if docs.len() != vectors.len() || vectors.iter().any(|v| v.len() != dimensions) {
            return Err("索引向量数量或维度不匹配。".into());
        }
        let id = crate::benchmark::dataset::hash(
            json!(["ollama", model, digest, dimensions, "l2", INDEX_VERSION])
                .to_string()
                .as_bytes(),
        );
        tx.execute(
            "INSERT OR IGNORE INTO rag_index_profiles VALUES(?1,?2,'ollama',?3,?4,?5,'l2',?6,?7)",
            params![
                workspace,
                id,
                model,
                digest,
                dimensions as u16,
                INDEX_VERSION,
                now()
            ],
        )
        .map_err(error)?;
        {
            let current: BTreeSet<_> = docs.iter().map(|doc| (doc.node.as_str(), doc.version, doc.chunk.as_str())).collect();
            let mut existing = tx.prepare("SELECT node_id,version,chunk_id FROM rag_vectors WHERE workspace_id=?1 AND index_id=?2").map_err(error)?;
            let existing = existing.query_map(params![workspace, id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
            }).map_err(error)?.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
            let mut remove = tx.prepare("DELETE FROM rag_vectors WHERE workspace_id=?1 AND index_id=?2 AND node_id=?3 AND version=?4 AND chunk_id=?5").map_err(error)?;
            for (node, version, chunk) in existing {
                if !current.contains(&(node.as_str(), version, chunk.as_str())) {
                    remove.execute(params![workspace, id, node, version, chunk]).map_err(error)?;
                }
            }
            let mut upsert = tx.prepare(
                "INSERT INTO rag_vectors VALUES(?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(workspace_id,index_id,node_id,version,chunk_id) DO UPDATE SET
                   content_hash=excluded.content_hash,vector=excluded.vector
                 WHERE rag_vectors.content_hash<>excluded.content_hash OR rag_vectors.vector<>excluded.vector",
            ).map_err(error)?;
            for (doc, vector) in docs.iter().zip(vectors) {
                let bytes: Vec<u8> = vector.into_iter().flat_map(f32::to_le_bytes).collect();
                upsert.execute(params![workspace, id, doc.node, doc.version, doc.content_hash(), bytes, doc.chunk]).map_err(error)?;
            }
        }
        tx.execute("INSERT INTO rag_workspace_settings VALUES(?1,?2,0,0) ON CONFLICT(workspace_id) DO UPDATE SET index_id=excluded.index_id WHERE rag_workspace_settings.index_id<>excluded.index_id",params![workspace,id]).map_err(error)?;
        tx.commit().map_err(error)
    }
}

pub(crate) fn evaluation_query(c: &Connection, workspace: &str, input: Vec<f32>) -> AppResult<Query> {
    let profile = usable_profile(c, workspace)?.ok_or("固定向量索引不可用。")?;
    if input.len() != profile.dimensions {
        return Err("固定查询向量维度错误。".into());
    }
    let timestamp = now();
    Ok(Query { requested_at: timestamp.clone(), profile, result: Ok(input), elapsed_micros: 0, started_at: timestamp, cloud_transfer:None })
}

pub(crate) async fn query(
    provider: &impl EmbeddingProvider,
    profile: Profile,
    text: String,
    requested_at: String,
    meter: &crate::runtime_usage::Meter,
) -> Query {
    let started_at = now();
    let clock = Instant::now();
    let result = if text.len() <= MAX_INPUT_BYTES {
        provider.embed(EmbeddingInput { model: &profile.model, revision: &profile.digest, input: &[text], dimensions: Some(profile.dimensions),meter:Some(meter) })
        .await
        .map(|mut rows| rows.remove(0))
    } else {
        Err("问题过长，已改用词语查找。".into())
    };
    Query {
        profile,
        result,
        elapsed_micros: clock.elapsed().as_micros() as u64,
        started_at,
        requested_at,
        cloud_transfer: None,
    }
}

pub(crate) fn search(c: &Connection, workspace: &str, text: &str, query: Option<&Query>) -> AppResult<Value> {
    search_for_runtime(c, workspace, text, query, None)
}

pub(crate) fn search_for_runtime(
    c: &Connection, workspace: &str, text: &str, query: Option<&Query>, rerank: Option<bool>,
) -> AppResult<Value> {
    let mut report = search_candidates(c, workspace, text, query)?;
    let rank = rerank.unwrap_or_else(|| {
        query.is_some_and(|query| query.profile.rerank) && report["semanticStatus"] == "completed"
    });
    if rank {
        rank_local(text, &mut report)?;
    }
    report["candidates"].as_array_mut().unwrap().truncate(20);
    Ok(report)
}

fn search_candidates(
    c: &Connection,
    workspace: &str,
    text: &str,
    query: Option<&Query>,
) -> AppResult<Value> {
    let clock = Instant::now();
    let mut report = crate::retrieval::search(c, workspace, text)?;
    let Some(query) = query else {
        return Ok(report);
    };
    report["startedAt"] = json!(query.started_at);
    report["elapsedMicros"] = json!(
        report["elapsedMicros"]
            .as_u64()
            .unwrap_or_default()
            .saturating_add(query.elapsed_micros)
    );
    let profile = &query.profile;
    let cloud = profile.cloud.as_ref();
    report["embedding"] = json!({"provider":cloud.map_or("ollama",|identity|identity.provider.as_str()),
       "model":profile.model,"digest":cloud.is_none().then_some(&profile.digest),"dimensions":profile.dimensions,
       "indexVersion":profile.index_version,"indexId":profile.id,"normalization":"l2","elapsedMicros":query.elapsed_micros,
       "cloudTransfer":query.cloud_transfer});
    report["stages"].as_array_mut().unwrap().insert(0, json!({
        "kind":"prepare_vector","elapsedMicros":query.elapsed_micros,"available":query.result.is_ok()
    }));
    let current = match cloud {
        Some(identity) => cloud::active(c,workspace,&identity.connection_id,&profile.model)?,
        None => active(c, workspace)?,
    };
    if current.as_ref() != Some(profile) {
        report["semanticStatus"] = json!("index_changed");
        return Ok(report);
    }
    let q = match &query.result {
        Ok(v) => v,
        Err(reason) => {
            report["semanticStatus"] = json!("unavailable");
            report["semanticNote"] = json!(reason);
            return Ok(report);
        }
    };
    // ponytail: bounded exact cosine scan. Measure representative workspaces before adding ANN.
    let semantic_clock = Instant::now();
    let (vector_join, grants_filter) = if cloud.is_some() {
        (VECTOR_JOIN.replace("JOIN rag_vectors", "JOIN model_cloud_vectors"),
          "AND EXISTS (SELECT 1 FROM model_cloud_knowledge g WHERE g.workspace_id=r.workspace_id AND g.node_id=r.node_id
            AND g.version=r.version AND g.connection_id=(SELECT connection_id FROM model_cloud_indexes i WHERE i.workspace_id=r.workspace_id AND i.id=r.index_id))")
    } else { (VECTOR_JOIN.to_owned(), "") };
    let sql = format!(
        "SELECT n.node_id,v.version,v.title,v.kind,COALESCE(ch.body,v.content),r.vector,r.content_hash,r.chunk_id
         {KNOWLEDGE} {vector_join} {ELIGIBLE} AND {LIVE_VECTOR} {grants_filter}"
    );
    let mut statement = c.prepare(&sql).map_err(error)?;
    let mut rows = statement
        .query(params![workspace, query.profile.id])
        .map_err(error)?;
    struct Hit {
        score: f64,
        chunk: String,
        version: i64,
        title: String,
        kind: String,
        text: String,
    }
    let mut semantic = BTreeMap::<String, Hit>::new();
    while let Some(row) = rows.next().map_err(error)? {
        let rusqlite::types::ValueRef::Blob(bytes) = row.get_ref(5).map_err(error)? else {
            return Err("向量索引数据不是二进制向量。".into());
        };
        if bytes.len() != q.len() * 4 {
            continue;
        }
        let score: f64 = bytes
            .chunks_exact(4)
            .zip(q)
            .map(|(a, b)| f32::from_le_bytes(a.try_into().unwrap()) as f64 * *b as f64)
            .sum();
        if score.is_finite() && score > 0.0 && score <= 1.001 {
            let id: String = row.get(0).map_err(error)?;
            let chunk_id: String = row.get(7).map_err(error)?;
            if semantic.get(&id).is_some_and(|best| {
                best.score > score || (best.score == score && best.chunk <= chunk_id)
            }) {
                continue;
            }
            let title: String = row.get(2).map_err(error)?;
            let text: String = row.get(4).map_err(error)?;
            let digest: String = row.get(6).map_err(error)?;
            if digest != crate::benchmark::dataset::hash(format!("{title}\n{text}").as_bytes()) {
                continue;
            }
            semantic.insert(id, Hit {
                score,
                chunk: chunk_id,
                version: row.get(1).map_err(error)?,
                title,
                kind: row.get(3).map_err(error)?,
                text,
            });
        }
    }
    let mut semantic: Vec<_> = semantic.into_iter().collect();
    semantic.sort_by(|a, b| b.1.score.total_cmp(&a.1.score).then(a.0.cmp(&b.0)));
    let capped = semantic.len() > 20;
    let mut content_query = c.prepare_cached(
        "SELECT v.content FROM knowledge_versions v JOIN knowledge_nodes n ON n.id=v.node_id
         WHERE n.workspace_id=?1 AND v.node_id=?2 AND v.version=?3",
    ).map_err(error)?;
    let semantic = semantic.into_iter().take(20).map(|(id, hit)| {
        let content: String = if hit.chunk.is_empty() {
            hit.text.clone()
        } else {
            content_query.query_row(params![workspace, id, hit.version], |r| r.get(0)).map_err(error)?
        };
        let mut item = json!({"nodeId":id,"version":hit.version,"title":hit.title,"kind":hit.kind,"content":content,"included":false,"reason":"pending_budget","costUnits":0});
        if !hit.chunk.is_empty() {
            item["chunkId"] = json!(hit.chunk);
            item["chunkText"] = json!(hit.text);
        }
        Ok((id, hit.chunk, hit.score, item))
    }).collect::<AppResult<Vec<_>>>()?;
    crate::retrieval::stage(&mut report, "semantic_lookup", semantic_clock.elapsed().as_micros() as u64);
    let fusion_clock = Instant::now();
    let mut union = BTreeMap::<(String, String), Value>::new();
    for mut item in std::mem::take(report["candidates"].as_array_mut().unwrap()) {
        item["lexicalScore"] = item["score"].clone();
        let node = item["nodeId"].as_str().unwrap().to_owned();
        let chunk = item["chunkId"].as_str().unwrap_or_default().to_owned();
        union.insert((node, chunk), item);
    }
    for (i, (id, chunk, score, item)) in semantic.into_iter().enumerate() {
        let item = union.entry((id, chunk)).or_insert(item);
        item["semanticRank"] = json!(i + 1);
        item["semanticScore"] = json!(score);
    }
    let mut fused: Vec<_> = union.into_values().collect();
    for item in &mut fused {
        let rank = |k| item[k].as_u64().map_or(0., |r| 1. / (60. + r as f64));
        item["score"] = json!(rank("lexicalRank") + rank("semanticRank"));
    }
    fused.sort_by(|a, b| {
        b["score"]
            .as_f64()
            .unwrap()
            .total_cmp(&a["score"].as_f64().unwrap())
            .then(
                b["lexicalRank"]
                    .is_number()
                    .cmp(&a["lexicalRank"].is_number()),
            )
            .then(a["nodeId"].as_str().cmp(&b["nodeId"].as_str()))
    });
    let mut nodes = BTreeSet::new();
    fused.retain(|item| nodes.insert(item["nodeId"].as_str().unwrap().to_owned()));
    for (i, item) in fused.iter_mut().enumerate() {
        item["fusionRank"] = json!(i + 1);
    }
    crate::retrieval::stage(&mut report, "merge_rankings", fusion_clock.elapsed().as_micros() as u64);
    report["algorithm"] = json!(if cloud.is_some() { "bm25-cloud-rrf-v1" } else { ALGORITHM });
    report["semanticStatus"] = json!("completed");
    report["status"] = json!(if fused.is_empty() {
        "no_match"
    } else {
        "matched"
    });
    report["candidatesTruncated"] =
        json!(report["candidatesTruncated"] == true || capped || fused.len() > 20);
    report["candidates"] = json!(fused);
    report["finishedAt"] = json!(now());
    report["elapsedMicros"] =
        json!((clock.elapsed().as_micros() as u64).saturating_add(query.elapsed_micros));
    Ok(report)
}

fn rank_local(text: &str, report: &mut Value) -> AppResult<()> {
    let clock = Instant::now();
    let items = report["candidates"].as_array_mut().unwrap();
    let candidates: Vec<_> = items.iter().map(|item| (
        item["title"].as_str().unwrap(),
        item["chunkText"].as_str().or_else(|| item["content"].as_str()).unwrap(),
    )).collect();
    if candidates.is_empty() { return Ok(()); }
    let scores = TermCoverage.rerank(RankingInput { query: text, candidates: &candidates }).into_inner()?;
    for (item, score) in items.iter_mut().zip(scores) {
        item["rerankScore"] = json!(score as u64);
    }
    items.sort_by_key(|item| std::cmp::Reverse(item["rerankScore"].as_u64().unwrap()));
    report["reranker"] = json!(TermCoverage::MODEL);
    report["rerankerTarget"] = json!(ModelIdentity { provider:"local", model:TermCoverage::MODEL, revision:None });
    let elapsed = clock.elapsed().as_micros() as u64;
    crate::retrieval::stage(report, "rerank", elapsed);
    report["elapsedMicros"] = json!(report["elapsedMicros"].as_u64().unwrap_or_default().saturating_add(elapsed));
    report["finishedAt"] = json!(now());
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Build {
    pub(crate) model: String,
    pub(crate) digest: String,
    pub(crate) confirmed: bool,
}

#[tauri::command]
pub async fn rag_index_status(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.rag_status(&workspace_id)
    })
    .await
}

#[tauri::command]
pub async fn rag_embedding_models(state: State<'_, AppState>) -> AppResult<Value> {
    let provider = state.provider.clone()?;
    let catalog = provider.benchmark_models().await?;
    let mut eligible = vec![];
    for model in catalog.models.into_iter().take(32) {
        if let Some(digest) = model.digest {
            let meta = provider.show(&model.name).await;
            if let Ok(meta) = meta
                && crate::ollama::is_local_gguf(&meta)
                && crate::local_runtime::capability(&meta, "embedding") == Some(true)
            {
                eligible.push(json!({"name":model.name,"digest":digest}));
            }
        }
    }
    Ok(json!(eligible))
}

#[tauri::command]
pub async fn rag_build_index(
    state: State<'_, AppState>,
    workspace_id: String,
    request_id: Option<String>,
    webview: tauri::Webview,
    status_channel: Option<tauri::ipc::JavaScriptChannelId>,
    input: Build,
) -> AppResult<Value> {
    if !input.confirmed
        || input.model.is_empty()
        || input.model.len() > 200
        || input.digest.len() != 64
        || !input
            .digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("请选定本地向量模型并确认构建。".into());
    }
    let db = state.database.clone()?;
    let lease = state.index_lease(&workspace_id, request_id, status_channel.map(|id| id.channel_on(webview))).await?;
    let id = lease.id.clone();
    let meter=crate::runtime_usage::Meter::new(db.clone(),workspace_id.clone(),id,"local_index");
    build_local_index(db,workspace_id,&state.provider.clone()?,input,lease.receiver(),&meter).await
}

pub(crate) async fn build_local_index(
    db: std::sync::Arc<Database>,
    workspace_id: String,
    provider: &crate::ollama::Ollama,
    input: Build,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    meter: &crate::runtime_usage::Meter,
) -> AppResult<Value> {
    let scope = workspace_id.clone();
    let (model, digest) = (input.model.clone(), input.digest.clone());
    let (docs, mut vectors) = blocking(db.clone(), move |db| {
        let c = db.connection()?;
        crate::benchmark::dataset::require_workspace(&c, &scope)?;
        Ok((documents(&c, &scope)?, reusable_local_vectors(&c, &scope, &model, &digest)?))
    })
    .await?;
    if docs.is_empty() || docs.len() > MAX_ROWS {
        return Err("当前工作区需有 1 至 2000 条可索引的知识或文件片段。".into());
    }
    let hashes: Vec<_> = docs.iter().map(Document::content_hash).collect();
    let mut queued = BTreeSet::new();
    let pending: Vec<_> = docs.iter().zip(&hashes)
        .filter(|(_, hash)| !vectors.contains_key(*hash) && queued.insert(*hash))
        .collect();
    for batch in pending.chunks(BATCH) {
        let text = batch.iter().map(|(doc, _)| doc.text.clone()).collect::<Vec<_>>();
        let expected = vectors.values().next().map(Vec::len);
        let generated = tokio::select! {biased;_=cancel.wait_for(|v|*v)=>return Err("索引构建已取消。".into()),r=provider.embed(EmbeddingInput{model:&input.model,revision:&input.digest,input:&text,dimensions:expected,meter:Some(meter)})=>r?};
        vectors.extend(batch.iter().map(|(_, hash)| (*hash).clone()).zip(generated));
    }
    if pending.is_empty() {
        tokio::select! {
            biased;
            _ = cancel.wait_for(|v| *v) => return Err("索引构建已取消。".into()),
            result = check_model(provider, &input.model, &input.digest) => result?,
        }
    }
    let collected = hashes.iter().map(|hash| vectors.get(hash).cloned().ok_or("索引向量不完整。".to_owned())).collect::<AppResult<_>>()?;
    blocking(db, move |db| {
        if *cancel.borrow() {
            return Err("索引构建已取消。".into());
        }
        db.install_rag_index(&workspace_id, &input.model, &input.digest, docs, collected)?;
        db.rag_status(&workspace_id)
    })
    .await
}

#[tauri::command]
pub fn rag_cancel_index(state: State<'_, AppState>, workspace_id: String, request_id: Option<String>) -> AppResult<()> {
    state.cancel_rag_index(&workspace_id, request_id.as_deref())
}

#[tauri::command]
pub async fn rag_set_index_options(
    state: State<'_, AppState>,
    workspace_id: String,
    enabled: bool,
    rerank: bool,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        if db
            .connection()?
            .execute(
                "UPDATE rag_workspace_settings SET enabled=?2,rerank=?3 WHERE workspace_id=?1",
                params![workspace_id, enabled, rerank],
            )
            .map_err(error)?
            != 1
        {
            return Err("请先构建工作区索引。".into());
        }
        db.rag_status(&workspace_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        knowledge_scope::Request,
        knowledge::SaveKnowledge,
        models::{ContextSelection, ModelSettings},
        sources::{PendingSource, SourcePreview, SourceRef},
    };

    #[test]
    fn query_route_retains_private_lookup_and_records_fallback() {
        let path = std::env::temp_dir().join(format!("cortex-query-route-{}", new_id()));
        let db = Database::open(&path).unwrap();
        let workspace = db.create_workspace("路由验证").unwrap().id;
        let other = db.create_workspace("独立工作区").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let node = db.save_knowledge(SaveKnowledge {
            workspace_id: workspace.clone(), node_id: None, expected_revision: None,
            title: "OPS-421 项目报销规则".into(), kind: "knowledge".into(),
            content: "我们项目的报销限额为 360 元。".into(), change_reason: "验收".into(),
            source_message_id: None, source_excerpt: None,
        }).unwrap();
        db.approve_knowledge(&workspace, &node, 1).unwrap();
        let snapshot = json!(db.capture_knowledge_snapshot(&workspace, "批准版").unwrap());
        let scope = Request::Snapshot { snapshot_id: snapshot["id"].as_str().unwrap().into() };
        let docs = documents(&db.connection().unwrap(), &workspace).unwrap();
        db.install_rag_index(&workspace, "fixture", &"f".repeat(64), docs, vec![vec![1., 0.]]).unwrap();
        let set_index = |enabled, rerank| db.connection().unwrap().execute(
            "UPDATE rag_workspace_settings SET enabled=?2,rerank=?3 WHERE workspace_id=?1",
            params![workspace, enabled, rerank],
        ).unwrap();
        set_index(true, false);
        let ask = |text: &str, preview, query: Option<&Query>, scoped: Option<&Request>, selected: &[ContextSelection]| {
            db.prepare_context_run_with_retrieval(&new_id(), &workspace, &chat, text,
                &ModelSettings::default(), selected, preview, query, scoped, false)
        };
        for (message, reason) in [("你好！", "standalone_greeting"), ("？…", "no_searchable_text")] {
            let result = ask(message, false, None, None, &[]).unwrap();
            let report = &result.manifest["retrieval"];
            assert_eq!(report["route"]["reason"], reason);
            assert_eq!(report["route"]["usedMethod"], "none");
            assert!(report["candidates"].as_array().unwrap().is_empty());
            assert_eq!(db.run_detail(&workspace, &result.run.id).unwrap().events.len(), 2);
            db.finish(&result.run, "", "cancelled", None, json!({})).unwrap();
        }
        for preview in [true, false] {
            assert!(ask("  \t", preview, None, None, &[]).is_err());
        }
        let question = "你好，我们项目的报销限额是多少？";
        let with_profile = crate::retrieval::route(&db.connection().unwrap(), &workspace, question, true, false, false, false).unwrap();
        let mut query = Query {
            profile: with_profile.profile.unwrap(), result: Ok(vec![1., 0.]),
            elapsed_micros: 15, started_at: now(), requested_at: now(),
            cloud_transfer: None,
        };
        for (text, automatic, scoped, manual, permits_embedding) in [
            ("谢谢。", true, false, false, false), ("…", true, false, false, false),
            ("OPS-421", true, false, false, false), (question, false, false, false, false),
            (question, true, true, false, false), ("Hi!", true, false, true, true),
            ("税", true, false, false, true),
        ] {
            let c = db.connection().unwrap();
            let route = crate::retrieval::route(&c, &workspace, text, automatic, scoped, false, manual).unwrap();
            assert_eq!(route.profile.is_some(), permits_embedding, "{text}");
        }
        let preview = ask(question, true, None, None, &[]).unwrap();
        let plan = &preview.manifest["retrieval"]["route"];
        assert_eq!(plan["plannedMethod"], "hybrid");
        assert_eq!(plan["usedMethod"], "fts5");
        assert_eq!(plan["fallbackReason"], "preview");
        let versioned = ask(question, true, None, Some(&scope), &[]).unwrap();
        assert_eq!(versioned.manifest["retrieval"]["route"]["usedMethod"], "version_aware");
        let id = ask("OPS-421", true, None, None, &[]).unwrap();
        assert_eq!(id.manifest["retrieval"]["route"]["reason"], "identifier");
        assert!(id.request.to_string().contains("360 元"));

        let hybrid = ask(question, false, Some(&query), None, &[]).unwrap();
        assert_eq!(hybrid.manifest["retrieval"]["route"]["usedMethod"], "hybrid");
        assert!(hybrid.request.to_string().contains("360 元"));
        assert_eq!(db.run_detail(&workspace, &hybrid.run.id).unwrap().context_manifest["retrieval"]["route"],
            hybrid.manifest["retrieval"]["route"]);
        db.finish(&hybrid.run, "", "cancelled", None, json!({})).unwrap();
        set_index(false, false);
        let expired = ask(question, true, Some(&query), None, &[]).unwrap();
        assert_eq!(expired.manifest["retrieval"]["route"]["fallbackReason"], "index_changed");
        set_index(true, true);
        query.profile = usable_profile(&db.connection().unwrap(), &workspace).unwrap().unwrap();
        let reranked = ask(question, true, Some(&query), None, &[]).unwrap();
        assert_eq!(reranked.manifest["retrieval"]["route"]["usedMethod"], "hybrid_rerank");
        query.result = Err("fixture unavailable".into());
        let fallback = ask(question, true, Some(&query), None, &[]).unwrap();
        assert_eq!(fallback.manifest["retrieval"]["route"]["fallbackReason"], "unavailable");
        assert_eq!(fallback.manifest["retrieval"]["route"]["usedMethod"], "fts5");
        assert!(fallback.request.to_string().contains("360 元"));

        db.set_conversation_retrieval(&workspace, &chat, false).unwrap();
        let selected = [ContextSelection { node_id: node, version: 1 }];
        let disabled = ask(question, true, None, None, &selected).unwrap();
        assert_eq!(disabled.manifest["retrieval"]["route"]["reason"], "conversation_disabled");
        assert!(disabled.request.to_string().contains("360 元"));
        let isolated = db.prepare_context_run(&new_id(), &other, "", question, &ModelSettings::default(), &[], true).unwrap();
        assert_eq!(isolated.manifest["retrieval"]["route"]["reason"], "no_embedding_index");
        assert!(!isolated.request.to_string().contains("360 元"));
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rag_chunks_recall_and_expand_only_approved_text() {
        let path = std::env::temp_dir().join(format!("cortex-rag-chunks-{}", new_id()));
        let db = Database::open(&path).unwrap();
        let workspace = db.create_workspace("片段验证").unwrap().id;
        let other = db.create_workspace("隔离").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let excerpt = format!(
            "# 部署\n{}## 恢复\n{}Checkpoint 服务使用离线快照。\n相邻说明。",
            "常规部署说明\n".repeat(90),
            "检查已有记录\n".repeat(43)
        );
        let section_line = excerpt.lines().position(|line| line == "## 恢复").unwrap() + 1;
        let content = format!("{excerpt}\n## 未批准的原文\nprivate_marker");
        let hash = crate::benchmark::dataset::hash(content.as_bytes());
        let source = db
            .import_source(PendingSource {
                workspace: workspace.clone(),
                path: "handbook.md".into(),
                raw_hash: hash.clone(),
                created: Instant::now(),
                preview: SourcePreview {
                    ticket: new_id(),
                    name: "handbook.md".into(),
                    content,
                    content_hash: hash,
                    previous_version: None,
                    previous_revision: None,
                },
            })
            .unwrap();
        let draft = SaveKnowledge {
            workspace_id: workspace.clone(),
            node_id: None,
            expected_revision: None,
            title: "部署手册".into(),
            kind: "knowledge".into(),
            content: excerpt.clone(),
            change_reason: "验收".into(),
            source_message_id: None,
            source_excerpt: None,
        };
        let id = db
            .save_knowledge_with_evidence(
                draft.clone(),
                Some(SourceRef {
                    source_id: source.id.clone(),
                    version: 1,
                    start_line: 1,
                    end_line: excerpt.lines().count(),
                }),
            )
            .unwrap();
        db.approve_knowledge(&workspace, &id, 1).unwrap();
        let docs = {
            let c = db.connection().unwrap();
            assert!(documents(&c, &other).unwrap().is_empty());
            assert_eq!(
                crate::retrieval::search(&c, &workspace, "private_marker").unwrap()["status"],
                "no_match"
            );
            documents(&c, &workspace).unwrap()
        };
        assert!(docs.len() > 2);
        let hit = docs
            .iter()
            .find(|doc| doc.text.contains("Checkpoint"))
            .unwrap()
            .chunk
            .clone();
        let vectors = docs
            .iter()
            .map(|doc| {
                if doc.chunk == hit {
                    vec![1., 0.]
                } else {
                    vec![0., 1.]
                }
            })
            .collect();
        db.install_rag_index(&workspace, "fixture", &"f".repeat(64), docs, vectors)
            .unwrap();
        db.connection()
            .unwrap()
            .execute(
                "UPDATE rag_workspace_settings SET enabled=1 WHERE workspace_id=?1",
                [&workspace],
            )
            .unwrap();
        let profile = usable_profile(&db.connection().unwrap(), &workspace).unwrap().unwrap();
        let q = Query {
            profile,
            result: Ok(vec![1., 0.]),
            elapsed_micros: 0,
            started_at: now(),
            requested_at: now(),
            cloud_transfer: None,
        };
        for (prompt, lexical) in [("Checkpoint 离线快照", true), ("怎样渡过灾难", false)]
        {
            let prepared = db
                .prepare_context_run_with_retrieval(
                    &new_id(),
                    &workspace,
                    &chat,
                    prompt,
                    &ModelSettings::default(),
                    &[],
                    true,
                    Some(&q),
                    None,
                    false,
                )
                .unwrap();
            let candidates = prepared.manifest["retrieval"]["candidates"]
                .as_array()
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0]["chunkId"], hit);
            assert_eq!(candidates[0]["lexicalRank"].is_number(), lexical);
            assert_eq!(candidates[0]["semanticRank"], 1);
            assert_eq!(candidates[0]["sourceLocation"]["name"], "handbook.md");
            assert!(candidates[0]["sourceLocation"]["match"]["startLine"].is_number());
            let reference = &prepared.manifest["knowledgeRefs"][0];
            assert_eq!(
                reference["sourceChunk"]["sectionPath"],
                json!(["部署", "恢复"])
            );
            assert_eq!(reference["sourceEvidence"][0]["startLine"], section_line);
            assert_eq!(reference["sourceChunk"]["expanded"], true);
            assert!(
                reference["content"]
                    .as_str()
                    .unwrap()
                    .contains("Checkpoint")
            );
            assert!(!prepared.request.to_string().contains("private_marker"));
            assert!(!prepared.request.to_string().contains("常规部署说明"));
        }
        let limited = ModelSettings { num_ctx: 1024, num_predict: 128, ..ModelSettings::default() };
        let (budget, used) = crate::db::chat_budget(&limited, "Checkpoint").unwrap();
        let question = format!("Checkpoint{}", "?".repeat(budget - used - 30));
        let prepared = db.prepare_context_run(&new_id(), &workspace, &chat, &question, &limited, &[], true).unwrap();
        let report = &prepared.manifest["retrieval"];
        let excluded = &report["candidates"][0];
        assert_eq!(excluded["reason"], "auto_budget");
        assert_eq!(excluded["sourceLocation"]["version"], 1);
        let area = &excluded["sourceLocation"]["match"];
        assert!(area["startLine"].as_u64().unwrap() <= (section_line + 44) as u64);
        assert!(area["endLine"].as_u64().unwrap() >= (section_line + 44) as u64);
        assert_eq!(report["stages"][0]["kind"], "lexical_lookup");
        assert_eq!(report["stages"][2]["kind"], "select_context");
        assert!(prepared.manifest["knowledgeRefs"].as_array().unwrap().is_empty());
        assert_eq!(
            search(&db.connection().unwrap(), &other, "Checkpoint", Some(&q)).unwrap()["status"],
            "no_match"
        );
        let edit = SaveKnowledge {
            node_id: Some(id.clone()),
            expected_revision: Some(2),
            content: "新版采用远端备份。".into(),
            ..draft
        };
        db.save_knowledge(edit).unwrap();
        db.approve_knowledge(&workspace, &id, 3).unwrap();
        assert!(usable_profile(&db.connection().unwrap(), &workspace).unwrap().is_none());
        assert_eq!(
            search(
                &db.connection().unwrap(),
                &workspace,
                "Checkpoint",
                Some(&q)
            )
            .unwrap()["status"],
            "no_match"
        );
        assert_eq!(
            search(&db.connection().unwrap(), &workspace, "远端备份", None).unwrap()["candidates"]
                [0]["version"],
            2
        );
        db.revoke_source(&workspace, &source.id, source.revision)
            .unwrap();
        assert_eq!(
            search(&db.connection().unwrap(), &workspace, "远端备份", None).unwrap()["status"],
            "no_match"
        );
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
}

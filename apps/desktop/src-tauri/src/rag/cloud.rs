use super::{BATCH, Document, MAX_INPUT_BYTES, MAX_ROWS, Profile, Query, error, select_documents};
use crate::{
    benchmark::dataset::hash,
    cloud::runtime_connections::{self, Capability, CloudTarget},
    commands::{AppState, blocking},
    db::{Database, json_error},
    models::{AppResult, ContextSelection, new_id, now},
    runtime_usage::Meter,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::State;

const VERSION: &str = "approved-cloud-chunks-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Identity {
    pub(super) connection_id: String,
    pub(super) provider: String,
}

pub(super) fn active(
    c: &Connection,
    workspace: &str,
    connection: &str,
    model: &str,
) -> AppResult<Option<Profile>> {
    c.query_row(
        "SELECT id,provider,dimensions FROM model_cloud_indexes WHERE workspace_id=?1 AND connection_id=?2 AND model=?3",
        params![workspace, connection, model],
        |row| Ok(Profile {
            id:row.get(0)?,model:model.into(),digest:String::new(),dimensions:row.get::<_,u16>(2)? as usize,
            rerank:false,index_version:VERSION.into(),
            cloud:Some(Identity {connection_id:connection.into(),provider:row.get(1)?}),
        }),
    ).optional().map_err(error)
}

pub(crate) fn usable(
    c: &Connection,
    workspace: &str,
    connection: &str,
    model: &str,
) -> AppResult<Option<Profile>> {
    let Some(index) = active(c, workspace, connection, model)? else {
        return Ok(None);
    };
    Ok((current_count(c, workspace, connection, &index.id)? > 0).then_some(index))
}

fn current_count(c: &Connection, workspace: &str, connection: &str, id: &str) -> AppResult<i64> {
    c.query_row(
        "SELECT count(DISTINCT n.node_id) FROM model_cloud_vectors v JOIN rag_current_knowledge n
          ON n.workspace_id=v.workspace_id AND n.node_id=v.node_id AND n.version=v.version
        JOIN model_cloud_knowledge g ON g.workspace_id=v.workspace_id AND g.node_id=v.node_id
          AND g.version=v.version AND g.connection_id=?2
        WHERE v.workspace_id=?1 AND v.index_id=?3 AND NOT EXISTS(
          SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id
          WHERE l.node_id=v.node_id AND l.knowledge_version=v.version AND s.enabled=0)",
        params![workspace, connection, id],
        |r| r.get::<_, i64>(0),
    )
    .map_err(error)
}

fn status(c: &Connection, workspace: &str, connection: &str, model: &str) -> AppResult<Value> {
    runtime_connections::get(c, workspace, connection)?;
    let Some(index) = active(c, workspace, connection, model)? else {
        return Ok(json!({"index":null,"readyVersions":0}));
    };
    let (timestamp, raw): (String, String) = c.query_row(
        "SELECT created_at,metadata_json FROM model_cloud_indexes WHERE workspace_id=?1 AND id=?2",
        params![workspace,index.id], |r| Ok((r.get(0)?,r.get(1)?)),
    ).map_err(error)?;
    Ok(
        json!({"index":index,"readyVersions":current_count(c,workspace,connection,&index.id)?,
        "createdAt":timestamp,"build":serde_json::from_str::<Value>(&raw).map_err(json_error)?}),
    )
}

pub(crate) async fn query(
    target: &CloudTarget,
    profile: Profile,
    text: String,
    requested_at: String,
    timeout: Duration,
    meter: &Meter,
) -> Query {
    let started_at = now();
    let clock = Instant::now();
    let mut transfer = json!({"provider":target.connection.provider.name(),"model":target.model,
        "connectionId":target.connection.id,"dataTypes":["question"],"attempted":false});
    let result = if text.len() > MAX_INPUT_BYTES {
        Err("问题过长，已改用词语查找。".into())
    } else if let Err(error) = target.check_external_text(&text) {
        Err(error)
    } else {
        target
            .embed(&[text], None, timeout, meter, "question", &[])
            .await
            .and_then(|(mut rows, tokens)| {
                transfer["inputTokens"] = json!(tokens);
                let vector = rows.remove(0);
                if vector.len() != profile.dimensions {
                    return Err("云端模型的向量维度变化，请重建索引。".into());
                }
                Ok(vector)
            })
    };
    let attempts = meter.attempts(Capability::Embedding).ok();
    transfer["attempts"] = json!(attempts);
    transfer["attempted"] = json!(attempts.map(|value| value > 0));
    if attempts == Some(0) {
        transfer["dataTypes"] = json!([]);
    }
    transfer["status"] = json!(if result.is_ok() {
        "completed"
    } else {
        "unavailable"
    });
    Query {
        requested_at,
        profile,
        result,
        elapsed_micros: clock.elapsed().as_micros() as u64,
        started_at,
        cloud_transfer: Some(transfer),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BuildInput {
    connection_id: String,
    model: String,
    versions: Vec<ContextSelection>,
}

#[tauri::command]
pub async fn get_cloud_rag_index(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    model: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        status(&*db.connection()?, &workspace_id, &connection_id, &model)
    })
    .await
}

#[tauri::command]
pub async fn build_cloud_rag_index(
    state: State<'_, AppState>,
    workspace_id: String,
    request_id: Option<String>,
    webview: tauri::Webview,
    status_channel: Option<tauri::ipc::JavaScriptChannelId>,
    input: BuildInput,
) -> AppResult<Value> {
    let database = state.database.clone()?;
    let lease = state.index_lease(&workspace_id, request_id, status_channel.map(|id| id.channel_on(webview))).await?;
    let _guard = state
        .runtime_cloud
        .clone()
        .try_lock_owned()
        .map_err(|_| "另一项云端任务正在进行，请先停止或等待完成。")?;
    let workspace = workspace_id.clone();
    let target = blocking(database.clone(), move |db| {
        runtime_connections::resolve_target(
            &db,
            &workspace,
            &input.connection_id,
            &input.model,
            Capability::Embedding,
        )
    })
    .await?;
    build(
        database,
        workspace_id,
        target,
        input.versions,
        lease.receiver(),
        Duration::from_secs(30),
        lease.id.clone(),
    )
    .await
}

pub(crate) async fn build(
    database: Arc<Database>,
    workspace_id: String,
    target: CloudTarget,
    versions: Vec<ContextSelection>,
    cancel: tokio::sync::watch::Receiver<bool>,
    timeout: Duration,
    operation: String,
) -> AppResult<Value> {
    let meter = Meter::new(
        database.clone(),
        workspace_id.clone(),
        operation,
        "cloud_index",
    );
    build_metered(database, workspace_id, &target, versions, cancel, timeout, &meter).await
}

pub(crate) async fn build_metered(
    database: Arc<Database>,
    workspace_id: String,
    target: &CloudTarget,
    versions: Vec<ContextSelection>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    timeout: Duration,
    meter: &Meter,
) -> AppResult<Value> {
    let selected: BTreeSet<_> = versions
        .iter()
        .map(|version| (&version.node_id, version.version))
        .collect();
    if selected.is_empty() || selected.len() != versions.len() || selected.len() > MAX_ROWS {
        return Err("请选择 1 至 2000 个已授权的知识版本。".into());
    }
    let scope = serde_json::to_string(&versions).map_err(json_error)?;
    let (workspace, connection, approved) = (
        workspace_id.clone(),
        target.connection.id.clone(),
        scope.clone(),
    );
    let docs = blocking(database.clone(), move |db| {
        select_documents(
            &*db.connection()?,
            &workspace,
            Some((&connection, &approved)),
        )
    })
    .await?;
    let indexed: BTreeSet<_> = docs.iter().map(|doc| (&doc.node, doc.version)).collect();
    if docs.len() > MAX_ROWS || indexed != selected {
        return Err(
            "部分知识版本已失效、撤销授权或超出索引范围。请刷新授权列表并缩小构建范围。".into(),
        );
    }
    for doc in &docs {
        target.check_external_text(&doc.text)?;
    }
    let clock = Instant::now();
    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(docs.len());
    let mut tokens = Some(0u64);
    for batch in docs.chunks(BATCH) {
        {
            let c = database.connection()?;
            for doc in batch {
                if !crate::cloud::privacy::permitted(
                    &c,
                    &workspace_id,
                    &target.connection.id,
                    &doc.node,
                    doc.version,
                )? {
                    return Err("知识授权已变化，索引未替换。".into());
                }
            }
        }
        let texts: Vec<_> = batch.iter().map(|doc| doc.text.clone()).collect();
        let knowledge: Vec<_> = batch
            .iter()
            .map(|doc| crate::cloud::privacy::TransferVersion {
                node_id: doc.node.clone(),
                version: doc.version,
                requires_grant: true,
            })
            .collect();
        let (mut batch_vectors, usage) = tokio::select! {
            biased;
            _ = cancel.wait_for(|stopped|*stopped) => return Err("已停止构建云端索引。".into()),
            response = target.embed(&texts,None,timeout,meter,"granted_knowledge",&knowledge) => response?,
        };
        if let Some(expected) = vectors.first().map(Vec::len)
            && batch_vectors.iter().any(|vector| vector.len() != expected)
        {
            return Err("云端返回的向量维度不一致，索引未替换。".into());
        }
        tokens = tokens
            .zip(usage)
            .and_then(|(before, after)| before.checked_add(after));
        vectors.append(&mut batch_vectors);
    }
    let metadata = json!({"dataTypes":["granted_knowledge"],"inputTokens":tokens,
        "vectorCount":vectors.len(),"requests":meter.attempts(Capability::Embedding)?,"elapsedMs":clock.elapsed().as_millis() as u64,
        "versions":versions});
    let connection = target.connection.id.clone();
    let model = target.model.clone();
    let provider = target.connection.provider.name();
    blocking(database, move |db| {
        if *cancel.borrow() {
            return Err("已停止构建云端索引。".into());
        }
        install(
            &db,
            &workspace_id,
            (&connection, &model, provider),
            &scope,
            &docs,
            &vectors,
            metadata,
        )?;
        status(
            &*db.connection()?,
            &workspace_id,
            &connection,
            &model,
        )
    })
    .await
}

fn install(
    db: &Database,
    workspace: &str,
    (connection, model, provider): (&str, &str, &str),
    scope: &str,
    docs: &[Document],
    vectors: &[Vec<f32>],
    metadata: Value,
) -> AppResult<()> {
    let mut c = db.connection()?;
    let tx = c.transaction().map_err(error)?;
    if select_documents(&tx, workspace, Some((connection, scope)))? != docs {
        return Err("索引构建期间知识发生变化，旧索引已保留。".into());
    }
    let dimensions = vectors.first().ok_or("没有可保存的向量。")?.len();
    if docs.len() != vectors.len()
        || !(1..=4096).contains(&dimensions)
        || vectors.iter().any(|v| v.len() != dimensions)
    {
        return Err("返回的索引向量数量或维度不一致。".into());
    }
    let index = new_id();
    tx.execute(
        "DELETE FROM model_cloud_indexes WHERE workspace_id=?1 AND connection_id=?2 AND model=?3",
        params![workspace, connection, model],
    )
    .map_err(error)?;
    tx.execute(
        "INSERT INTO model_cloud_indexes VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            workspace,
            connection,
            model,
            index,
            provider,
            dimensions as i64,
            now(),
            metadata.to_string()
        ],
    )
    .map_err(error)?;
    for (doc, vector) in docs.iter().zip(vectors) {
        let bytes: Vec<u8> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
        tx.execute(
            "INSERT INTO model_cloud_vectors VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                workspace,
                index,
                doc.node,
                doc.version,
                doc.chunk,
                hash(doc.text.as_bytes()),
                bytes
            ],
        )
        .map_err(error)?;
    }
    tx.commit().map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{knowledge::SaveKnowledge, model_runtime::http_fixture, runtime_profiles::ranking};

    #[tokio::test]
    async fn cloud_rag_index_and_rerank_only_transmit_approved_content() {
        let db = Arc::new(Database::rag_evaluation_database().unwrap());
        let ws = db.create_workspace("向量工作区").unwrap().id;
        let other = db.create_workspace("独立工作区").unwrap().id;
        let make = |workspace: &str, body: &str| {
            let id = db
                .save_knowledge(SaveKnowledge {
                    workspace_id: workspace.to_owned(),
                    node_id: None,
                    expected_revision: None,
                    title: "本地资料".into(),
                    kind: "knowledge".into(),
                    content: body.into(),
                    change_reason: "验证".into(),
                    source_message_id: None,
                    source_excerpt: None,
                })
                .unwrap();
            db.approve_knowledge(workspace, &id, 1).unwrap();
            ContextSelection {
                node_id: id,
                version: 1,
            }
        };
        let alice = make(&ws, "granted-alice");
        let bob = make(&ws, "granted-bob");
        let private = make(&ws, "private-never-sent");
        let foreign = make(&other, "foreign-never-sent");
        let (url, server) = http_fixture::serve(3, |i, path, body| {
            assert_eq!(body["model"], "fixture");
            let serialized = body.to_string();
            assert!(!serialized.contains("never-sent"));
            match (i, path) {
                (0, "/embeddings") => {
                    let docs = body["input"].as_array().unwrap();
                    assert_eq!(docs.len(), 2);
                    assert!(
                        serialized.contains("granted-alice") && serialized.contains("granted-bob")
                    );
                    let data = docs.iter().enumerate().map(|(i,doc)| json!({
                        "index":i,"embedding":if doc.as_str().unwrap().contains("alice") {[1,0]} else {[0,1]},
                    })).collect::<Vec<_>>();
                    (200, json!({"data":data,"usage":{"prompt_tokens":36}}))
                }
                (1, "/embeddings") => {
                    assert_eq!(body["input"], json!(["question-only"]));
                    (
                        200,
                        json!({"data":[{"index":0,"embedding":[1,0]}],"usage":{"prompt_tokens":5}}),
                    )
                }
                (2, "/rerank") => {
                    assert_eq!(body["query"], "question-only");
                    let docs = body["documents"].as_array().unwrap();
                    assert_eq!(docs.len(), 2);
                    assert!(docs[0].as_str().unwrap().contains("alice"));
                    assert!(docs[1].as_str().unwrap().contains("bob"));
                    (
                        200,
                        json!({"results":[{"index":1,"relevance_score":0.98},{"index":0,"relevance_score":0.41}],
                        "meta":{"billed_units":{"input_tokens":20}}}),
                    )
                }
                _ => panic!("unexpected fixture path {path}"),
            }
        });
        let target = CloudTarget::loopback_test(url.clone(), "fixture");
        let connection = target.connection.id.clone();
        {
            let c = db.connection().unwrap();
            c.execute(
                "INSERT INTO model_cloud_connections VALUES(?1,?2,?3)",
                params![
                    ws,
                    connection,
                    serde_json::to_string(&target.connection).unwrap()
                ],
            )
            .unwrap();
            for row in [&alice, &bob] {
                c.execute(
                    "INSERT INTO model_cloud_knowledge VALUES(?1,?2,?3,?4,?5)",
                    params![ws, connection, row.node_id, row.version, now()],
                )
                .unwrap();
            }
            assert!(
                c.execute(
                    "INSERT INTO model_cloud_knowledge VALUES(?1,?2,?3,?4,?5)",
                    params![ws, connection, foreign.node_id, foreign.version, now()]
                )
                .is_err()
            );
        }
        let matching_target = || {
            let mut target = CloudTarget::loopback_test(url.clone(), "fixture");
            target.connection.id.clone_from(&connection);
            target
        };
        let (_stop, cancel) = tokio::sync::watch::channel(false);
        let deadline = Duration::from_secs(6);
        assert!(
            build(
                db.clone(),
                ws.clone(),
                matching_target(),
                vec![private.clone()],
                cancel.clone(),
                deadline,
                new_id(),
            )
            .await
            .is_err()
        );
        let state = build(
            db.clone(),
            ws.clone(),
            target,
            vec![alice.clone(), bob.clone()],
            cancel.clone(),
            deadline,
            new_id(),
        )
        .await
        .unwrap();
        assert_eq!(state["readyVersions"], 2);
        assert_eq!(state["build"]["inputTokens"], 36);
        assert_eq!(state["build"]["vectorCount"], 2);
        let index = usable(&db.connection().unwrap(), &ws, &connection, "fixture")
            .unwrap()
            .unwrap();
        assert!(
            usable(&db.connection().unwrap(), &other, &connection, "fixture")
                .unwrap()
                .is_none()
        );
        let meter = Meter::new(db.clone(), ws.clone(), new_id(), "chat");
        let query = query(
            &matching_target(),
            index.clone(),
            "question-only".into(),
            now(),
            deadline,
            &meter,
        )
        .await;
        {
            let c = db.connection().unwrap();
            let report = super::super::search(&c, &ws, "question-only", Some(&query)).unwrap();
            assert_eq!(report["embedding"]["cloudTransfer"]["inputTokens"], 5);
            assert_eq!(report["candidates"][0]["nodeId"], alice.node_id);
            assert_eq!(report["candidates"][0]["semanticRank"], 1);
        }
        let candidate = |row: &ContextSelection, content: &str| {
            json!({
                "nodeId":row.node_id,"version":row.version,"title":"本地资料","content":content,
            })
        };
        let mut report = json!({"candidates":[candidate(&alice,"granted-alice"),candidate(&private,"private-never-sent"),
            candidate(&bob,"granted-bob")],"stages":[],"elapsedMicros":0});
        let target = matching_target();
        let selected = ranking::permitted_inputs(
            &db.connection().unwrap(),
            &ws,
            &target,
            &report,
            &[alice.clone(), bob.clone(), private.clone()],
        )
        .unwrap();
        let ranking = ranking::rerank(&target, "question-only", selected, deadline, &meter).await;
        ranking
            .apply(&db.connection().unwrap(), &ws, &mut report)
            .unwrap();
        assert_eq!(report["candidates"][0]["nodeId"], bob.node_id);
        assert_eq!(report["candidates"][1]["nodeId"], private.node_id);
        assert_eq!(report["cloudRerank"]["appliedCount"], 2);
        assert_eq!(report["cloudRerank"]["inputTokens"], 20);
        let c = db.connection().unwrap();
        c.execute("DELETE FROM model_cloud_knowledge WHERE workspace_id=?1 AND connection_id=?2 AND node_id=?3",
            params![ws,connection,alice.node_id]).unwrap();
        let after = super::super::search(&c, &ws, "question-only", Some(&query)).unwrap();
        assert!(
            after["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["nodeId"] != alice.node_id)
        );
        assert_eq!(
            active(&c, &ws, &connection, "fixture").unwrap().unwrap().id,
            index.id
        );
        server.join().unwrap();
    }
}

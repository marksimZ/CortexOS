use super::runtime_connections;
use crate::{
    commands::{AppState, blocking},
    db::{error, json_error},
    models::{AppResult, ContextSelection, now},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::State;

pub(crate) fn check_text(text: &str) -> AppResult<()> {
    let private_key = text.split("-----BEGIN ").skip(1).any(|section| {
        section
            .lines()
            .next()
            .and_then(|line| line.find("PRIVATE KEY-----"))
            .is_some_and(|offset| offset <= 64)
    });
    let token = text
        .split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_'))
        .any(|word| {
            [
                ("sk-", 24),
                ("ghp_", 36),
                ("github_pat_", 32),
                ("xoxb-", 24),
                ("xoxp-", 24),
                ("sk-ant-", 30),
                ("glpat-", 24),
            ]
            .iter()
            .any(|(prefix, size)| word.starts_with(prefix) && word.len() >= *size)
                || word.len() == 20
                    && (word.starts_with("AKIA") || word.starts_with("ASIA"))
                    && word
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        });
    if private_key || token {
        return Err("发送内容疑似包含密钥或访问令牌，请在本机移除后再发送。".into());
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Rules {
    pub(crate) block_email: bool,
    pub(crate) block_phone: bool,
    pub(crate) sync_memory_and_history: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            block_email: false,
            block_phone: false,
            sync_memory_and_history: true,
        }
    }
}

impl Rules {
    pub(crate) fn read(c: &Connection, workspace: &str) -> AppResult<Self> {
        let raw: Option<String> = c
            .query_row(
                "SELECT config_json FROM model_cloud_privacy WHERE workspace_id=?1",
                [workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        raw.map(|value| serde_json::from_str(&value).map_err(json_error))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn validate(&self, text: &str) -> AppResult<()> {
        check_text(text)?;
        if self.block_email
            && text
                .split(|ch: char| {
                    !ch.is_ascii_alphanumeric() && !matches!(ch, '@' | '.' | '_' | '-' | '+')
                })
                .any(|word| {
                    word.split_once('@').is_some_and(|(local, domain)| {
                        let domain = domain.trim_end_matches('.');
                        !local.is_empty()
                            && domain.rsplit_once('.').is_some_and(|(host, tld)| {
                                !host.is_empty()
                                    && tld.len() >= 2
                                    && tld.bytes().all(|c| c.is_ascii_alphabetic())
                            })
                    })
                })
        {
            return Err("此工作区已开启邮箱过滤，请在发送前移除邮箱地址。".into());
        }
        if self.block_phone
            && text.split(|ch: char| !ch.is_ascii_digit()).any(|number| {
                let digits = number.as_bytes();
                digits.len() == 11 && digits[0] == b'1' && (b'3'..=b'9').contains(&digits[1])
            })
        {
            return Err("此工作区已开启手机号过滤，请在发送前移除手机号。".into());
        }
        Ok(())
    }
}

pub(crate) fn check_workspace(c: &Connection, workspace: &str, texts: &[&str]) -> AppResult<()> {
    let rules = Rules::read(c, workspace)?;
    for text in texts {
        rules.validate(text)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn model_cloud_privacy_rules(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Option<Rules>,
) -> AppResult<Rules> {
    blocking(state.database.clone()?,move|db|{
        let c=db.connection()?;
        crate::benchmark::dataset::require_workspace(&c,&workspace_id)?;
        if let Some(input)=input {
            c.execute("INSERT INTO model_cloud_privacy VALUES(?1,?2) ON CONFLICT(workspace_id) DO UPDATE SET config_json=excluded.config_json",
                params![workspace_id,serde_json::to_string(&input).map_err(json_error)?]).map_err(error)?;
            Ok(input)
        } else {Rules::read(&c,&workspace_id)}
    }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Grant {
    node_id: String,
    version: i64,
    title: String,
    granted_at: String,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Candidate {
    node_id: String,
    version: i64,
    title: String,
}

#[derive(Serialize)]
pub(crate) struct Grants {
    grants: Vec<Grant>,
    candidates: Vec<Candidate>,
}

pub(crate) fn is_current(
    c: &Connection,
    workspace: &str,
    node: &str,
    version: i64,
) -> AppResult<bool> {
    let current = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM rag_current_knowledge WHERE workspace_id=?1 AND node_id=?2 AND version=?3)",
        params![workspace, node, version], |r| r.get::<_, bool>(0),
    ).map_err(error)?;
    Ok(current && crate::sources::available(c, node, version)?)
}

pub(crate) struct TransferVersion {
    pub(crate) node_id: String,
    pub(crate) version: i64,
    pub(crate) requires_grant: bool,
}

pub(crate) fn context_versions(manifest: &serde_json::Value) -> AppResult<Vec<TransferVersion>> {
    manifest["knowledgeRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(manifest["inheritedKnowledgeRefs"].as_array().into_iter().flatten())
        .map(|reference| {
            Ok(TransferVersion {
                node_id: reference["nodeId"]
                    .as_str()
                    .ok_or("资料记录缺少知识编号。")?
                    .into(),
                version: reference["version"].as_i64().ok_or("资料记录缺少版本。")?,
                requires_grant: reference["reason"] != "user_selected_effective_version",
            })
        })
        .collect()
}

pub(crate) fn run_transfer_lineage(
    c: &Connection,
    workspace: &str,
    run: Option<&str>,
) -> AppResult<Option<std::collections::BTreeSet<(String, i64)>>> {
    let Some(run) = run else { return Ok(None); };
    let saved: Option<(String, bool)> = c.query_row(
        "SELECT r.context_manifest_json, COALESCE(s.request_json NOT LIKE '%本工作区近期其他会话的情景摘要%',0)
         FROM execution_runs r LEFT JOIN context_snapshots s ON s.run_id=r.id
         WHERE r.workspace_id=?1 AND r.id=?2 AND r.status='completed'",
        params![workspace, run],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(error)?;
    let Some((saved, no_legacy_episode)) = saved else { return Ok(None); };
    let manifest: serde_json::Value = serde_json::from_str(&saved).map_err(json_error)?;
    // Legacy snapshots only identify independent turns. Their descendants may already
    // contain prior assistant text with no knowledge provenance.
    let known_legacy_turn = manifest["transferLineageKnown"].is_null()
        && no_legacy_episode
        && manifest["messageRefs"].as_array().is_some_and(|refs| refs.len() == 1);
    if manifest["transferLineageKnown"] != true && !known_legacy_turn {
        return Ok(None);
    }
    Ok(Some(context_versions(&manifest)?.into_iter()
        .map(|reference| (reference.node_id, reference.version))
        .collect()))
}

pub(crate) fn lineage_is_shareable(
    c: &Connection,
    workspace: &str,
    versions: Option<&std::collections::BTreeSet<(String, i64)>>,
) -> AppResult<bool> {
    let Some(versions) = versions else { return Ok(false); };
    for (node, version) in versions {
        if is_local_only(c, workspace, node)? || !is_current(c, workspace, node, *version)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn verify_transfer(
    c: &Connection,
    workspace: &str,
    connection: &str,
    versions: &[TransferVersion],
) -> AppResult<()> {
    verify_chat_transfer(c, workspace, connection, versions, false)
}

pub(crate) fn verify_chat_transfer(
    c: &Connection,
    workspace: &str,
    connection: &str,
    versions: &[TransferVersion],
    sync_context: bool,
) -> AppResult<()> {
    if sync_context && !Rules::read(c, workspace)?.sync_memory_and_history {
        return Err("云端记忆与历史同步已关闭，请重新准备问题。".into());
    }
    for version in versions {
        if is_local_only(c, workspace, &version.node_id)? {
            return Err("部分知识已标记为“仅限本地”，无法发送到云端。".into());
        }
        let allowed = if version.requires_grant && !sync_context {
            permitted(c, workspace, connection, &version.node_id, version.version)?
        } else {
            is_current(c, workspace, &version.node_id, version.version)?
        };
        if !allowed {
            return Err("知识版本或发送授权已变化，请重新检查要发送的资料。".into());
        }
    }
    Ok(())
}

pub(crate) fn is_local_only(c: &Connection, workspace: &str, node: &str) -> AppResult<bool> {
    c.query_row(
        "SELECT local_only FROM knowledge_local_only WHERE workspace_id=?1 AND node_id=?2",
        params![workspace, node],
        |r| r.get::<_, bool>(0),
    )
    .optional()
    .map_err(error)
    .map(Option::unwrap_or_default)
}

pub(crate) fn permitted(
    c: &Connection,
    workspace: &str,
    connection: &str,
    node: &str,
    version: i64,
) -> AppResult<bool> {
    if is_local_only(c, workspace, node)? {
        return Ok(false);
    }
    let granted = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM model_cloud_knowledge WHERE workspace_id=?1 AND connection_id=?2 AND node_id=?3 AND version=?4)",
        params![workspace, connection, node, version], |r|r.get::<_,bool>(0),
    ).map_err(error)?;
    Ok(granted && is_current(c, workspace, node, version)?)
}

#[tauri::command]
pub async fn set_knowledge_local_only(
    state: State<'_, AppState>,
    workspace_id: String,
    node_id: String,
    local_only: bool,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        crate::benchmark::dataset::require_workspace(&c, &workspace_id)?;
        let exists: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_nodes WHERE workspace_id=?1 AND id=?2)",
                params![workspace_id, node_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if !exists {
            return Err("知识不存在，或不属于当前工作区。".into());
        }
        c.execute(
            "INSERT INTO knowledge_local_only(workspace_id,node_id,local_only,updated_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(workspace_id,node_id) DO UPDATE SET local_only=excluded.local_only,updated_at=excluded.updated_at",
            params![workspace_id, node_id, local_only, now()],
        )
        .map_err(error)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_model_cloud_knowledge(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    include_candidates: Option<bool>,
) -> AppResult<Grants> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        runtime_connections::get(&c, &workspace_id, &connection_id)?;
        let mut stmt = c.prepare("SELECT g.node_id,g.version,v.title,g.granted_at FROM model_cloud_knowledge g
           JOIN knowledge_versions v ON v.node_id=g.node_id AND v.version=g.version
           WHERE g.workspace_id=?1 AND g.connection_id=?2 ORDER BY v.title,g.node_id,g.version"
        ).map_err(error)?;
        let rows = stmt.query_map(params![workspace_id, connection_id], |r| Ok(
            (r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)
        )).map_err(error)?;
        let grants = rows.map(|row| {
            let (node_id, version, title, granted_at) = row.map_err(error)?;
            let available = !is_local_only(&c, &workspace_id, &node_id)?
                && is_current(&c, &workspace_id, &node_id, version)?;
            Ok(Grant { node_id, version, title, granted_at, available })
        }).collect::<AppResult<Vec<_>>>()?;
        let mut candidates = Vec::new();
        if include_candidates == Some(true) {
            let mut stmt = c.prepare("SELECT n.node_id,n.version,v.title FROM rag_current_knowledge n
                JOIN knowledge_versions v ON v.node_id=n.node_id AND v.version=n.version
                WHERE n.workspace_id=?1 AND NOT EXISTS(
                    SELECT 1 FROM model_cloud_knowledge g WHERE g.workspace_id=n.workspace_id
                    AND g.connection_id=?2 AND g.node_id=n.node_id AND g.version=n.version)
                AND NOT EXISTS(SELECT 1 FROM knowledge_source_links l JOIN source_documents s ON s.id=l.source_id
                    WHERE l.node_id=n.node_id AND l.knowledge_version=n.version AND s.enabled=0)
                AND NOT EXISTS(SELECT 1 FROM knowledge_local_only p
                    WHERE p.workspace_id=n.workspace_id AND p.node_id=n.node_id AND p.local_only=1)
                ORDER BY v.title,n.node_id LIMIT 2000"
            ).map_err(error)?;
            let rows = stmt.query_map(params![workspace_id,connection_id], |row| Ok(Candidate {
                node_id:row.get(0)?,version:row.get(1)?,title:row.get(2)?,
            })).map_err(error)?;
            candidates = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(error)?;
        }
        Ok(Grants { grants, candidates })
    }).await
}

#[tauri::command]
pub async fn set_model_cloud_knowledge(
    state: State<'_, AppState>,
    workspace_id: String,
    connection_id: String,
    selection: ContextSelection,
    allowed: bool,
) -> AppResult<()> {
    let _operation = state
        .runtime_cloud
        .try_lock()
        .map_err(|_| "请先等待或停止当前云端任务。")?;
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        runtime_connections::get(&c, &workspace_id, &connection_id)?;
        if allowed {
            if is_local_only(&c, &workspace_id, &selection.node_id)? {
                return Err("仅限本地的知识无法授权发送到云端。".into());
            }
            if !is_current(&c, &workspace_id, &selection.node_id, selection.version)? {
                return Err("请选择当前工作区已经生效的知识版本。".into());
            }
            c.execute("INSERT OR IGNORE INTO model_cloud_knowledge VALUES(?1,?2,?3,?4,?5)",
                params![workspace_id,connection_id,selection.node_id,selection.version,now()]).map_err(error)?;
        } else {
            c.execute("DELETE FROM model_cloud_knowledge WHERE workspace_id=?1 AND connection_id=?2 AND node_id=?3 AND version=?4",
                params![workspace_id,connection_id,selection.node_id,selection.version]).map_err(error)?;
        }
        Ok(())
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_privacy_filters_realistic_secrets_and_allows_document_text() {
        assert!(super::check_text("参考 sk-example 的文档").is_ok());
        let synthetic_aws_key = format!("access key AKIA{}", "A".repeat(16));
        for text in [
            "credential sk-testexampleonly123456789",
            "\n-----BEGIN OPENSSH PRIVATE KEY-----\n",
            synthetic_aws_key.as_str(),
            "https://example.com/?key=github_pat_synthetic_fixture_only_123456789",
        ] {
            assert!(super::check_text(text).is_err());
        }
    }

    #[tokio::test]
    async fn cloud_privacy_stops_retry_after_revocation_and_filters_contact_before_dispatch() {
        use crate::{
            cloud::runtime_connections::{Capability, CloudTarget},
            db::Database,
            knowledge::SaveKnowledge,
            model_runtime::http_fixture,
            models::new_id,
            runtime_usage::Meter,
        };
        use serde_json::json;
        use std::{sync::Arc, time::Duration};
        let db = Arc::new(Database::rag_evaluation_database().unwrap());
        let workspace = db.create_workspace("隐私规则").unwrap().id;
        let other = db.create_workspace("另一个工作区").unwrap().id;
        let node = db
            .save_knowledge(SaveKnowledge {
                workspace_id: workspace.clone(),
                node_id: None,
                expected_revision: None,
                title: "授权资料".into(),
                kind: "knowledge".into(),
                content: "正常文本".into(),
                change_reason: "验证".into(),
                source_message_id: None,
                source_excerpt: None,
            })
            .unwrap();
        db.approve_knowledge(&workspace, &node, 1).unwrap();
        let recipient = new_id();
        let (db_server, ws_server, conn_server) =
            (db.clone(), workspace.clone(), recipient.clone());
        let (url, server) =
            http_fixture::serve(1, move |_, path, body| {
                assert_eq!(path, "/embeddings");
                assert_eq!(body["input"], json!(["正常文本"]));
                db_server.connection().unwrap().execute(
                "DELETE FROM model_cloud_knowledge WHERE workspace_id=?1 AND connection_id=?2",
                params![ws_server,conn_server],
            ).unwrap();
                (429, json!({"error":{"type":"rate_limit"}}))
            });
        let mut target = CloudTarget::loopback_test(url, "fixture");
        target.connection.id.clone_from(&recipient);
        {
            let c = db.connection().unwrap();
            c.execute(
                "INSERT INTO model_cloud_connections VALUES(?1,?2,?3)",
                params![
                    workspace,
                    recipient,
                    serde_json::to_string(&target.connection).unwrap()
                ],
            )
            .unwrap();
            c.execute(
                "INSERT INTO model_cloud_knowledge VALUES(?1,?2,?3,1,?4)",
                params![workspace, recipient, node, now()],
            )
            .unwrap();
            c.execute(
                "INSERT INTO model_cloud_privacy VALUES(?1,?2)",
                params![
                    workspace,
                    json!({"blockEmail":true,"blockPhone":true}).to_string()
                ],
            )
            .unwrap();
            c.execute(
                "INSERT INTO model_runtime_policies VALUES(?1,?2)",
                params![workspace, json!({"retryLimit":1}).to_string()],
            )
            .unwrap();
            assert!(check_workspace(&c, &other, &["user@example.test"]).is_ok());
        }
        let meter = Meter::new(db.clone(), workspace, new_id(), "cloud_index");
        let timeout = Duration::from_secs(3);
        for input in ["user@example.test", "联系 13800000000"] {
            assert!(
                target
                    .embed(&[input.into()], None, timeout, &meter, "question", &[])
                    .await
                    .unwrap_err()
                    .contains("过滤")
            );
        }
        assert_eq!(meter.attempts(Capability::Embedding).unwrap(), 0);
        let references = [TransferVersion {
            node_id: node,
            version: 1,
            requires_grant: true,
        }];
        let error = target
            .embed(
                &["正常文本".into()],
                None,
                timeout,
                &meter,
                "granted_knowledge",
                &references,
            )
            .await
            .unwrap_err();
        assert!(error.contains("授权已变化"), "{error}");
        assert_eq!(meter.attempts(Capability::Embedding).unwrap(), 1);
        server.join().unwrap();
    }
}

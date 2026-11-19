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

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Rules {
    block_email: bool,
    block_phone: bool,
}

impl Rules {
    fn read(c: &Connection, workspace: &str) -> AppResult<Self> {
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

pub(crate) fn verify_transfer(
    c: &Connection,
    workspace: &str,
    connection: &str,
    versions: &[TransferVersion],
) -> AppResult<()> {
    for version in versions {
        let allowed = if version.requires_grant {
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

pub(crate) fn permitted(
    c: &Connection,
    workspace: &str,
    connection: &str,
    node: &str,
    version: i64,
) -> AppResult<bool> {
    let granted = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM model_cloud_knowledge WHERE workspace_id=?1 AND connection_id=?2 AND node_id=?3 AND version=?4)",
        params![workspace, connection, node, version], |r|r.get::<_,bool>(0),
    ).map_err(error)?;
    Ok(granted && is_current(c, workspace, node, version)?)
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
            let available = is_current(&c, &workspace_id, &node_id, version)?;
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

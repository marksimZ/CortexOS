use crate::{
    benchmark::dataset::require_workspace,
    cloud::runtime_connections::{self, Capability},
    commands::{AppState, blocking},
    db::{error, json_error},
    models::{AppResult, ContextSelection, new_id, now},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

pub(crate) mod ranking;
pub(crate) mod routing;

const PROFILE_MISSING: &str = "找不到当前工作区的运行方案。";

#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    Local,
    Cloud,
    Hybrid,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Target {
    Off,
    Local {
        model: Option<String>,
    },
    Cloud {
        connection_id: String,
        model: String,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProfileInput {
    name: String,
    pub(crate) mode: Mode,
    pub(crate) generation: Target,
    pub(crate) embedding: Target,
    pub(crate) reranking: Target,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub(crate) id: String,
    #[serde(flatten)]
    pub(crate) input: ProfileInput,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profiles {
    active_id: Option<String>,
    profiles: Vec<Profile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CloudConsent {
    profile_id: String,
    updated_at: String,
    pub(crate) share_selection: bool,
    #[serde(default)]
    pub(crate) granted_versions: Vec<ContextSelection>,
    #[serde(default)]
    pub(crate) ranking_versions: Vec<ContextSelection>,
}

impl Target {
    pub(crate) fn resolve_cloud(
        &self,
        db: &crate::db::Database,
        workspace: &str,
        capability: Capability,
    ) -> AppResult<Option<runtime_connections::CloudTarget>> {
        match self {
            Self::Cloud {
                connection_id,
                model,
            } => {
                runtime_connections::resolve_target(db, workspace, connection_id, model, capability)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }
}

pub(crate) struct CloudTargets {
    pub(crate) generation: Option<runtime_connections::CloudTarget>,
    pub(crate) embedding: Option<runtime_connections::CloudTarget>,
    pub(crate) reranking: Option<runtime_connections::CloudTarget>,
}

impl CloudTargets {
    pub(crate) fn resolve(
        db: &crate::db::Database,
        workspace: &str,
        profile: Option<&Profile>,
    ) -> AppResult<Self> {
        Ok(match profile {
            Some(profile) => Self {
                generation: profile.input.generation.resolve_cloud(
                    db,
                    workspace,
                    Capability::Generation,
                )?,
                embedding: profile.input.embedding.resolve_cloud(
                    db,
                    workspace,
                    Capability::Embedding,
                )?,
                reranking: profile.input.reranking.resolve_cloud(
                    db,
                    workspace,
                    Capability::Reranking,
                )?,
            },
            None => Self {
                generation: None,
                embedding: None,
                reranking: None,
            },
        })
    }
}

pub(crate) struct CloudChat<'a> {
    pub(crate) target: &'a runtime_connections::CloudTarget,
    pub(crate) share_selection: bool,
    pub(crate) granted_versions: &'a [ContextSelection],
    pub(crate) public_fixture: bool,
}

pub(crate) struct ChatRuntime<'a> {
    pub(crate) profile: &'a Profile,
    pub(crate) cloud: Option<CloudChat<'a>>,
    pub(crate) ranking: Option<&'a ranking::CloudRanking>,
}

impl Profile {
    pub(crate) fn default_local() -> Self {
        Self {
            id: "default-local".into(),
            input: ProfileInput {
                name: "默认本地模型".into(),
                mode: Mode::Local,
                generation: Target::Local { model: None },
                embedding: Target::Local { model: None },
                reranking: Target::Local { model: None },
            },
            updated_at: String::new(),
        }
    }

    pub(crate) fn cloud_consent(&self, consent: Option<CloudConsent>) -> AppResult<CloudConsent> {
        let consent = consent
            .filter(|c| c.profile_id == self.id && c.updated_at == self.updated_at)
            .ok_or("请确认本次发送给云端服务的内容。运行方案更新后需重新确认。")?;
        if consent.granted_versions.len() > 2000 || consent.ranking_versions.len() > 2000 {
            return Err("单次最多确认 2000 个知识版本，请缩小授权范围。".into());
        }
        Ok(consent)
    }

    pub(crate) fn select_local_embedding(
        &self,
        index: Option<crate::rag::Profile>,
    ) -> Option<crate::rag::Profile> {
        let Target::Local { model } = &self.input.embedding else {
            return None;
        };
        index.filter(|item| model.as_deref().is_none_or(|model| item.model() == model))
    }

    pub(crate) fn local_rerank(&self) -> bool {
        matches!(self.input.reranking, Target::Local { .. })
    }

    pub(crate) fn has_cloud_target(&self) -> bool {
        self.input
            .targets()
            .iter()
            .any(|(target, _)| matches!(target, Target::Cloud { .. }))
    }

    pub(crate) fn audit(&self) -> Value {
        json!({"id":self.id,"name":self.input.name,"mode":self.input.mode,"updatedAt":self.updated_at,
            "generation":self.input.generation,"embedding":self.input.embedding,"reranking":self.input.reranking})
    }

    fn preflight_activation(&self, c: &Connection, workspace: &str) -> AppResult<()> {
        self.input.check_cloud_targets(c, workspace)
    }
}

impl ProfileInput {
    fn targets(&self) -> [(&Target, Capability); 3] {
        [
            (&self.generation, Capability::Generation),
            (&self.embedding, Capability::Embedding),
            (&self.reranking, Capability::Reranking),
        ]
    }

    fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("请填写 1 至 80 字的运行方案名称。".into());
        }
        if matches!(self.generation, Target::Off) {
            return Err("运行方案需要一个回答模型。".into());
        }
        let mut local = false;
        let mut cloud = false;
        for (target, role) in self.targets() {
            match target {
                Target::Off => {}
                Target::Local { model } => {
                    local = true;
                    if model.as_ref().is_some_and(|name| {
                        name.trim() != name
                            || name.is_empty()
                            || name.len() > 200
                            || name.chars().any(|c| c.is_control() || c.is_whitespace())
                    }) || (role == Capability::Reranking && model.is_some())
                    {
                        return Err("请选择有效的本机模型。".into());
                    }
                }
                Target::Cloud {
                    connection_id,
                    model,
                } => {
                    cloud = true;
                    if uuid::Uuid::parse_str(connection_id).is_err()
                        || !runtime_connections::is_model_id(model)
                    {
                        return Err("请选择已添加的连接并填写有效的云端模型标识。".into());
                    }
                }
            }
        }
        match (self.mode, local, cloud) {
            (Mode::Local, true, false)
            | (Mode::Cloud, false, true)
            | (Mode::Hybrid, true, true) => Ok(()),
            _ => Err("运行模式与各环节选用的模型不一致。".into()),
        }
    }

    pub(crate) fn check_cloud_targets(&self, c: &Connection, workspace: &str) -> AppResult<()> {
        for (target, capability) in self.targets() {
            if let Target::Cloud {
                connection_id,
                model,
            } = target
            {
                let connection = runtime_connections::get(c, workspace, connection_id)?;
                if connection.verified_capability(model, capability).is_none() {
                    return Err("请先在云端模型设置中验证所选模型的对应能力。".into());
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn get(c: &Connection, workspace: &str, id: &str) -> AppResult<Profile> {
    let raw: String = c
        .query_row(
            "SELECT profile_json FROM model_runtime_profiles WHERE workspace_id=?1 AND id=?2",
            params![workspace, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(error)?
        .ok_or(PROFILE_MISSING)?;
    decode(&raw, id)
}

fn decode(raw: &str, id: &str) -> AppResult<Profile> {
    let value: Profile = serde_json::from_str(raw).map_err(json_error)?;
    if value.id != id {
        return Err(PROFILE_MISSING.into());
    }
    value.input.validate()?;
    Ok(value)
}

pub(crate) fn active(c: &Connection, workspace: &str) -> AppResult<Option<Profile>> {
    let row: Option<(String, String)> = c
        .query_row(
            "SELECT id,profile_json FROM model_runtime_profiles WHERE workspace_id=?1 AND active=1",
            [workspace],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(error)?;
    row.map(|(id, raw)| decode(&raw, &id)).transpose()
}

#[tauri::command]
pub async fn list_model_runtime_profiles(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Profiles> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let active_id = active(&c, &workspace_id)?.map(|profile| profile.id);
        let mut stmt = c.prepare(
            "SELECT id,profile_json FROM model_runtime_profiles WHERE workspace_id=?1 ORDER BY id",
        ).map_err(error)?;
        let rows = stmt
            .query_map([&workspace_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(error)?;
        let mut profiles = rows
            .map(|row| {
                let (id, raw) = row.map_err(error)?;
                decode(&raw, &id)
            })
            .collect::<AppResult<Vec<_>>>()?;
        profiles.sort_by(|a, b| a.input.name.cmp(&b.input.name).then(a.id.cmp(&b.id)));
        Ok(Profiles {
            active_id,
            profiles,
        })
    })
    .await
}

#[tauri::command]
pub async fn save_model_runtime_profile(
    state: State<'_, AppState>,
    workspace_id: String,
    profile_id: Option<String>,
    mut input: ProfileInput,
) -> AppResult<Profile> {
    input.name = input.name.trim().to_owned();
    input.validate()?;
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let updating = profile_id.is_some();
        let id = profile_id.unwrap_or_else(new_id);
        if updating {
            get(&c, &workspace_id, &id)?;
        } else {
            let count: i64 = c
                .query_row(
                    "SELECT count(*) FROM model_runtime_profiles WHERE workspace_id=?1",
                    [&workspace_id],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if count >= 32 {
                return Err("当前工作区最多可保存 32 套运行方案。".into());
            }
        }
        let profile = Profile {
            id,
            input,
            updated_at: now(),
        };
        if updating && active(&c, &workspace_id)?.is_some_and(|active| active.id == profile.id) {
            profile.preflight_activation(&c, &workspace_id)?;
        }
        let raw = serde_json::to_string(&profile).map_err(json_error)?;
        let sql = if updating {
            "UPDATE model_runtime_profiles SET profile_json=?3 WHERE workspace_id=?1 AND id=?2"
        } else {
            "INSERT INTO model_runtime_profiles(workspace_id,id,profile_json) VALUES(?1,?2,?3)"
        };
        c.execute(sql, params![workspace_id, profile.id, raw])
            .map_err(error)?;
        Ok(profile)
    })
    .await
}

#[tauri::command]
pub async fn activate_model_runtime_profile(
    state: State<'_, AppState>,
    workspace_id: String,
    profile_id: Option<String>,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        let mut c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        let tx = c.transaction().map_err(error)?;
        if let Some(id) = &profile_id {
            get(&tx, &workspace_id, id)?.preflight_activation(&tx, &workspace_id)?;
        }
        tx.execute(
            "UPDATE model_runtime_profiles SET active=0 WHERE workspace_id=?1 AND active=1",
            [&workspace_id],
        )
        .map_err(error)?;
        if let Some(id) = &profile_id {
            tx.execute(
                "UPDATE model_runtime_profiles SET active=1 WHERE workspace_id=?1 AND id=?2",
                params![workspace_id, id],
            )
            .map_err(error)?;
        }
        tx.commit().map_err(error)
    })
    .await
}

#[tauri::command]
pub async fn delete_model_runtime_profile(
    state: State<'_, AppState>,
    workspace_id: String,
    profile_id: String,
) -> AppResult<()> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        if active(&c, &workspace_id)?.is_some_and(|profile| profile.id == profile_id) {
            return Err("请先切换到其他运行方案。".into());
        }
        if routing::Routing::read(&c,&workspace_id)?.references(&profile_id) {
            return Err("请先从任务路由中移除这个方案。".into());
        }
        let n = c
            .execute(
                "DELETE FROM model_runtime_profiles WHERE workspace_id=?1 AND id=?2",
                params![workspace_id, profile_id],
            )
            .map_err(error)?;
        if n != 1 {
            return Err(PROFILE_MISSING.into());
        }
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud::runtime_connections::CloudTarget,
        db::Database,
        knowledge::SaveKnowledge,
        models::{ContextSelection, ExecutionPurpose, ModelSettings},
    };

    #[test]
    fn runtime_profiles_validate_modes_and_scope_active_selection() {
        let db = Database::rag_evaluation_database().unwrap();
        let a = db.create_workspace("方案工作区甲").unwrap().id;
        let b = db.create_workspace("方案工作区乙").unwrap().id;
        let c = db.connection().unwrap();
        let local = Target::Local { model: None };
        let cloud = Target::Cloud {
            connection_id: new_id(),
            model: "example-model".into(),
        };
        let mut input = ProfileInput {
            name: "本地方案".into(),
            mode: Mode::Local,
            generation: local.clone(),
            embedding: Target::Off,
            reranking: Target::Off,
        };
        assert!(input.validate().is_ok());
        let profile = Profile {
            id: new_id(),
            input: input.clone(),
            updated_at: now(),
        };
        let raw = serde_json::to_string(&profile).unwrap();
        c.execute(
            "INSERT INTO model_runtime_profiles VALUES (?1,?2,?3,1)",
            params![a, profile.id, raw],
        )
        .unwrap();
        assert_eq!(active(&c, &a).unwrap().unwrap().id, profile.id);
        assert!(active(&c, &b).unwrap().is_none());
        assert!(get(&c, &b, &profile.id).is_err());
        assert!(
            c.execute(
                "INSERT INTO model_runtime_profiles VALUES (?1,?2,?3,1)",
                params![a, new_id(), raw]
            )
            .is_err()
        );

        input.embedding = cloud.clone();
        assert!(input.validate().is_err());
        input.mode = Mode::Hybrid;
        assert!(input.validate().is_ok());
        assert!(input.check_cloud_targets(&c, &a).is_err());
        input.generation = cloud;
        input.mode = Mode::Cloud;
        assert!(input.validate().is_ok());
        input.generation = Target::Off;
        assert!(input.validate().is_err());
    }

    #[tokio::test]
    async fn runtime_profiles_cloud_chat_sends_snapshot_and_only_consented_fields() {
        use std::{
            io::{BufRead, BufReader, Read, Write},
            net::TcpListener,
            sync::Arc,
            time::Duration,
        };
        let db = Arc::new(Database::rag_evaluation_database().unwrap());
        let workspace = db.create_workspace("发送范围").unwrap().id;
        let other = db.create_workspace("其他工作区").unwrap().id;
        let chat = db.create_conversation(&workspace).unwrap().id;
        let add_knowledge = |scope: &str, title: &str, content: &str| {
            let id = db
                .save_knowledge(SaveKnowledge {
                    workspace_id: scope.into(),
                    node_id: None,
                    expected_revision: None,
                    title: title.into(),
                    kind: "knowledge".into(),
                    content: content.into(),
                    change_reason: "测试".into(),
                    source_message_id: None,
                    source_excerpt: None,
                })
                .unwrap();
            db.approve_knowledge(scope, &id, 1).unwrap();
            ContextSelection {
                node_id: id,
                version: 1,
            }
        };
        let approved = add_knowledge(&workspace, "采购规则", "selected-marker");
        let private = add_knowledge(&workspace, "私有采购限制", "private-auto-marker");
        let granted = add_knowledge(&workspace, "采购限制说明", "granted-auto-marker");
        let not_confirmed = add_knowledge(&workspace, "采购限制附录", "unconfirmed-grant-marker");
        let foreign = add_knowledge(&other, "其他资料", "foreign-workspace-marker");
        let settings = ModelSettings {
            model: "fixture-model".into(),
            ..ModelSettings::default()
        };
        let history = db
            .prepare_run(
                &new_id(),
                &workspace,
                &chat,
                "private-history-marker",
                &settings,
            )
            .unwrap();
        db.execution_finish(
            &history.run,
            ExecutionPurpose::Chat,
            "private-answer-marker",
            "completed",
            None,
            json!({}),
        )
        .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/fixture/", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let target = CloudTarget::loopback_test(base, &settings.model);
        {
            let c = db.connection().unwrap();
            c.execute(
                "INSERT INTO model_cloud_connections VALUES(?1,?2,?3)",
                params![
                    workspace,
                    target.connection.id,
                    serde_json::to_string(&target.connection).unwrap(),
                ],
            )
            .unwrap();
            for version in [&granted, &not_confirmed] {
                c.execute(
                    "INSERT INTO model_cloud_knowledge VALUES(?1,?2,?3,?4,?5)",
                    params![
                        workspace,
                        target.connection.id,
                        version.node_id,
                        version.version,
                        now(),
                    ],
                )
                .unwrap();
            }
        }
        let profile = Profile {
            id: new_id(),
            updated_at: now(),
            input: ProfileInput {
                name: "云端方案".into(),
                mode: Mode::Cloud,
                generation: Target::Cloud {
                    connection_id: target.connection.id.clone(),
                    model: settings.model.clone(),
                },
                embedding: Target::Off,
                reranking: Target::Off,
            },
        };
        assert!(profile.cloud_consent(None).is_err());
        assert!(
            profile
                .cloud_consent(Some(CloudConsent {
                    profile_id: profile.id.clone(),
                    updated_at: "old".into(),
                    share_selection: true,
                    granted_versions: vec![],
                    ranking_versions: vec![],
                }))
                .is_err()
        );
        let prepare = |selected: &[ContextSelection],
                       share: bool,
                       grants: &[ContextSelection],
                       preview: bool| {
            db.prepare_chat_runtime(
                &new_id(),
                &workspace,
                &chat,
                "采购限制？",
                &settings,
                selected,
                preview,
                None,
                None,
                false,
                Some(&ChatRuntime {
                    profile: &profile,
                    cloud: Some(CloudChat {
                        target: &target,
                        share_selection: share,
                        granted_versions: grants,
                        public_fixture: false,
                    }),
                    ranking: None,
                }),
                None,
            )
        };
        let question_only = prepare(std::slice::from_ref(&approved), false, &[], true).unwrap();
        assert!(
            question_only.manifest["knowledgeRefs"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            question_only.request["messages"][1]["content"],
            "采购限制？"
        );
        assert!(prepare(&[foreign], true, &[], true).is_err());
        let prepared = prepare(&[approved], true, &[granted, private], false).unwrap();
        assert_eq!(
            prepared.manifest["cloudTransfer"]["dataTypes"],
            json!(["question", "selected_knowledge", "granted_knowledge"])
        );
        assert_eq!(
            prepared.manifest["messageRefs"].as_array().unwrap().len(),
            1
        );
        let run = prepared.run.id.clone();
        let expected = prepared.request["body"].clone();
        let saved_request = db
            .run_detail(&workspace, &run)
            .unwrap()
            .snapshot
            .unwrap()
            .request;
        assert_eq!(saved_request["body"], expected);
        let service = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(6)))
                .unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /fixture/chat/completions "));
            let mut length = 0;
            loop {
                line.clear();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((field, value)) = line.split_once(':')
                    && field.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut buffer = vec![0; length];
            reader.read_exact(&mut buffer).unwrap();
            let actual: Value = serde_json::from_slice(&buffer).unwrap();
            assert_eq!(actual, expected);
            let prompt = actual["messages"][1]["content"].as_str().unwrap();
            assert!(prompt.contains("selected-marker"));
            assert!(prompt.contains("granted-auto-marker"));
            for blocked in [
                "private-history-marker",
                "private-answer-marker",
                "private-auto-marker",
                "foreign-workspace-marker",
                "unconfirmed-grant-marker",
                "sourceEvidence",
                "synthetic-cloud-test-key",
            ] {
                assert!(!prompt.contains(blocked), "unexpected field {blocked}");
            }
            let body =
                json!({"choices":[{"finish_reason":"stop","message":{"content":"参照[资料1]。"}}],
                "usage":{"prompt_tokens":21,"completion_tokens":8}})
                .to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let (_cancel, receiver) = tokio::sync::watch::channel(false);
        let output =
            crate::generation::execute_chat(db.clone(),None,Some(target),prepared,settings,None,receiver,|_|{}).await;
        assert!(output.saved);
        assert_eq!(output.update.status, "completed");
        service.join().unwrap();
        let detail = db.run_detail(&workspace, &run).unwrap();
        assert!(
            detail
                .events
                .iter()
                .any(|event| event.kind == "model_request_started"
                    && event.payload["transport"] == "cloud")
        );
        assert!(
            detail
                .events
                .iter()
                .any(|event| event.kind == "response_completed"
                    && event.payload["metrics"]["promptEvalCount"] == 21)
        );
        assert!(detail.events.iter().all(|event| {
            !event
                .payload
                .to_string()
                .contains("synthetic-cloud-test-key")
        }));
    }
}

use super::{Profile, Target, active, get};
use crate::{
    benchmark::dataset::require_workspace,
    cloud::runtime_connections::{self, Capability},
    commands::{AppState, blocking},
    db::{error, json_error},
    models::{AppResult, ModelSettings},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Preference {
    #[default]
    Preferred,
    LowerCost,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct TaskRule {
    preferred: Option<String>,
    alternative: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Routing {
    preference: Preference,
    question: TaskRule,
    knowledge: TaskRule,
    pub(crate) local_backup: Option<String>,
}

impl Routing {
    pub(crate) fn read(c: &Connection, workspace: &str) -> AppResult<Self> {
        let raw: Option<String> = c
            .query_row(
                "SELECT config_json FROM model_chat_routing WHERE workspace_id=?1",
                [workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        raw.map(|value| serde_json::from_str(&value).map_err(json_error))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn validate(&self, c: &Connection, workspace: &str) -> AppResult<()> {
        if self.local_backup.as_ref().is_some_and(|model| {
            model.is_empty()
                || model.len() > 200
                || model.chars().any(|c| c.is_control() || c.is_whitespace())
        }) {
            return Err("请选择有效的本机备用模型。".into());
        }
        for id in self.profile_ids() {
            get(c, workspace, id)?;
        }
        Ok(())
    }

    fn profile_ids(&self) -> impl Iterator<Item = &str> {
        [
            &self.question.preferred,
            &self.question.alternative,
            &self.knowledge.preferred,
            &self.knowledge.alternative,
        ]
        .into_iter()
        .filter_map(|id| id.as_deref())
    }

    pub(crate) fn references(&self, id: &str) -> bool {
        self.profile_ids().any(|value| value == id)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Selection {
    pub(crate) profile: Option<Profile>,
    pub(crate) decision: Value,
    pub(crate) local_backup: Option<String>,
}

pub(crate) fn select_generation(
    c: &Connection,
    workspace: &str,
    knowledge: bool,
    settings: &ModelSettings,
    generation: Option<Target>,
) -> AppResult<Selection> {
    require_workspace(c, workspace)?;
    let mut automatic = select(c, workspace, knowledge, settings)?;
    if !knowledge && let Some(profile) = &mut automatic.profile {
        profile.input.embedding = Target::Off;
        profile.input.reranking = Target::Off;
        profile.input.mode = match profile.input.generation {
            Target::Cloud { .. } => super::Mode::Cloud,
            _ => super::Mode::Local,
        };
    }
    let Some(generation) = generation else {
        return Ok(automatic);
    };
    if matches!(generation, Target::Off | Target::Local { model: None }) {
        return Err("请选择有效的回答模型。".into());
    }
    let inherited_id = automatic.profile.as_ref().map(|profile| profile.id.clone());
    let mut profile = automatic.profile.unwrap_or_else(|| {
        let mut defaults = Profile::default_local();
        if !knowledge || matches!(generation, Target::Cloud { .. }) {
            defaults.input.embedding = Target::Off;
            defaults.input.reranking = Target::Off;
        }
        defaults
    });
    profile.id = "manual-generation".into();
    profile.input.name = "手动选择回答模型".into();
    profile.input.generation = generation;
    let targets = profile.input.targets();
    let uses_cloud = targets.iter().any(|(target, _)| matches!(target, Target::Cloud { .. }));
    let uses_local = targets.iter().any(|(target, _)| matches!(target, Target::Local { .. }));
    profile.input.mode = match (uses_cloud, uses_local) {
        (true, true) => super::Mode::Hybrid,
        (true, false) => super::Mode::Cloud,
        _ => super::Mode::Local,
    };
    profile.input.validate()?;
    let mut destinations = Vec::new();
    for (target, capability) in profile.input.targets() {
        if let Target::Cloud { connection_id, model } = target {
            let connection = runtime_connections::get(c, workspace, connection_id)?;
            if capability == Capability::Generation {
                if !connection.can_generate(model) {
                    return Err("所选云端连接凭据无效或不支持该生成模型。".into());
                }
                let checked = connection
                    .verified_capability(model, capability)
                    .map(|p| p.checked_at.as_str())
                    .unwrap_or("unprobed");
                destinations.push(json!([connection.provider, connection.display_destination(), checked]));
            } else {
                let probe = connection
                    .verified_capability(model, capability)
                    .ok_or("请先在云端模型设置中验证所选模型的对应能力。")?;
                destinations.push(json!([connection.provider, connection.display_destination(), probe.checked_at]));
            }
        }
    }
    let snapshot = serde_json::to_vec(&(workspace, &profile, destinations)).map_err(json_error)?;
    profile.updated_at = crate::benchmark::dataset::hash(&snapshot);
    Ok(Selection {
        decision: json!({
            "task":if knowledge {"knowledge"} else {"question"},
            "reason":"manual_generation", "preference":automatic.decision["preference"],
            "preferredProfileId":automatic.decision["preferredProfileId"],
            "preferredEstimatedMicros":null, "alternative":null,
            "selectedProfileId":inherited_id,
        }),
        profile: Some(profile),
        local_backup: None,
    })
}

pub(crate) fn select(
    c: &Connection,
    workspace: &str,
    knowledge: bool,
    settings: &ModelSettings,
) -> AppResult<Selection> {
    let config = Routing::read(c, workspace)?;
    let rule = if knowledge {
        &config.knowledge
    } else {
        &config.question
    };
    let mut chosen = match &rule.preferred {
        Some(id) => Some(get(c, workspace, id)?),
        None => active(c, workspace)?,
    };
    let preferred_id = chosen.as_ref().map(|profile| profile.id.clone());
    let primary = estimate(c, workspace, chosen.as_ref(), knowledge, settings);
    let mut compared = None;
    let mut reason = if rule.preferred.is_some() {
        "task_preference"
    } else {
        "workspace_default"
    };
    if config.preference == Preference::LowerCost
        && let Some(id) = &rule.alternative
    {
        let alternative = get(c, workspace, id)?;
        let cost = estimate(c, workspace, Some(&alternative), knowledge, settings);
        compared = Some(json!({"profileId":alternative.id,"estimatedMicros":cost}));
        if cost.is_some_and(|value| primary.is_none_or(|initial| value < initial)) {
            chosen = Some(alternative);
            reason = "lower_estimated_cost";
        } else if primary.is_none() && cost.is_none() {
            reason = "prices_unavailable";
        }
    }
    let decision = json!({"task":if knowledge {"knowledge"} else {"question"},
        "preference":config.preference,"reason":reason,"preferredProfileId":preferred_id,
        "preferredEstimatedMicros":primary,"alternative":compared,
        "selectedProfileId":chosen.as_ref().map(|p|&p.id)});
    Ok(Selection {
        profile: chosen,
        decision,
        local_backup: config.local_backup,
    })
}

fn estimate(
    c: &Connection,
    workspace: &str,
    profile: Option<&Profile>,
    knowledge: bool,
    settings: &ModelSettings,
) -> Option<u64> {
    let Some(profile) = profile else {
        return Some(0);
    };
    let roles = [
        (&profile.input.generation, Capability::Generation),
        (&profile.input.embedding, Capability::Embedding),
        (&profile.input.reranking, Capability::Reranking),
    ];
    let mut cost = 0u64;
    for (target, capability) in roles {
        let Target::Cloud {
            connection_id,
            model,
        } = target
        else {
            continue;
        };
        if capability != Capability::Generation && !knowledge {
            continue;
        }
        let connecting = runtime_connections::get(c, workspace, connection_id).ok()?;
        connecting.verified_capability(model, capability)?;
        let generates = capability == Capability::Generation;
        let estimated_input = if capability == Capability::Reranking {
            20 * 4096 + settings.num_ctx as u64
        } else {
            settings.num_ctx as u64
        };
        let max_output = if generates {
            settings.reserved_output_budget() as u64
        } else {
            0
        };
        cost = cost.checked_add(crate::runtime_usage::price(
            connecting.limits_for(model),
            Some(estimated_input),
            Some(max_output),
            generates,
        )?)?;
    }
    Some(cost)
}

#[tauri::command]
pub async fn model_chat_routing(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Option<Routing>,
) -> AppResult<Routing> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        if let Some(input) = input {
            input.validate(&c, &workspace_id)?;
            c.execute(
                "INSERT INTO model_chat_routing VALUES(?1,?2) ON CONFLICT(workspace_id)
                DO UPDATE SET config_json=excluded.config_json",
                params![
                    workspace_id,
                    serde_json::to_string(&input).map_err(json_error)?
                ],
            )
            .map_err(error)?;
            Ok(input)
        } else {
            Routing::read(&c, &workspace_id)
        }
    })
    .await
}

#[tauri::command]
pub async fn chat_runtime_route(
    state: State<'_, AppState>,
    workspace_id: String,
    knowledge: bool,
    generation_target: Option<Target>,
) -> AppResult<Selection> {
    blocking(state.database.clone()?, move |db| {
        let settings = db.settings()?;
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        select_generation(&c, &workspace_id, knowledge, &settings, generation_target)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud::{
            ConnectionState,
            runtime_connections::{CloudTarget, ModelLimits, Probe},
        },
        db::Database,
        models::{new_id, now},
        runtime_profiles::{Mode, ProfileInput},
    };

    #[test]
    fn chat_router_prices_only_workspace_candidates_and_honors_task_preferences() {
        let db = Database::rag_evaluation_database().unwrap();
        let ws = db.create_workspace("路由").unwrap().id;
        let other = db.create_workspace("隔离").unwrap().id;
        let mut cloud =
            CloudTarget::loopback_test("http://127.0.0.1:1/".parse().unwrap(), "remote");
        cloud.connection.probes.push(Probe {
            model: "remote".into(),
            capability: Capability::Generation,
            state: ConnectionState::Verified,
            checked_at: now(),
            dimensions: None,
            usage: Value::Null,
        });
        let local = Profile {
            id: new_id(),
            updated_at: now(),
            input: ProfileInput {
                name: "本机".into(),
                mode: Mode::Local,
                generation: Target::Local { model: None },
                embedding: Target::Local { model: Some("vectors:latest".into()) },
                reranking: Target::Off,
            },
        };
        let paid = Profile {
            id: new_id(),
            updated_at: now(),
            input: ProfileInput {
                name: "远端".into(),
                mode: Mode::Cloud,
                generation: Target::Cloud {
                    connection_id: cloud.connection.id.clone(),
                    model: "remote".into(),
                },
                embedding: Target::Off,
                reranking: Target::Off,
            },
        };
        let c = db.connection().unwrap();
        for profile in [&local, &paid] {
            c.execute(
                "INSERT INTO model_runtime_profiles VALUES(?1,?2,?3,0)",
                params![ws, profile.id, serde_json::to_string(profile).unwrap()],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO model_cloud_connections VALUES(?1,?2,?3)",
            params![
                ws,
                cloud.connection.id,
                serde_json::to_string(&cloud.connection).unwrap()
            ],
        )
        .unwrap();
        let mut config = Routing {
            preference: Preference::Preferred,
            question: TaskRule {
                preferred: Some(local.id.clone()),
                alternative: None,
            },
            knowledge: TaskRule {
                preferred: Some(paid.id.clone()),
                alternative: Some(local.id.clone()),
            },
            local_backup: Some("backup".into()),
        };
        let save = |config: &Routing| {
            c.execute("INSERT INTO model_chat_routing VALUES(?1,?2) ON CONFLICT(workspace_id) DO UPDATE SET config_json=excluded.config_json",
            params![ws,serde_json::to_string(config).unwrap()]).unwrap()
        };
        assert!(config.validate(&c, &other).is_err());
        save(&config);
        let settings = ModelSettings::default();
        assert_eq!(
            select(&c, &ws, true, &settings)
                .unwrap()
                .profile
                .unwrap()
                .id,
            paid.id
        );
        assert_eq!(
            select(&c, &ws, false, &settings)
                .unwrap()
                .profile
                .unwrap()
                .id,
            local.id
        );
        assert!(
            select(&c, &other, true, &settings)
                .unwrap()
                .profile
                .is_none()
        );
        cloud.connection.limits.push(ModelLimits {
            model: "remote".into(),
            context_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_micro_usd_per_million: None,
            output_micro_usd_per_million: None,
            micro_usd_per_request: Some(35),
        });
        c.execute("UPDATE model_cloud_connections SET configuration_json=?1 WHERE workspace_id=?2 AND id=?3",
            params![serde_json::to_string(&cloud.connection).unwrap(),ws,cloud.connection.id]).unwrap();
        config.preference = Preference::LowerCost;
        save(&config);
        let economy = select(&c, &ws, true, &settings).unwrap();
        assert_eq!(economy.profile.unwrap().id, local.id);
        assert_eq!(economy.decision["reason"], "lower_estimated_cost");
        assert_eq!(economy.decision["preferredEstimatedMicros"], 35);
        assert_eq!(economy.local_backup.as_deref(), Some("backup"));

        let cloud_generation = Target::Cloud {
            connection_id: cloud.connection.id.clone(),
            model: "remote".into(),
        };
        let manual = select_generation(&c, &ws, true, &settings, Some(cloud_generation.clone()))
            .unwrap();
        let model = manual.profile.unwrap();
        assert_eq!(model.id, "manual-generation");
        assert!(matches!(model.input.generation, Target::Cloud { .. }));
        assert!(matches!(model.input.mode, Mode::Hybrid));
        assert!(matches!(model.input.embedding, Target::Local { model: Some(name) } if name == "vectors:latest"));
        assert_eq!(manual.decision["reason"], "manual_generation");
        assert!(manual.local_backup.is_none());
        assert!(select_generation(&c, &other, true, &settings, Some(cloud_generation)).is_err());
        let local = select_generation(&c, &ws, true, &settings, Some(Target::Local {
            model: Some("installed:latest".into()),
        })).unwrap();
        assert!(matches!(local.profile.unwrap().input.generation, Target::Local { model: Some(name) } if name == "installed:latest"));
        assert!(local.local_backup.is_none());

        // An unavailable retrieval connection must only affect turns using knowledge.
        let mut hybrid = get(&c, &ws, config.question.preferred.as_ref().unwrap()).unwrap();
        hybrid.input.mode = Mode::Hybrid;
        hybrid.input.embedding = Target::Cloud { connection_id: new_id(), model: "vectors".into() };
        c.execute("UPDATE model_runtime_profiles SET profile_json=?1 WHERE workspace_id=?2 AND id=?3",
            params![serde_json::to_string(&hybrid).unwrap(), ws, hybrid.id]).unwrap();
        let generation = Target::Local { model: Some("installed:latest".into()) };
        for target in [None, Some(generation)] {
            let question = select_generation(&c, &ws, false, &settings, target).unwrap().profile.unwrap();
            assert!(matches!(question.input.mode, Mode::Local));
            assert!(!question.has_cloud_target());
            assert!(matches!(question.input.embedding, Target::Off));
        }
    }
}

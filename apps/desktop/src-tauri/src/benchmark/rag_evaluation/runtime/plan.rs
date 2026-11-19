use super::*;
use crate::{
    cloud::runtime_connections::{self, Capability},
    runtime_profiles::{Mode, Profile, Target},
    runtime_usage,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    arms: [ArmInput; 2],
    case_id: Option<String>,
    pub(super) max_cost_micros: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArmInput {
    profile_id: Option<String>,
    policy: Policy,
}

pub(super) struct Arm {
    pub profile: Profile,
    pub policy: Policy,
    pub settings: ModelSettings,
    pub local_generation: Option<String>,
    pub local_embedding: Option<(String, String)>,
    pub audit: Value,
}

pub(super) struct Plan {
    pub arms: [Arm; 2],
    pub case_ids: Vec<String>,
    pub audit: Value,
    pub hash: String,
    pub local_preferences: crate::local_runtime::Preferences,
}

fn candidate(db: &Database, workspace: &str, input: &ArmInput) -> AppResult<Arm> {
    let mut settings = db.settings()?;
    let mut profile = {
        let c = db.connection()?;
        require_workspace(&c, workspace)?;
        input
            .profile_id
            .as_deref()
            .map(|id| crate::runtime_profiles::get(&c, workspace, id))
            .transpose()?
            .unwrap_or_else(Profile::default_local)
    };
    let source_mode = profile.input.mode;
    let targets = &mut profile.input;
    if !input.policy.semantic() {
        targets.embedding = Target::Off;
    } else if matches!(targets.embedding, Target::Off) {
        return Err("语义检索需要在运行方案中选择向量模型。".into());
    }
    if input.policy != Policy::HybridRerank {
        targets.reranking = Target::Off;
    } else if matches!(targets.reranking, Target::Off) {
        return Err("混合检索加重排需要在运行方案中启用重排。".into());
    }
    let cloud = [&targets.generation, &targets.embedding, &targets.reranking]
        .into_iter()
        .filter(|target| matches!(target, Target::Cloud { .. }))
        .count();
    let local = [&targets.generation, &targets.embedding, &targets.reranking]
        .into_iter()
        .any(|target| matches!(target, Target::Local { .. }));
    targets.mode = match (cloud, local) {
        (0, _) => Mode::Local,
        (_, true) => Mode::Hybrid,
        _ => Mode::Cloud,
    };
    match &mut targets.generation {
        Target::Local { model } => {
            settings.model = model.get_or_insert_with(|| settings.model.clone()).clone()
        }
        Target::Cloud { model, .. } => settings.model = model.clone(),
        Target::Off => return Err("运行方案缺少回答模型。".into()),
    }
    if let Target::Local { model } = &mut targets.embedding
        && model.is_none()
    {
        *model = db.rag_status(workspace)?["index"]["model"]
            .as_str()
            .map(str::to_owned);
        if model.is_none() {
            return Err("请为运行方案选定本地向量模型，或先设置默认向量模型。".into());
        }
    }
    settings.validate()?;
    let audit = json!({"profile":profile.audit(),"configuredMode":source_mode,"settings":settings,"strategy":input.policy});
    Ok(Arm {
        profile,
        policy: input.policy,
        settings,
        local_generation: None,
        local_embedding: None,
        audit,
    })
}

impl Arm {
    fn targets(&self) -> [(&Target, Capability); 3] {
        let input = &self.profile.input;
        [
            (&input.generation, Capability::Generation),
            (&input.embedding, Capability::Embedding),
            (&input.reranking, Capability::Reranking),
        ]
    }

    pub(super) fn uses_local(&self) -> bool {
        self.targets()
            .iter()
            .any(|(target, _)| matches!(target, Target::Local { .. }))
    }

    fn model_audit(&mut self, db: &Database, workspace: &str, tags: &Value) -> AppResult<()> {
        let fingerprint = |model: &str| -> AppResult<String> {
            tags["models"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|row| row["name"] == model)
                .and_then(|row| row["digest"].as_str())
                .map(|digest| digest.trim_start_matches("sha256:"))
                .filter(|digest| {
                    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
                })
                .map(str::to_owned)
                .ok_or_else(|| format!("本地模型 {model} 未安装或缺少版本指纹。"))
        };
        let local_generation = match &self.profile.input.generation {
            Target::Local { model: Some(model) } => Some(fingerprint(model)?),
            _ => None,
        };
        let local_embedding = match &self.profile.input.embedding {
            Target::Local { model: Some(model) } => Some((model.clone(), fingerprint(model)?)),
            _ => None,
        };
        let mut cloud = Vec::new();
        for (target, capability) in self.targets() {
            if let Target::Cloud {
                connection_id,
                model,
            } = target
            {
                let c = db.connection()?;
                let entry = runtime_connections::get(&c, workspace, connection_id)?;
                let probe = entry
                    .verified_capability(model, capability)
                    .ok_or("所选云端模型未完成对应能力验证。")?;
                let limits = entry.limits_for(model);
                if runtime_usage::price(
                    limits,
                    Some(1),
                    Some(1),
                    capability == Capability::Generation,
                )
                .is_none()
                {
                    return Err("请先为实验使用的云端环节填写适用费率。".into());
                }
                cloud.push(json!({"capability":capability,"connectionId":connection_id,
                    "provider":entry.provider,"model":model,"verifiedAt":probe.checked_at,
                    "limits":limits,"configurationHash":hash(json!(entry).to_string().as_bytes()),
                    "destination":entry.display_destination()}));
            }
        }
        self.local_generation = local_generation;
        self.local_embedding = local_embedding;
        self.audit["localGenerationDigest"] = json!(self.local_generation);
        self.audit["localEmbedding"] = self.local_embedding.as_ref().map_or(
            Value::Null,
            |(model, digest)| json!({"model":model,"digest":digest}),
        );
        self.audit["cloud"] = json!(cloud);
        Ok(())
    }
}

pub(super) async fn prepare(
    db: &Database,
    workspace: &str,
    suite: &Suite,
    input: &Input,
    provider: Option<&Ollama>,
) -> AppResult<Plan> {
    let [left, right] = &input.arms;
    let mut arms = [
        candidate(db, workspace, left)?,
        candidate(db, workspace, right)?,
    ];
    let has_cloud = arms.iter().any(|arm| arm.profile.has_cloud_target());
    if has_cloud
        && input
            .max_cost_micros
            .is_none_or(|cost| cost == 0 || cost > i64::MAX as u64)
    {
        return Err("含云端请求的对照需要填写正数费用上限。".into());
    }
    let tags = if arms.iter().any(|arm| {
        matches!(arm.profile.input.generation, Target::Local { .. })
            || matches!(arm.profile.input.embedding, Target::Local { .. })
    }) {
        provider
            .ok_or("本地 Ollama 服务不可用。")?
            .get("/api/tags")
            .await?
    } else {
        Value::Null
    };
    for arm in &mut arms {
        arm.model_audit(db, workspace, &tags)?;
    }
    if arms[0].policy == arms[1].policy
        && arms[0].audit["profile"]["generation"] == arms[1].audit["profile"]["generation"]
        && arms[0].audit["profile"]["embedding"] == arms[1].audit["profile"]["embedding"]
        && arms[0].audit["profile"]["reranking"] == arms[1].audit["profile"]["reranking"]
    {
        return Err("请为两组选择不同的模型或查找策略。".into());
    }
    let case_ids = suite
        .cases
        .iter()
        .filter(|case| input.case_id.as_ref().is_none_or(|id| id == &case.id))
        .map(|case| case.id.clone())
        .collect::<Vec<_>>();
    if case_ids.is_empty() {
        return Err("固定题库里没有这道题。".into());
    }
    let max_cost = input.max_cost_micros.filter(|_| has_cloud);
    let local_preferences = {
        let c = db.connection()?;
        crate::local_runtime::Preferences::read(&c)?
    };
    let audit = json!({"workspaceId":workspace,"dataset":suite.metadata(),"scorer":score::RUNTIME_VERSION,
        "caseIds":case_ids,"arms":[arms[0].audit,arms[1].audit],"containsCloudCalls":has_cloud,
        "costCapMicros":max_cost,"maxCostCurrency":"USD","transferScope":"bundled_corpus_questions_and_context",
        "localGenerationPreferences":local_preferences,
        "usesLocalModels":arms.iter().any(|arm|arm.local_generation.is_some() || arm.local_embedding.is_some())});
    Ok(Plan {
        hash: hash(audit.to_string().as_bytes()),
        audit,
        arms,
        case_ids,
        local_preferences,
    })
}

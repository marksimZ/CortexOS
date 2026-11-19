use super::*;
use std::collections::{BTreeMap, BTreeSet};

const SOURCE: &str = include_str!("../../../datasets/cortex-rag-v1/suite.json");

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Suite {
    pub id: String,
    pub version: u32,
    pub snapshot_date: String,
    pub documents: Vec<Document>,
    relations: Vec<Relation>,
    pub cases: Vec<Case>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub id: String,
    title: String,
    versions: Vec<Revision>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    from: String,
    until: Option<String>,
    text: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Relation {
    from: String,
    to: String,
    kind: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Case {
    pub id: String,
    pub kind: String,
    pub question: String,
    pub fact_date: Option<String>,
    pub gold: Vec<(String, i64)>,
    pub semantic_hints: Vec<String>,
    #[serde(default)]
    pub conflict: bool,
    pub replay: Replay,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Replay {
    #[serde(default)]
    pub abstained: bool,
    #[serde(default)]
    pub conflict_detected: bool,
    pub claims: Vec<(String, usize)>,
}

impl Suite {
    pub fn load() -> AppResult<Self> {
        let invalid = || "固定检索题库内容不一致。".to_owned();
        let suite: Self = serde_json::from_str(SOURCE).map_err(|_| invalid())?;
        knowledge_scope::check_date(&suite.snapshot_date)?;
        let documents: BTreeMap<_, _> = suite
            .documents
            .iter()
            .map(|doc| (doc.id.as_str(), doc))
            .collect();
        if documents.len() != suite.documents.len()
            || documents.is_empty()
            || suite.cases.is_empty()
        {
            return Err(invalid());
        }
        for doc in &suite.documents {
            if doc.versions.is_empty()
                || !doc
                    .versions
                    .windows(2)
                    .all(|pair| pair[0].from < pair[1].from)
            {
                return Err(invalid());
            }
            for version in &doc.versions {
                knowledge_scope::Validity {
                    valid_from: Some(version.from.clone()),
                    valid_to: version.until.clone(),
                }
                .validate()?;
                if version.text.trim().is_empty() {
                    return Err(invalid());
                }
            }
        }
        let mut ids = BTreeSet::new();
        for case in &suite.cases {
            let date = case.fact_date.as_deref().unwrap_or(&suite.snapshot_date);
            knowledge_scope::check_date(date)?;
            if !ids.insert(&case.id)
                || case.question.trim().is_empty()
                || case
                    .semantic_hints
                    .iter()
                    .any(|id| !documents.contains_key(id.as_str()))
                || case.gold.iter().collect::<BTreeSet<_>>().len() != case.gold.len()
                || case.replay.claims.iter().any(|(text, ordinal)| {
                    text.is_empty() || text.contains("[资料") || *ordinal == 0
                })
            {
                return Err(invalid());
            }
            for (id, version) in &case.gold {
                let doc = documents.get(id.as_str()).ok_or_else(invalid)?;
                let effective = doc.versions.iter().rposition(|v| v.from.as_str() <= date);
                if effective != usize::try_from(*version - 1).ok()
                    || effective.is_none_or(|i| {
                        doc.versions[i]
                            .until
                            .as_deref()
                            .is_some_and(|until| until <= date)
                    })
                {
                    return Err(invalid());
                }
            }
        }
        Ok(suite)
    }

    pub fn metadata(&self) -> Value {
        let snapshot = json!({"date":self.snapshot_date,"documents":self.documents,"relations":self.relations});
        json!({"id":self.id,"version":self.version,"snapshotDate":self.snapshot_date,
            "snapshotHash":hash(snapshot.to_string().as_bytes()),"datasetHash":hash(json!(self).to_string().as_bytes()),
            "scorerVersion":score::VERSION,"policyVersion":POLICY_VERSION,"caseCount":self.cases.len(),
            "documentCount":self.documents.len()})
    }

    pub fn vector(&self, hints: &[String]) -> Vec<f32> {
        let count = self
            .documents
            .iter()
            .filter(|doc| hints.contains(&doc.id))
            .count();
        let strength = 1.0 / (count.max(1) as f32).sqrt();
        let mut vector: Vec<_> = self
            .documents
            .iter()
            .map(|doc| {
                if hints.contains(&doc.id) {
                    strength
                } else {
                    0.0
                }
            })
            .collect();
        vector.push(if count == 0 { 1.0 } else { 0.0 });
        vector
    }

    pub fn database(&self, snapshot_hash: &str, semantic: bool) -> AppResult<Database> {
        self.database_at(snapshot_hash, semantic, &self.snapshot_date)
    }

    pub fn database_at(
        &self,
        snapshot_hash: &str,
        semantic: bool,
        date: &str,
    ) -> AppResult<Database> {
        knowledge_scope::check_date(date)?;
        let database = Database::rag_evaluation_database()?;
        {
            let mut connection = database.connection()?;
            let tx = connection.transaction().map_err(error)?;
            let view: String = tx.query_row("SELECT sql FROM sqlite_schema WHERE type='view' AND name='rag_current_knowledge'", [], |r| r.get(0)).map_err(error)?;
            let clock = "date('now','localtime')";
            if !view.contains(clock) {
                return Err("固定评测无法设置基准日期。".into());
            }
            // The bundled date was parsed and round-tripped before it enters this fixture-only view.
            tx.execute_batch(&format!(
                "DROP VIEW rag_current_knowledge; {};",
                view.replace(clock, &format!("'{date}'"))
            ))
            .map_err(error)?;
            let timestamp = format!("{}T00:00:00.000Z", self.snapshot_date);
            tx.execute(
                "INSERT INTO workspaces VALUES(?1,'固定检索题库',?2,?2)",
                params![WORKSPACE, timestamp],
            )
            .map_err(error)?;
            for doc in &self.documents {
                let latest = doc.versions.len() as i64;
                let current = doc
                    .versions
                    .iter()
                    .rposition(|version| {
                        version.from.as_str() <= date
                            && version.until.as_deref().is_none_or(|until| until > date)
                    })
                    .map(|i| i as i64 + 1);
                let source = format!("source-{}", doc.id);
                let name = format!("{}.md", doc.id);
                tx.execute("INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at) VALUES(?1,?2,?3,?4,?3,?5,?5)", params![doc.id,WORKSPACE,latest,current,timestamp]).map_err(error)?;
                tx.execute(
                    "INSERT INTO source_documents VALUES(?1,?2,?3,?3,?4,1,?4,?5,?5)",
                    params![source, WORKSPACE, name, latest, timestamp],
                )
                .map_err(error)?;
                for (index, revision) in doc.versions.iter().enumerate() {
                    let version = (index + 1) as i64;
                    let at = format!("{}T00:00:00.000Z", revision.from);
                    let digest = hash(revision.text.as_bytes());
                    tx.execute("INSERT INTO knowledge_versions VALUES(?1,?2,?3,'knowledge',?4,'user','固定评测资料',?5)", params![doc.id,version,doc.title,revision.text,at]).map_err(error)?;
                    tx.execute(
                        "INSERT INTO knowledge_approval_events VALUES(?1,?2,?3,'user',?4)",
                        params![format!("{}:{version}", doc.id), doc.id, version, at],
                    )
                    .map_err(error)?;
                    tx.execute(
                        "INSERT INTO knowledge_validity VALUES(?1,?2,?3,?4)",
                        params![doc.id, version, revision.from, revision.until],
                    )
                    .map_err(error)?;
                    tx.execute(
                        "INSERT INTO source_versions VALUES(?1,?2,?3,?4,?4,?5)",
                        params![source, version, revision.text, digest, at],
                    )
                    .map_err(error)?;
                    tx.execute(
                        "INSERT INTO knowledge_source_links VALUES(?1,?2,?3,?2,1,?4,?5)",
                        params![
                            doc.id,
                            version,
                            source,
                            revision.text.split('\n').count() as i64,
                            revision.text
                        ],
                    )
                    .map_err(error)?;
                }
            }
            for (index, relation) in self.relations.iter().enumerate() {
                let id = format!("edge-{}", index + 1);
                tx.execute(
                    "INSERT INTO knowledge_edges VALUES(?1,?2,1)",
                    params![id, WORKSPACE],
                )
                .map_err(error)?;
                tx.execute("INSERT INTO knowledge_edge_versions VALUES(?1,1,?2,?3,?4,?5,1,'固定评测关系',?6)", params![id,WORKSPACE,relation.from,relation.to,relation.kind,timestamp]).map_err(error)?;
            }
            crate::retrieval::rebuild(&tx)?;
            tx.commit().map_err(error)?;
        }
        if semantic {
            let vectors = self
                .documents
                .iter()
                .map(|doc| (doc.id.as_str(), self.vector(std::slice::from_ref(&doc.id))))
                .collect();
            database.install_evaluation_vectors(WORKSPACE, snapshot_hash, &vectors)?;
        }
        Ok(database)
    }
}

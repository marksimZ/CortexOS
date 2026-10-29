use crate::{
    db::Database,
    models::{AppResult, now},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../../datasets/cortex-core-60/manifest.json");
const CASES: &str = include_str!("../../datasets/cortex-core-60/cases.json");
const SCHEMA: &str = include_str!("../../datasets/cortex-core-60/case.schema.json");
const RULES: &str = include_str!("../../datasets/cortex-core-60/scoring-rules.json");
#[path = "dataset_v2.rs"]
mod v2;

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn invalid() -> String {
    "测试集校验失败，请保留数据并检查应用版本。".into()
}
fn encode<T: Serialize + ?Sized>(v: &T) -> AppResult<String> {
    serde_json::to_string(v).map_err(|_| invalid())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case_id: String,
    pub dataset_version: u32,
    pub category: String,
    pub input: Input,
    pub expected: Expected,
    pub scoring: Scoring,
    pub difficulty: String,
    pub output_format: String,
    pub requirements: Vec<String>,
    pub fixture: Vec<Source>,
    pub content_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub prompt: String,
    pub response_schema: Option<Value>,
    pub tools: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scoring {
    pub id: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub lines: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub source_id: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assertion {
    pub args: Vec<Value>,
    pub output: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expected {
    Aliases {
        answers: Vec<String>,
    },
    Integer {
        answer: i64,
    },
    Python {
        function: String,
        tests: Vec<Assertion>,
    },
    Evidence {
        disposition: String,
        answers: Vec<String>,
        citations: Vec<Citation>,
    },
    Json {
        answer: Value,
    },
    Tool {
        name: String,
        arguments: Value,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSummary {
    pub id: String,
    pub version: u32,
    pub content_hash: String,
    pub manifest: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Value>,
}

pub struct Bundle {
    pub manifest: Value,
    pub cases: Vec<Case>,
    pub content_hash: String,
}

fn nonempty(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|s| !s.trim().is_empty())
}

// Checks only the bounded schema vocabulary used by our authored fixtures.
// Model-output JSON Schema validation belongs to the scoring slice.
fn example_matches(value: &Value, schema: &Value) -> bool {
    let typed = match schema["type"].as_str() {
        Some("object") => value.as_object().is_some_and(|object| {
            let Some(properties) = schema["properties"].as_object() else {
                return false;
            };
            schema["additionalProperties"] == false
                && object.len() == properties.len()
                && properties
                    .iter()
                    .all(|(key, rule)| object.get(key).is_some_and(|v| example_matches(v, rule)))
                && schema["required"].as_array().is_some_and(|r| {
                    r.len() == properties.len() && properties.keys().all(|k| r.contains(&json!(k)))
                })
        }),
        Some("array") => value.as_array().is_some_and(|a| {
            a.len() as u64 >= schema["minItems"].as_u64().unwrap_or(0)
                && a.len() as u64 <= schema["maxItems"].as_u64().unwrap_or(u64::MAX)
                && a.iter().all(|v| example_matches(v, &schema["items"]))
        }),
        Some("integer") => value.as_i64().is_some_and(|n| {
            n >= schema["minimum"].as_i64().unwrap_or(i64::MIN)
                && n <= schema["maximum"].as_i64().unwrap_or(i64::MAX)
        }),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => false,
    };
    typed
        && schema
            .get("enum")
            .is_none_or(|e| e.as_array().is_some_and(|a| a.contains(value)))
}

pub fn builtin() -> AppResult<Bundle> {
    validate(MANIFEST, CASES, SCHEMA, RULES)
}

pub fn builtin_version(version: u32) -> AppResult<Bundle> {
    match version {
        1 => builtin(),
        2 => v2::builtin(),
        _ => Err(invalid()),
    }
}

pub(crate) fn validate_runtime_case(case: &Case) -> AppResult<()> {
    if case.dataset_version != 2 && !case.case_id.starts_with("custom-") {
        return Err(invalid());
    }
    let mut raw = serde_json::to_value(case).map_err(|_| invalid())?;
    raw.as_object_mut()
        .ok_or_else(invalid)?
        .remove("content_hash");
    if hash(encode(&raw)?.as_bytes()) != case.content_hash {
        return Err(invalid());
    }
    let rules: Value = serde_json::from_str(include_str!(
        "../../datasets/cortex-core-60-v2/scoring-rules.json"
    ))
    .map_err(|_| invalid())?;
    if case.case_id.starts_with("custom-") {
        v2::validate_custom(case, &rules)
    } else {
        v2::validate_case(case, &rules)
    }
}

fn validate(manifest: &str, cases: &str, schema: &str, rules: &str) -> AppResult<Bundle> {
    let m: Value = serde_json::from_str(manifest).map_err(|_| invalid())?;
    let rules_value: Value = serde_json::from_str(rules).map_err(|_| invalid())?;
    let schema_value: Value = serde_json::from_str(schema).map_err(|_| invalid())?;
    let version =
        u32::try_from(m["version"].as_u64().ok_or_else(invalid)?).map_err(|_| invalid())?;
    if m["id"] != "cortex-core-60"
        || ![1, 2].contains(&version)
        || m["schema_version"] != version
        || m["case_count"] != 60
        || m["review_status"] != "needs_human_review"
        || rules_value["version"] != version
        || rules_value["document_type"] != "scoring_specification"
        || schema_value["properties"]["dataset_version"]["const"] != version
        || schema_value["properties"]["scoring"]["properties"]["version"]["const"] != version
    {
        return Err(invalid());
    }
    for (file, bytes) in [
        ("cases.json", cases),
        ("case.schema.json", schema),
        ("scoring-rules.json", rules),
    ] {
        if m["files"][file].as_str() != Some(hash(bytes.as_bytes()).as_str()) {
            return Err(invalid());
        }
    }
    let raw: Vec<Value> = serde_json::from_str(cases).map_err(|_| invalid())?;
    let parsed: Vec<Case> = serde_json::from_str(cases).map_err(|_| invalid())?;
    let mut order = Vec::new();
    for (prefix, count) in [
        ("zh", 15),
        ("math", 15),
        ("code", 10),
        ("rag", 10),
        ("schema", 5),
        ("tool", 5),
    ] {
        for i in 1..=count {
            order.push(format!("{prefix}-{i:03}"));
        }
    }
    if parsed.len() != 60
        || m["case_order"] != json!(order)
        || m["distribution"]
            != json!({"zh_knowledge":15,"math_logic":15,"coding":10,"rag_citation":10,"structured_tools":10})
    {
        return Err(invalid());
    }
    for ((c, raw), id) in parsed.iter().zip(raw).zip(order) {
        let mut unhashed = raw;
        unhashed
            .as_object_mut()
            .ok_or_else(invalid)?
            .remove("content_hash");
        if c.content_hash != hash(encode(&unhashed)?.as_bytes())
            || c.case_id != id
            || c.dataset_version != version
            || c.input.prompt.trim().is_empty()
            || c.input.prompt.len() > 16_384
            || !["easy", "medium", "hard"].contains(&c.difficulty.as_str())
            || c.requirements.iter().collect::<BTreeSet<_>>().len() != c.requirements.len()
        {
            return Err(invalid());
        }
        if version == 2 {
            v2::validate_case(c, &rules_value)?;
            continue;
        }
        if c.scoring.contract.is_some() {
            return Err(invalid());
        }
        let (category, scoring, format, requirements) = match &c.expected {
            Expected::Aliases { answers } if nonempty(answers) => {
                ("zh_knowledge", "exact_alias", "text", vec!["completion"])
            }
            Expected::Integer { .. } => {
                ("math_logic", "integer_equal", "integer", vec!["completion"])
            }
            Expected::Python { function, tests }
                if !function.is_empty()
                    && function.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    && tests.len() >= 3 =>
            {
                (
                    "coding",
                    "python_assertions",
                    "python",
                    vec!["completion", "python_sandbox"],
                )
            }
            Expected::Evidence {
                disposition,
                answers,
                citations,
            } => {
                let ids: BTreeSet<_> = c.fixture.iter().map(|s| &s.id).collect();
                if c.fixture.is_empty()
                    || ids.len() != c.fixture.len()
                    || c.fixture
                        .iter()
                        .any(|s| s.id.is_empty() || s.title.is_empty() || !nonempty(&s.lines))
                    || citations.iter().any(|span| {
                        c.fixture
                            .iter()
                            .find(|s| s.id == span.source_id)
                            .is_none_or(|s| {
                                span.start == 0 || span.end < span.start || span.end > s.lines.len()
                            })
                    })
                    || citations
                        .iter()
                        .map(|s| (&s.source_id, s.start, s.end))
                        .collect::<BTreeSet<_>>()
                        .len()
                        != citations.len()
                {
                    return Err(invalid());
                }
                match disposition.as_str() {
                    "answerable" if nonempty(answers) && !citations.is_empty() => (),
                    "unanswerable" if answers.is_empty() && citations.is_empty() => (),
                    "conflict"
                        if nonempty(answers)
                            && citations
                                .iter()
                                .map(|s| &s.source_id)
                                .collect::<BTreeSet<_>>()
                                .len()
                                >= 2 =>
                    {
                        ()
                    }
                    _ => return Err(invalid()),
                }
                ("rag_citation", "evidence_match", "json", vec!["completion"])
            }
            Expected::Json { answer }
                if c.input
                    .response_schema
                    .as_ref()
                    .is_some_and(|schema| example_matches(answer, schema)) =>
            {
                (
                    "structured_tools",
                    "schema_semantic",
                    "json",
                    vec!["completion", "json_schema"],
                )
            }
            Expected::Tool { name, arguments }
                if c.input.tools.len() == 1
                    && c.input.tools[0]["type"] == "function"
                    && c.input.tools[0]["function"]["name"] == *name
                    && example_matches(arguments, &c.input.tools[0]["function"]["parameters"]) =>
            {
                (
                    "structured_tools",
                    "native_tool_equal",
                    "tool_calls",
                    vec!["completion", "tool_calls"],
                )
            }
            _ => return Err(invalid()),
        };
        let expected_scoring = match id.split('-').next() {
            Some("zh") => "exact_alias",
            Some("math") => "integer_equal",
            Some("code") => "python_assertions",
            Some("rag") => "evidence_match",
            Some("schema") => "schema_semantic",
            Some("tool") => "native_tool_equal",
            _ => return Err(invalid()),
        };
        if scoring != expected_scoring
            || c.category != category
            || c.scoring.id != scoring
            || c.scoring.version != 1
            || c.output_format != format
            || c.requirements != requirements
            || (!matches!(c.expected, Expected::Evidence { .. }) && !c.fixture.is_empty())
            || (!matches!(c.expected, Expected::Json { .. }) && c.input.response_schema.is_some())
            || (!matches!(c.expected, Expected::Tool { .. }) && !c.input.tools.is_empty())
            || !rules_value["rules"].as_array().is_some_and(|rules| {
                rules
                    .iter()
                    .any(|r| r["id"] == scoring && r["version"] == 1)
            })
        {
            return Err(invalid());
        }
    }
    Ok(Bundle {
        manifest: m,
        cases: parsed,
        content_hash: hash(manifest.as_bytes()),
    })
}

pub fn require_workspace(connection: &Connection, workspace_id: &str) -> AppResult<()> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)",
            params![workspace_id],
            |r| r.get(0),
        )
        .map_err(|_| invalid())?;
    if exists {
        Ok(())
    } else {
        Err("所选工作区不存在。".into())
    }
}

impl Database {
    pub fn benchmark_dataset_cases(
        &self,
        workspace: &str,
        id: &str,
        version: u32,
    ) -> AppResult<Vec<Case>> {
        if id != "cortex-core-60" {
            return self.personalized_cases(workspace, id, version);
        }
        self.benchmark_datasets(workspace)?;
        let bundle = builtin_version(version)?;
        let c = self.connection()?;
        bundle
            .cases
            .iter()
            .map(|case| read_case(&c, id, version, &case.case_id))
            .collect()
    }
    /// Installs only immutable built-in definitions, atomically; never touches chat or knowledge.
    pub fn benchmark_datasets(&self, workspace_id: &str) -> AppResult<Vec<DatasetSummary>> {
        let mut c = self.connection()?;
        require_workspace(&c, workspace_id)?;
        let tx = c.transaction().map_err(|_| invalid())?;
        let mut summaries = Vec::new();
        for version in [1, 2] {
            let bundle = builtin_version(version)?;
            let existing: Option<(String,String,String)> = tx.query_row("SELECT content_hash,manifest_json,provenance_json FROM benchmark_datasets WHERE id='cortex-core-60' AND version=?1",params![version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_| invalid())?;
            if let Some((existing, manifest, provenance)) = existing {
                if existing != bundle.content_hash
                    || manifest != encode(&bundle.manifest)?
                    || provenance != encode(&bundle.manifest["provenance"])?
                {
                    return Err(
                        "已有同版本测试集与应用内容不一致；请使用新版本，不能覆盖历史题目。".into(),
                    );
                }
            } else {
                tx.execute(
                    "INSERT INTO benchmark_datasets VALUES(?1,?2,?3,?2,?4,?5,?6,?7)",
                    params![
                        "cortex-core-60",
                        version,
                        bundle.manifest["name"].as_str().ok_or_else(invalid)?,
                        bundle.content_hash,
                        encode(&bundle.manifest)?,
                        encode(&bundle.manifest["provenance"])?,
                        now()
                    ],
                )
                .map_err(|_| invalid())?;
                for case in &bundle.cases {
                    tx.execute(
                    "INSERT INTO benchmark_cases VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    params![
                        "cortex-core-60",
                        version,
                        case.case_id,
                        case.category,
                        encode(&case.input)?,
                        encode(&case.expected)?,
                        encode(&case.scoring)?,
                        case.difficulty,
                        case.output_format,
                        encode(&case.requirements)?,
                        encode(&case.fixture)?,
                        case.content_hash
                    ],
                )
                .map_err(|_| invalid())?;
                }
            }
            // Check stored rows as well as the bundle, including after an idempotent re-open.
            let count: u32 = tx.query_row("SELECT count(*) FROM benchmark_cases WHERE dataset_id='cortex-core-60' AND dataset_version=?1",params![version],|r|r.get(0)).map_err(|_| invalid())?;
            if count != 60 {
                return Err(invalid());
            }
            for case in &bundle.cases {
                if encode(&read_case(&tx, "cortex-core-60", version, &case.case_id)?)?
                    != encode(case)?
                {
                    return Err(invalid());
                }
            }
            summaries.push(DatasetSummary {
                id: "cortex-core-60".into(),
                version,
                content_hash: bundle.content_hash,
                manifest: bundle.manifest,
                runtime: (version==2).then(||json!({"engineVersion":super::evaluation::ENGINE_VERSION,"singleCaseExecution":true,"scorers":["exact_alias","integer_equal","schema_semantic","evidence_match","native_tool_equal","python_assertions"],"pythonRequires":"local_linux_docker_and_explicit_image_id","frozenManifestDescribes":"stage_a_creation_state"})),
            });
        }
        tx.commit().map_err(|_| invalid())?;
        drop(c);
        summaries.extend(self.personalized_datasets(workspace_id)?);
        Ok(summaries)
    }
    pub fn benchmark_case(
        &self,
        workspace_id: &str,
        dataset_id: &str,
        version: u32,
        case_id: &str,
    ) -> AppResult<Case> {
        if dataset_id != "cortex-core-60" {
            self.personalized_dataset(workspace_id, dataset_id, version)?;
        } else {
            self.benchmark_datasets(workspace_id)?;
        }
        let c = self.connection()?;
        read_case(&c, dataset_id, version, case_id)
    }
}

fn read_case(c: &Connection, dataset: &str, version: u32, id: &str) -> AppResult<Case> {
    c.query_row("SELECT case_id,category,input_json,expected_json,scoring_json,difficulty,output_format,requirements_json,fixture_json,content_hash FROM benchmark_cases WHERE dataset_id=?1 AND dataset_version=?2 AND case_id=?3",params![dataset,version,id],|r| {
        let parse = |i| -> rusqlite::Result<Value> { let s:String=r.get(i)?; serde_json::from_str(&s).map_err(|_|rusqlite::Error::InvalidQuery) };
        let value=json!({"case_id":r.get::<_,String>(0)?,"dataset_version":version,"category":r.get::<_,String>(1)?,"input":parse(2)?,"expected":parse(3)?,"scoring":parse(4)?,"difficulty":r.get::<_,String>(5)?,"output_format":r.get::<_,String>(6)?,"requirements":parse(7)?,"fixture":parse(8)?,"content_hash":r.get::<_,String>(9)?});
        serde_json::from_value(value).map_err(|_|rusqlite::Error::InvalidQuery)
    }).map_err(|_| "所选题目不存在或格式不受支持。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edited_case(edit: impl FnOnce(&mut Value)) -> AppResult<Bundle> {
        let mut cases: Value = serde_json::from_str(CASES).unwrap();
        edit(&mut cases);
        for c in cases.as_array_mut().unwrap() {
            c.as_object_mut().unwrap().remove("content_hash");
            c["content_hash"] = json!(hash(encode(c).unwrap().as_bytes()));
        }
        let cases = encode(&cases).unwrap();
        let mut manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        manifest["files"]["cases.json"] = json!(hash(cases.as_bytes()));
        validate(&encode(&manifest).unwrap(), &cases, SCHEMA, RULES)
    }

    #[test]
    fn benchmark_bundle_checks_all_sixty_and_rejects_bad_standards() {
        let bundle = builtin().unwrap();
        assert_eq!(bundle.cases.len(), 60);
        assert_eq!(bundle.manifest["review_status"], "needs_human_review");
        assert!(validate(MANIFEST, &format!("{CASES} "), SCHEMA, RULES).is_err());
        assert!(edited_case(|c| c[1]["case_id"] = json!("zh-001")).is_err());
        assert!(
            edited_case(|c| {
                let mut replacement = c[15].clone();
                replacement["case_id"] = json!("zh-001");
                c[0] = replacement;
            })
            .is_err()
        );
        assert!(edited_case(|c| c[0]["expected"]["answers"] = json!([])).is_err());
        assert!(edited_case(|c| c[40]["expected"]["citations"][0]["start"] = json!(0)).is_err());
        assert!(edited_case(|c| c[40]["expected"]["citations"][0]["end"] = json!(900)).is_err());
        assert!(
            edited_case(|c| c[40]["expected"]["citations"][0]["source_id"] = json!("missing"))
                .is_err()
        );
        assert!(edited_case(|c| c[50]["expected"]["answer"]["age"] = json!("28")).is_err());
        assert!(edited_case(|c| c[55]["expected"]["name"] = json!("other_tool")).is_err());
        assert!(edited_case(|c| c[0]["scoring"]["version"] = json!(2)).is_err());
        assert!(edited_case(|c| c[0]["unexpected"] = json!(true)).is_err());
    }

    #[test]
    fn benchmark_bundle_install_is_atomic_idempotent_and_preserves_chat() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../artifacts/benchmark-slice2")
            .join(crate::models::new_id());
        let db = Database::open(&directory).unwrap();
        let workspace = db.create_workspace("slice2 fixture").unwrap();
        let conversation = db.create_conversation(&workspace.id).unwrap();
        let before = encode(&db.bootstrap().unwrap()).unwrap();
        assert!(db.benchmark_datasets("missing").is_err());
        db.connection().unwrap().execute_batch("CREATE TRIGGER fail_fixture BEFORE INSERT ON benchmark_cases WHEN NEW.case_id='math-002' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(db.benchmark_datasets(&workspace.id).is_err());
        assert_eq!(
            db.connection()
                .unwrap()
                .query_row("SELECT count(*) FROM benchmark_datasets", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        db.connection()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_fixture")
            .unwrap();
        let first = db.benchmark_datasets(&workspace.id).unwrap();
        assert_eq!(first[0].content_hash, builtin().unwrap().content_hash);
        assert_eq!(
            db.benchmark_datasets(&workspace.id).unwrap()[0].content_hash,
            first[0].content_hash
        );
        assert_eq!(
            db.benchmark_case(&workspace.id, "cortex-core-60", 1, "rag-008")
                .unwrap()
                .fixture[0]
                .lines[1],
            "服务台位于二层。"
        );
        assert!(
            db.benchmark_case(&workspace.id, "cortex-core-60", 3, "rag-008")
                .is_err()
        );
        assert!(
            db.benchmark_case("missing", "cortex-core-60", 1, "rag-008")
                .is_err()
        );
        assert_eq!(encode(&db.bootstrap().unwrap()).unwrap(), before);
        assert!(
            db.messages(&workspace.id, &conversation.id)
                .unwrap()
                .is_empty()
        );
        let c = db.connection().unwrap();
        for table in [
            "execution_runs",
            "knowledge_nodes",
            "benchmark_runs",
            "benchmark_attempts",
        ] {
            assert_eq!(
                c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
        assert!(
            c.execute("UPDATE benchmark_cases SET expected_json='{}'", [])
                .is_err()
        );
        drop(c);
        drop(db);
        let reopened = Database::open(&directory).unwrap();
        assert_eq!(
            reopened.benchmark_datasets(&workspace.id).unwrap()[0].content_hash,
            first[0].content_hash
        );
    }
}

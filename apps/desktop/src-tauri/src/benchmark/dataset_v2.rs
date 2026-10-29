//! Bounded validation of authored v2 metadata. Not a runtime/model-output scorer.
use super::*;

const MANIFEST_V2: &str = include_str!("../../datasets/cortex-core-60-v2/manifest.json");
const CASES_V2: &str = include_str!("../../datasets/cortex-core-60-v2/cases.json");
const SCHEMA_V2: &str = include_str!("../../datasets/cortex-core-60-v2/case.schema.json");
const RULES_V2: &str = include_str!("../../datasets/cortex-core-60-v2/scoring-rules.json");
const NOTES_V2: &str = include_str!("../../datasets/cortex-core-60-v2/revision-notes.json");
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

pub(super) fn builtin() -> AppResult<Bundle> {
    let bundle = validate(MANIFEST_V2, CASES_V2, SCHEMA_V2, RULES_V2)?;
    let notes: Value = serde_json::from_str(NOTES_V2).map_err(|_| invalid())?;
    let old = super::builtin()?;
    let entries = notes["entries"].as_array().ok_or_else(invalid)?;
    if bundle.manifest["files"]["revision-notes.json"] != hash(NOTES_V2.as_bytes())
        || bundle.manifest["level"] != "smoke"
        || bundle.manifest["runtime_status"] != "not_enabled_in_stage_a"
        || notes["from_version"] != 1
        || notes["to_version"] != 2
        || entries.len() != 60
    {
        return Err(invalid());
    }
    for ((entry, before), after) in entries.iter().zip(&old.cases).zip(&bundle.cases) {
        if entry["case_id"] != after.case_id
            || entry["old_hash"] != before.content_hash
            || entry["new_hash"] != after.content_hash
            || !entry["changes"].as_array().is_some_and(|a| !a.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(bundle)
}

fn keys(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == allowed.len() && allowed.iter().all(|k| o.contains_key(*k)))
}

// The authored schema vocabulary only. General JSON Schema conformance is Stage B.
fn authored_example(value: &Value, schema: &Value) -> bool {
    let Some(o) = schema.as_object() else {
        return false;
    };
    if o.keys().any(|k| {
        ![
            "$schema",
            "type",
            "properties",
            "required",
            "additionalProperties",
            "items",
            "minItems",
            "maxItems",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "enum",
            "format",
        ]
        .contains(&k.as_str())
    }) || schema.get("$schema").is_some_and(|v| v != DIALECT)
    {
        return false;
    }
    let matches_type = |t: &str| match t {
        "object" => value.as_object().is_some_and(|v| {
            let Some(properties) = schema["properties"].as_object() else {
                return false;
            };
            let Some(required) = schema["required"].as_array() else {
                return false;
            };
            schema["additionalProperties"] == false
                && required.iter().all(|r| {
                    r.as_str()
                        .is_some_and(|k| properties.contains_key(k) && v.contains_key(k))
                })
                && v.iter()
                    .all(|(k, x)| properties.get(k).is_some_and(|s| authored_example(x, s)))
        }),
        "array" => value.as_array().is_some_and(|v| {
            v.len() as u64 >= schema["minItems"].as_u64().unwrap_or(0)
                && v.len() as u64 <= schema["maxItems"].as_u64().unwrap_or(u64::MAX)
                && v.iter().all(|x| authored_example(x, &schema["items"]))
        }),
        // Authored integer standards are deliberately literal i64, not model output.
        "integer" => value.as_i64().is_some_and(|n| {
            n >= schema["minimum"].as_i64().unwrap_or(i64::MIN)
                && n <= schema["maximum"].as_i64().unwrap_or(i64::MAX)
        }),
        "string" => value.as_str().is_some_and(|s| {
            s.chars().count() as u64 >= schema["minLength"].as_u64().unwrap_or(0)
                && s.chars().count() as u64 <= schema["maxLength"].as_u64().unwrap_or(u64::MAX)
                && schema.get("format").is_none_or(|f| {
                    f == "date"
                        && s.len() == 10
                        && chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                            .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == s)
                })
        }),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    let typed = schema["type"].as_str().is_some_and(matches_type)
        || schema["type"].as_array().is_some_and(|a| {
            !a.is_empty() && a.iter().any(|t| t.as_str().is_some_and(matches_type))
        });
    typed
        && schema
            .get("enum")
            .is_none_or(|e| e.as_array().is_some_and(|a| a.contains(value)))
}

fn valid_span(case: &Case, span: &Value) -> bool {
    let (Some(id), Some(start), Some(end)) = (
        span["source_id"].as_str(),
        span["start"].as_u64(),
        span["end"].as_u64(),
    ) else {
        return false;
    };
    case.fixture
        .iter()
        .find(|s| s.id == id)
        .is_some_and(|s| start >= 1 && end >= start && end <= s.lines.len() as u64)
}

// Custom datasets use the same six contracts; Core's authored case IDs remain unchanged.
pub(super) fn validate_custom(c: &Case, rules: &Value) -> AppResult<()> {
    if c.dataset_version == 0
        || c.case_id.len() > 80
        || !c
            .case_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || c.input.prompt.trim().is_empty()
        || c.input.prompt.len() > 24000
        || !["easy", "medium", "hard"].contains(&c.difficulty.as_str())
        || serde_json::to_vec(c).map_err(|_| invalid())?.len() > 100000
    {
        return Err(invalid());
    }
    if c.scoring.id == "python_assertions" {
        let cfg = c.scoring.contract.as_ref().ok_or_else(invalid)?;
        let Expected::Python { function, tests } = &c.expected else {
            return Err(invalid());
        };
        let parameters = cfg["parameter_schemas"].as_array().ok_or_else(invalid)?;
        let valid_type = |s: &Value| {
            matches!(
                s["type"].as_str(),
                Some("integer" | "string" | "boolean" | "array")
            )
        };
        if c.scoring.version != 2
            || c.category != "coding"
            || c.output_format != "python"
            || c.requirements != ["completion", "python_sandbox"]
            || !c.fixture.is_empty()
            || c.input.response_schema.is_some()
            || !c.input.tools.is_empty()
            || cfg["python_version"] != "3.13"
            || cfg["strict_python_types"] != true
            || cfg["input_mutation"] != "forbidden"
            || cfg["preconditions"] != json!([])
            || function.is_empty()
            || function.len() > 64
            || function.starts_with(|x: char| x.is_ascii_digit())
            || !function
                .bytes()
                .all(|x| x.is_ascii_alphanumeric() || x == b'_')
            || parameters.is_empty()
            || parameters.len() > 5
            || !valid_type(&cfg["return_schema"])
            || tests.len() < 7
            || tests.len() > 32
            || !tests.iter().all(|t| {
                t.args.len() == parameters.len()
                    && t.args
                        .iter()
                        .zip(parameters)
                        .all(|(v, s)| authored_example(v, s))
                    && authored_example(&t.output, &cfg["return_schema"])
            })
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let mut mapped = c.clone();
    mapped.case_id = match c.scoring.id.as_str() {
        "exact_alias" => "zh-001",
        "integer_equal" => "math-001",
        "schema_semantic" => "schema-001",
        "evidence_match" => "rag-001",
        "native_tool_equal" => "tool-001",
        _ => return Err(invalid()),
    }
    .into();
    validate_case(&mapped, rules)
}

pub(super) fn validate_case(c: &Case, rules: &Value) -> AppResult<()> {
    let cfg = c.scoring.contract.as_ref().ok_or_else(invalid)?;
    let (rule, category, format, requirements) = match c.case_id.split('-').next() {
        Some("zh") => ("exact_alias", "zh_knowledge", "text", vec!["completion"]),
        Some("math") => ("integer_equal", "math_logic", "integer", vec!["completion"]),
        Some("code") => (
            "python_assertions",
            "coding",
            "python",
            vec!["completion", "python_sandbox"],
        ),
        Some("rag") => ("evidence_match", "rag_citation", "json", vec!["completion"]),
        Some("schema") => (
            "schema_semantic",
            "structured_tools",
            "json",
            vec!["completion", "json_schema"],
        ),
        Some("tool") => (
            "native_tool_equal",
            "structured_tools",
            "tool_calls",
            vec!["completion", "tool_calls"],
        ),
        _ => return Err(invalid()),
    };
    if c.scoring.id != rule
        || c.scoring.version != 2
        || c.category != category
        || c.output_format != format
        || c.requirements != requirements
        || !rules["rules"].as_array().is_some_and(|a| {
            a.iter()
                .filter(|r| r["id"] == rule && r["version"] == 2)
                .count()
                == 1
        })
        || (rule != "evidence_match" && !c.fixture.is_empty())
        || (!["evidence_match", "schema_semantic"].contains(&rule)
            && c.input.response_schema.is_some())
        || (rule != "native_tool_equal" && !c.input.tools.is_empty())
    {
        return Err(invalid());
    }
    let good = match (&c.expected, rule) {
        (Expected::Aliases { answers }, "exact_alias") => {
            nonempty(answers)
                && answers.iter().collect::<BTreeSet<_>>().len() == answers.len()
                && *cfg
                    == json!({"normalization":"trim_unicode_whitespace","case_sensitive":true,"unicode_folding":false})
        }
        (Expected::Integer { .. }, "integer_equal") => {
            *cfg == json!({"normalization":"trim_unicode_whitespace","syntax":"ascii_minus_optional_digits","integer_range":"i64"})
        }
        (Expected::Python { function, tests }, "python_assertions") => {
            let mut fields = vec![
                "python_version",
                "parameter_schemas",
                "return_schema",
                "strict_python_types",
                "input_mutation",
                "preconditions",
            ];
            if c.case_id == "code-003" {
                fields.push("text_units");
            }
            if c.case_id == "code-008" {
                fields.push("whitespace_codepoints");
            }
            let Some(parameters) = cfg["parameter_schemas"].as_array() else {
                return Err(invalid());
            };
            keys(cfg, &fields)
                && cfg["python_version"] == "3.13"
                && cfg["strict_python_types"] == true
                && cfg["input_mutation"] == "forbidden"
                && parameters.len()
                    == match c.case_id.as_str() {
                        "code-006" => 3,
                        "code-009" => 2,
                        _ => 1,
                    }
                && !function.is_empty()
                && function.chars().all(|x| x.is_ascii_lowercase() || x == '_')
                && tests.len() >= 7
                && tests.iter().all(|t| {
                    t.args.len() == parameters.len()
                        && t.args
                            .iter()
                            .zip(parameters)
                            .all(|(v, s)| authored_example(v, s))
                        && authored_example(&t.output, &cfg["return_schema"])
                })
                && (c.case_id != "code-003" || cfg["text_units"] == "unicode_codepoints")
                && (c.case_id != "code-008"
                    || cfg["whitespace_codepoints"]
                        == json!([
                            9, 10, 11, 12, 13, 28, 29, 30, 31, 32, 133, 160, 5760, 8192, 8193,
                            8194, 8195, 8196, 8197, 8198, 8199, 8200, 8201, 8202, 8232, 8233, 8239,
                            8287, 12288
                        ]))
                && cfg["preconditions"]
                    == match c.case_id.as_str() {
                        "code-006" => json!(["low<=high"]),
                        "code-009" => json!(["a_and_b_sorted_ascending"]),
                        _ => json!([]),
                    }
                && tests.iter().all(|t| match c.case_id.as_str() {
                    "code-006" => t.args[1].as_i64() <= t.args[2].as_i64(),
                    "code-009" => t.args.iter().all(|a| {
                        a.as_array()
                            .is_some_and(|a| a.windows(2).all(|p| p[0].as_i64() <= p[1].as_i64()))
                    }),
                    _ => true,
                })
        }
        (Expected::Json { answer }, "schema_semantic") => {
            keys(
                cfg,
                &[
                    "dialect",
                    "duplicate_keys",
                    "comparison",
                    "array_order",
                    "format_assertion",
                ],
            ) && cfg["dialect"] == DIALECT
                && cfg["duplicate_keys"] == "reject"
                && cfg["comparison"] == "typed_json"
                && cfg["array_order"] == "significant"
                && cfg["format_assertion"] == true
                && c.input
                    .response_schema
                    .as_ref()
                    .is_some_and(|s| s["$schema"] == DIALECT && authored_example(answer, s))
        }
        (Expected::Json { answer }, "evidence_match") => validate_evidence(c, answer, cfg),
        (Expected::Tool { name, arguments }, "native_tool_equal") => {
            keys(
                cfg,
                &[
                    "dialect",
                    "duplicate_keys",
                    "comparison",
                    "call_count",
                    "native_only",
                    "execute",
                ],
            ) && cfg["dialect"] == DIALECT
                && cfg["duplicate_keys"] == "reject"
                && cfg["comparison"] == "typed_json"
                && cfg["call_count"] == 1
                && cfg["native_only"] == true
                && cfg["execute"] == false
                && c.input.tools.len() == 1
                && c.input.tools[0]["type"] == "function"
                && c.input.tools[0]["function"]["name"] == *name
                && authored_example(arguments, &c.input.tools[0]["function"]["parameters"])
        }
        _ => false,
    };
    if good { Ok(()) } else { Err(invalid()) }
}

fn validate_evidence(c: &Case, answer: &Value, cfg: &Value) -> bool {
    if !keys(
        cfg,
        &[
            "as_of",
            "fact_aliases",
            "evidence",
            "citation_policy",
            "duplicate_citations",
            "extra_facts",
            "answer_and_citations",
            "full_pass_requires",
            "format_assertion",
            "source_policy",
            "schema_use",
        ],
    ) || cfg["schema_use"] != "grading_only"
        || cfg["citation_policy"] != "enumerated_supporting_spans"
        || cfg["duplicate_citations"] != "reject"
        || cfg["extra_facts"] != "reject"
        || cfg["answer_and_citations"] != "score_separately"
        || cfg["full_pass_requires"] != "all_dimensions"
        || cfg["format_assertion"] != true
        || !c
            .input
            .response_schema
            .as_ref()
            .is_some_and(|s| s["$schema"] == DIALECT && authored_example(answer, s))
        || c.fixture.is_empty()
        || c.fixture
            .iter()
            .any(|s| s.id.is_empty() || s.title.is_empty() || !nonempty(&s.lines))
        || c.fixture
            .iter()
            .map(|s| &s.id)
            .collect::<BTreeSet<_>>()
            .len()
            != c.fixture.len()
    {
        return false;
    }
    let (Some(facts), Some(evidence), Some(aliases), Some(citations), Some(policy)) = (
        answer["facts"].as_object(),
        cfg["evidence"].as_object(),
        cfg["fact_aliases"].as_object(),
        answer["citations"].as_array(),
        cfg["source_policy"].as_object(),
    ) else {
        return false;
    };
    let Some(as_of) = cfg["as_of"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
    else {
        return false;
    };
    if facts.len() != evidence.len()
        || citations.len() != facts.len()
        || aliases.iter().any(|(k, v)| {
            !facts.contains_key(k)
                || !v.as_array().is_some_and(|a| {
                    !a.is_empty()
                        && a.contains(&facts[k])
                        && a.iter().all(|v| v.is_string() && facts[k].is_string())
                })
        })
    {
        return false;
    }
    match answer["status"].as_str() {
        Some("answerable") if !facts.is_empty() && answer["reason"].is_null() => (),
        Some("unanswerable") if facts.is_empty() && answer["reason"] == "insufficient_evidence" => {
            ()
        }
        Some("conflict")
            if facts.len() >= 2
                && answer["reason"] == "conflicting_evidence"
                && citations
                    .iter()
                    .filter_map(|s| s["source_id"].as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    >= 2 =>
        {
            ()
        }
        _ => return false,
    }
    if policy.iter().any(|(id, p)| {
        !c.fixture.iter().any(|s| s.id == *id)
            || p.as_object().is_none_or(|o| {
                o.keys()
                    .any(|k| !["effective_from", "supersedes"].contains(&k.as_str()))
            })
            || p["effective_from"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_none()
            || p.get("supersedes").is_some_and(|old| {
                old.as_str()
                    .is_none_or(|old| old == id || !policy.contains_key(old))
            })
    }) {
        return false;
    }
    facts.keys().all(|claim| {
        let refs: Vec<_> = citations.iter().filter(|s| s["claim_id"] == *claim).collect();
        if refs.len() != 1 || !valid_span(c, refs[0]) { return false; }
        let reference = refs[0];
        if !policy.is_empty() {
            let Some(id) = reference["source_id"].as_str() else { return false; };
            let Some(active) = policy.get(id).and_then(|p| p["effective_from"].as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) else { return false; };
            if active > as_of || policy.values().any(|p| p["supersedes"] == id && p["effective_from"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).is_some_and(|d| d <= as_of)) { return false; }
        }
        evidence[claim].as_array().is_some_and(|spans| spans.contains(&json!({"source_id":reference["source_id"],"start":reference["start"],"end":reference["end"]})) && spans.iter().all(|s| keys(s,&["source_id","start","end"]) && valid_span(c,s) && s["source_id"] == reference["source_id"] && s["start"].as_u64() <= reference["start"].as_u64() && s["end"].as_u64() >= reference["end"].as_u64()))
    })
}

#[cfg(test)]
#[path = "dataset_v2_tests.rs"]
mod tests;

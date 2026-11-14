use super::{
    dataset::{Case, Expected},
    evaluation_json as json,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const ENGINE_VERSION: &str = "core60-runtime-2.2";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Passed,
    Failed,
    Unsupported,
    #[serde(rename = "Not Run")]
    NotRun,
    #[serde(rename = "Execution Error")]
    ExecutionError,
    #[serde(rename = "Scoring Error")]
    ScoringError,
    #[serde(rename = "Needs Review")]
    NeedsReview,
    Cancelled,
    #[serde(rename = "In Progress")]
    InProgress,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
    pub state: State,
    pub metrics: Value,
    pub details: Value,
}
impl Score {
    pub fn state(state: State, reason: impl Into<String>) -> Self {
        Self {
            state,
            metrics: json!({"correctness":match state {State::Passed=>Some(1),State::Failed=>Some(0),_=>None}}),
            details: json!({"reason":reason.into(),"engineVersion":ENGINE_VERSION}),
        }
    }
    fn graded(pass: bool, metrics: Value, details: Value) -> Self {
        let mut score = Self::state(if pass { State::Passed } else { State::Failed }, "graded");
        score
            .metrics
            .as_object_mut()
            .unwrap()
            .extend(metrics.as_object().unwrap().clone());
        score.details["dimensions"] = details;
        score
    }
}

pub fn evaluate(case: &Case, text: &str, calls: &[Value]) -> Score {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        evaluate_inner(case, text, calls)
    }))
    .unwrap_or_else(|_| Score::state(State::ScoringError, "evaluator_panicked"))
}
fn evaluate_inner(case: &Case, text: &str, calls: &[Value]) -> Score {
    if super::dataset::validate_runtime_case(case).is_err() {
        return Score::state(State::ScoringError, "dataset_or_contract_integrity_error");
    }
    if case.scoring.id != "native_tool_equal" && !calls.is_empty() {
        return Score::graded(
            false,
            json!({"unexpectedNativeCalls":calls.len()}),
            json!({"reason":"unrequested_native_calls"}),
        );
    }
    if case.scoring.version != 2 || case.scoring.contract.is_none() {
        return Score::state(State::ScoringError, "unsupported_scoring_contract");
    }
    let cfg = case.scoring.contract.as_ref().unwrap();
    let result: Result<Score, String> = (|| match (&case.expected, case.scoring.id.as_str()) {
        (Expected::Aliases { answers }, "exact_alias") => {
            if *cfg
                != json!({"normalization":"trim_unicode_whitespace","case_sensitive":true,"unicode_folding":false})
                || answers.is_empty()
            {
                return Err("invalid_alias_contract".into());
            }
            Ok(Score::graded(
                answers.iter().any(|v| v == text.trim()),
                json!({"formatValid":true}),
                json!({"normalization":"trim_unicode_whitespace_only"}),
            ))
        }
        (Expected::Integer { answer }, "integer_equal") => {
            if *cfg
                != json!({"normalization":"trim_unicode_whitespace","syntax":"ascii_minus_optional_digits","integer_range":"i64"})
            {
                return Err("invalid_integer_contract".into());
            }
            let s = text.trim();
            let digits = s.strip_prefix('-').unwrap_or(s);
            let parsed = (!digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()))
                .then(|| s.parse::<i64>().ok())
                .flatten();
            Ok(Score::graded(
                parsed == Some(*answer),
                json!({"formatValid":parsed.is_some()}),
                json!({"syntax":"ascii_minus_optional_digits"}),
            ))
        }
        (Expected::Json { answer }, "schema_semantic") => {
            let schema = case
                .input
                .response_schema
                .as_ref()
                .ok_or("missing_schema")?;
            // Check the scorer configuration even if the candidate is malformed.
            if !json::schema_valid(schema, answer)? {
                return Err("invalid_json_standard".into());
            }
            if cfg["comparison"] != "typed_json" || cfg["duplicate_keys"] != "reject" {
                return Err("invalid_json_contract".into());
            }
            match json::decode(text) {
                Err(e) => Ok(Score::graded(
                    false,
                    json!({"jsonValid":false,"schemaValid":false,"semanticEqual":false}),
                    json!({"parseError":e}),
                )),
                Ok(actual) => {
                    let valid = json::schema_valid(schema, &actual)?;
                    let equal = json::equal(answer, &actual);
                    Ok(Score::graded(
                        valid && equal,
                        json!({"jsonValid":true,"schemaValid":valid,"semanticEqual":equal}),
                        json!({}),
                    ))
                }
            }
        }
        (Expected::Json { answer }, "evidence_match") => rag(case, answer, cfg, text),
        (Expected::Tool { name, arguments }, "native_tool_equal") => {
            if cfg["native_only"] != true
                || cfg["call_count"] != 1
                || cfg["execute"] != false
                || case.input.tools.len() != 1
            {
                return Err("invalid_native_tool_contract".into());
            }
            let schema = &case.input.tools[0]["function"]["parameters"];
            if !json::schema_valid(schema, arguments)? {
                return Err("invalid_tool_standard".into());
            }
            let one = calls.len() == 1;
            let actual = calls.first().cloned().unwrap_or(Value::Null);
            let named = one && actual["function"]["name"] == *name;
            let valid = one
                && actual["function"]["arguments"].is_object()
                && json::schema_valid(schema, &actual["function"]["arguments"])?;
            let equal = one && json::equal(arguments, &actual["function"]["arguments"]);
            Ok(Score::graded(
                one && named && valid && equal,
                json!({"nativeCallPresent":!calls.is_empty(),"callCountValid":one,"toolNameValid":named,"argumentsSchemaValid":valid,"argumentsCorrect":equal}),
                json!({"actualCalls":calls,"toolsExecuted":false}),
            ))
        }
        (Expected::Python { .. }, "python_assertions") => {
            Err("python_requires_docker_controller".into())
        }
        _ => Err("unsupported_evaluator_or_standard".into()),
    })();
    result.unwrap_or_else(|reason| Score::state(State::ScoringError, reason))
}

fn rag(case: &Case, expected: &Value, cfg: &Value, text: &str) -> Result<Score, String> {
    let schema = case
        .input
        .response_schema
        .as_ref()
        .ok_or("missing_rag_schema")?;
    if !json::schema_valid(schema, expected)?
        || cfg["schema_use"] != "grading_only"
        || cfg["citation_policy"] != "enumerated_supporting_spans"
    {
        return Err("invalid_rag_contract".into());
    }
    let expected_facts = expected["facts"].as_object().ok_or("invalid_gold_facts")?;
    let evidence = cfg["evidence"]
        .as_object()
        .ok_or("missing_evidence_annotation")?;
    let as_of = chrono::DateTime::parse_from_rfc3339(cfg["as_of"].as_str().ok_or("missing_as_of")?)
        .map_err(|_| "invalid_as_of")?;
    let actual = match json::decode(text) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Score::graded(
                false,
                json!({"jsonValid":false,"answerCorrect":false,"citationPrecision":null,"citationCoverage":if expected_facts.is_empty(){None}else{Some(0.0)}}),
                json!({"parseError":e}),
            ));
        }
    };
    let valid = json::schema_valid(schema, &actual)?;
    let status = actual["status"] == expected["status"];
    let reason = actual["reason"] == expected["reason"];
    let facts = actual["facts"].as_object();
    let correct_claims: std::collections::BTreeSet<_> = expected_facts
        .iter()
        .filter(|(k, v)| {
            facts.and_then(|f| f.get(*k)).is_some_and(|a| {
                json::equal(a, v)
                    || cfg["fact_aliases"][*k]
                        .as_array()
                        .is_some_and(|aliases| aliases.iter().any(|v| json::equal(v, a)))
            })
        })
        .map(|(k, _)| k.as_str())
        .collect();
    let facts_correct = facts.is_some_and(|f| f.len() == expected_facts.len())
        && correct_claims.len() == expected_facts.len();
    let mut supported = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut details = Vec::new();
    let mut correct_citations = 0;
    let citations = actual["citations"].as_array().cloned().unwrap_or_default();
    for citation in &citations {
        let claim = citation["claim_id"].as_str().unwrap_or("");
        let source = citation["source_id"].as_str().unwrap_or("");
        let position = case
            .fixture
            .iter()
            .find(|s| s.id == source)
            .is_some_and(|s| {
                json::unsigned(&citation["start"])
                    .zip(json::unsigned(&citation["end"]))
                    .is_some_and(|(a, b)| a >= 1 && b >= a && b <= s.lines.len() as u64)
            });
        let range = json!({"source_id":citation["source_id"],"start":citation["start"],"end":citation["end"]});
        let annotated = evidence
            .get(claim)
            .and_then(Value::as_array)
            .is_some_and(|spans| spans.iter().any(|v| json::equal(v, &range)));
        let duplicate = !seen.insert((
            claim.to_string(),
            source.to_string(),
            json::unsigned(&citation["start"]),
            json::unsigned(&citation["end"]),
        ));
        let mut effective = true;
        if let Some(policy) = cfg["source_policy"].as_object().filter(|p| !p.is_empty()) {
            effective = policy
                .get(source)
                .and_then(|p| p["effective_from"].as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|d| d <= as_of)
                && !policy.values().any(|p| {
                    p["supersedes"] == source
                        && p["effective_from"]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .is_some_and(|d| d <= as_of)
                });
        }
        let supports =
            position && annotated && effective && !duplicate && correct_claims.contains(claim);
        if supports {
            correct_citations += 1;
            supported.insert(claim.to_string());
        }
        details.push(json!({"citation":citation,"positionValid":position,"annotatedSupport":annotated,"effectiveVersion":effective,"duplicate":duplicate,"supportsClaim":supports}));
    }
    let precision =
        (!citations.is_empty()).then(|| correct_citations as f64 / citations.len() as f64);
    let coverage =
        (!expected_facts.is_empty()).then(|| supported.len() as f64 / expected_facts.len() as f64);
    let citations_correct =
        correct_citations == citations.len() && supported.len() == expected_facts.len();
    let answer_correct = status && reason && facts_correct;
    let version_correct = cfg["source_policy"]
        .as_object()
        .filter(|p| !p.is_empty())
        .map(|_| {
            !citations.is_empty()
                && details
                    .iter()
                    .all(|d| d["effectiveVersion"] == true && d["annotatedSupport"] == true)
        });
    Ok(Score::graded(
        valid && answer_correct && citations_correct,
        json!({"jsonValid":true,"schemaValid":valid,"answerabilityCorrect":status,"answerCorrect":answer_correct,"factsCorrect":facts_correct,"refusalCorrect":if expected["status"]=="unanswerable"{Some(answer_correct)}else{None},"conflictCorrect":if expected["status"]=="conflict"{Some(answer_correct&&citations_correct)}else{None},"citationsPresent":!citations.is_empty(),"citationCount":citations.len(),"supportedCitationCount":correct_citations,"citationPrecision":precision,"citationCoverage":coverage,"citationsCorrect":citations_correct}),
        json!({"asOf":cfg["as_of"],"knowledgeVersionCorrect":version_correct,"citations":details}),
    ))
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub planned: usize,
    pub attempted: usize,
    pub execution_succeeded: usize,
    pub scored: usize,
    pub passed: usize,
    pub states: std::collections::BTreeMap<String, usize>,
    pub execution_coverage: Option<f64>,
    pub scoring_coverage: Option<f64>,
    pub execution_success_rate: Option<f64>,
    pub correctness: Option<f64>,
}
pub fn statistics(items: &[(Score, bool, bool)]) -> Statistics {
    let mut s = Statistics::default();
    s.planned = items.len();
    for state in [
        State::Passed,
        State::Failed,
        State::Unsupported,
        State::NotRun,
        State::ExecutionError,
        State::ScoringError,
        State::NeedsReview,
        State::Cancelled,
        State::InProgress,
    ] {
        s.states.insert(
            serde_json::to_value(state)
                .unwrap()
                .as_str()
                .unwrap()
                .into(),
            0,
        );
    }
    for (score, attempted, completed) in items {
        *s.states
            .get_mut(serde_json::to_value(score.state).unwrap().as_str().unwrap())
            .unwrap() += 1;
        s.attempted += usize::from(*attempted);
        s.execution_succeeded += usize::from(*completed);
        s.scored += usize::from(matches!(score.state, State::Passed | State::Failed));
        s.passed += usize::from(score.state == State::Passed);
    }
    let ratio = |a: usize, b: usize| (b > 0).then(|| a as f64 / b as f64);
    s.execution_coverage = ratio(s.attempted, s.planned);
    s.scoring_coverage = ratio(s.scored, s.planned);
    s.execution_success_rate = ratio(s.execution_succeeded, s.attempted);
    s.correctness = ratio(s.passed, s.scored);
    s
}

#[cfg(test)]
#[path = "evaluation_tests.rs"]
mod tests;

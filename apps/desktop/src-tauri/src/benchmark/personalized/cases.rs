use super::repository::RepositorySnapshot;
use crate::{
    benchmark::{
        dataset::{self, Case, Expected},
        evaluation::{self, State},
    },
    models::{AppResult, new_id},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Location {
    pub path: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    pub case: Value,
    pub rationale: String,
    pub sources: Vec<Location>,
}
pub fn seal(case: &mut Case) -> AppResult<()> {
    let mut raw = serde_json::to_value(&case).map_err(|_| "题目无法编码。")?;
    raw.as_object_mut()
        .ok_or("题目无效。")?
        .remove("content_hash");
    case.content_hash = dataset::hash(raw.to_string().as_bytes());
    Ok(())
}
pub fn assigned(mut candidate: Candidate) -> AppResult<Candidate> {
    let obj = candidate.case.as_object_mut().ok_or("生成题目须为对象。")?;
    obj.insert("case_id".into(), json!(format!("custom-{}", new_id())));
    obj.insert("dataset_version".into(), json!(1));
    obj.insert("content_hash".into(), json!(""));
    if let Ok(mut case) = serde_json::from_value::<Case>(candidate.case.clone()) {
        seal(&mut case)?;
        candidate.case = serde_json::to_value(case).map_err(|_| "题目无法编码。")?;
    }
    Ok(candidate)
}
pub fn validate(
    candidate: &Candidate,
    snapshot: Option<&RepositorySnapshot>,
    allowed: &[Location],
) -> Value {
    let mut errors = vec![];
    let mut warnings =
        vec!["题意、标准答案与证据语义仍需人工确认；静态检查不代表题目正确。".to_string()];
    let case = serde_json::from_value::<Case>(candidate.case.clone());
    let mut supported = true;
    match &case {
        Err(_) => {
            supported = false;
            errors.push("题目结构或评分类型不受支持。".into());
        }
        Ok(case) => {
            if ![
                "exact_alias",
                "integer_equal",
                "schema_semantic",
                "evidence_match",
                "python_assertions",
                "native_tool_equal",
            ]
            .contains(&case.scoring.id.as_str())
                || case.scoring.version != 2
            {
                supported = false;
            }
            if dataset::validate_runtime_case(case).is_err() {
                errors.push("题目或评分合同校验失败，请按示例修订。".into());
            } else if let Expected::Python { .. } = &case.expected {
                warnings.push(
                    "Python 断言仅通过结构检查，运行时仍需 Docker；请审核边界输入与预期值。".into(),
                );
            } else {
                let (answer, calls) = match &case.expected {
                    Expected::Aliases { answers } => (answers[0].clone(), vec![]),
                    Expected::Integer { answer } => (answer.to_string(), vec![]),
                    Expected::Json { answer } => (answer.to_string(), vec![]),
                    Expected::Tool { name, arguments } => (
                        String::new(),
                        vec![json!({"function":{"name":name,"arguments":arguments}})],
                    ),
                    _ => (String::new(), vec![]),
                };
                if evaluation::evaluate(case, &answer, &calls).state != State::Passed {
                    errors.push("标准答案无法通过声明的评分规则。".into());
                }
            }
        }
    }
    if candidate.rationale.trim().is_empty() || candidate.rationale.len() > 4000 {
        errors.push("请提供有限长度的出题依据。".into());
    }
    if candidate.sources.len() > 20 {
        errors.push("来源位置超过20处。".into());
    }
    for loc in &candidate.sources {
        let valid = snapshot
            .and_then(|s| s.files.iter().find(|f| f.path == loc.path))
            .is_some_and(|f| {
                loc.start > 0 && loc.end >= loc.start && loc.end <= f.content.lines().count()
            })
            && allowed
                .iter()
                .any(|a| a.path == loc.path && a.start <= loc.start && a.end >= loc.end);
        if !valid {
            errors.push(format!("来源未获发送授权或行号无效：{}", loc.path));
        }
    }
    if snapshot.is_some() && allowed.is_empty() {
        warnings.push("本次仅提供项目摘要；题目不能声称已验证具体代码行为。".into());
    }
    json!({"supported":supported,"valid":errors.is_empty(),"errors":errors,"warnings":warnings,"humanReview":"required"})
}

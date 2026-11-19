use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const VERSION: &str = "cortex-rag-rubric-1";
const K: usize = 5;

fn ratio(n: usize, d: usize) -> Option<f64> {
    (d != 0).then(|| n as f64 / d as f64)
}

fn key(item: &Value) -> (&str, i64) {
    (
        item["nodeId"].as_str().unwrap_or_default(),
        item["version"].as_i64().unwrap_or_default(),
    )
}

fn retrieval(case: &Case, manifest: &Value) -> AppResult<Value> {
    let report = &manifest["retrieval"];
    let candidates = report["candidates"]
        .as_array()
        .ok_or("固定评测候选记录缺失。")?;
    let context = manifest["knowledgeRefs"]
        .as_array()
        .ok_or("固定评测资料清单缺失。")?;
    let gold: BTreeSet<_> = case
        .gold
        .iter()
        .map(|(id, version)| (id.as_str(), *version))
        .collect();
    let mut seen = BTreeSet::new();
    let ranks: Vec<_> = candidates
        .iter()
        .take(K)
        .enumerate()
        .filter_map(|(index, item)| {
            let id = key(item);
            (gold.contains(&id) && seen.insert(id)).then_some(index + 1)
        })
        .collect();
    let context_hits: BTreeSet<_> = context
        .iter()
        .map(key)
        .filter(|key| gold.contains(key))
        .collect();
    let discount = |rank: usize| 1.0 / ((rank + 1) as f64).log2();
    let ndcg = (!gold.is_empty()).then(|| {
        ranks.iter().copied().map(discount).sum::<f64>()
            / (1..=gold.len().min(K)).map(discount).sum::<f64>()
    });
    let bytes = |item: &Value| item["content"].as_str().map_or(0, str::len);
    let useful_bytes = context
        .iter()
        .filter(|item| gold.contains(&key(item)))
        .map(bytes)
        .sum();
    let all_bytes = context.iter().map(bytes).sum();
    let detected = report["conflicts"]
        .as_array()
        .is_some_and(|edges| !edges.is_empty());
    let metrics = json!({
        "recallAt5":ratio(ranks.len(),gold.len()),
        "mrr":(!gold.is_empty()).then(|| ranks.first().map_or(0., |rank| 1. / *rank as f64)),
        "ndcg":ndcg,"contextPrecision":ratio(context_hits.len(),context.len()),
        "contextRecall":ratio(context_hits.len(),gold.len()),"contextEfficiency":ratio(useful_bytes,all_bytes),
        "conflictDetectionAccuracy":u8::from(detected == case.conflict),
        "retrievalLatencyMs":report["elapsedMicros"].as_u64().map(|micros| micros as f64 / 1000.0)
    });
    Ok(
        json!({"id":case.id,"kind":case.kind,"question":case.question,"expected":case.gold,
        "factDate":case.fact_date,"metrics":metrics,"usedMethod":report["route"]["usedMethod"],
        "algorithm":report["algorithm"],"reranker":report["reranker"],"route":report["route"],
        "expectedConflict":case.conflict,"detectedConflict":detected,"expectedAbstention":gold.is_empty(),
        "caseScope":manifest["knowledgeScope"],"usedReferences":context,
        "candidates":candidates.iter().map(|item| json!({"nodeId":item["nodeId"],"version":item["version"],"title":item["title"],
            "sourceLocation":item["sourceLocation"],"reason":item["reason"],"included":item["included"],
            "lexicalRank":item["lexicalRank"],"semanticRank":item["semanticRank"],"fusionRank":item["fusionRank"],"rerankScore":item["rerankScore"]})).collect::<Vec<_>>(),
        "graph":report["graph"],"conflicts":report["conflicts"]}),
    )
}

pub(super) fn evaluate(case: &Case, manifest: &Value) -> AppResult<Value> {
    let mut result = retrieval(case, manifest)?;
    let context = manifest["knowledgeRefs"].as_array().unwrap();
    let gold: BTreeSet<_> = case
        .gold
        .iter()
        .map(|(id, version)| (id.as_str(), *version))
        .collect();
    let replay_text = if case.replay.abstained {
        "资料不足，无法作答。".to_owned()
    } else {
        let claims = case
            .replay
            .claims
            .iter()
            .map(|(claim, ordinal)| format!("{claim} [资料{ordinal}]"))
            .collect::<Vec<_>>()
            .join("\n");
        if case.replay.conflict_detected {
            format!("这两份资料互相冲突。\n{claims}")
        } else {
            claims
        }
    };
    let checked =
        crate::citations::check(&replay_text, manifest).ok_or("固定回复的引用检查没有结果。")?;
    let occurrences = checked["occurrences"]
        .as_array()
        .ok_or("固定回复的引用位置缺失。")?;
    let mut grounding = 0;
    let mut supported = 0;
    let mut cited_gold = BTreeSet::new();
    let mut checks = Vec::new();
    for (index, occurrence) in occurrences.iter().enumerate() {
        let claim = case
            .replay
            .claims
            .get(index)
            .ok_or("固定回复声明与引用数量不符。")?;
        let reference = occurrence["referenceId"]
            .as_str()
            .and_then(|id| context.iter().find(|item| item["citationId"] == id));
        let grounded = reference
            .and_then(|item| item["content"].as_str())
            .is_some_and(|text| text.contains(&claim.0));
        let correct = grounded && reference.is_some_and(|item| gold.contains(&key(item)));
        if grounded {
            grounding += 1;
        }
        if correct {
            supported += 1;
            cited_gold.insert(key(reference.unwrap()));
        }
        checks.push(
            json!({"claim":claim.0,"marker":occurrence["label"],"reference":reference.map(key),
            "locationError":occurrence["error"],"grounded":grounded,"supportsExpected":correct}),
        );
    }
    result["metrics"]["citationPrecision"] = json!(ratio(supported, occurrences.len()));
    result["metrics"]["citationRecall"] = json!(ratio(cited_gold.len(), gold.len()));
    result["metrics"]["faithfulness"] = json!(ratio(grounding, case.replay.claims.len()));
    result["metrics"]["abstentionAccuracy"] =
        json!(u8::from(case.replay.abstained == gold.is_empty()));
    result["replay"] =
        json!({"text":replay_text,"checks":checks,"abstained":case.replay.abstained});
    Ok(result)
}

pub(super) const RUNTIME_VERSION: &str = "cortex-rag-response-rubric-2";

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '*' | '`' | '。' | '！' | '？'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn retention_days(rule: &str) -> Option<&str> {
    rule.strip_prefix("日志保留")
        .and_then(|value| value.strip_suffix('天'))
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn matches_fact(text: &str, rule: &str) -> bool {
    let Some(days) = retention_days(rule) else {
        return normalize(text).contains(rule);
    };

    let expected = format!("{days}天");
    text.split(['\n', '。', '！', '？', '；', ';'])
        .any(|sentence| {
            let normalized = normalize(sentence);
            let mut statement = normalized.trim_end_matches(['.', '!', '?']);
            while let Some((before, marker)) = statement.rsplit_once("[资料") {
                if !marker
                    .strip_suffix(']')
                    .is_some_and(|label| !label.contains(['[', ']']))
                {
                    break;
                }
                statement = before;
            }
            let Some(value) = statement.strip_prefix("日志保留") else {
                return false;
            };
            let value = ["期限", "天数", "期"]
                .into_iter()
                .find_map(|prefix| value.strip_prefix(prefix))
                .unwrap_or(value);
            let value = value.strip_prefix(['是', '为', '：', ':']).unwrap_or(value);
            value == expected
        })
}

pub(super) fn grade(case: &Case, manifest: &Value, response: Option<&str>) -> AppResult<Value> {
    let mut result = retrieval(case, manifest)?;
    let Some(response) = response else {
        return Ok(result);
    };
    let normalized = normalize(response);
    let rules = case
        .replay
        .claims
        .iter()
        .map(|(fact, _)| normalize(fact))
        .collect::<Vec<_>>();
    let claims = rules
        .iter()
        .map(|rule| matches_fact(response, rule))
        .collect::<Vec<_>>();
    let abstained = matches!(
        normalized.as_str(),
        "资料不足，无法作答" | "资料不足" | "无法根据所提供的资料回答" | "没有足够的资料回答"
    );
    let conflict = normalized.contains("冲突") || normalized.contains("不一致");
    let answer = if case.gold.is_empty() {
        f64::from(abstained)
    } else {
        let matched = claims.iter().filter(|found| **found).count();
        let total = claims.len() + usize::from(case.conflict);
        ratio(matched + usize::from(case.conflict && conflict), total).unwrap_or_default()
    };
    let checked = crate::citations::check(response, manifest).ok_or("实际回答缺少引用协议。")?;
    let context = manifest["knowledgeRefs"].as_array().unwrap();
    let mut cited = BTreeSet::new();
    let mut valid = 0;
    let mut checks = Vec::new();
    // Citation offsets are UTF-16. For log retention, include any continuation of the cited clause.
    let encoded = response.encode_utf16().collect::<Vec<_>>();
    for occurrence in checked["occurrences"].as_array().unwrap() {
        let reference = occurrence["referenceId"]
            .as_str()
            .and_then(|id| context.iter().find(|r| r["citationId"] == id));
        let start = occurrence["start"].as_u64().unwrap_or_default() as usize;
        let preceding = String::from_utf16_lossy(&encoded[..start]);
        let preceding =
            preceding.trim_end_matches(|c: char| matches!(c, '*' | '`') || c.is_whitespace());
        let statement = preceding
            .trim_end_matches('。')
            .rsplit(['\n', '。', '！', '？', '；'])
            .next()
            .unwrap_or_default();
        let cited_clause = if preceding.ends_with(['。', '！', '？', '；']) {
            statement.to_owned()
        } else {
            let end = occurrence["end"].as_u64().unwrap_or_default() as usize;
            let following = String::from_utf16_lossy(&encoded[end..]);
            let tail = following
                .split(['\n', '。', '！', '？', '；', ';'])
                .next()
                .unwrap_or_default();
            format!(
                "{}{}{}",
                statement,
                occurrence["label"].as_str().unwrap_or_default(),
                tail
            )
        };
        let excerpt = reference.map(|r| {
            let a = occurrence["startLine"].as_u64().unwrap() as usize;
            let b = occurrence["endLine"].as_u64().unwrap() as usize;
            r["content"]
                .as_str()
                .unwrap_or_default()
                .split('\n')
                .skip(a - 1)
                .take(b - a + 1)
                .collect::<Vec<_>>()
                .join("\n")
        });
        let supported = reference.zip(excerpt.as_ref()).is_some_and(|(r, excerpt)| {
            case.gold
                .iter()
                .any(|(id, version)| key(r) == (id, *version))
                && rules.iter().any(|fact| {
                    let claim = if retention_days(fact).is_some() {
                        &cited_clause
                    } else {
                        statement
                    };
                    matches_fact(claim, fact) && matches_fact(excerpt, fact)
                })
        });
        if supported {
            valid += 1;
            cited.insert(key(reference.unwrap()));
        }
        checks.push(json!({"location":occurrence,"supportsExpectedFact":supported}));
    }
    result["metrics"]["answerQuality"] = json!(answer);
    result["metrics"]["citationPrecision"] = json!(ratio(valid, checks.len()));
    result["metrics"]["citationRecall"] = json!(ratio(cited.len(), case.gold.len()));
    result["responseScoring"] = json!({"rubric":RUNTIME_VERSION,"facts":case.replay.claims.iter().zip(claims)
        .map(|((fact,_),matched)|json!({"expected":fact,"matched":matched})).collect::<Vec<_>>(),
        "abstained":abstained,"conflictMentioned":conflict,"citations":checks});
    Ok(result)
}

pub(super) fn summarize(rows: &[Value]) -> Value {
    let mut metrics = BTreeMap::<&str, (f64, usize)>::new();
    for row in rows {
        if let Some(values) = row["metrics"].as_object() {
            for (name, value) in values {
                let total = metrics.entry(name).or_default();
                if let Some(value) = value.as_f64() {
                    total.0 += value;
                    total.1 += 1;
                }
            }
        }
    }
    json!(
        metrics
            .into_iter()
            .map(|(name, (total, count))| (
                name,
                json!({"value":(count > 0).then(|| total / count as f64),"cases":count})
            ))
            .collect::<BTreeMap<_, _>>()
    )
}

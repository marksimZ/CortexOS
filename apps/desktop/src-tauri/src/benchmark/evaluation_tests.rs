use super::*;
fn case(id: &str) -> Case {
    super::super::dataset::builtin_version(2)
        .unwrap()
        .cases
        .into_iter()
        .find(|c| c.case_id == id)
        .unwrap()
}
fn text(v: Value) -> String {
    serde_json::to_string(&v).unwrap()
}
fn gold(c: &Case) -> String {
    match &c.expected {
        Expected::Json { answer } => text(answer.clone()),
        _ => panic!(),
    }
}
fn expect(c: &Case, value: Value, state: State) {
    assert_eq!(evaluate(c, &text(value), &[]).state, state);
}

#[test]
fn scoring_alias_and_integer_are_contract_specific() {
    let chemical = case("zh-007");
    assert_eq!(evaluate(&chemical, " H2O\n", &[]).state, State::Passed);
    for s in ["h2o", "H₂O", "水：H2O"] {
        assert_eq!(evaluate(&chemical, s, &[]).state, State::Failed);
    }
    assert_eq!(evaluate(&case("zh-003"), "論語", &[]).state, State::Passed);
    assert_eq!(evaluate(&case("zh-014"), "增添", &[]).state, State::Passed);
    let math = case("math-001");
    for s in ["95", " 95\n", "00095"] {
        assert_eq!(evaluate(&math, s, &[]).state, State::Passed);
    }
    for s in [
        "+95",
        "95.0",
        "９５",
        "9+86",
        "95元",
        "96",
        "-",
        "9999999999999999999999999999999999",
    ] {
        assert_eq!(evaluate(&math, s, &[]).state, State::Failed);
    }
    let mut bad = chemical;
    bad.scoring.contract.as_mut().unwrap()["unicode_folding"] = json!(true);
    assert_eq!(evaluate(&bad, "H2O", &[]).state, State::ScoringError);
}

#[test]
fn scoring_json_structural_semantic_and_exact_numbers() {
    let c = case("schema-001");
    for s in [
        r#"{"age":28,"name":"林禾"}"#,
        r#"{"name":"林禾","age":28.0}"#,
        r#"{"name":"林禾","age":2.8e1}"#,
    ] {
        assert_eq!(evaluate(&c, s, &[]).state, State::Passed);
    }
    for s in [
        r#"{"name":"林禾"}"#,
        r#"{"name":"林禾","age":"28"}"#,
        r#"{"name":"林禾","age":true}"#,
        r#"{"name":"林禾","age":28,"extra":0}"#,
        r#"{"name":"林禾","age":28,"age":29}"#,
        r#"{"name":"林禾","age":28.0000000000000000001}"#,
        r#"{"name":"林禾","age":{"$serde_json::private::Number":"28"}}"#,
        r#"{"name":"林禾","age":28}{}"#,
        "```json\n{}\n```",
        "NaN",
    ] {
        assert_eq!(evaluate(&c, s, &[]).state, State::Failed, "{s}");
    }
    expect(
        &case("schema-003"),
        json!({"colors":["绿","蓝","红"]}),
        State::Failed,
    );
    expect(
        &case("schema-005"),
        json!({"sku":"A-17","stock":0}),
        State::Failed,
    );
    expect(
        &case("schema-005"),
        json!({"sku":"A-17","stock":false,"note":null}),
        State::Failed,
    );
    expect(
        &case("schema-004"),
        json!({"date":"2026-02-30","city":"杭州"}),
        State::Failed,
    );
    let mut invalid = c;
    invalid.input.response_schema.as_mut().unwrap()["$ref"] = json!("file:///private/schema.json");
    assert_eq!(evaluate(&invalid, "{}", &[]).state, State::ScoringError);
    assert!(super::super::evaluation_json::equal(
        &serde_json::from_str("100000000000000000000000000000000001").unwrap(),
        &serde_json::from_str("100000000000000000000000000000000001.0").unwrap()
    ));
    assert!(!super::super::evaluation_json::equal(
        &json!(true),
        &json!(1)
    ));
}

#[test]
fn scoring_rag_answers_and_support_are_independent() {
    for id in 1..=10 {
        let c = case(&format!("rag-{id:03}"));
        assert_eq!(evaluate(&c, &gold(&c), &[]).state, State::Passed, "{id}");
    }
    let c = case("rag-008");
    let mut answer: Value = serde_json::from_str(&gold(&c)).unwrap();
    answer["citations"] = json!([]);
    let no_citation = evaluate(&c, &text(answer.clone()), &[]);
    assert_eq!(no_citation.state, State::Failed);
    assert_eq!(no_citation.metrics["answerCorrect"], true);
    assert!(no_citation.metrics["citationPrecision"].is_null());
    assert_eq!(no_citation.metrics["citationCoverage"], 0.0);
    answer["citations"] = json!([{"claim_id":"floor","source_id":"map","start":1,"end":1}]);
    let wrong = evaluate(&c, &text(answer.clone()), &[]);
    assert_eq!(wrong.state, State::Failed);
    assert_eq!(
        wrong.details["dimensions"]["citations"][0]["positionValid"],
        true
    );
    assert_eq!(wrong.metrics["supportedCitationCount"], 0);
    for (start, end) in [(2, 2), (1, 2), (2, 3), (1, 3)] {
        answer["citations"][0]["start"] = json!(start);
        answer["citations"][0]["end"] = json!(end);
        expect(&c, answer.clone(), State::Passed);
    }
    answer["citations"][0]["start"] = serde_json::from_str("1.0").unwrap();
    expect(&c, answer.clone(), State::Passed);
    answer["citations"][0]["end"] = json!(99);
    expect(&c, answer.clone(), State::Failed);
    answer["citations"][0]["source_id"] = json!("missing");
    expect(&c, answer, State::Failed);
    let c = case("rag-001");
    let mut a: Value = serde_json::from_str(&gold(&c)).unwrap();
    a["facts"]["opening_time"] = json!("上午九点");
    expect(&c, a, State::Passed);
}

#[test]
fn scoring_rag_refusal_conflict_and_effective_version() {
    let c = case("rag-004");
    let mut a: Value = serde_json::from_str(&gold(&c)).unwrap();
    let refusal = evaluate(&c, &text(a.clone()), &[]);
    assert_eq!(refusal.state, State::Passed);
    assert!(refusal.metrics["citationPrecision"].is_null());
    a["facts"] = json!({"saturday_price_yuan":42});
    expect(&c, a, State::Failed);
    let c = case("rag-006");
    let mut a: Value = serde_json::from_str(&gold(&c)).unwrap();
    a["citations"].as_array_mut().unwrap().pop();
    expect(&c, a, State::Failed);
    let c = case("rag-010");
    let mut a: Value = serde_json::from_str(&gold(&c)).unwrap();
    a["citations"][0]["source_id"] = json!("old");
    expect(&c, a, State::Failed);
    let mut bad = c;
    bad.scoring.contract.as_mut().unwrap()["as_of"] = json!("nonsense");
    assert_eq!(evaluate(&bad, "{}", &[]).state, State::ScoringError);
}

#[test]
fn scoring_native_calls_require_actual_fields_and_exact_contract() {
    for n in 1..=5 {
        let c = case(&format!("tool-{n:03}"));
        let Expected::Tool { name, arguments } = &c.expected else {
            panic!()
        };
        let call = json!({"function":{"name":name,"arguments":arguments}});
        assert_eq!(evaluate(&c, "", &[call.clone()]).state, State::Passed);
        assert_eq!(evaluate(&c, &text(call.clone()), &[]).state, State::Failed);
        assert_eq!(
            evaluate(&c, "", &[call.clone(), call.clone()]).state,
            State::Failed
        );
        let mut bad = call.clone();
        bad["function"]["name"] = json!("wrong_tool");
        assert_eq!(evaluate(&c, "", &[bad]).state, State::Failed);
        let mut bad = call.clone();
        bad["function"]["arguments"] = json!({});
        assert_eq!(evaluate(&c, "", &[bad]).state, State::Failed);
        let mut bad = call.clone();
        bad["function"]["arguments"]["extra"] = json!(1);
        assert_eq!(evaluate(&c, "", &[bad]).state, State::Failed);
    }
    let c = case("tool-002");
    let good = serde_json::from_str(
        r#"{"function":{"name":"search_notes","arguments":{"limit":3.0,"query":"发布清单"}}}"#,
    )
    .unwrap();
    assert_eq!(evaluate(&c, "", &[good]).state, State::Passed);
    for limit in [json!(2), json!("3"), json!(true)] {
        assert_eq!(evaluate(&c,"",&[json!({"function":{"name":"search_notes","arguments":{"query":"发布清单","limit":limit}}})]).state,State::Failed);
    }
}

#[test]
fn scoring_tool_fragments_are_preserved_and_merged_only_by_index() {
    use super::super::evaluation_json::assemble_calls;
    let first =
        json!({"function":{"index":0,"name":"search_notes","arguments":{"query":"发布清单"}}});
    let second = json!({"function":{"index":0,"name":"search_notes","arguments":{"limit":3}}});
    let calls = assemble_calls(&[vec![first.clone()], vec![second.clone()]]).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(evaluate(&case("tool-002"), "", &calls).state, State::Passed);
    let mut bad = second;
    bad["function"]["name"] = json!("other");
    assert!(assemble_calls(&[vec![first], vec![bad]]).is_err());
}

#[test]
fn scoring_status_denominators_exclude_unscored_items_without_hiding_coverage() {
    let samples = vec![
        (Score::state(State::Passed, "ok"), true, true),
        (Score::state(State::Failed, "wrong"), true, true),
        (
            Score::state(State::Unsupported, "advertised_no_tools"),
            false,
            false,
        ),
        (Score::state(State::NotRun, "pending"), false, false),
        (
            Score::state(State::ExecutionError, "docker_missing"),
            false,
            false,
        ),
        (
            Score::state(State::ScoringError, "grader_failure"),
            true,
            true,
        ),
    ];
    let s = statistics(&samples);
    assert_eq!(s.planned, 6);
    assert_eq!(s.scored, 2);
    assert_eq!(s.correctness, Some(0.5));
    assert_eq!(s.scoring_coverage, Some(2.0 / 6.0));
    assert_eq!(s.execution_success_rate, Some(1.0));
    for (score, _, _) in &samples[2..] {
        assert!(score.metrics["correctness"].is_null());
    }
    assert!(statistics(&[]).correctness.is_none());
    let c = case("schema-001");
    let one = evaluate(&c, &gold(&c), &[]);
    let two = evaluate(&c, &gold(&c), &[]);
    assert_eq!(
        serde_json::to_value(one).unwrap(),
        serde_json::to_value(two).unwrap()
    );
}

#[test]
fn scoring_all_fifty_non_python_standards_are_deterministic() {
    let bundle = super::super::dataset::builtin_version(2).unwrap();
    let mut count = 0;
    for c in &bundle.cases {
        let (output, calls) = match &c.expected {
            Expected::Aliases { answers } => (answers[0].clone(), vec![]),
            Expected::Integer { answer } => (answer.to_string(), vec![]),
            Expected::Json { answer } => (answer.to_string(), vec![]),
            Expected::Tool { name, arguments } => (
                String::new(),
                vec![json!({"function":{"name":name,"arguments":arguments}})],
            ),
            Expected::Python { .. } => continue,
            _ => panic!(),
        };
        let one = evaluate(c, &output, &calls);
        let two = evaluate(c, &output, &calls);
        assert_eq!(one.state, State::Passed, "{}: {}", c.case_id, one.details);
        assert_eq!(
            serde_json::to_value(one).unwrap(),
            serde_json::to_value(two).unwrap()
        );
        count += 1;
    }
    assert_eq!(count, 50);
    let c = case("rag-008");
    let mut a: Value = serde_json::from_str(&gold(&c)).unwrap();
    let mut repeated = a["citations"][0].clone();
    repeated["start"] = serde_json::from_str("2.0").unwrap();
    a["citations"].as_array_mut().unwrap().push(repeated);
    expect(&c, a, State::Failed);
    let mut p = crate::ollama::Ndjson::strict();
    assert!(p.push(b"{\"message\":{\"role\":\"assistant\",\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"search_notes\",\"arguments\":{\"limit\":3,\"limit\":2}}}]},\"done\":true}\n")[0].is_err());
}

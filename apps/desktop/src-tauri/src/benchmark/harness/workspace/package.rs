use super::*;
pub(in crate::benchmark::harness) fn bundle(task: &Value) -> AppResult<Value> {
    let body = &task["payload"]["task"]["body"];
    if body["project"].is_object() {
        let project = &body["project"];
        let task_json = json!({"version":"python-project-1","taskId":task["payload"]["case"]["case_id"],"taskVersion":task["version"],"kind":project["kind"],"prompt":body["spec"]["prompt"],"entry":project["entry"],"function":body["spec"]["function"],"parameters":body["spec"]["parameters"],"returnType":body["spec"]["returnType"],"files":body["policy"]["allowedPaths"],"writePaths":project["writePaths"]}).to_string();
        if task_json.len() > 32_000 {
            return Err("项目任务说明过大。".into());
        }
        return Ok(
            json!({"taskJson":task_json,"taskHash":hash(task_json.as_bytes()),"originalFiles":project["original"],"scope":"python_project","entry":project["entry"],"writePaths":project["writePaths"],"kind":project["kind"]}),
        );
    }
    let path = body["source"]["path"].as_str().ok_or("任务源文件缺失。")?;
    if body["policy"]["writePaths"] != json!([path])
        || body["policy"]["allowedPaths"] != json!([path])
    {
        return Err("当前副本只支持已审核的单文件函数任务。".into());
    }
    let source = body["source"]["excerpt"].as_str().ok_or("函数原文缺失。")?;
    let spec = &body["spec"];
    let task_json=json!({"version":"function-workspace-1","taskId":task["payload"]["case"]["case_id"],"taskVersion":task["version"],"prompt":spec["prompt"],"function":spec["function"],"parameters":spec["parameters"],"returnType":spec["returnType"],"target":{"path":path,"startLine":body["source"]["startLine"],"endLine":body["source"]["endLine"],"contentScope":"selected_function_only"},"constraints":{"pureFunction":true,"modifyInput":false,"network":false,"fileAccess":false,"writePaths":[format!("project/{path}")]}}).to_string();
    if task_json.len() > 65536 || source.len() > 24000 {
        return Err("任务说明或函数超出副本上限。".into());
    }
    Ok(
        json!({"taskJson":task_json,"taskHash":hash(task_json.as_bytes()),"originalCode":source,"originalCodeHash":hash(source.as_bytes()),"target":format!("project/{path}"),"scope":"selected_function_only"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_bundle_and_file_inventory_preserve_scope_without_answer_fields() {
        let task = json!({"version":1,"payload":{"case":{"case_id":"fixture"},"task":{"body":{"source":{"path":"src/value.py","excerpt":"def f(x):\n    return x","startLine":2,"endLine":3},"policy":{"writePaths":["src/value.py"],"allowedPaths":["src/value.py"]},"spec":{"prompt":"Return the input","function":"f","parameters":[{"name":"x","kind":"integer"}],"returnType":"integer","referenceCode":"HIDDEN_REFERENCE","wrongCode":"HIDDEN_WRONG","examples":[{"output":"HIDDEN_EXPECTED"}]}}}}});
        let bundle = bundle(&task).unwrap();
        let raw = bundle.to_string();
        for forbidden in [
            "HIDDEN_REFERENCE",
            "HIDDEN_WRONG",
            "HIDDEN_EXPECTED",
            "referenceCode",
            "examples",
            "wrongCode",
        ] {
            assert!(!raw.contains(forbidden));
        }
        let mut bad = task.clone();
        bad["payload"]["task"]["body"]["policy"]["writePaths"] =
            json!(["src/value.py", "outside.py"]);
        assert!(super::bundle(&bad).is_err());
        let code = bundle["originalCode"].as_str().unwrap();
        let plan = json!({"bundle":bundle});
        let observed = json!({"protocol":1,"python":[3,13],"taskHash":plan["bundle"]["taskHash"],"code":code,"files":[{"path":"task.json","hash":plan["bundle"]["taskHash"],"mode":292,"uid":0},{"path":"project/src/value.py","hash":hash(code.as_bytes()),"mode":438,"uid":0}]});
        assert!(verify_observation(&plan, &observed, code).is_ok());
        let mut bad = observed.clone();
        bad["files"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"extra.py"}));
        assert!(verify_observation(&plan, &bad, code).is_err());
        let (excerpt, proposed) = replace_range(
            "# 前言\r\ndef f(x):\r\n    return x\r\n# 尾部",
            2,
            3,
            "def f(x):\n    return x+1",
        )
        .unwrap();
        assert_eq!(excerpt, "def f(x):\n    return x");
        assert_eq!(proposed, "# 前言\r\ndef f(x):\n    return x+1\n# 尾部");
        assert!(replace_range("a\n", 0, 1, "b").is_err());
        assert!(replace_range("a\n", 1, 2, "b").is_err());
        let mut bad = observed.clone();
        bad["files"][0]["mode"] = json!(438);
        assert!(verify_observation(&plan, &bad, code).is_err());
        let mut bad = observed.clone();
        bad["files"][1]["hash"] = json!("forged");
        assert!(verify_observation(&plan, &bad, code).is_err());
        let mut bad = observed.clone();
        bad["taskHash"] = json!("forged");
        assert!(verify_observation(&plan, &bad, code).is_err());
    }
}
pub(in crate::benchmark::harness) fn verify_observation(
    plan: &Value,
    observed: &Value,
    expected_code: &str,
) -> AppResult<()> {
    let bundle = &plan["bundle"];
    let files = observed["files"].as_array().ok_or("副本文件清单缺失。")?;
    let python = if plan["purpose"] == "harness_aider_session" {
        json!([3, 12])
    } else {
        json!([3, 13])
    };
    if observed["protocol"] != 1
        || observed["python"] != python
        || observed["taskHash"] != bundle["taskHash"]
        || observed["code"] != expected_code
        || files.len() != 2
        || files.iter().any(|f| f["uid"] != 0)
        || !files.iter().any(|f| {
            f["path"] == "task.json" && f["hash"] == bundle["taskHash"] && f["mode"] == 0o444
        })
        || !files.iter().any(|f| {
            f["path"] == bundle["target"]
                && f["hash"] == hash(expected_code.as_bytes())
                && f["mode"] == 0o666
        })
    {
        return Err("副本内容、权限或允许文件范围不一致，拒绝交给测试。".into());
    }
    Ok(())
}

pub(in crate::benchmark::harness) fn verify_project(
    plan: &Value,
    observed: &Value,
    initial: bool,
) -> AppResult<Value> {
    let bundle = &plan["bundle"];
    let origin = bundle["originalFiles"]
        .as_object()
        .ok_or("原项目文件清单缺失。")?;
    let writes = bundle["writePaths"]
        .as_array()
        .ok_or("项目修改范围缺失。")?;
    let contents = observed["contents"]
        .as_object()
        .ok_or("项目副本文件缺失。")?;
    let files = observed["files"].as_array().ok_or("项目副本清单缺失。")?;
    let version = if plan["purpose"] == "harness_aider_session" {
        json!([3, 12])
    } else {
        json!([3, 13])
    };
    if observed["protocol"] != 2
        || observed["python"] != version
        || observed["taskHash"] != bundle["taskHash"]
        || origin.len() != contents.len()
        || origin.len() + 1 != files.len()
        || files.iter().any(|f| f["uid"] != 0)
        || !files.iter().any(|f| {
            f["path"] == "task.json" && f["mode"] == 0o444 && f["hash"] == bundle["taskHash"]
        })
    {
        return Err("workspace_project_inventory_mismatch".into());
    }
    for (path, original) in origin {
        let code = contents
            .get(path)
            .and_then(Value::as_str)
            .ok_or("项目文件内容缺失。")?;
        let writable = writes.iter().any(|v| v == path);
        if ((initial || !writable) && original != code)
            || !files.iter().any(|f| {
                f["path"] == format!("project/{path}")
                    && f["hash"] == hash(code.as_bytes())
                    && f["mode"] == if writable { 0o666 } else { 0o444 }
            })
        {
            return Err("workspace_project_edit_outside_scope".into());
        }
    }
    Ok(observed["contents"].clone())
}

pub(in crate::benchmark::harness) fn project_patch(
    plan: &Value,
    contents: &Value,
    provenance: &str,
) -> AppResult<Value> {
    let original = plan["bundle"]["originalFiles"]
        .as_object()
        .ok_or("原项目文件缺失。")?;
    let files: Vec<_> = original.iter().filter(|(path, before)| contents[*path] != **before).map(|(path, before)| {
        let after = &contents[path];
        json!({"path":path,"before":before,"after":after,"originalFileHash":hash(before.as_str().unwrap_or_default().as_bytes()),"proposedFileHash":hash(after.as_str().unwrap_or_default().as_bytes())})
    }).collect();
    if files.is_empty() || (plan["bundle"]["kind"] == "multi_file" && files.len() < 2) {
        return Err("project_patch_incomplete".into());
    }
    Ok(
        json!({"format":"project-replacement-1","provenance":provenance,"files":files,"writtenToOriginal":false,"taskExecution":"candidate_collected"}),
    )
}
pub(super) fn patch(db: &Database, workspace: &str, plan: &Value, code: &str) -> AppResult<Value> {
    patch_with_provenance(db, workspace, plan, code, "manual_handoff")
}
pub(in crate::benchmark::harness) fn patch_with_provenance(
    db: &Database,
    workspace: &str,
    plan: &Value,
    code: &str,
    provenance: &str,
) -> AppResult<Value> {
    let task = db.preflight_task(workspace, &task_input(plan)?)?;
    let body = &task["payload"]["task"]["body"];
    let project = tasks::checked_snapshot(
        db,
        workspace,
        body["origin"]["repositoryId"]
            .as_str()
            .ok_or("项目来源缺失。")?,
    )?;
    let file = project
        .files
        .iter()
        .find(|f| f.path == body["source"]["path"])
        .ok_or("源文件缺失。")?;
    let start = body["source"]["startLine"]
        .as_u64()
        .ok_or("来源范围缺失。")? as usize;
    let end = body["source"]["endLine"].as_u64().ok_or("来源范围缺失。")? as usize;
    let (excerpt, proposed) = replace_range(&file.content, start, end, code)?;
    if excerpt != plan["bundle"]["originalCode"] {
        return Err("原函数范围已不一致。".into());
    }
    Ok(
        json!({"format":"function-replacement-1","provenance":provenance,"path":file.path,"startLine":start,"endLine":end,"originalFileHash":file.content_hash,"before":plan["bundle"]["originalCode"],"after":code,"codeHash":hash(code.as_bytes()),"proposedFileHash":hash(proposed.as_bytes()),"proposedFile":proposed,"unchangedOutsideRange":true,"writtenToOriginal":false,"taskExecution":if provenance=="manual_handoff"{"Not Run"}else{"candidate_collected"}}),
    )
}
fn replace_range(
    source: &str,
    start: usize,
    end: usize,
    code: &str,
) -> AppResult<(String, String)> {
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    if start == 0 || end < start || end > lines.len() {
        return Err("原函数范围已不一致。".into());
    }
    let mut replacement = code.to_string();
    if lines[end - 1].ends_with('\n') && !replacement.ends_with('\n') {
        replacement.push('\n');
    }
    let proposed = format!(
        "{}{}{}",
        lines[..start - 1].concat(),
        replacement,
        lines[end..].concat()
    );
    Ok((
        lines[start - 1..end]
            .concat()
            .lines()
            .collect::<Vec<_>>()
            .join("\n"),
        proposed,
    ))
}

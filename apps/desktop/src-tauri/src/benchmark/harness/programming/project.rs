use super::*;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 24_000;
const VERSION: &str = "python-project-1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Spec {
    kind: String,
    files: Vec<String>,
    reference: BTreeMap<String, String>,
    negative: BTreeMap<String, String>,
}

pub(super) fn prepare(spec: &Spec, snapshot: &RepositorySnapshot, entry: &str) -> AppResult<Value> {
    if !matches!(
        spec.kind.as_str(),
        "multi_file" | "regression" | "add_tests" | "database_compatibility"
    ) {
        return Err("请选择有效的项目任务类型。".into());
    }
    let paths: BTreeSet<_> = spec.files.iter().collect();
    if !(2..=8).contains(&paths.len())
        || paths.len() != spec.files.len()
        || !paths.iter().any(|p| p.as_str() == entry)
        || spec.reference.is_empty()
        || spec.reference.keys().ne(spec.negative.keys())
        || spec
            .reference
            .keys()
            .any(|p| !paths.contains(p) || !p.ends_with(".py"))
    {
        return Err("请选择 2 至 8 个不同的快照文件，含入口文件。两份对照须填写同一组允许修改的 Python 文件。".into());
    }
    let mut original = BTreeMap::new();
    for path in paths {
        let (base, extension) = path
            .rsplit_once('.')
            .ok_or("项目文件必须为 .py 或 .sql。")?;
        if !matches!(extension, "py" | "sql")
            || path.len() > 160
            || !base.split('/').all(identifier)
        {
            return Err("项目文件路径只支持普通 Python 模块名或 SQL 资料名。".into());
        }
        let file = snapshot
            .files
            .iter()
            .find(|file| &file.path == path)
            .ok_or("所选文件不在固定项目快照中。")?;
        original.insert(path.clone(), file.content.clone());
    }
    let complete = |changes: &BTreeMap<String, String>| -> AppResult<BTreeMap<String, String>> {
        let mut files = original.clone();
        for (path, code) in changes {
            if code.contains('\0') {
                return Err("Python 文本含无效字符。".into());
            }
            files.insert(path.clone(), code.clone());
        }
        if files.values().map(String::len).sum::<usize>() > MAX_BYTES
            || serde_json::to_vec(&files).map_err(json_error)?.len() > 32_000
        {
            return Err("项目所选文件正文合计最多 24 KB，请缩小副本范围。".into());
        }
        Ok(files)
    };
    let original = complete(&BTreeMap::new())?;
    let reference = complete(&spec.reference)?;
    let negative = complete(&spec.negative)?;
    let changes = spec
        .reference
        .keys()
        .filter(|p| original[*p] != reference[*p])
        .count();
    if reference == negative || changes == 0 || (spec.kind == "multi_file" && changes < 2) {
        return Err(
            "参考实现须与原代码及错误实现不同。跨文件题的参考实现须实际修改至少两个文件。".into(),
        );
    }
    if spec.kind == "add_tests"
        && spec.reference.keys().any(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            !name.starts_with("test_") && !name.ends_with("_test.py")
        })
    {
        return Err(
            "补写测试题只允许修改 test_*.py 或 *_test.py 测试文件，业务实现保持只读。".into(),
        );
    }
    if spec.kind == "add_tests" && spec.reference.contains_key(entry) {
        return Err(
            "补写测试题需要只读的评分入口，由该文件调用待补写的测试并检查能否识别错误实现。".into(),
        );
    }
    if spec.kind == "database_compatibility"
        && !spec.files.iter().any(|path| path.ends_with(".sql"))
    {
        return Err("数据库兼容题需包含只读的 SQL 快照或迁移资料。".into());
    }
    Ok(
        json!({"version":VERSION,"kind":spec.kind,"entry":entry,"original":original,"reference":reference,"wrong":negative,"writePaths":spec.reference.keys().collect::<Vec<_>>()}),
    )
}

pub(super) fn variants(body: &Value) -> [(&str, Value); 3] {
    let project = &body["project"];
    if project.is_object() {
        return [
            ("reference", project["reference"].clone()),
            ("unchanged", project["original"].clone()),
            ("wrong", project["wrong"].clone()),
        ];
    }
    [
        ("reference", body["spec"]["referenceCode"].clone()),
        ("unchanged", body["source"]["excerpt"].clone()),
        ("wrong", body["spec"]["wrongCode"].clone()),
    ]
}

pub(in crate::benchmark::harness) fn content_hash(content: &Value) -> String {
    match content.as_str() {
        Some(code) => hash(code.as_bytes()),
        None => hash(content.to_string().as_bytes()),
    }
}

use crate::benchmark::dataset::hash;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CATEGORIES: &[(&str, &str)] = &[
    ("languages", "编程语言"),
    ("stack", "框架与数据库"),
    ("environment", "系统与环境"),
    ("build", "构建与测试"),
    ("git", "Git工作流"),
    ("constraints", "项目规范"),
    ("tools", "终端与外部工具"),
    ("security", "权限与安全"),
    ("approval", "阶段与人工审批"),
    ("models", "模型与费用"),
    ("scale", "项目规模与跨文件修改"),
    ("context", "上下文与项目指令"),
];
pub fn criteria() -> Value {
    json!({
 "windows_native":{"label":"原生Windows入口","group":"compatibility"},
 "cli":{"label":"CLI自动化入口","group":"automation"},
 "events":{"label":"结构化执行事件","group":"automation"},
 "local_model":{"label":"本地模型接入方式（具体模型待测）","group":"compatibility"},
 "agents_md":{"label":"AGENTS.md项目约定","group":"context"},
 "project_instructions":{"label":"项目指令文件","group":"context"},
 "mcp":{"label":"MCP接入（执行模式需另核实）","group":"automation"},
 "sandbox":{"label":"沙盒隔离机制","group":"governance"},
 "human_approval":{"label":"无界面自动化中的人工审批","group":"governance"},
 "repository_editing":{"label":"仓库文件修改入口","group":"context"}})
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub source_id: String,
    pub path: String,
    pub content_hash: String,
    pub start_line: usize,
    pub end_line: usize,
    pub excerpt: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub category: String,
    pub original_value: String,
    pub value: String,
    pub criterion: Option<String>,
    pub status: String,
    pub confidence: String,
    pub hard: bool,
    pub evidence: Option<Evidence>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub source_id: String,
    pub path: String,
    pub content_hash: String,
    pub content: String,
}

// Matches are review candidates, including negated, historical and example statements.
// A keyword match never establishes a hard requirement or a verified capability.
pub fn extract(documents: &[Document]) -> Vec<Requirement> {
    let rules: &[(&str, &[&str], Option<&str>)] = &[
        (
            "languages",
            &[
                "Rust",
                "TypeScript",
                "JavaScript",
                "Python",
                "Golang",
                "Java",
                "C++",
            ],
            None,
        ),
        (
            "stack",
            &[
                "React",
                "Tauri",
                "Vue",
                "Django",
                "FastAPI",
                "SQLite",
                "PostgreSQL",
                "MySQL",
                "rusqlite",
                "sqlx",
            ],
            None,
        ),
        (
            "environment",
            &["Windows", "WebView2", "Linux", "macOS", "WSL", "Docker"],
            None,
        ),
        (
            "build",
            &[
                "npm",
                "pnpm",
                "Cargo",
                "pytest",
                "Playwright",
                "Vitest",
                "MSVC",
                "构建",
                "测试",
            ],
            None,
        ),
        ("git", &["Git", "提交", "分支", "推送", "worktree"], None),
        (
            "constraints",
            &[
                "规范",
                "必须",
                "禁止",
                "不得",
                "must",
                "shall",
                "constraint",
            ],
            None,
        ),
        (
            "tools",
            &["终端", "terminal", "MCP", "CLI", "外部工具"],
            None,
        ),
        (
            "security",
            &[
                "权限",
                "安全",
                "sandbox",
                "沙盒",
                "只读",
                "read-only",
                "sensitive",
                "privacy",
            ],
            None,
        ),
        (
            "approval",
            &[
                "审批",
                "人工确认",
                "分阶段",
                "验收",
                "approval",
                "confirmation",
                "milestone",
            ],
            None,
        ),
        (
            "models",
            &[
                "Ollama", "模型", "model", "API", "预算", "成本", "cost", "token",
            ],
            None,
        ),
        (
            "scale",
            &[
                "跨文件",
                "项目规模",
                "大型",
                "cross-file",
                "repository",
                "仓库",
            ],
            None,
        ),
        (
            "context",
            &["AGENTS.md", "CLAUDE.md", "上下文", "context", "指令文件"],
            None,
        ),
    ];
    let mut result = Vec::new();
    for doc in documents {
        for (index, line) in doc.content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            for (category, terms, _) in rules {
                let hits: Vec<_> = terms
                    .iter()
                    .filter(|t| matches_term(line, t))
                    .copied()
                    .collect();
                if hits.is_empty() {
                    continue;
                }
                // Keep the statement itself; do not convert “no cloud” into “requires cloud”.
                let value = line.trim().to_string();
                let id = hash(
                    format!("{}:{}:{}:{}", doc.source_id, doc.path, index + 1, category).as_bytes(),
                );
                let criterion = match *category {
                    "environment" if matches_term(line, "Windows") => Some("windows_native"),
                    "tools" if matches_term(line, "MCP") => Some("mcp"),
                    "tools" if matches_term(line, "CLI") => Some("cli"),
                    "tools" if line.contains("执行事件") => Some("events"),
                    "security" if matches_term(line, "sandbox") || line.contains("沙盒") => {
                        Some("sandbox")
                    }
                    "approval"
                        if line.contains("审批")
                            || matches_term(line, "approval")
                            || matches_term(line, "confirmation") =>
                    {
                        Some("human_approval")
                    }
                    "models" if matches_term(line, "Ollama") || line.contains("本地模型") => {
                        Some("local_model")
                    }
                    "context" if matches_term(line, "AGENTS.md") => Some("agents_md"),
                    "context" if matches_term(line, "CLAUDE.md") => Some("project_instructions"),
                    "scale" if line.contains("跨文件") => Some("repository_editing"),
                    _ => None,
                }
                .map(str::to_string);
                let ambiguous = [
                    "not",
                    "no",
                    "never",
                    "future",
                    "optional",
                    "defer",
                    "example",
                    "later",
                    "禁止",
                    "不得",
                    "不要",
                    "不允许",
                    "暂不",
                    "未",
                    "未来",
                    "后续",
                    "例如",
                    "举例",
                    "候选",
                ]
                .iter()
                .any(|term| matches_term(line, term));
                let criterion = if ambiguous { None } else { criterion };
                result.push(Requirement {
                    id,
                    category: (*category).into(),
                    original_value: value.clone(),
                    value,
                    criterion,
                    status: "candidate".into(),
                    confidence: "literal_match_unverified".into(),
                    hard: false,
                    evidence: Some(Evidence {
                        source_id: doc.source_id.clone(),
                        path: doc.path.clone(),
                        content_hash: doc.content_hash.clone(),
                        start_line: index + 1,
                        end_line: index + 1,
                        excerpt: line.into(),
                    }),
                });
            }
        }
    }
    for (category, _) in CATEGORIES {
        if !result.iter().any(|r| r.category == *category) {
            result.push(Requirement {
                id: format!("unknown-{category}"),
                category: (*category).into(),
                original_value: String::new(),
                value: String::new(),
                criterion: None,
                status: "unknown".into(),
                confidence: "unknown".into(),
                hard: false,
                evidence: None,
            });
        }
    }
    result
}
fn matches_term(line: &str, term: &str) -> bool {
    let line = line.to_lowercase();
    let term = term.to_lowercase();
    line.match_indices(&term).any(|(i, _)| {
        if !term.is_ascii() {
            return true;
        }
        let word = |c: Option<&u8>| c.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
        !word(i.checked_sub(1).and_then(|j| line.as_bytes().get(j)))
            && !word(line.as_bytes().get(i + term.len()))
    })
}

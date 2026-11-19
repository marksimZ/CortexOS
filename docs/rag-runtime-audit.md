# Phase E／F 现状审计

2026-11-14。基线为 Phase D 提交 `8e82c42`。只读核对了 Phase D 的独立验收库。迁移记录连续至 11，SQLite 3.50.4 支持 FTS5，`quick_check` 返回 `ok`，外键检查无返回。该验收库有 4 份评测数据集，知识、文件快照和上下文表为空。下列已有能力依据代码和 SQL 约束。

| 能力 | 状态 | 依据与缺口 |
| --- | --- | --- |
| FTS5 词法召回 | 已完整实现 | [`0006`](../apps/desktop/src-tauri/migrations/0006_knowledge_retrieval.sql)、[`retrieval.rs`](../apps/desktop/src-tauri/src/retrieval.rs)。BM25、标题权重、工作区及有效版本过滤，自动召回保存候选。 |
| 中文和中英混合 | 部分实现 | [`retrieval::terms`](../apps/desktop/src-tauri/src/retrieval.rs#L14) 使用中日韩连续字对、其他文字数字转小写，查询去重及限长。单个汉字、拼写变体和语义改写缺少完整覆盖。 |
| 导入和切片 | 部分实现 | [`sources.rs`](../apps/desktop/src-tauri/src/sources.rs#L70) 流程是明确选文件、预览、提交。支持不超过 2 MiB 的 UTF-8 `.txt`／`.md`，保存原始和规范化内容哈希及行坐标。来源快照与知识版本绑定。当前缺少分片、标题路径、代码符号和父子索引。 |
| 向量检索与精排 | 尚未实现 | 迁移 1–11 和验收库均无向量或 Chunk 表。对话入口仅调用 BM25。 [`db.rs`](../apps/desktop/src-tauri/src/db.rs#L582) |
| 知识版本与冲突 | 部分实现 | 不可改版本和人工批准事件、已生效版本、恢复为新草案、显式 `contradicts` 关系、不可改的单次对话快照已存在。缺少事实生效日期、固定知识快照、隔离实验分支和冲突召回。 [`0002`](../apps/desktop/src-tauri/migrations/0002_knowledge.sql)、[`0003`](../apps/desktop/src-tauri/migrations/0003_graph.sql)、[`knowledge.rs`](../apps/desktop/src-tauri/src/knowledge.rs#L267) |
| 真实 3D 图谱 | 已完整实现 | 知识图由 SQLite 的节点及关系版本提供，界面另有演示数据入口。执行图按实际事件生成，关系版本有作用域和历史。缺少将关系用于检索的逻辑。 [`graph_query.rs`](../apps/desktop/src-tauri/src/graph_query.rs#L324)、[`execution.rs`](../apps/desktop/src-tauri/src/execution.rs)、[`Graph.tsx`](../apps/desktop/src/Graph.tsx#L118) |
| 上下文和预算 | 部分实现 | 手动资料优先，自动知识最多 5 项且至多用剩余的一半，再添加近期完整对话。字节预算是保守估算，已有实际快照与哈希，缺少文档父片段扩展和分阶段精确用量。 [`db.rs`](../apps/desktop/src-tauri/src/db.rs#L555)、[`0004`](../apps/desktop/src-tauri/migrations/0004_context_audit.sql) |
| 回答依据 | 部分实现 | 记录实际入选知识、版本、原文范围与内容哈希。引用校验针对已发送的快照，原文停用后仍可查旧证据。候选摘要还缺少混合排名、原因和分阶段成本。 [`citations.rs`](../apps/desktop/src-tauri/src/citations.rs)、[`sources.rs`](../apps/desktop/src-tauri/src/sources.rs#L211)、[`RetrievalSummary.tsx`](../apps/desktop/src/components/RetrievalSummary.tsx) |
| Phase C1 连接与凭据 | 部分实现 | 已用 Windows Credential Manager 保存一条云连接，Rust 持有密钥，前端获得状态。OpenAI、Anthropic、Gemini 有目录和生成适配。凭据验证读取目录，无知识上传。还缺可共存的多连接、兼容 API、独立能力配置。 [`cloud/mod.rs`](../apps/desktop/src-tauri/src/cloud/mod.rs#L136)、[`vault.rs`](../apps/desktop/src-tauri/src/cloud/vault.rs)、[`provider.rs`](../apps/desktop/src-tauri/src/cloud/provider.rs) |
| Phase C 执行、评分 | 需要重构 | 本地聊天和本地评测共享 Ollama 生成。云端完成路径在评测侧单独接线。评分已有确定性事实、引用、冲突和拒答判断，以及历史重评分。应复用云端调用与评分，收拢本地/云端执行路由及用量，暂无独立 RAG 策略评估集。 [`generation.rs`](../apps/desktop/src-tauri/src/generation.rs)、[`cloud_run.rs`](../apps/desktop/src-tauri/src/benchmark/personalized/cloud_run.rs)、[`evaluation.rs`](../apps/desktop/src-tauri/src/benchmark/evaluation.rs#L168) |
| 增量迁移和兼容 | 已完整实现 | 迁移校验哈希、升级前一致性备份、事务回滚。检索索引可根据当前批准知识重建。新增索引、兼容适配与报告格式需继续独立验证。 [`db.rs`](../apps/desktop/src-tauri/src/db.rs#L57)、[`harness_migration_tests.rs`](../apps/desktop/src-tauri/src/db/harness_migration_tests.rs) |

本轮目录检查时，本机 Ollama 回环服务未响应。没有调用或加载模型。

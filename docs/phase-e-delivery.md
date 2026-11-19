# Phase E 交付报告

## 使用入口

- 「设置 → 模型连接」可为工作区构建本地向量索引。索引绑定模型指纹、维度、归一化规则和版本。重建完整写入后才切换。
- 「知识」可查看文件片段、知识生效日期、版本及替代关系。对话资料区可使用当前知识、固定快照、指定日期或实验分支，支持一至两跳的关系查找。
- 「回答依据」和「图谱 → 执行过程」显示实际查找方式、分段耗时、候选名次、精排分数、入选原因、知识和文件版本、原文范围及最终上下文。
- 「评测 → 检索评测 → 固定回放」可保存两组策略对照，查看历史和逐题依据。

## 实现与证据

| 步骤 | 实际交付 | 代码 |
| --- | --- | --- |
| E0 | 现状审计和统一流水线，保留 SQLite 和既有快照 | [审计](rag-runtime-audit.md)、[实施计划](rag-runtime-plan.md) |
| E1 | FTS5/BM25、Ollama 向量、RRF、去重、版本与工作区过滤、内置词项重排 | [检索](../apps/desktop/src-tauri/src/retrieval.rs)、[索引](../apps/desktop/src-tauri/src/rag.rs) |
| E2 | Markdown 标题、围栏代码、常见符号、纯文本段落、原文行列及子片召回，按预算展开同节父片 | [分片](../apps/desktop/src-tauri/src/source_chunks.rs)、[来源](../apps/desktop/src-tauri/src/sources.rs) |
| E3 | 当前、指定日期、快照及分支。依据事实日期判断生效，按审批顺序识别替代，从已登记反驳边取双方证据 | [范围](../apps/desktop/src-tauri/src/knowledge_scope.rs)、[冲突](../apps/desktop/src-tauri/src/knowledge_conflicts.rs) |
| E4 | 基于已有关系版本记录方向、边修订、跨文件路径及知识簇摘要 | [关系查找](../apps/desktop/src-tauri/src/knowledge_graph_retrieval.rs) |
| E5 | 轻量路由选择跳过、词语、混合、精排、关系及版本范围，保存计划、实际策略与回退原因 | [路由](../apps/desktop/src-tauri/src/retrieval.rs)、[上下文组装](../apps/desktop/src-tauri/src/db.rs) |
| E6 | 回答依据与执行图共用实际查找记录，支持原文和回答定位 | [查找检查](../apps/desktop/src/components/RetrievalInspection.tsx)、[执行图](../apps/desktop/src/ExecutionTrace.tsx) |
| E7 | 9 道合成题、11 份文件，冻结知识和评分指纹。在隔离库实际运行检索，回放固定向量和回复 | [题库](../apps/desktop/src-tauri/datasets/cortex-rag-v1/suite.json)、[回放](../apps/desktop/src-tauri/src/benchmark/rag_evaluation.rs) |

## 固定回放结果

| 场景 | 观察结果 |
| --- | --- |
| 跨文件关系 RAG-09 | 词语查找选用 1/3 预期资料，关系查找选用 3/3。实际保存两跳路径，关系侧前五排序质量为 1 |
| 模糊语义 RAG-02 | 固定向量策略的首位为预期资料，该候选无词法排名 |
| 历史事实 RAG-05 | 指定 2026-02-15 选用 v1，原文支持率为 1。当前范围回放引用因版本不符得 0 |
| 冲突、过期和无答案 | 已登记冲突可查。过期知识从当前上下文排除。无答案题的召回率无分母 |

相同输入重复运行后，除查找耗时外，逐题评测内容一致。评分器报告 Recall@5、MRR@5、nDCG@5、上下文准确率和召回率、相关字节比例、引用准确率和召回率、原文支持率、拒答、冲突检出及检索时间。模型实际回答的评分见 [Phase F 报告](phase-f-delivery.md)。

## 验证与范围

- 分阶段 Rust 检查覆盖数据库隔离与迁移、向量维度失配、切片、版本与来源过滤、旧快照和图谱路径。E2 的原生 WebView2 隔离回环检查 2 项通过。E6 后续在非活动桌面的 WebView2 完成补验，覆盖混合检索、回退、原文查看、回答引用定位和 3D。保存 1366×768、1600×900、1920×1080 的浅深主题截图。命令和证据见 [里程碑](milestone-log.md)。
- 文档导入沿用已确认的 UTF-8 .txt 和 .md 范围。Markdown 围栏内识别部分 Rust、Python、JavaScript 声明。独立代码文件和 PDF 留待扩展。查找记录显示每条证据的实际字节预算，Token 取模型服务对整次请求的报告。
- 固定回放可用于检索与判分版本回归。实机语义质量需用已安装且获准的向量模型在同一题库实测。评分器未使用模型判分。

# CortexOS 文档

当前版本为 [v0.2.0 预发布版](https://github.com/marksimZ/CortexOS/releases/tag/v0.2.0)，状态更新于 2026-10-23。

## 使用与下载

| 文档 | 解决的问题 |
| --- | --- |
| [安装与首次使用](getting-started.md) | 下载、校验、准备本地模型和第一次提问 |
| [使用指南](user-guide.md) | 对话、知识、文件、引用和两种图谱的操作 |
| [常见问题](troubleshooting.md) | 连接失败、资料未命中、显示与数据问题 |
| [路线与功能状态](roadmap.md) | 已完成、待验收与计划中的能力 |
| [版本记录](../CHANGELOG.md) | 已发布版本及重要变化 |
| [安全与隐私](security.md) | 数据保存、联网、文件访问和信任边界 |

## 开发与贡献

当前最新交付：[窗口控制修复与标准批量评测](benchmark-standard-batches.md)，等待本切片验收。此前 [Phase C 适配计划](benchmark-phase-c-plan.md)、[C3–C5 项目导入、出题与专属评测](benchmark-phase-c3-c5.md)。C1/C2 已验收；当前开发版交给用户验证，真实云端推理与 Docker 实测仍待验收，正式 v0.2.0 发布物保持原样。

先读 [贡献与开发](../CONTRIBUTING.md)，再按任务查阅：

| 文档 | 用途 |
| --- | --- |
| [产品范围](PRD.md) | 当前用户任务和功能边界 |
| [架构](architecture.md) | 模块、数据流和代码入口 |
| [数据库契约](database-schema.md) | 数据不变量与迁移；实际结构以 SQL 为准 |
| [开发与交互规范](development-standards.md) | 开发约束和验收要求 |
| [HTML 参考界面规范](design/html-prototype-ui.md) | 当前共享视觉与交互标准，替代早期 V3 |
| [界面用词规范](design/plain-language.md) | 普通话表达与技术准确性 |
| [检查方法](test-plan.md) | 专项检查入口及真实模型检查 |
| [Windows 交付证据](windows-delivery.md) | 安装包校验、验证范围和人工待办 |

## 设计依据与历史记录

Benchmark Studio支持标准及个性化评测、批量停止/恢复、单题追溯、模型比较、JSON/CSV导出及独立重新评分。Core60仅作Level 1 Smoke，保留v1/v2及历史评分；当前[最后一步](benchmark-acceptance-slice8.md)使用本机实际模型验收，Docker真实隔离执行仍是独立环境门槛。历史文档保留当时实现状态，最新交付见实施计划和里程碑：

| 文档 | 内容 |
| --- | --- |
| [评测产品与现状审计](benchmark-prd.md) | 实际功能、代码证据、本机模型和范围 |
| [评测接入架构](benchmark-architecture.md) | Runner、共享执行和现有UI/3D复用 |
| [模型接口与预算](benchmark-provider-design.md) | 能力差异、云端密钥与费用保护 |
| [评测数据库](benchmark-database-schema.md) | 新表、用途隔离、迁移和恢复 |
| [评分与测量方法](benchmark-evaluation-methodology.md) | 60题结构、判分、指标及报告口径 |
| [评测分步计划](benchmark-implementation-plan.md) | Phase 1具体切片、验收和待确认选择 |
| [题目与模型预检](benchmark-preflight.md) | 已实现命令、数据hash、状态及验证边界 |
| [单题开发执行](benchmark-execution-slice3.md) | 共享占用、冻结请求、基础评分及详情/取消接口 |
| [60题审阅表](benchmark-dataset-review.md) | 题目与标准的独立人工复核入口 |
| [Core60逐题审计](benchmark-core60-audit.md) | 全部60题、32条代码断言、评分实现边界及问题清单 |
| [Core60 v2修订与验收](benchmark-core60-v2.md) | 阶段A交付、60题差异、93条断言与双版本兼容；已获用户验收 |
| [v2运行时评分](benchmark-scoring-stage-b.md) | 阶段B六类判定、状态/分母、原生闭环及Docker实测待办 |
| [三级测试集设计](benchmark-tiers-design.md) | Smoke、120题能力集及固定快照认知实验的设计提案 |
| [独立指标与评分方案](benchmark-scoring-v2.md) | 指标与评分设计；当前单题实现及未接入部分见阶段B |

[development.md](../development.md) 是原始产品总纲，包含未来目标，不是当前完成清单。[实施计划](implementation-plan.md) 和 [纠偏计划](refactoring-plan.md) 保留分阶段决策；当前状态以 [路线](roadmap.md) 为入口，实际完成以 [里程碑](milestone-log.md) 和代码为依据。

[ADR 目录](ADR/) 保存架构决策。[差距审计](gap-analysis.md)、[认知架构 V2](cognitive-architecture-v2.md)、[图谱体验 V2](graph-experience-v2.md) 是对应阶段的设计与审计记录；其中“尚未实现”可能已在后续切片完成。旧视觉文档保留参考分析，当前按用户最新 HTML 参考规范实施。

性能报告记录特定环境和版本的测量：[早期基线](graph-baseline.md)、[批量绘制](graph-performance-p1-c2a.md)、[流式隔离](streaming-performance.md)、[按需读取](graph-on-demand-reads.md)、[多中心探索](graph-multi-center.md)、[图谱阅读](graph-reading.md)。报告里的规模上限、绘制回调率和测试替身不能当成产品吞吐或屏幕 FPS 承诺。

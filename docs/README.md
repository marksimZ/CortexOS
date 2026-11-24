# CortexOS 文档

> 2026-11-17 Phase E 与 Phase F 开发交付，见 [知识检索](phase-e-delivery.md)、[统一本地与云端模型](phase-f-delivery.md)和[迁移兼容性](phase-ef-migrations.md)。

> 2026-11-14 [Harness Fit Lab Phase D 补齐交付](harness-fit-phase-d-completion.md)，包含多文件任务、仓库验证、权限记录、真实两模型对照、历史十题补验和报告导出。

> 2026-11-14 [Harness Fit Lab Phase D 首版](harness-fit-d4-d5.md) 已通过用户验收。交付记录包含三工具真实对照、报告与 3D 回放、JSON/CSV 导出及原计划剩余项。

> 2026-11-10当前交付：[Aider工具准备与无模型检查](harness-fit-d3-aider.md)，[分片计划](harness-fit-d3-aider-plan.md)。固定依赖、实际CLI、用途隔离、停止恢复及原生验收通过；工具自身隔离和模型通道仍未验证，停在d.1验收。此前[Claude本地单题](harness-fit-d3-claude-session.md)已验收。

> 2026-11-03当前交付：[D3-3b Claude沙盒方案](harness-fit-d3-claude-sandbox-plan.md)与[实际结果](harness-fit-d3-claude-sandbox.md)。依赖/独立权限已分别获准，固定回复驱动真实CLI的13项反向保护、原生恢复与历史兼容通过；没有模型调用或项目任务。停在3b验收，剩余模型通道/单题、Aider、组合、报告/图谱。

> 2026-11-02当前交付：[D3-3 Claude计划](harness-fit-d3-claude-plan.md)与[实际状态](harness-fit-d3-claude.md)。D3-3a官方工具验签/离线镜像、真实CLI与停止恢复、数据兼容及三尺寸浅深检查已完成，停在验收。自动化帮助、沙盒依赖/默认权限仍有阻断，内层沙盒和模型通道未验证；无本地模型调用，不自动进入下一片。

> 2026-11-01当前交付：[D3-2c受控本地单题](harness-fit-d3-session.md)、[本片计划](harness-fit-d3-session-plan.md)。真实Codex/Ollama三请求完成一个审核函数，独立7/7，旧失败保留。停在验收，下一执行器/组合/报告继续分片。实际模型逐批弹窗确认。

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

2026-10-31当前交付：[D3-2b内层沙盒兼容](harness-fit-d3-sandbox.md)。用户批准后完成固定策略对照与反向探针，接入仅用于Codex的策略选择、版本/hash及原始历史兼容；必要边界、原生停止/恢复/清理和三尺寸浅深截图通过。没有模型或Agent任务，D3-2整体未完成，下一片再接受控模型通道。实际原生结果与窗口见里程碑；下文为历史交付。

2026-10-31当前交付：[D3-2a Codex镜像与隔离检查](harness-fit-d3-codex-runtime.md)，[D3-2实施方案](harness-fit-d3-codex-plan.md)。官方Linux CLI版本/指纹/参数已验证，目录保护因实际环境限制未就绪；检查、取消、强杀恢复、清理和历史已交付，真实编程及模型调用保持未运行。D3-1已获用户继续授权，D3-2整体未完成；后续先审查沙盒兼容，不擅自放宽安全设置。以下条目保留历史交付说明。

2026-10-30当前交付：[D3-1任务副本与独立测试](harness-fit-d3-workspace.md)，[D3后续顺序](harness-fit-d3-plan.md)。已发布Python任务支持临时函数副本、人工修改/范围校验、独立测试和停止/退出清理；未启动Codex或模型，停在本增量验收。[D3-0执行前检查](harness-fit-d3-preflight.md)和[D2b-1真实对照与人工发布](harness-fit-d2b-validation.md)为已交付依赖。后续开发验证实际调用/加载本地模型前须先弹窗确认。

已验收依赖：[草案准备](harness-fit-d2b-drafts.md)、[D2a只读任务](harness-fit-d2.md)、[D1项目适配](harness-fit-d1.md)及[D0就绪审计](harness-execution-readiness.md)。[D2b-0真实隔离与原Python题独立补验](harness-docker-verification.md)保留10个安全场景及20次独立调用记录：10项可评分并通过、10项输出截断待复核，原历史记录保留。此前[Benchmark完整计划验收](benchmark-acceptance-slice8.md)及[C3–C5项目导入、出题与专属评测](benchmark-phase-c3-c5.md)保留实际证据。正式v0.2.0发布物保持原样。

先读 [贡献与开发](../CONTRIBUTING.md)，再按任务查阅：

| 文档 | 用途 |
| --- | --- |
| [产品范围](PRD.md) | 当前用户任务和功能边界 |
| [架构](architecture.md) | 模块、数据流和代码入口 |
| [数据库契约](database-schema.md) | 数据不变量与迁移；实际结构以 SQL 为准 |
| [开发与交互规范](development-standards.md) | 开发约束和验收要求 |
| [Phase E／F 审计](rag-runtime-audit.md)与[实施计划](rag-runtime-plan.md) | 检索升级、证据追溯及统一本地与云端模型 |
| [界面规范](../design.md) | 当前设计语言（Apple × Linear 极简）、组件规格与硬性规则 |
| [极简视觉规范](design/ui-design-system.md) | 已并入根目录「当前设计语言」一节，此处仅为索引 |
| [界面用词规范](design/plain-language.md) | 普通话表达与技术准确性 |
| [检查方法](test-plan.md) | 专项检查入口及真实模型检查 |
| [Windows 交付证据](windows-delivery.md) | 安装包校验、验证范围和人工待办 |

## 设计依据与历史记录

Benchmark Studio支持标准及个性化评测、批量停止/恢复、单题追溯、模型比较、JSON/CSV导出及独立重新评分。Core60仅作Level 1 Smoke，保留v1/v2及历史评分；[本机完整计划验收](benchmark-acceptance-slice8.md)和后续Docker补验分别保留证据。历史文档保留当时实现状态，最新交付见实施计划和里程碑：

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

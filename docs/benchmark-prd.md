# Benchmark Studio 产品范围与现状审计

2026-10-23 · Phase 0已获确认。以下现状审计保留基线`f146359`、应用版本0.2.0；后续已完成Phase 1切片1的数据隔离和迁移，尚无评测产品入口。当前逐片状态以[实施计划](benchmark-implementation-plan.md)为准，设计目标不能作为全部已实现清单。

## 产品目标

在当前工作区增加“评测”入口，用固定题目、明确配置、原始回答和可核查评分，比较模型能力、速度、资源及后续知识增强收益。结果的适用范围限定为所选题目、模型版本、设备和测试条件；输出分别展示各项指标和覆盖率。

Phase 0当时只审计和设计。后续逐阶段交付，Phase 1首先实现本地串行评测，云端接入、真实知识增强对照及3D差异按后续门槛推进；当前只完成其中的数据基础切片。

## 实际起点与代码证据

根目录未找到`design.md`。现有有效设计标准是[HTML参考界面规范](design/html-prototype-ui.md)和`apps/desktop/src/styles/`的Tokens；沿用它们。原始总纲是[development.md](../development.md)，当前状态以代码和[里程碑](milestone-log.md)为准。

下列路径相对`apps/desktop/`，省略目录的Rust文件均在`src-tauri/src/`，行号为本次审计基线。已实现表示存在完整代码路径；历史专项结果与本轮重新执行的检查分别记录。

| 提示词中的能力 | 核实状态 | 代码证据与限制 |
| --- | --- | --- |
| 本地模型与对话 | 已实现连接检查、已安装模型选择、流式回答、停止、重试和持久化 | `src-tauri/src/ollama.rs:187`列模型、`:219`检查GGUF/本地来源、`:246`调用；`commands.rs:190`开始、`:168`取消；`src/Settings.tsx`选择模型。应用内没有权重下载、显式卸载和加载状态管理 |
| 云端模型 | 尚未实现 | `ollama.rs:119/156`固定loopback、禁用代理/重定向；`:219`拒绝云端别名；`commands.rs:19`仅持有Ollama，没有云端Provider或密钥存储 |
| 上下文与“记忆” | 已有会话历史、已确认知识和容量选择；没有独立长期记忆、自动摘要或压缩引擎 | `db.rs:390`的`prepare_context_run`、`src/studio.ts:97`的清单类型。预算使用保守文本单位，不能当真实Token计数 |
| 知识版本与审核 | 已实现 | `knowledge.rs:95/144/267`读取、修订、审核；迁移`0002_knowledge.sql`保护不可变版本，生效指针独立于最新草稿 |
| 知识检索 | 已实现词法检索，尚无向量检索或重排器 | `retrieval.rs:6/103`为FTS5、中日韩字符对及BM25；`:38`仅索引可用生效版本；文件全文不直接进入检索 |
| 文件与证据 | 已实现用户选择文件、版本副本、行范围及停用 | `sources.rs:70/270/350/355`，迁移`0007_source_evidence.sql`；原文件变化不会自动重导入 |
| 引用验证 | 已实现编号及行范围定位，不判事实或支持关系 | `citations.rs:4`和`src/studio.ts:71`明确`reference_locations_only`；Benchmark须另加有标准依据的评分器 |
| 执行记录 | 已实现真实事件、请求快照、输出和终态 | `db.rs:589/706/722/727/794`；迁移0001/0004/0006。现有`ExecutionStep`是事件投影，没有独立步骤表；每次运行必须关联会话和消息 |
| 恢复与并发 | 已有聊天中断标记和部分输出保存；没有评测队列续跑 | `db.rs:844`重启恢复，`commands.rs:19/190`单活动生成，`0001_initial.sql:20`限制全库一个running执行；不能另起一个互不协调的Runner |
| 3D图谱 | 已有数据库知识图及真实执行图，演示图另有明确入口 | `src/Graph.tsx`、`graph_query.rs:186`分页、`execution.rs:34`及`src/execution-data.ts:47`执行投影；没有两次评测差异视图 |
| 导航、UI和状态 | 已有共享外壳、浅深主题、Markdown、固定面板操作和跨页定位 | `src/components/AppShell.tsx`、`SelectMenu.tsx`、`src/navigation.ts:2/29`；当前四页，返回定位只接受聊天/图谱。状态为React hooks/useStudio，没有Zustand或通用路由库 |
| Benchmark/数据集/评分/费用 | 尚未实现 | `src-tauri/src/lib.rs`、`build.rs`命令清单，`migrations/0001–0007`及前端页面均没有评测实体/入口；现有性能检查脚本不是产品化模型评测 |

实际技术栈：React19、TypeScript、Vite、Tauri2、Rust、rusqlite、reqwest、SQLite FTS5、Three.js/R3F/Drei，版本见[package.json](../apps/desktop/package.json)和[Cargo.toml](../apps/desktop/src-tauri/Cargo.toml)。复用现有依赖，首版图表优先语义表格和少量SVG。

## 本机只读核实

2026-10-23通过`/api/version`、`/api/tags`、`/api/ps`和`POST /api/show`读取元数据，没有生成、卸载、下载或云端调用。完整本地证据为`artifacts/benchmark-phase0/environment.json`。

| 项目 | 本次观察 |
| --- | --- |
| Ollama | 服务版本0.40.6；本次shell PATH未找到ollama命令，不代表未安装服务 |
| 已安装模型 | `qwen3:4b-instruct`、`qwen3:4b`；均为GGUF、4.0B、Q4_K_M，且本次ps无已加载模型 |
| 模型身份 | instruct digest=`0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`；4b digest=`359d7dd4bcdab3d86b87d73ac27966f4dbb9f5efdfcc75d34a8764a09474fae7` |
| 能力 | 两者均自报completion/tools/thinking；未做生成探针，不等于结构化输出或工具调用已实测通过。自报上下文262144不是本设备可运行容量，首轮仍从4096开始 |
| GPU | RTX4070 Laptop，nvidia-smi报告总显存8188MiB，当前已用711MiB，驱动610.74；这是单次整卡观察，不是评测峰值或推理专属占用 |
| 编程隔离环境 | PATH未找到docker命令；尚未验证可用容器运行时，也未安装任何软件 |

Gemma3 4B、Phi-4 Mini和Llama3.2 3B作为首轮候选，确切Ollama标签、量化、许可和能力待用户选择安装后核实。其余候选同样按实际元数据登记，名称或参数量不构成能力保证。当前没有四种模型的真实对比结果。

## Phase 1用户流程与范围

1. 进入“评测”，从已安装模型列表多选；配置保存在评测模板，不覆盖聊天设置。
2. 选择固定版本60题，查看类别、评分方式和运行所需能力；不满足条件的题目明确显示原因。
3. 选择重复次数、上下文/输出上限、冷/热测试条件。启动前展示调用数量和资源释放范围。
4. 串行执行，同一时刻只有一个模型请求；切页不停止任务，生成中的聊天和评测互斥。
5. 查看真实进度、单题回答、判分依据和可用指标；取消保留结果，重启后手动继续未完成项。
6. 用表格比较同条件结果，按题目打开详情；导出本地JSON完整记录及CSV指标。

60题分布：中文知识15、数学逻辑15、编程10、固定资料引用10、结构化/受控工具10。固定资料引用是数据集能力基线；生产知识检索、版本切换及增强A/B在Phase 3接入。编程题执行必须有隔离环境；没有环境时保留题目并标记“未运行：缺少隔离环境”，不能宣称60题全部验收。

首版60题均为文字任务，不代表视觉或音频能力评测。Gemma/Ministral等候选的多模态能力先按实际元数据登记，后续有媒体测试集和评分协议再开放。

Phase 1包含最小中断恢复及JSON/CSV导出，落实总提示词第一阶段验收第8/9项；Phase 2扩展付费请求恢复，Phase 4增加完整实验报告，避免将基本能力全部推迟。

## 界面组织

只增加一个一级入口“评测”。内部为“实验 / 测试集 / 模型”三个任务入口；配置、监控、结果、历史是实验的不同状态，单题在右侧抽屉查看。3D差异到Phase 4再增加，精确数值始终使用2D表格。

沿用252px全局侧栏、Soft Glass及蓝紫/薄荷Tokens，不新增仪表盘卡片墙。列表显示名称、配置、时间和状态；监控页主区为进度和结果行，次要解释进入按需展开区。详情抽屉固定标题、关闭及必要操作，正文独立滚动。主界面保持清晰的大白话，技术字段在详情准确展示。

## 用户已确认的产品选择

| 选择 | 推荐方案 | 对后续实现的影响 |
| --- | --- | --- |
| 编程题执行 | 用户明确授权并配置Docker Linux隔离环境，首版只支持Python纯函数题 | 无环境可交付其余类别，但10道编程题不能通过完整门槛；普通Python子进程不是安全替代品 |
| 首轮模型 | 从当前Qwen基线开始，新增三种候选的安装/空间另行确认 | Phase 0没有下载；跨四家模型完整对比需要新增权重 |
| 云端首接入 | Phase 2先接OpenAI及OpenAI兼容接口，再接Anthropic/Gemini | 使用用户自有Key，私有资料发送另行明确授权；本轮不产生费用 |
| 执行记录接入 | 复用现有记录，新增评测用途会话，隔离日常列表 | 用途隔离和0008在切片1实现；共享执行服务在切片3提取，详见架构/数据库文档 |

上述方案已获用户确认。本轮只实施切片1；系统安装、额外权重下载和云端调用不随设计确认自动执行。

配套：[架构](benchmark-architecture.md) · [Provider](benchmark-provider-design.md) · [数据库](benchmark-database-schema.md) · [评测方法](benchmark-evaluation-methodology.md) · [分步计划](benchmark-implementation-plan.md)。

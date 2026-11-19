# Phase G0 全项目架构审计

审计日期为 2026-11-18。本轮止于审计与实施规划，基于当前含 Phase E/F 变更的实际工作区。HEAD 为 8e82c42，工作区尚有 E/F 增量；本报告描述工作区代码。原始观测保存在 [G0 产物目录](../../artifacts/phase-g0-2026-11-18/)。

## 当前架构与测量结论

项目已经具备模块化单体的基础。React 通过受限 Tauri 命令进入 Rust，Rust 统一持有 SQLite、Ollama Client、任务槽和云端互斥锁。聊天、标准评测、个性化评测及 Harness 保留领域入口，共用执行记录和底层能力。主要收益来自减少不必要读取、限制缓存和开发产物生命周期，以及把模块边界落实到加载与查询。

| 范围 | 已核对的实现 | 依据及判断 |
| --- | --- | --- |
| 数据访问 | AppState 持有一个 Arc<Database>，Database 内为 Mutex<Connection>；阻塞读写通常进入 spawn_blocking | [commands.rs](../../apps/desktop/src-tauri/src/commands.rs)、[db.rs](../../apps/desktop/src-tauri/src/db.rs)。没有发现每个页面另建连接池的实现 |
| 任务互斥 | TaskSlot::claim 返回 RAII Lease，聊天、Benchmark、索引、维护共用一个活动槽；释放 Lease 清理状态 | [tasks.rs](../../apps/desktop/src-tauri/src/tasks.rs)。现有串行性对资源保护有效，后续调度应以其为起点 |
| 模型能力 | EmbeddingProvider、RerankerProvider、GenerationProvider 已独立；运行方案引用各环节模型 | [model_runtime.rs](../../apps/desktop/src-tauri/src/model_runtime.rs)、[runtime_profiles.rs](../../apps/desktop/src-tauri/src/runtime_profiles.rs) |
| 云端协议 | 旧 ProviderAdapter 与新 Endpoint 入口并存，Endpoint 已复用 completion 的请求和 Token 解析 | [cloud/provider.rs](../../apps/desktop/src-tauri/src/cloud/provider.rs)、[cloud/runtime_protocol.rs](../../apps/desktop/src-tauri/src/cloud/runtime_protocol.rs)。保留领域凭据和用途边界，优先检查 HTTP Client 生命周期 |
| 执行及审计 | 聊天和评测复用执行与快照；Harness 使用带 purpose、计划 hash 和追加事件的独立合同 | [generation.rs](../../apps/desktop/src-tauri/src/generation.rs)、[benchmark/harness/session/records.rs](../../apps/desktop/src-tauri/src/benchmark/harness/session/records.rs)。这些合同承担不同校验责任 |
| 图谱 | 知识关系和执行事件分别投影；GraphViewport 已懒加载、实例批绘制、按需渲染 | [Graph.tsx](../../apps/desktop/src/Graph.tsx)、[ExecutionTrace.tsx](../../apps/desktop/src/ExecutionTrace.tsx)、[GraphViewport.tsx](../../apps/desktop/src/GraphViewport.tsx) |
| 存储生命周期 | 已有来源停用、快照、Harness 清理与引用校验，各类开发缓存分散 | [sources.rs](../../apps/desktop/src-tauri/src/sources.rs)、[benchmark/python](../../apps/desktop/src-tauri/src/benchmark/python/)。目前没有统一的统计、预览与确认回收入口 |

## 按优先级排序的问题

| 编号 | 现象与证据 | 影响、预计收益及风险 |
| --- | --- | --- |
| A1 | ContextPicker 挂载时 list_knowledge 后对每条生效知识执行 get_knowledge，再从完整版本中找生效版。折叠 details 不阻止 effect。[ContextPicker.tsx](../../apps/desktop/src/ContextPicker.tsx:16) | 1000 条合成知识产生 1000 个详情请求。改为按需读取及服务端提供当前有效摘要，可消除这一批逐条往返。需保持生效日、来源停用、草稿和版本变化保护 |
| A2 | readGraphScope 每次加载新 200 条时从 offset=0 重读已加载前缀，并重新取关系。[graph-query.ts](../../apps/desktop/src/graph-query.ts:71) | 1000 节点的正常加载过程共 15 次节点分页及 5 次关系请求；同一快照可期望降到 5 次节点分页。需要快照失效、邻域合并和取消验证 |
| A3 | App 静态导入 Chat、Knowledge、Graph、Settings、Benchmark，Benchmark 又静态导入所有评测工作台。[App.tsx](../../apps/desktop/src/App.tsx:5)、[BenchmarkPage.tsx](../../apps/desktop/src/benchmark/BenchmarkPage.tsx:1) | 首页入口 JS 为 899089 字节。路由按需加载可推迟非当前页面的解析。页面和共享编辑器同文件导出，简单加 lazy 可能仍被其他静态引用拉入，需要先明确共享边界 |
| A4 | ChatPage 全量映射 messages，ConversationList 全量映射 conversations。ChatMessage 接收整个 studio 对象且本身未 memo。[Chat.tsx](../../apps/desktop/src/Chat.tsx:249)、[ConversationList.tsx](../../apps/desktop/src/components/ConversationList.tsx:44) | 合成 1000 消息实际挂载 1000 个 article、16787 个 DOM 节点。输入和流式状态可能传播到旧消息。先采提交次数，再做稳定 props 和长列表窗口化；必须保留引用定位、复制、Markdown 和滚动行为 |
| A5 | 云端 catalog 与 post_json 每次构造新的 reqwest Client；Ollama 已长期复用 Client。[cloud/provider.rs](../../apps/desktop/src-tauri/src/cloud/provider.rs:51)、[cloud/completion.rs](../../apps/desktop/src-tauri/src/cloud/completion.rs:256) | 跨请求连接池无法持续复用。云端 TLS/网络收益尚未测量。G1 用回环及独立连接计数验证后评估共享传输，保留地址约束、敏感 header、超时和凭据绑定 |
| A6 | 本地索引构建每次读取全部可索引文档、逐批请求向量，提交时删掉当前 profile 的向量再写入。[rag.rs](../../apps/desktop/src-tauri/src/rag.rs:549) | 未改变的文档重复生成向量。按模型指纹和内容 hash 复用，可使“内容不变重建”的推理请求目标降为 0。需要删除、来源停用、模型变化及失败回滚验证 |
| A7 | SQLite 单连接互斥，向量检索在锁内扫描向量，解码及重新 hash 内容；报告/快照解析也有锁内工作。[rag.rs](../../apps/desktop/src-tauri/src/rag.rs:365)、[db.rs](../../apps/desktop/src-tauri/src/db.rs:343) | 大数据下读写会排队。本轮小库 SQL 低于毫秒级，尚无依据直接引入连接池或向量服务器。先测锁等待和 2000 条真实维度，再决定缩小锁区 |
| A8 | activeRun 每秒同时 get_run 和刷新整段消息；详情面板另每秒读取运行，即使已终止。F7 每 1.8 秒取完整报告。[studio.ts](../../apps/desktop/src/studio.ts:451)、[CognitiveInspector.tsx](../../apps/desktop/src/components/CognitiveInspector.tsx:49)、[RagRuntimeEvaluation.tsx](../../apps/desktop/src/benchmark/RagRuntimeEvaluation.tsx:92) | 候选、上下文和历史事件可能重复跨 IPC。先测重叠请求和传输体积，再停止终态轮询及使用增量版本。取消和恢复仍需能获取最终状态 |

## 已有合理约束

- 主任务已有统一互斥，未发现必须合并多套并行模型队列的问题。Benchmark 批处理和 Harness 会话控制器包含用途及容器所有权校验。G3 可以复用状态转换和清理合同，合并时保留各领域适配器。
- 标准与个性化报告已有概要、分页和单题详情接口，见 [report_read.rs](../../apps/desktop/src-tauri/src/benchmark/report_read.rs)。F7 与 Harness 的完整事件读取应参照这一已有模式，优先复用现有行为。
- 当前图谱坐标由确定性公式生成，布局没有 force 模拟或后台 Worker。图谱 1000 节点、5000 关系的合成检查中只有 4 次绘制提交，15 轮空闲观察都没有新增绘制。Worker 是否必要需依据后续布局测量。
- GraphCamera 的指针、滚轮、键盘，GraphViewport 的上下文丢失监听及标签层观察器都有相应清理。部分 Harness 草案故意保留模块级 Map 和 beforeunload 提示来保留未保存编辑。这些状态应在 G2 明确保留期和清除动作，目前证据不足以判定泄漏。

## 写入量、恢复与锁竞争

[generation.rs](../../apps/desktop/src-tauri/src/generation.rs:232) 每 250ms 检查脏输出并做部分回答 checkpoint。SQLite 使用 WAL 与 FULL 同步，这是中断保留能力的一部分。只有输出变化才写入，并非无条件每秒写四次。本轮没有采集磁盘写字节/秒或每次 fsync 延迟，不能声称它已成为瓶颈。未来调整必须同时保留原有停止与恢复语义。

[本轮只读写入观测](../../artifacts/phase-g0-2026-11-18/execution-write-observation.json) 显示，真实批次库共有 108 个执行事件，payload 为 44817 字节；6 次聊天相对同一初始 fixture 的 42 个事件新增 66 个事件。请求快照共 29541 字节，两个 F7 报告为 60209 字节。数据库主文件 888832、WAL 1841672 字节，均为采样时文件大小，不是累计写入量或峰值 WAL。小批结果尚未显示日志量失控；完整快照在轮询和大报告中的重复传输更值得先测。

[db.rs](../../apps/desktop/src-tauri/src/db.rs:1331) 启动时在统一恢复入口处理 started/running 记录，涉及云端、向量、Benchmark 及各 Harness 用途。它保证不静默重发，但随历史任务量增加应测 WHERE state/status 与 JSON purpose 条件的扫描量。当前约 0.7 秒的小库启动不证明大型历史库也相同。G1 在副本采查询计划和锁等待，G3 才决定是否收敛恢复合同。

## G3 的收敛边界

| 候选 | 可复用基础 | 规划 |
| --- | --- | --- |
| Provider Registry | 当前能力合同、连接目录、运行方案 | 先统一能力查询与传输配置，保留不同凭据和协议约束 |
| Resource Scheduler | TaskSlot、Lease、模型状态及云端预算 | 在现有任务合同上增加资源描述、可观测排队和取消；图谱作为 UI 预算参与协调 |
| Job Lifecycle | Benchmark 状态及 Harness 用途事件 | 先列出状态和恢复矩阵，再抽真正一致的终止及清理操作 |
| Execution Trace | execution_events、Harness hash 事件、3D 投影 | 统一只读摘要和导航，保存原始事件合同及历史解释 |
| Data Access | Database、blocking、工作区条件 | 优先稳定查询合同与分页；读池作为实测后的选择 |
| Cache / Permission | 现有 profile hash、授权知识、来源状态、Docker 所有权 | 复用引用检查与预览。权限校验继续在实际访问入口执行 |

G1–G4 的具体顺序、收益假设及验收线见 [优化路线](optimization-roadmap.md)。本轮没有合并业务模块，也没有清理用户数据或可重建索引。

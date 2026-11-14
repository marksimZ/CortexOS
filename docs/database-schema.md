# 数据模型契约

> 2026-11-03 D3-3b：schema仍11，无迁移。harness_claude_runtime的新计划claude-runtime-check-2绑定claude-program-2、检查源码hash、任务/镜像及独立Claude策略描述；v1按原定义读取。复用原表/事件/恢复，临时策略文件按Codex/Claude作用域和指纹分别清理。11条旧检查/会话及事件逐列相等，新旧6条Claude记录hash均正确、普通数据不变。固定回复的Token/费用不是模型用量或成绩；canRun=false。后续若修改探针实现，新增版本并保留旧定义，不静默重写历史。见[沙盒交付](harness-fit-d3-claude-sandbox.md)。

> 2026-11-02 D3-3a交付：schema仍11，无新迁移。harness_claude_runtime复用generation_jobs/generated_cases，claude-runtime-check-1绑定工作区/分析/已发布函数版本/hash及固定程序/镜像定义；与Codex共用内部流程但按两个固定用途严格读取、写入、恢复和清理。真实完成、取消、强杀恢复/清理三条记录及hash已核验，8条旧Codex检查/会话及事件逐列不变，迁移checksum和普通数据计数保持。CLI观察不写作模型成绩，Claude不借用Codex准入或专用seccomp。见[本轮状态](harness-fit-d3-claude.md)。

> 2026-11-01 D3-2c：schema仍11，无迁移。新增用途harness_codex_session，复用generation_jobs/generated_cases；计划绑定工作区、任务/模型/隔离检查/hash、两个固定镜像、预算及副本，事件追加hash。运行/清理/历史/恢复按用途及工作区限制；running/grading重开只变interrupted，不自动推理。新版附适配器源码hash，旧无描述记录保留；第一批失败与新一批通过分开，不重算旧评分。不写普通聊天、知识、原项目或普通BenchmarkRun。见[受控单题](harness-fit-d3-session.md)。

> 2026-10-31 D3-2b：schema仍11，harness_codex_runtime新计划codex-runtime-check-2绑定固定保护策略ID/hash/来源及codex-sandbox-2；v1按原样读取，不补写或重算。观察追加实际容器保护配置与十项反向探针结果；策略临时文件清理失败保持cleanup_required。没有模型/聊天/知识或普通成绩写入。见[兼容说明](harness-fit-d3-sandbox.md)。

> 2026-10-31 D3-2a：schema仍11。harness_codex_runtime复用generation_jobs/generated_cases；创建前固定检查/任务/镜像/端点及容器归属，探针/错误/清理只追加带hash的观察。状态checking→completed/failed/cancelled；清理失败保留cleanup_required，重开checking→interrupted、不自动执行。所有读写/清理限制用途与工作区，任务读取再核对分析与检查归属。流程完成与沙盒通过分开，modelCalls=0、taskExecution=Not Run，不写聊天/知识/普通成绩。见[Codex环境检查](harness-fit-d3-codex-runtime.md)。

> 2026-10-30 D3-1：schema仍11。harness_workspace用途复用generation_jobs/generated_cases，计划绑定检查/任务/来源/hash、固定镜像及允许包；环境、容器归属、提交、候选、评分和清理只追加。提交内容与ready→grading在同一事务，所有容器清理后才能标终态。一个数据库至多一个未关闭副本；恢复仅将opening/ready/grading标interrupted，不恢复执行。候选按原范围生成完整文件提案，不写原文件/原快照，评分为人工交接，不写普通BenchmarkRun或Model×Harness成绩。见[副本与交接](harness-fit-d3-workspace.md)。

> 2026-10-30 D3-0：schema仍11。执行前检查以purpose=harness_execution_preflight复用benchmark_generation_jobs，仅追加完成的检查计划/hash，不创建BenchmarkRun、模型请求或成绩。绑定工作区/分析版本、已发布任务ID/version/hash、固定项目来源与验证镜像，保存实际CLI/Docker观察和分别列出的阻断项；canRun=false、taskExecution=Not Run。读取同时限制工作区/分析版本/用途，最多最近30次检查、100个任务版本。旧检查保持当时快照，不随环境变化静默重算；独立运行器和任务执行授权尚未建立。见[D3计划](harness-fit-d3-plan.md)。

> 2026-10-30 D2b-1：schema仍11。验证以harness_programming_validation用途复用generation_jobs/cases，计划绑定草案revision/hash、代码hash、Image ID及评分器；环境、容器归属、评分和清理均追加记录。恢复仅将本用途validating标为interrupted，不重跑。人工发布以harness_programming_tasks复用datasets/reviews，单任务多版本、payload hash及审核/验证快照不可变，不写普通benchmark_cases或聊天/知识。见[对照与发布](harness-fit-d2b-validation.md)。

> 2026-10-30 D2b草案准备：schema仍11。复用benchmark_generation_jobs及benchmark_generated_cases，独立用途`harness_programming_drafts`；计划绑定分析版本/hash及项目来源范围，每次保存追加修订和内容hash。原D2a、普通个性化、标准题库及Runner均不读取此用途；没有批准、发布、评分或执行记录写入。安装与Docker验证被用户暂存，运行状态始终Not Run。见[草案交付](harness-fit-d2b-drafts.md)。

> 2026-10-29 D2a：schema仍11，无新迁移。任务计划/修订/审核/发布分别复用benchmark_generation_jobs、benchmark_generated_cases、benchmark_dataset_reviews、benchmark_datasets；JSON purpose=harness_tasks，普通个性化缺省用途兼容旧记录。任务manifest保留完整任务及hash，不加入普通benchmark_cases或Runner。详见[D2a存储与隔离](harness-fit-d2.md)。以下为历史契约。

> 2026-10-29 D1开发基线：0001–0011。0011仅新增harness_fit_assessments追加版本表，正文绑定来源、需求修改、权重、目录快照及Document Fit结果；导入文档复用benchmark_repository_snapshots。历史迁移字节不变，升级前仍备份/事务执行。真实Harness执行表与D2–D5未创建；详见[交付与兼容性](harness-fit-d1.md)。以下保留历史基线。

> 当前开发基线（2026-10-23）：已实现0001–0008迁移，新增Benchmark用途隔离及存储基础，详见[评测数据库](benchmark-database-schema.md)。已发布v0.2.0安装包仍为schema7；本轮没有升级正式用户库或更新安装包。后文保留各阶段历史契约，实际结构以[迁移SQL](../apps/desktop/src-tauri/migrations/)为准。

本文按阶段记录数据库契约，采用 ADR 0002/0003 的语义；后文 0004–0007 记录已实现增量，所有实际结构以迁移 SQL 为准。

## 共用约束

稳定 UUID；统一 UTC 时间；外键每连接开启。具有工作区范围的表提供 `(workspace_id, id)` 唯一约束，需要跨表引用时使用复合外键或同事务显式校验，防止合法 ID 被跨工作区使用。删除先使用归档/软删除状态，永久删除另行定义依赖清理。

## Phase 1 最小表

| 表 | 核心字段及约束 |
| --- | --- |
| schema_migrations | version PK、checksum、applied_at |
| workspaces | id PK、name、created_at、updated_at |
| conversations | id PK、workspace_id FK、title、created_at、updated_at |
| messages | id PK、workspace_id、conversation_id、seq、role CHECK、content、status CHECK、model_run_id NOT NULL、created_at、updated_at；UNIQUE(conversation_id,seq) |
| execution_runs | id PK、workspace_id、conversation_id、assistant_message_id、model_id、model_config_json、context_manifest_json、status CHECK、error_message、started_at、finished_at |
| execution_events | id PK、run_id FK、seq、kind、occurred_at、payload_json；UNIQUE(run_id, seq) |
| app_settings | key PK、value_json；保存已校验模型参数 |

`execution_runs.status` 是当前状态投影。`execution_events` 经应用接口只允许插入，并用数据库触发器拒绝常规更新/删除；其不可变保证面向应用使用，不声称能阻止拥有数据库文件的管理员修改。整工作区永久清理需要独立授权的维护路径与备份策略，在实现删除功能前定义。

运行指向回答消息的复合外键为 `DEFERRABLE INITIALLY DEFERRED`；消息指向运行使用同工作区、同会话复合外键。在同一事务内依次创建运行、用户消息、回答占位及事件，提交时校验完整关系。唯一部分索引限制全库只有一个 running。结束更新带 `WHERE status='running'`，迟到终态与检查点不会覆盖已结束内容。每次重试创建新 run。

当前数据库文件为应用数据目录下的 `cortexos.sqlite3`。连接启用外键、WAL、FULL 同步及 3 秒 busy timeout；启动检查 SQLite FTS5 编译能力。迁移版本与 SHA-256 校验不匹配时显示错误并保留数据，不自动重建。Windows 用独占 `cortexos.lock` 句柄保护打开和恢复，锁随进程退出释放。强制中断可能损失最后 250 ms 以来尚未提交的片段，不能保证未落盘字节恢复。

模型完成帧的 `done_reason` 与实际 token/耗时统计保存在 `response_completed` 事件 payload。接口中的消息 `finishReason` 从该事件投影读取；`length` 在界面显示达到输出上限，该轮不用于后续上下文。运行 completed 表示响应流结束，不自动宣称回答内容完整或正确。

## Phase 2 知识修订

| 表 | 规则 |
| --- | --- |
| knowledge_nodes | id、workspace_id、kind、latest_version、effective_version nullable、revision；归档后续实现 |
| knowledge_versions | node_id + version 复合 PK，标题/正文/作者/修改原因/时间不可变 |
| knowledge_approval_events | node_id、version、actor、created_at；2A 仅记录确认，拒绝与撤销后续实现 |
| knowledge_edges | 稳定关系 ID、workspace_id、当前修订指针 |
| knowledge_edge_versions | edge_id + version PK、端点、relation、状态与修改原因 |
| graph_layouts | id、workspace_id、graph_kind、name、camera_json |
| knowledge_positions | layout_id + node_id PK、x/y/z |
| execution_positions | layout_id + event_id PK、x/y/z，与知识位置分表 |

节点状态按生效版本与审批记录计算，前端映射为待确认/已确认/有争议等明确文字。作者不能直接修改旧内容；恢复旧内容同样产生新版本。确认操作带 `expected_revision`，冲突时保留用户草稿并提示重新比较。

## Phase 4 来源与检索

授权来源、来源内容版本、分段、FTS5、证据关联及上下文快照在对应阶段建表。证据关联必须标识 node_id + node_version、source_version_id 及真实段落/行定位。哈希用于识别内容，不能替代可重放内容。

撤销授权阻止未来读取和检索；是否保留已导入内容、历史快照和引用由明确清理选项控制。备份/导出保持引用一致，恢复后执行外键和完整性检查。


### Phase 2A 实施契约

迁移 0002 创建 knowledge_nodes、knowledge_versions、knowledge_approval_events。节点保存工作区、latest_version、effective_version（可空）、revision、更新时间，以及可空的原回答 conversation/message 引用、原文摘录和后端计算的消息 SHA-256。来源是创建时不可变的起源记录，不代表修改后内容已经被原文证实。创建时验证原回答属于同一工作区、角色 assistant、已结束且摘录是原文的连续子串。手动创建没有回答来源。

版本包含标题、类型（context/knowledge/hypothesis/decision/evidence/conflict）、正文、作者 user、修改原因和时间；每次保存新增版本，旧版本用触发器禁止更新/删除。确认事件记录版本、user、时间，不改变版本内容。确认仅允许最新版本，事务内检查 expected_revision 并更新 effective_version/revision。新草案保持原 effective_version。来源引用、节点指针使用外键（版本指针延迟检查），拒绝跨工作区来源。

迁移器检查全部已应用版本的顺序和 checksum，旧库升级前用 VACUUM INTO 生成同目录唯一的迁移前备份；备份失败则停止升级。迁移在事务中完成。2A 不实现删除、撤销确认、关系或自动知识检索；确认代表用户审核，不代表事实验证。


### Phase 2B 实施契约

0003 迁移新增 knowledge_edges（稳定 id/workspace_id/latest_version）与 knowledge_edge_versions（edge_id/version/workspace_id/from_node_id/to_node_id/relation/active/change_reason/created_at）。关系版本不可修改/删除；端点以复合外键限制同工作区。关系保存需预期版本、同工作区两端点、有效关系类型、非自环和非重复有效关系。停用追加 active=false 修订，历史保留。方向按“起点 relation 终点”阅读；关系关联稳定节点，节点内容更新不自动构成关系重新审核。

graph_layouts 每个工作区一份 knowledge 视图，含 revision、camera/target 与更新时间；knowledge_positions 按 workspace_id/node_id 保存有限坐标。保存需预期视图 revision，防止两个页面覆盖；位置和视角保存不改变知识/关系版本。演示图仅在前端内存中，显式标记，不能保存为工作区内容。迁移沿用升级前一致性备份。首批仅验收小图，不预先实现动态力导向布局。


## P0-A：0004_context_audit（2026-10-18）

新增 context_snapshots：run_id 为主键并外键引用 execution_runs，保存 request_json、request_hash、manifest_json、created_at。request_json 是 provider 实际发送的同一 serde_json::Value 序列化结果，含 system/历史/知识/当前问题及 model、options、stream、think、keep_alive；timeout 仍在 run 的 ModelSettings。SHA-256 基于 serde_json 当前默认有序对象序列化的 UTF-8 字节，后续变更序列化规则需增加格式版本。

context_knowledge_items 保存 run、ordinal、node_id、version，约束唯一节点及有效知识版本；额外触发器检查 run 与节点同工作区。两表均以触发器禁止更新和删除。知识正文和哈希在 manifest 的 knowledgeRefs 中，完整资料消息也在请求快照中。保存运行、输入输出消息、快照、引用、初始事件为一个事务；任一写入失败整笔回滚，模型不会开始请求。读取快照沿用 get_run 的工作区校验。

manifest schemaVersion=2，保留原 messageRefs/systemHash/systemVersion/budget 字段，增加 knowledgeRefs（标题、类型、版本、正文、哈希及选择理由）和 excludedMessages（历史预算/轮次或不完整轮次原因）。新请求的知识只解析 effective_version，拒绝重复/超过20项/非当前工作区/不支持类型/版本冲突。旧运行没有快照合法，get_run 返回 snapshot:null；迁移不回填。原三份迁移内容保持不变，升级前自动一致性备份。


## 0005_execution_views（P0-B）

`execution_runs(workspace_id,id)` 增加唯一索引；`execution_layouts` 以 run_id 为主键，保存 workspace_id、revision、camera_json、target_json、updated_at，复合外键引用同一工作区运行。写入检查有限坐标、相机距离与 expectedRevision，事务内比较后递增版本。知识布局及执行事件保持独立。

执行节点是事件、快照与消息的读取投影，坐标按事件顺序生成。当前无需 execution_positions 或另一份 ExecutionStep 表；工作流真正产生依赖时再扩充。迁移文件保持 LF，已有迁移内容和校验和保持不变。

## 0006_knowledge_retrieval（P1-A）

knowledge_fts为FTS5派生索引，node_id/workspace_id/version为未索引定位列，title_terms/body_terms为预处理词。迁移事务调用Rust回填已生效版本，approve_knowledge事务同步更新。保持原迁移字节，0006使用LF。

conversation_retrieval_settings保存conversation_id主键、workspace_id、enabled布尔，复合外键约束会话归属；缺省true。retrieval_results以run_id/ordinal为主键，保存node_id/version、score、included、reason、cost_units；版本外键、工作区触发器和不可变保护。候选完整快照与查询算法/预算信息保存在manifest schema3及事件payload中。无命中不插候选行，manifest和事件仍区分无命中与关闭。


## 0007_source_evidence

source_documents按工作区/规范路径唯一，保存当前版本、启用状态和并发revision；source_versions保存不可变UTF-8规范化文本、文本哈希、原字节哈希及时间。source_events追加imported/reimported/reauthorized/revoked。knowledge_source_links按(node_id,knowledge_version)固定source_id/source_version与1基起止行/摘录；复合外键与工作区触发器阻止串域，更新/删除不可变。来源行定位基于移除BOM、CRLF转LF后的快照。

知识修订继承原来源链接，含义是内容起源而非新版本已被该文件证明。停用与重导入和相关FTS更新共用事务。既有0001–0006文件保持原校验和，升级沿用备份与完整事务；旧无文件来源知识不受影响。来源快照不会自动建成已审核知识，也不直接进入FTS。

## P1-B2 数据协议（无迁移）

manifest schemaVersion=4 增加 citationProtocol=knowledge_lines_v1，knowledgeRefs 增加请求内 citationId，原知识/文件版本和正文不变。编号/引用说明也出现在实际 request_json 中，容量计算覆盖完整资料消息。

execution_events 新增实际事件种类 citations_checked：payload.version=1、scope=reference_locations_only、availableCount、references（仅有效引用涉及的固定知识项）、occurrences（输出 UTF-16 起止偏移、原标记、referenceId、1基闭区间 startLine/endLine、错误原因）。同一 finish 事务只对 running→终态成功更新的运行追加一次，继承既有不可变保护。messages API 的 citations 字段为该事件的只读投影，未完成和旧记录返回 null。数据库仍为迁移版本7；既有 SQL 文件校验和不变。


## 0008–0010 Benchmark 增量迁移

当前开发数据库为 schema 10；前述 schema 7 段落为历史状态。0008 为用途隔离与评测存储，0009 为工作区偏好版本，0010 为只读项目快照、生成计划与候选修订、人工审核和串行批次。旧迁移保持原文和校验和，沿用迁移前备份与单事务失败回滚。详细字段与边界见 [Benchmark 数据结构](benchmark-database-schema.md)及 [C3–C5 报告](benchmark-phase-c3-c5.md)。评测继续使用 purpose=benchmark，不改变聊天和生产知识。旧程序回退使用升级前备份。

# Benchmark 数据与迁移方案

2026-10-29切片7补充：复用 `benchmark_evaluations` 追加独立评分，`details_json.regrade` 保存 session/批次/源评分、输入及引擎哈希、模型/题目/实际批次轮次和统计口径；没有新SQL迁移。旧记录没有该字段，仍按原评分读取；默认概览/结果/详情显式排除独立评分，各轮历史单独分页。确认时复核源数据指纹，事务中整体提交，取消/失败整体回滚；不覆盖原评分、原输出、任务状态及计量。导出包含全部尝试和各评分版本，固定用途/工作区/批次检查，见 [切片7](benchmark-report-slice7.md) 与 [协议](benchmark-export-schema.md)。以下为历史阶段记录。

2026-10-24阶段B补充：[运行时评分](benchmark-scoring-stage-b.md)复用现有表，无新SQL迁移。v2评价以独立engine/evaluator版本、caseScoring/规则hash保存，输出hash覆盖文字及真实原生帧；核心状态和扩展生命周期保存在评分JSON中，原数据库状态约束不变。v1继续原评价；没有重新评分接口或覆盖旧结果的操作，未打开正式用户数据库。

2026-10-23 · 切片1已实现0008，切片2复用benchmark_datasets/cases安装不可变草案；当前切片3复用run/model/case_run/attempt/evaluation/event表打通开发单题。共享insert_execution在同一事务保存会话/消息/执行/请求，输出终态与评分终态分事务，评分失败保留原文并记录错误。不新增迁移；源码为0001–0008，已发布v0.2.0仍为schema7。验证仅用隔离库，未打开或升级正式用户数据库。批量排程/恢复等后续接口仍是设计。

## 保留的事实表

| 已有结构 | 复用目的 | 已核实约束 |
| --- | --- | --- |
| workspaces / conversations / messages | 归属、内部评测会话、原始回答 | 0001：同工作区复合外键，运行与回答消息循环引用延迟检查 |
| execution_runs / execution_events | 一次实际请求、状态和真实事件 | 0001：一个running执行，事件拒绝普通更新/删除 |
| context_snapshots / context_knowledge_items | 精确请求、清单和真实知识版本引用 | 0004：不可变，知识同域；不能插入虚构知识ID冒充数据集资料 |
| knowledge_versions / approvals、sources及其版本 | 后续实验材料的来源 | 0002/0007：版本不可变，草稿/生效及停用语义保留 |
| execution_layouts / graph_layouts | 现有视角 | 0003/0005：知识与执行视图分离，乐观修订 |

`db.rs::open/migrate`集中登记迁移、核对连续版本与checksum、旧库VACUUM INTO备份、事务升级及恢复。迁移逻辑提为私有函数以验证真实失败路径；旧0001–0007字节保持原样。

## Phase 1切片1：0008_benchmark_foundation

已新增10张基础表、必要索引、JSON/状态约束、复合外键和不可变保护。现有聊天接口只读取/写入chat用途，评测记录的专用业务接口、完整题目发布校验和报告导出尚未实现。当前没有途径在正式UI启动评测。

给conversations增加`purpose TEXT NOT NULL DEFAULT 'chat'`，只接受chat/benchmark。原会话自动归入chat。每次实际尝试在当前工作区新建benchmark用途会话，不复用生产对话历史。普通会话列表、改名、加载/发送和聊天运行列表在后端限定chat用途；评测读取及执行图显式校验对应尝试归属。这个字段不等同独立的用户访问权限。

已将创建会话的无列名INSERT改为显式列名；历史旧schema的迁移fixture保持原五列定义。bootstrap活动运行和list_runs过滤chat；普通运行读取及执行图入口拒绝benchmark。终态写入还校验run、workspace、conversation与assistant message整组身份，防止错写其他用途的消息。

以下为已实现表的职责摘要，实际字段以[0008 SQL](../apps/desktop/src-tauri/migrations/0008_benchmark_foundation.sql)为准。产品ID沿用UUID、时间用UTC；配置JSON版本与完整数据集验证在后续写入接口中落实，本切片不填充产品数据。

| 表 | 主键/关键字段 | 规则 |
| --- | --- | --- |
| benchmark_datasets | `(id,version)`；name、content_hash、schema_version、manifest_json、license/provenance、created_at | 发布的数据集版本不可变；草稿编辑后发布新版本 |
| benchmark_cases | `(dataset_id,dataset_version,case_id)`；category、input_json、expected_json、scoring_json、difficulty、output_format、requirements_json、fixture_json、content_hash | 复合FK到数据集；恰当区分保密判分答案与可发送材料；旧题不更新 |
| benchmark_suites | id、workspace_id、name、revision、config_json、updated_at | 可编辑配置模板，乐观并发；运行时复制冻结，不直接指向可变模板内容 |
| benchmark_runs | id、workspace_id、dataset_id/version、suite_id可空、launch_key、status、config_snapshot_json/hash、hardware_snapshot_json、app/source/evaluator版本、created/started/finished_at | `(workspace_id,id)`唯一；launch_key同工作区唯一，防双击重复建任务；六种实验状态；冻结配置字段不可改 |
| benchmark_run_models | `(run_id,model_slot)`；workspace_id、profile_snapshot_json/hash | 复合FK到同域run；具体模型身份、量化、能力、请求参数及检查时间不可变 |
| benchmark_case_runs | id、workspace_id、run_id、model_slot、case_id、dataset_id/version、repetition、arm、status投影 | 同域run及模型槽位FK、完整题目FK；唯一`(run_id,model_slot,case_id,repetition,arm)`；Phase 1 arm=single |
| benchmark_attempts | id、workspace_id、case_run_id、attempt_no、execution_run_id可空、status、request_disposition、started/finished_at、error_kind | 尝试序号唯一；execution_run_id唯一且复合FK到同域既有run；预检阻断可没有实际run；重试新增行 |
| benchmark_evaluations | id、workspace_id、attempt_id、evaluator_id/version、scoring_config_hash、output_hash、metrics_json、details_json、status、created_at | 评分追加保存；再次评分创建新记录，保留原评分和选择依据；不复制回答正文 |
| benchmark_resource_samples | `(attempt_id,seq)`；t_rel_ms、source、scope、device、values_json、availability、sample_interval_ms | 有界采样；值必须有单位/来源/范围，缺失为null及原因；随尝试只追加 |
| benchmark_events | `(run_id,seq)`；kind、occurred_at、attempt_id可空、payload_json | 实验开始/停止/恢复、预检阻断和真实模型管理事件，追加不可变；实际回答步骤仍在execution_events |

HardwareProfile首版是run内不可变硬件快照；ModelProfile首版是模型槽位快照。BenchmarkMetric/Result由真实输出、资源样本与evaluation投影得到，不再复制一套消息和执行表。BenchmarkComparison首版是查询条件及导出清单，不预建空表。

### 数据校验与索引

- 所有工作区命令显式检查workspace_id；关联FK和必要触发器同时阻止跨域尝试、评分、模型槽位及run。
- case_run的数据集版本必须等于所属run版本，重复序号在run配置范围内；模型槽位必须存在。不能只校验case_id存在。
- execution_run必须来自benchmark用途会话，且只能绑定一个attempt；同一attempt至多一个实际run。请求快照/消息/运行与attempt绑定在一个事务提交，失败则都不发送。
- 每个case_run至多一个running attempt；每个数据库至多一个running benchmark。保留既有一个running execution索引，跨聊天/评测还由共享应用占用保护。
- 常用索引：runs(workspace_id,created_at,id)、case_runs(run_id,status,model_slot,case_id,repetition)、attempts(case_run_id,attempt_no)、evaluations(attempt_id,created_at)、samples(attempt_id,t_rel_ms)。具体分页按稳定ID作次序兜底，结果数不因UI切页重新请求模型。
- 为数据集内容、配置快照、事件、已结束attempt及评分加不可变保护；运行中状态和已落盘输出检查点沿用受限更新。模型不能写题目答案、评分规则或结果表。
- 模型/输出内容、JSON大小和样本数在后端设上限；超出返回明确失败或截断标记，不能静默把截断答案算成成功。

### 状态和提交窗口

切片1启动恢复会保留两种用途的输出并追加实际中断事实；普通活动投影仍不暴露评测。已完成ExecutionRun但尝试未落盘终态时只对齐已有状态，不重发、不生成评分。以下调度/评分事务由切片3–5实施。

启动事务先冻结计划并写全部逻辑项。调度事务认领pending项并追加尝试；网络调用发生在事务提交后。模型结束沿用`Database::finish`保存输出与真实终态，随后保存评分。

两次提交之间若崩溃，恢复器先看已关联ExecutionRun：已完成输出直接采用并补评分；失败/停止/中断保留原尝试，用户决定是否新增尝试；未创建实际run的预检项可重新检查。由此避免“评分没写入导致再调用模型”。本地恢复键只防本应用重复调度，不等同云API计费幂等。

`completed`实验表示计划项均已终结，仍可包含答错、失败和unsupported；各类数量单独呈现。blocked环境项不能伪装为已测试。取消未发送题目保留pending并由实验cancelled覆盖显示，恢复时仅重新认领未完成项。

## Phase 2后续迁移

候选0009按实际Provider实现加入：

| 表 | 内容 |
| --- | --- |
| model_providers | id、workspace_id、协议、origin/base_path、credential_ref、revision；默认按工作区隔离，没有明文Key |
| model_profiles | Provider、模型ID、用户参数与修订；每次run继续复制快照，旧结果不跟随配置变化 |
| provider_pricing_snapshots | 版本、模型、来源、取得时间、币种、计价单位和十进制收费规则，不追写旧价格 |
| benchmark_budget_entries | run/attempt、reservation/settlement/unknown_adjustment、最小货币单位整数、pricing_snapshot引用、请求ID和时间；预算事件追加 |

预算限额属于run冻结配置。预留和剩余额度检查同事务，取消或未知结果保留未决责任；密钥轮换只更新凭据引用/系统存储，不重写快照或把密钥混进JSON。Provider配置默认工作区隔离，后续确有共享需求再明确授权域。

当前快照request_json是Ollama形状，Phase 2须在manifest增加Provider和wire格式版本，并升级前端安全阅读器；历史Ollama快照仍按原格式读取，不把云端请求伪装为Ollama结构。无需重建context_snapshots表。

另一个兼容点是`models.rs::ModelSettings`和`db.rs::run_detail`：当前运行配置只反序列化本地参数，不能直接保存云端配置。Phase 2增加有版本的RunModelConfig解析，旧无版本JSON仍按本地配置读取；API新增Provider可辨识的modelConfig，本地settings保留兼容投影，云端settings为null。CognitiveInspector、ExecutionTrace及TypeScript类型按Provider呈现，不能给云端补造num_ctx等本地值。app_settings和日常聊天继续使用原本地设置，密钥不进入任一种配置快照。

`src/execution-data.ts::projectExecution`当前模型检查/发送说明写明“本机模型”；云端接入时须按实际Provider/事件证据解释，不能把云端响应写成已检查本机模型。

## Phase 3知识快照与Phase 4比较

候选0010加入benchmark_context_sets（同域、版本化manifest/hash）及benchmark_context_items（固定node/version/正文/hash/来源版本/采集时状态）。原始知识和来源使用现有FK或明确可空的出处，快照正文保证可重放。专用benchmark_knowledge_fts按context_set隔离，复用分词与排序逻辑，不改生产knowledge_fts或effective_version。

同时保存当次实际检索候选、排序、预算排除原因及最终材料；只有真正纳入请求的生产知识版本才写context_knowledge_items。固定数据集资料保存fixture_refs，不伪造生产知识引用。来源撤销后阻止新的请求，历史实验继续可读。

Phase 4确有保存比较需要时再加入benchmark_comparisons（两个run、题目对齐规则、过滤及度量选择、revision）和比较视角；不占用现有单run的execution_layouts。运行、case、真实执行和知识仍使用已有稳定ID。

## 升级、备份与恢复门槛

1. 在隔离的版本7合成库及空库试升级；原0001–0007校验值必须逐项不变。检查旧消息、审批、来源、图谱、快照及视角逐表相等。
2. 沿用升级前一致性备份；无空间或无写权限时停止升级。注入新迁移中途失败，确认事务回滚且旧库可读，不出现一半新表。
3. 再次启动不重复迁移，新增外键/不可变触发器/唯一活动任务约束确实生效；恢复不重复调模型。
4. 旧版应用应拒绝更高schema；恢复使用明确选定备份和兼容应用。不会自动降级或用旧文件覆盖新结果。
5. Phase 1的实验报告导出不等于全库备份恢复，既有完整数据管理待办独立保留。

0008已在空库和版本7隔离合成库验证，后续阶段的表、索引和编号仍为设计。本切片新增生产知识来源/上下文/检索保护触发器；未来真实知识增强实验需专门迁移及授权快照，不能绕过这些基线限制。

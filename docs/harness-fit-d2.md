# Phase D2a：项目任务生成与审核

2026-10-29。用户在D1后授权继续，并确认先交付任务生成与审核闭环，编程修改任务另做D2b。方案见[D2分片计划](harness-fit-d2-plan.md)。本轮没有执行第三方Harness、模型推理、项目脚本或付费请求。

## 使用流程

评测 → 项目适配 → 确认有原文的项目要求 → 保存分析版本 → 测试任务。

选择已确认规范，或导入/选择实际项目快照并选取代码行 → 本地生成候选 → 打开原文、合法答案及逐项评分对照 → 修订表述/依据 → 逐题批准当前修订并填写审核人 → 发布任务集新版本。通过草案历史继续审核，通过已发布版本只读查看；历史结果不会随新修订改变。

规范题按指定资料和行号复述原文，标准答案来自真实快照，保留大小写/标点，仅忽略首尾空白；它检验原文定位与复述，不声称验证执行规范的实际行为。代码定位题要求输出包含给定连续行的相对路径；后端在固定快照查找所有匹配文件，任一合法路径可得分，不将文本匹配冒充调用链理解。生成无需云端密钥，也不会借用第三方认证。

每次1–12项、每个片段1–30行且最多8KB；同一段规范或相同定位题拒绝重复生成。只使用已经确认且有原文的需求；人工补充但没有原文的要求不自动出题。需求的用户修改表述与原文区分：复述题回答实际来源内容，不能把修改后的意思当作资料原文。

项目导入复用Phase C的扫描器与SQLite快照，支持系统文件夹/ZIP选择；沿用2000条目、单文件512KiB、文本20MiB、ZIP50MiB及链接/敏感内容过滤。D1的单文档快照只用于规范题，不作为代码项目；普通个性化项目列表也排除这类单文档条目，避免缺少项目摘要字段。

## 评分依据和审核边界

复用`exact_alias` v2及`core60-runtime-2.1`；每项创建/修订/发布时实际调用现有评分器：全部合法参考答案必须Passed，空答案、确定错误答案、多余工具调用必须Failed。保存逐条输入、预期、实际状态及检查结果。评分规则检查通过只代表这些对照符合预期，不证明用户改写的题目语义正确，也不是模型/Harness成绩；每次改写后必须重新人工批准。

任务保存原文快照ID、文件/hash、起止行、引用、分析版本/hash、项目快照/hash、生成规则版本、实际Case合同、输出格式与限制。文件只读清单、禁止写入/联网/终端/工具/副作用、120秒与16KiB输出限制为未来执行合同；D2a没有执行器，不声称资源限额已经在真实Harness中生效。只读任务不涉及参考补丁或修复前失败基线，明确标为不适用。

## 存储、隔离与版本

没有新增表或迁移；schema仍为11，0001–0011保持原样。

| 已有存储 | D2a用途 |
| --- | --- |
| benchmark_repository_snapshots | 保存用户选取的资料/项目副本，不改原文件 |
| benchmark_generation_jobs | plan_json.purpose=harness_tasks；绑定分析/来源及生成规则，计划hash校验 |
| benchmark_generated_cases | task JSON及评分对照；修订只追加，拒绝过期revision |
| benchmark_datasets | manifest包含完整任务、计划、评分版本和内容hash；发布version递增 |
| benchmark_dataset_reviews | 精确记录批准的题目ID/revision、审核人和发布版本 |

任务集hash覆盖固定计划、全部发布任务及评分器版本，发布的Case绑定该任务集version并重算Case内容hash；候选修订与发布版本分别记录。发布再次从原快照生成并比对来源/答案/合同，不信任前端的“已通过”；事务内复核最新revision和expectedVersion，拒绝重复/过期发布，失败不留下半个任务集。

普通个性化生成和题库列表/读取均限定personalized用途；历史缺省purpose按原personalized处理。Harness任务集保存在manifest，不插入普通benchmark_cases；旧Runner经personalized_dataset/benchmark_case不能读取或运行它。没有新聊天、知识、记忆或模型执行记录；生成/审核不伪装成ExecutionRun。D3后续应通过明确的Harness任务入口读取固定manifest，复用既有Runner基础设施，而非绕过此隔离。

## 实际修改文件

| 文件（均相对仓库） | 作用 |
| --- | --- |
| apps/desktop/src-tauri/src/benchmark/harness/tasks.rs | 固定来源生成、现有评分对照、用途限定、只追加修订/发布/历史、七个业务命令 |
| apps/desktop/src-tauri/src/benchmark/harness/tasks/tests.rs | 必要的来源/用途/并发修订/发布/重开及无效输入检查 |
| apps/desktop/src-tauri/src/benchmark/harness/mod.rs | 接入tasks子模块 |
| apps/desktop/src-tauri/src/benchmark/personalized/{repository,generation,datasets}.rs | 共享受限扫描选择器，保留原云端准入；按JSON用途隔离任务与普通题库 |
| apps/desktop/src-tauri/{build.rs,src/lib.rs,capabilities/default.json} | 登记实际七个本地业务命令，不开放通用终端/文件能力 |
| apps/desktop/src/benchmark/harness/{TaskWorkbench,TaskReview}.tsx、taskTypes.ts | 来源选择、审核、版本查看、原文面板、草稿与错误恢复 |
| apps/desktop/src/benchmark/harness/{HarnessFitLab.tsx,types.ts,harness.css} | 新增第四步、项目摘要类型、既有Tokens列表/折叠样式；按后续验收调整为四步同列宽、透明步骤/版本操作行与正文独立滚动，去除重复标题 |
| apps/desktop/tests/harness-tasks.spec.ts、playwright.config.ts | 真实ZIP文件选择、Rust/SQLite/UI闭环与三尺寸浅深截图 |
| docs/harness-fit-d2{-plan,}.md及实施/数据库/界面/测试/索引/里程碑 | 本轮范围、证据和后续门槛 |

## 后续与未验收项

D2b再增加功能实现、Bug修复、跨文件修改、回归/补测、数据库兼容及工具/安全行为任务；须有可复现基线、正确参考补丁、无修改/错误补丁负对照和非目标回归。当前Docker未检测到，不能在宿主机执行这些验证，更不自动进入D3。

没有真实Harness/模型Task Performance或Model×Harness结果，没有新安装包/发布/远端推送。系统150%与读屏没有在D2a重新实测；历史用户验收不能替代新界面验收。真实付费云端与之前20项Docker编程阻断保持原状态。实际执行命令、截图和结果见里程碑；本轮完成后停在D2a验收。

## 实际验证结果

必要Rust检查为新增2项与现有个性化兼容1项，均通过；未运行全量单元。离线Rust检查、TypeScript及最终Windows调试构建通过。最终原生3个场景22.0秒通过，覆盖D2闭环、D1真实文件选择和原准入；三尺寸浅深截图已核对代表原图，固定审核栏命中无重叠。独立SQLite为schema11、两份来源、两版任务集，模型执行/知识/来源知识表新增记录均0，外键检查无错误。详见里程碑的完整命令与证据路径。

本轮验收窗口已打开并恢复到前台，入口“评测 → 项目适配 → 测试任务”，使用独立QA数据；代码项目为受控示例，可另行选择真实项目。截图位于`artifacts/harness-d2-acceptance/ready.png`。窗口没有启动模型或Harness任务。

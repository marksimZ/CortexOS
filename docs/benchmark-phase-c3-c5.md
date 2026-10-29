# Phase C：项目资料、出题与专属评测

2026-10-27。用户确认 C2 验收，并授权完成 C3–C5。本轮交付开发版，保留 v0.2.0 发布物及 Core 60 v1/v2 文件；不生成 Capability 120 或 Cognitive 32。

## 可操作流程

1. 评测 → 个性化评测 → 验证云端连接 → 保存或跳过使用偏好。
2. 可选：通过系统窗口选择项目文件夹或 ZIP。检查本地摘要、文件与行号；需要发送代码时，把具体范围加入清单。
3. 设置 1–12 道题、能力类别和难度；混合难度可分别填写基础、中等、困难题数。填写所选模型的单价、价格依据、币种和本任务预算。
4. 预览固定的发送计划，检查实际发送的全部内容、模型、输入预留、输出上限和费用。勾选确认后发送一次；修改设置后需要重新预览。绑定密钥不构成发送授权。
5. 候选题经过结构、标准答案自洽、来源范围及重复检查，保持“待人工审核”。展开答案与规则核对，必要时编辑候选 JSON 并保存新修订；填写审核人，逐题批准后发布。
6. 选择固定题库版本，以及最多 6 个不同模型（本地模型和最多一个当前连接的云端模型）。再次确认评测计划后串行运行。云端评测也会发送题目资料，因此独立预览和确认。
7. 报告显示已评分正确率、评分覆盖率、执行成功率、状态数量、各能力/难度、观察到的 Token、速度和费用估计。展开单题查看输出与评分，定位 3D 执行过程，或通过系统保存窗口导出 JSON。

生成历史和评测历史保存在工作区。中断后仅在用户明确确认下继续尚未发送的题目；已发送、失败或中断的题目不自动重试。需要重跑时建立新评测，保留旧记录。

## 复用与数据边界

- 复用现有六类评分器、`benchmark_runs / attempts / evaluations / events`、请求快照、执行记录与全局任务锁。`runner_v2` 提取可等待的同一执行入口供批量调度使用，未建立第二套评分存储。
- 已批准数据集使用 `personal-<生成任务 ID>` 与独立递增版本。题目由后端分配 `custom-` ID，固定内容哈希、评分合同 v2、使用偏好、项目快照、生成计划、模型、审核人与候选修订。修改重新发布为新版本，保留历史题库与成绩。
- 普通聊天仍只读取 `purpose=chat`。所有评测会话使用 `purpose=benchmark`，不检索或写入生产知识库，不写长期记忆。资料是题库内固定 fixture，不会作为已审核知识发布。
- 个性化读写、生成、恢复、报告和导出通过 C1 后端准入。停止任务按工作区和任务归属检查，即使凭据失效也可停止。Core 60、本地标准评测及其执行图不要求云端连接。
- 3D 读取与视角保存增加明确用途入口。普通聊天接口继续拒绝评测会话；评测结果直接定位既有执行画布，保留真实请求快照和事件，没有添加虚构检索或内部思维链。

## 项目读取与联网

扫描只读、不执行脚本、不安装依赖。最多 2000 条目、32 层目录、单文件 512KB、文本合计 20MB；ZIP 文件最多 50MB。ZIP 在内存按条目读取，不解压到宿主机；检查目录记录、展开大小、压缩比、重复/大小写冲突、越界路径、分卷/ZIP64与符号链接。Windows 打开文件后通过实际句柄路径再次确认授权目录，拒绝重解析点。

忽略依赖、构建产物、隐藏路径、敏感文件名、二进制/非 UTF-8 和疑似凭据文本。快照只保存相对路径。过滤不能识别全部秘密，发送前必须人工检查。

语言、目录、函数声明、API/数据库/测试框架位置属于有界静态线索；依赖识别目前解析 `package.json`，其他语言的完整依赖及语义分析仍有限。界面明确展示分析依据，不声称完整理解项目。摘要模式仅发送使用偏好及结构分析，代码模式只附加批准的行范围；仓库文字作为不可信数据，不能提升权限。

云端通过现有 Provider Adapter 扩展 OpenAI Responses、Anthropic Messages 和 Gemini generateContent。固定官方 HTTPS 域名、禁止重定向、限制输入/响应、连接/总超时、原生结构化工具对象解析，工具只评分不执行。凭据留在 Windows 凭据管理器，前端/SQLite/导出不保存密钥。更换或删除连接会影响后续准入，删除时取消在途请求；已发送请求仍可能计费。

单次生成最多 1 请求，无自动重试；批次上限固定为题数乘模型数。费用按用户提供的价格与输入字节预留、输出上限计算；云端评测包含 schema/tools 的请求体大小。保留实际用量、请求哈希、价格来源与时间。缓存、思考 Token、服务商计费规则及网络中断可能使账单不同；用量未知不作为零费用。默认免费价格或未知价格不能启动预算保护任务。

## 数据库与兼容性

新增 `0010_personalized_benchmark.sql`，保留 0001–0009 原文及校验和：

| 表 | 作用 |
| --- | --- |
| benchmark_repository_snapshots | 按工作区保存不可变文本快照和分析 |
| benchmark_generation_jobs | 固定计划/哈希、状态、响应与用量 |
| benchmark_generated_cases | 追加候选修订与校验结果 |
| benchmark_dataset_reviews | 固定人工审核选择和发布版本 |
| benchmark_batches | 固定模型/题库/预算/偏好计划 |
| benchmark_batch_items | 串行状态与既有运行 ID |

沿用迁移前备份与事务回滚；测试覆盖 v9→v10 中途失败、回滚、重试及备份可读。重启将发送中生成任务和运行中批次标为中断，不自动重发；通过工作区约束恢复子运行关联。发布和候选采用不可变历史保护。

旧版程序不能直接读取 schema 10。回退使用升级前备份与兼容程序，不能通过删迁移记录“降级”。正式 v0.2.0 安装包没有被本轮替换。

## 实际修改文件

以下为本轮 C3–C5 的文件，工作树已有的 Phase B、C1/C2 和界面微调不重复算作本轮修改。

- 后端新增：`apps/desktop/src-tauri/src/benchmark/personalized/{mod,repository,cases,plan,generation,datasets,batch,cloud_run,report,tests}.rs`、`cloud/completion.rs`、`cloud/completion_tests.rs`。
- 后端接入：`benchmark/{mod,dataset,dataset_v2,evaluation,repository,runner,runner_v2,scoring_repository}.rs`；相关原测试的 `StartInput` 增加可选数据集字段；`cloud/{mod,provider,tests}.rs`；`commands.rs`、`execution.rs`、`db.rs`、`db/profile_tests.rs`。
- 权限/构建：`src-tauri/{Cargo.toml,Cargo.lock,build.rs,capabilities/default.json,src/lib.rs}`，新增上述迁移。新增 ZIP 读取依赖，没有更换前端框架或 npm 依赖。
- 前端新增：`src/benchmark/personalized.ts` 和 `src/benchmark/{PersonalizedWorkbench,RepositoryImport,PriceFields,GenerationSetup,DatasetReview,BatchSetup,PersonalizedReport}.tsx`。
- 前端接入：`benchmark/BenchmarkPage.tsx`、`benchmark/ProfileEditor.tsx`、`benchmark/benchmark.css`、`ExecutionTrace.tsx`、`studio.ts`。沿用 Soft Glass、SelectMenu、FileName、现有滚动和按钮规范。候选编辑草稿保留在当前进程，不包含凭据。
- 检查：`tests/personalized.spec.ts`、`tests/cloud-connection.spec.ts`、`tests/cloud-native.spec.ts`、`playwright.config.ts`。
- 文档：本报告、Phase C 计划、实施计划、文档入口、数据库结构、测试计划和里程碑。

## 已执行验证

- `cargo test --lib personalized --offline`：最终 11 项通过。覆盖恶意 ZIP/链接、只读扫描及限额、来源与行范围、预算/确认哈希/重复发送、异常响应/重复题目、不可变发布/修订、六类合同、迁移恢复、批次恢复和模拟云端全链路。不是实际模型能力成绩。
- `cargo test --lib benchmark:: --offline`：32 项通过、1 项真实 Docker 检查按既有规则跳过。包含 Core60 v1/v2、评分状态/分母、原生工具、JSON、RAG、Python 控制器替身及存储回归；与上一项有重叠，不合并统计。
- `npm run typecheck`、`cargo check --lib --offline` 和 `npm.cmd run tauri -- build --debug --no-bundle` 通过。保留 Vite 大块提示。
- 浏览器：`personalized.spec.ts --project=edge` 三种窗口的完整引导/确认/审核/报告通过，真实组件截图保存在 `artifacts/phase-c-ui-final`；数据与接口是明确的 UI 替身。`cloud-connection.spec.ts` 4 项通过。
- 原生 WebView2：连接准入及导航 1 项、知识编辑与执行图谱 2 项、标准评测 v1/v2 运行 2 项通过，使用隔离数据库与合成模型响应。证据分别在 `artifacts/phase-c-native-guard`、`phase-c-native-regression`、`phase-c-runner-final`。没有把浏览器截图称为真实云端原生端到端。

最初的受限原生检查在 CDP 握手阶段超时/连接重置，尚未执行断言；换用已授权桌面验证权限并串行运行后，上述 5 项通过。一次离线格式化因缓存缺失未运行，未安装格式化工具。实现中发现的连接锁重入、个人题库版本误走 Core60 v2 限制及界面检查脚本语法问题，均已修正并复验。

## 验收边界与下一步

- 未使用真实云端密钥做推理，三家真实账号的生成权限、模型 Schema 支持、配额、企业网络及账单需验收；模型目录成功不保证这些能力。
- 本机没有 Docker；真实容器执行/清理仍待办。当前缺环境时保存运行错误，绝不回退宿主机执行。
- 自动校验不证明题意、答案、引用语义或 Python 边界测试正确，人工审核仍必要。首版仅接受已有六类合同，真实仓库修改、开放式代码重构及多步工具工作流不纳入自动成绩。
- 推荐只基于本次完整同题数据的实际成绩，保留并列和运行依据；不同题库没有统一总榜，显存等缺失指标不填零。
- 本轮没有重新做系统 150% 缩放与读屏；之前用户对旧版本的验证不作为新增页面的验证。验收时可先用小型无敏感项目及少量题目，检查费用与发送范围，再开始真实任务。

开发交付后停止，交给用户验收；不自动进入下一阶段或发布安装包。

协议依据：[OpenAI 文本生成](https://developers.openai.com/api/docs/guides/text)、[原生工具](https://developers.openai.com/api/docs/guides/function-calling)、[结构化输出](https://developers.openai.com/api/docs/guides/structured-outputs)、[Anthropic Messages](https://platform.claude.com/docs/en/api/messages/create)、[Gemini generateContent](https://ai.google.dev/api/generate-content)。协议适配与真实模型可用性分别验证。


## 已打开的验收窗口

已启动最终调试版并用原生 WebView2 确认“评测 → 个性化评测”的连接页可见，1440×900。验收数据位于 `artifacts/phase-c-acceptance/2026-10-27-fa44a6e7/data`，通过 SQLite 备份接口从原库只读复制；原库仍为 schema 7。只有副本升级到 schema 10，并保留可读的迁移前 schema 7 备份（文件名按首个待应用版本 8 命名）。新窗口独立保存验收配置和凭据引用，不复制密钥。窗口截图 `artifacts/phase-c-acceptance/window.png` 仅作本地证据，不用于公开展示。

窗口进程 23156，原生可见检查为 true；系统未允许 SetForegroundWindow，因此未声称已强制置顶。如没有自动显示在当前前台，可从任务栏切换 CortexOS。`WaitForInputIdle` 不适用于此调试进程，后续以真实页面和窗口可见性核实。窗口留给用户操作，未自动发送模型请求。

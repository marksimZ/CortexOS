# Phase D1：项目文档与Harness静态适配

2026-10-29。D0见[执行就绪报告](harness-execution-readiness.md)，实施顺序见[D1方案](harness-fit-d1-plan.md)。本轮只交付Document Fit，完成后停止等待验收；D2–D5及真实Harness执行未启用。

## 使用流程

评测 → 项目适配 → 导入规范文档或选择已有项目快照 → 分析所选资料 → 逐类确认/修改/忽略线索 → 选择需要的能力、确认后标“必须满足” → 查看结果及调整权重 → 保存新版本。

支持development.md、AGENTS.md、README.md等UTF-8 .md/.txt。系统文件选择器由Rust处理，单文件2MiB；复用原来源读取器的本机磁盘、敏感文件名、符号链接/重解析点、实际文件句柄、编码与大小检查。取消不创建快照。导入副本复用不可变benchmark_repository_snapshots，不加入生产知识检索。已有快照只解析文档及其Repository Profile虚拟JSON，不读取新的未授权目录。

本次选取1–12份快照，文档合计4MiB、最多100份，每份最多10000行、单行最多4096字符；最多800条线索。超限明确拒绝并保留当前内容，不截掉尾部冒充完整分析。旧快照超出最新50条选择清单仍可从已保存分析引用读取，后端历史版本可按版本号定位。

## 提取与判断口径

12类：语言、框架/数据库、系统环境、构建测试、Git、项目规范、终端/外部工具、安全权限、阶段审批、模型/费用、规模/跨文件、上下文/指令。采用有界本地规则，**不是语义理解完整性保证**。每条保存类别、原始表述、当前表述、明确能力条件、硬要求标记、原文快照ID/文件/行/引用/hash、提取置信状态及用户审核状态。缺信息生成未知项；用户可填写未知项，记录为用户修改，不能伪装成文档原文。

否定、未来/可选/举例等线索保留原文而不自动关联成正向能力。其他自动关联也只是待确认建议。能力条件表示“需要支持此能力”；无法表达或尚未关联的约束保留未知，不推断已兼容。源引用由后端从不可变快照重新导出校验，前端不能修改原文、hash或置信字段冒充证据。

按必须条件先筛：资料明确不支持则排除；必须条件未知则待核实；资料支持也只表示可继续核查，不表示本机实测。四类0–5权重可调整（至少一项非0）。同一能力重复提及只计一次；项目背景/忽略项不计分；权重0不取消硬要求检查。下界=资料支持权重/总权重，上界=(资料支持+未知)/总权重，证据覆盖=(资料支持+不兼容)/总权重；无分母时N/A。展示每行贡献和来源，不合成模型跑分。

初步建议在未排除项中优先考虑必须条件有资料支持、下界较高的候选，并保留并列；证据不足则不点名推荐。候选需求数量始终显示，用户未确认时仍是暂算。Doc Fit支持四态显示，本轮没有Harness实测兼容证据，verifiedCount恒为0。

## 能力目录

内置OpenAI Codex、Claude Code、Aider、OpenHands、Cursor五项，版本`harness-catalog-2026-10-29.1`。目录内容来自本轮实际查阅的官方文档，每条有来源URL、查阅日、事实说明及未知状态。各Harness实际安装版本为未知，目录快照版本不等于CLI发行版或运行验收版本。没有固定未经核实的模型标签、上下文长度、价格或通用认证方式。

例如[Codex非交互文档](https://learn.chatgpt.com/docs/non-interactive-mode)支持CLI/JSONL资料依据，[Claude程序化使用](https://code.claude.com/docs/en/headless)、[Aider脚本模式](https://aider.chat/docs/scripting.html)、[OpenHands headless](https://docs.openhands.dev/openhands/usage/cli/headless)、[Cursor输出格式](https://cursor.com/docs/cli/reference/output-format)分别支撑对应目录字段。OpenHands此headless模式固定自动批准，因此与必须逐步人工审批存在资料层面的冲突。MCP在交互终端可用不能自动证明headless支持，未查证的组合保持未知。

目录由版本化JSON维护，事实变化应增加目录版本；历史分析保存整个目录和hash，不随应用目录升级静默重算。CortexOS的云端密钥与CLI认证彼此独立，本轮不读取第三方Harness认证文件或探测其推理权限。

## 实际修改文件

| 范围 | 文件与变化 |
| --- | --- |
| 本地服务 | `apps/desktop/src-tauri/src/benchmark/harness/{mod,extract,fit}.rs`：有界提取、来源重校验、匹配、版本读取/追加及5个业务命令 |
| 目录 | `apps/desktop/src-tauri/catalogs/harness-catalog-v1.json`：五候选与逐事实官方来源 |
| 数据库 | `migrations/0011_harness_document_fit.sql`、`src/db.rs`：单一追加式harness_fit_assessments表，复用现有备份/迁移事务 |
| 注册 | `src/lib.rs`、`src/benchmark/mod.rs`、`build.rs`、`capabilities/default.json`：只注册已实现的D1业务接口，不开放通用文件/终端能力 |
| 前端 | `src/benchmark/harness/{HarnessFitLab,RequirementEditor,FitReport}.tsx`、`types.ts`、`harness.css`：资料侧列、分类确认、可解释结果、权重、原文位置及版本历史 |
| 接入 | `src/benchmark/BenchmarkPage.tsx`：增加项目适配子页；标准/个人入口保留原准入逻辑 |
| 必要检查 | `src/benchmark/harness/tests.rs`、`src/db/harness_migration_tests.rs`、`tests/harness-fit.spec.ts`及Playwright配置；复用并扩展文件对话框助手的可选标题；旧cloud-native检查改用已统一的顶部页面标题 |
| 文档 | D0、D1方案及本文，实施/数据库/测试/HTML规范、索引、里程碑 |

## 存储与兼容性

schema10→11；0001–0010原迁移字节不变，无新增依赖。新表以(workspace_id,version)为主键，正文JSON、内容hash与时间；更新和删除均被触发器阻止。正文绑定源快照、原始/用户修改要求、置信/确认状态、权重、目录/hash、引擎/结果、说明及前一版本hash。expectedVersion防止覆盖其他窗口的新版本。需求修改保存为新分析版本，不改项目文档或现有Repository Profile。

升级前复用VACUUM INTO备份。专项先从schema10及旧聊天记录升级，故意失败验证事务回滚与可读备份，再重试真实0011；历史聊天保留。D1读写不调用Runner/Provider，不创建ExecutionRun或模型成绩。后续D2–D5将复用已有运行基础设施；本轮没有建第二套运行器/评分/图谱。

## 验收边界

规则匹配需人工确认，无法保证推断所有自然语言约束；明确未映射的约束不能作为兼容证明。无真实Harness安装/认证/代码编辑或终端验证；Task Performance、Model×Harness实验、任务生成/补丁评分、D5图谱均未实现。Docker/WSL缺失保留D0门槛，历史20项阻断未改。本轮未使用付费云端、下载模型/依赖或变更宿主配置。实际系统150%/读屏和跨硬件未测。

当前截图及命令结果见milestone-log；数据来自真实development.md本地导入与实际SQLite，不是云端生成的推荐文字或静态图片背景。完成D1后停止，不自动进入D2或推送发布。

## 实际验证结果

- `cargo test harness_ --lib --offline -- --nocapture`：最终3项通过（0.08秒，81项未运行），含新表/来源边界和故意迁移失败恢复；不累计多轮复验为新增用例。
- TypeScript、Rust offline check及Windows调试构建通过；最终构建Rust29.88秒，仍有既有Vite大块提示。
- 原生D1及既有个性化准入两个场景通过；最终D1交付版专项1项通过（19.8秒，场景17.5秒）。实际系统文件选择和取消、来源行号、确认/硬要求、权重、两版历史/重载、三尺寸浅深、固定保存可达、只读查看不产生未保存提示均检查。
- 独立库schema11，2个分析版本、1份RepositorySnapshot；ExecutionRun/知识节点/知识来源均0，外键检查无错误。没有模型请求或运行型成绩。资料及自动化审核操作仅用于独立验收库。
- 实际截图：`artifacts/harness-d1-verified/harness-fit-local-document-edc9c--reviewed-evidence-versions-desktop/fit-{light,dark}-{1366,1600,1920}.png`。本机验收窗口图为`artifacts/harness-d1-acceptance/ready.png`，已查看。没有单独以浏览器替身结果冒充原生检查。
- 验收窗口进程3480，已恢复显示并获准切到前台；进入“评测 → 项目适配”。查看现有版本不重算或写新版本。后续D2须另行确认，Docker环境缺失仍保持未就绪。

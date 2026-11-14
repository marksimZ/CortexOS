# D3-2c：Codex 本地单题执行

2026-11-01。实施范围与预算见[本片方案](harness-fit-d3-session-plan.md)。仅已审核、版本固定的 Python 纯函数任务。Codex 会话、普通聊天、任务生成及人工交接分别保存；没有新增数据库迁移、依赖或模型下载。

## 实际实现

- 三个工作区内业务接口：`get_harness_codex_session`、`start_harness_codex_session`、`cleanup_harness_codex_session`。启动前复核已通过的兼容隔离检查及其 hash、任务/分析/项目快照、实际本地模型 digest 和工具能力；沿用全局 TaskSlot，不读取个人登录。
- 容器保持 network=none，无端口映射、宿主目录或 Docker Socket。容器中的回环 HTTP 只服务 Codex；通过本次 Docker 标准输入输出交给 Rust，Rust 校验后仅使用现有 Ollama `/api/chat`。不提供通用代理或终端接口。
- 每任务最多四次请求，编程阶段 180 秒，单次 120 秒；上下文 4096、单次输出 512 Token、temperature=0、keep_alive=0。原生 API 明确收到这些设置。Provider/模型错误、协议不支持、预算终止、工具调用及独立评分分别保留，不把普通文本转换为工具调用。
- 文本任务只向模型提供 `exec_command` 和 `write_stdin`。后者须使用实际存在的会话；模型额外权限请求被拒绝。固定 Linux CLI 的非交互模式自动拒绝提权；保持 workspace-write、内层沙盒、无外网，不伪称已实现交互审批。
- 任务包只有题目和选定函数，没有参考实现与测试。明确告诉工具副本的真实位置 `/workspace/project/<来源路径>`，不要求小模型从原始项目路径猜测副本路径。文件按原范围生成提案，始终不写回原项目。
- 收集时验证实际文件、所有者、权限、hash、题目及允许范围；失败保留有效的部分文件，未变化明确标为“未修改”。成功完成 CLI 回合后，先清理编程副本，再复用原固定 Python 镜像与评分器逐断言测试。模型会话完成与测试通过分开显示。
- 取消/正常退出清理已记录归属的容器；强杀后的 running/grading 仅变为 interrupted，不重新调用模型。用户手动清理后终结。清理失败保留 cleanup_required，阻止新会话，历史事件只追加。

## 版本与隔离

schema 仍 11。复用 `benchmark_generation_jobs` / `benchmark_generated_cases`，用途 `harness_codex_session`，计划版本 `codex-session-1`。计划绑定模型请求、任务版本/hash、原隔离检查/hash、执行及评分两个 Image ID、端点、容器名、预算、包、评分器。事件逐条 hash；所有读取、启动、结束、清理及恢复使用用途/工作区约束。

新版计划附 `codex-stdio-text-2` 描述与 relay/file IO/bridge/workflow 源码 hash，便于区分工具范围和提示模板。早期无此描述的本片记录继续原样读取；不会根据当前代码重算旧成绩。当前不写普通 BenchmarkRun，不生成排行榜；完整 Model×Harness 对照留到 D4。

原 `codex-bwrap-1` 描述中的 `codex_runtime_only` 保留为历史原始值，新会话单独记录实际用途范围 `harness_codex_session_only`。过滤器字节、系统调用和 hash 未改变；不因扩展到本片而改旧检查计划。

## 实际兼容修正

同限制无模型复现发现 Codex 初始化自己的 SQLite WAL 达到 1MiB 后收到 SIGXFSZ。仅会话的单文件 fsize 调为 4MiB，实际 WAL 观察为 2,154,792 字节；HOME 仍共 4MiB，/tmp 仍 16MiB，CPU 1、内存 256MiB、PID 64、非 root/capDrop ALL 和只读根保持。普通 Python 评分和人工副本不改。

真实工具还因无法创建保护路径 `/workspace/.git` 而失败。Codex 副本预建 `.git/.agents/.codex/.aws` 四个空 root/0555 目录；固定 IO 策略要求其存在且为空，仍只允许两个任务文件。原人工副本使用默认严格清单；没有放开新文件或目录写入。

## 本地验证记录

必要协议边界检查通过；实际命令、首次失败和最终构建见[里程碑](milestone-log.md)。真实 Codex + Docker + 原生 WebView2、受控 Provider 检查通过：实际命令修改、7 条断言、4 次请求预算、错误工具名拒绝、停止、强杀恢复/手动清理、历史保持和三尺寸浅深主题。受控回复只验证通道，明确不算模型能力或真实模型成绩。

第一批真实模型仅使用用户弹窗批准的 qwen3:4b-instruct / `0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`。会话 `7bf018fc-f2d3-46e6-9485-6710a244d9e1` 发出四次请求，第四次达到输出上限；8017 输入、743 输出 Token。模型选错工具和路径，文件未改；保留 failed/model_output_limit、原始回复与 Not Run 测试，清理成功，未自动重试。不是通过成绩。

路径与文本工具范围修正后，用户另行批准同题相同预算的新一批单题验证。会话 `105d3b8e-7bd1-4247-b7aa-ceebea136071` 真实发出三次请求：读原文件、写现有文件、完成回合；4974 输入、357 输出 Token。候选为 `return max(0, n)`，独立测试 7/7 Passed，全部容器清理，旧失败记录字节不变。仅证明本片单题闭环，不作为模型能力排行榜。

结果分别保存在 `artifacts/harness-codex-session-real/` 与 `artifacts/harness-codex-session-real-text2/`。受控专项最终目录 `artifacts/harness-codex-session-controlled-final/`；三库只读核对、归属容器和模型释放观察见新版结果目录的 `database-verification.json`。

## 本轮修改文件

- `apps/desktop/src-tauri/src/benchmark/harness/session.rs` 与 `session/{records,bridge}.rs`：用途记录、准入、预算和协议转接、独立评分与停止恢复。
- `apps/desktop/src-tauri/src/benchmark/python/session.rs`、`codex_session.py`、`workspace_io.py`：复用 Docker 创建/检查/清理、固定双向通道、Codex 空保护目录策略。原人工副本保持默认行为。
- `benchmark/harness/{mod,workspace}.rs`、`workspace/package.rs`、`runtime/records.rs`、`benchmark/python.rs`：内部 helper 复用与模块注册；旧人工交接来源保持原值。
- `apps/desktop/src-tauri/{build.rs,capabilities/default.json,src/lib.rs,src/db.rs}`：三个精确接口、正常关闭清理与本用途启动中断恢复。
- `apps/desktop/src/benchmark/harness/{CodexSession,CodexRuntime,ExecutionPreflight}.tsx`：单题确认、历史/真实文件/评分、未就绪时仍可读历史，以及历史检查与当前执行的明确区分。沿用已有样式。
- `apps/desktop/tests/harness-codex-session.spec.ts`、`playwright.config.ts`、`harness-codex-runtime.spec.ts`：有界原生专项、显式真实推理开关、旧提示文案定位同步。
- `scripts/harness/{probe_codex_transport,verify_session_delivery}.py`：无模型协议观察与只读交付核对。
- `AGENTS.md` 与本片方案/交付、D3计划、实施计划、数据/UI/测试说明、文档索引及里程碑：保存用户要求、实际范围与证据。

## 限制和剩余步骤

这是文本/函数工具协议子集，不支持图片、联网工具、交互审批、整个仓库或跨文件任务。Responses 事件在原生响应收全后发送，不表示逐 Token 流式首字延迟；显存没有在本片做峰值采样，停止接收不保证 Ollama 立即停止计算。keep_alive=0 后是否释放以只读 `/api/ps` 观察为准。

类型/构建和有界信任边界检查之外，没有跑无关全套单元；没有故意等待满 180 秒或做耗尽磁盘实验。三尺寸截图属于真实 WebView2 的 CSS 视口检查，不等同于新一轮 Windows 缩放、读屏和物理窗口检查。

本片验收前不进入下一 Harness。后续顺序：D3-3 逐个接 Claude Code / Aider；D4 固定任务下的 Model×Harness 组合对照；D5 报告与图谱。D2 全仓库/跨文件任务、云端与其他模型另行规划。每步交付均报告已完成、剩余和实际阻断。

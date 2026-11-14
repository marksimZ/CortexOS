# D3-3c：Claude 本地单题通道

2026-11-09更新。状态：实现、受控原生专项、旧Codex回归及已授权真实单题完成；验收窗口已打开，等待用户验收。

## 已实现

- 复用执行记录、函数副本、停止/关闭清理、崩溃恢复、实际文件收集和独立 Python 评分。Claude 使用 `harness_claude_session` / `claude-session-1`，绑定同工作区已完成且已清理的 Claude 检查及任务版本/hash。schema 仍为 11，没有迁移或生产数据改写。
- 新增 Messages 文本/Bash → Ollama 原生 chat 的受限适配；工具调用读取结构化对象，检查 Schema、调用 ID 与结果绑定，拒绝未知工具、后台/解除沙盒参数、输出截断与缺失用量。Provider 每次转发前核对模型指纹。
- 预算固定为四请求、4096 上下文、512 输出、单次120秒、编程阶段180秒；容器只能通过本机后端的标准输入输出通道访问所选 Provider，无直接宿主网络。每次实际请求/响应及拒绝的协议请求保留审计。
- 新 Claude relay 使用已验收专用镜像与固定兼容策略。非 root、只读根、无网络/宿主挂载/Socket、原资源限额继续生效；保护 `.claude` 等空目录，只有已存在目标文件可原地编辑。未增加权限或依赖。
- 共用现有单题表单：实际模型选择、逐批显存提示、历史、停止/清理、真实修改、独立测试和原始记录。旧检查仍表示当时未调用模型，不用新功能覆盖历史状态。

## 实际验证

- `cargo check --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`：通过；初次警告已去除。
- `cargo test --offline --manifest-path apps/desktop/src-tauri/Cargo.toml bridge_ --lib`：必要的两项协议信任边界通过，97项未运行。
- Python AST：`workspace_io.py`、`claude_session.py` 两文件语法通过。
- `npm --prefix apps/desktop run build`、最终 `typecheck`：通过。保留既有 Vite 大块提示，没有顺手改打包架构。
- `cargo build --offline --features tauri/custom-protocol --manifest-path apps/desktop/src-tauri/Cargo.toml`：内嵌页面桌面构建通过。
- Claude 原生专项：`CORTEXOS_HARNESS_CLAUDE_SESSION=1 npx playwright test --project harness-claude-session --output ../../artifacts/harness-claude-session-controlled4`，1条通过，42.8秒。固定回复驱动真实 CLI 修改函数、独立评分、4请求截停、未知工具拒绝、取消、强杀后标中断与手动清理、历史不变、跨用途/工作区拒绝。不是模型能力成绩。
- Codex 受控回归：`CORTEXOS_HARNESS_SESSION=1 npx playwright test --project harness-codex-session --grep "controlled transport" --output ../../artifacts/harness-claude-session-codex-regression`，1条通过，39.7秒。没有运行原有真实模型测试。
- Claude专项生成且人工查看1366×768、1600×900、1920×1080浅深六张真实WebView2截图；无横向溢出。没有新增150%系统缩放或读屏结论。
- 只读SQLite审计：19条旧任务及全部旧事件完全不变；5条新Claude会话的计划/事件hash正确；除新会话及测试主动创建的隔离工作区外，其他表内容逐行一致。证据 `artifacts/harness-claude-session-controlled4/database-verification.json`。
- Docker按Claude/Codex会话用途查询，均无残留容器。`git diff --check`通过，仅已有CRLF提示。

## 保留的失败与修正

1. 初次普通cargo构建加载开发URL，原生辅助函数等待内嵌URL导致240秒超时，未开始任务。改用 `tauri/custom-protocol`，辅助函数新增30秒URL等待上限。尝试顶层 `custom-protocol` 参数曾提示不存在，未构建；实际有效的是依赖feature名。
2. `controlled2/3`首请求被拒绝，实际为CLI默认 `thinking=adaptive`。保留失败及协议原文；只在容器专属环境设置官方禁用参数、输出512与effort=unset，后端独立执行原有预算；`controlled4`通过。参考[官方变量](https://code.claude.com/docs/en/env-vars)。
3. 只读全表审计首次对WITHOUT ROWID表排序失败，改为按行内容排序后完整比对通过，没有写数据库。

## 实际修改文件

- `apps/desktop/src-tauri/src/benchmark/harness/session.rs`、`session/records.rs`、`session/bridge.rs`，新 `session/anthropic.rs`、`session/claude.rs`。
- `apps/desktop/src-tauri/src/benchmark/python/session.rs`、`workspace_io.py`，新 `claude_session.py`。
- `apps/desktop/src-tauri/src/benchmark/harness/runtime/records.rs`（仅将读取方法限制在harness内复用）。
- `apps/desktop/src-tauri/src/db.rs`、`lib.rs`、`build.rs`、`capabilities/default.json`：用途恢复及三个专属命令。
- `apps/desktop/src/benchmark/harness/CodexSession.tsx`、`ClaudeRuntime.tsx`：复用组件与Claude入口。
- 新 `apps/desktop/tests/harness-claude-session.spec.ts`；`playwright.config.ts`、`tests/native.ts`。
- 本说明、实施方案、实施计划、里程碑。

## 已授权真实单题与最终审计

用户明确允许一次 qwen3:4b-instruct（指纹 `0edcdef34593eac1aa2be9c7d06c432dcf81945adca5eca2f27662c18f168ba0`）有界会话。实际记录时间2026-11-05，任务ID `bcced34a-2318-4bf4-82b4-14f36c3ed37a`。执行一次显式授权专项：`npx.cmd playwright test --project harness-claude-session --grep "one explicitly authorized Claude" --output ../../artifacts/harness-claude-session-real`，1条通过（约1.1分钟）。未重开会话、未自动重试、未下载模型或使用云端。

- 真实请求4次：读取文件、一次引号不完整的Python命令、同会话内修正命令、完成响应。失败命令及后续修正完整保留，没有隐去失败。
- `adjust(n)`原文件副本从直接返回n改为负数返回0、其他返回n；7项独立Docker断言全部通过。修改建议未写回原项目。
- Provider计数累计输入4489、输出234 Token；输入为四轮累计，非单轮上下文。Provider报告总耗时累计26.8510535秒，不等于完整任务墙钟耗时。每次输出均在512上限内。
- 会话记录clean=true，pendingGraders为空，7个评分容器均记录removed=true。
- 2026-11-09只读审计核对19条旧任务及全部旧事件原样保留，其他领域表逐行一致，新计划/事件hash正确；证据 `artifacts/harness-claude-session-real/database-verification.json`。

## 验收窗口与边界

上次打开验收窗口时，自动审批服务因额度限制无法完成审核，操作没有执行。2026-11-09恢复正常审批后，运行 `node artifacts/harness-claude-session-acceptance/launch.cjs` 打开独立QA数据库；调用 `show.ps1 -AppProcessId 19724` 核对原生visible=true、foregroundGranted=false。窗口可从任务栏切换；已查看真实ready.png，定位“Claude 单题执行”，结果、历史数量与保存快照相同。仅查看历史，没有新推理、工具执行或新容器。

本片实现及验证完成，停在用户验收。单题通过仅证明当前固定模型、任务和预算下闭环成立，不代表通用项目开发能力。系统150%/读屏本轮未重测；无新增代码变更，未重复单元测试。剩余D3-3d Aider、D4组合对照、D5报告/图谱；均未开始。后续模型批次另行弹窗。

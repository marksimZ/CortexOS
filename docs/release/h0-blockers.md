# Phase H0 发布阻断项

验收日期为 2026-11-20。依据 [G5 发布验收](../optimization/phase-g5-release-validation.md)、[Phase G 验收记录](../optimization/phase-g-acceptance.md)、[G0 基线](../optimization/performance-baseline.md)、[G 优化路线](../optimization/optimization-roadmap.md) 和 [开发规范](../../development.md)。仓库工作区与 Git 历史均未找到 `design.md`，界面沿用 [HTML 原型规范](../../design.md)。

| 问题 | 状态 | 发布影响 |
| --- | --- | --- |
| 标准评测在要求立即释放时仍发送五分钟保留 | 已关闭。原生固定回复和有界真实单题均通过。 | 保留单题实测证据及其适用范围。 |
| 启动、RAG 与空闲资源回退嫌疑 | 已完成同机交替对照与分段测量，详见[性能复核](h0-performance-review.md)。 | 当前证据未定位到需要修改业务代码的回退。正式安装包性能另验。 |
| 真实 Windows 安装身份与交互 | 待独立 Windows 环境验收，详见[安装矩阵](h0-windows-validation.md)。 | 阻断正式发布。 |
| 安装包签名、许可证和第三方分发材料 | 已检查候选包和依赖来源，准出待办见[发布决定](h0-release-decision.md)。 | 阻断正式发布。 |

## 模型保留策略

| 入口 | 配置来源 | 运行时与归档 |
| --- | --- | --- |
| 对话 | 用户对话偏好，应用于 Ollama 请求 | [对话组装](../../apps/desktop/src-tauri/src/db.rs) 读取 [本地资源偏好](../../apps/desktop/src-tauri/src/local_runtime.rs)，实际请求进入快照。 |
| 本地标准评测 | 仅作用于当前已确认的 Core 60 v2 计划 | 未指定时保留历史默认的 `"5m"`。勾选即时释放后，本批每题发送 `keep_alive:0`，由 Ollama 在请求结束后执行。计划 JSON 和哈希覆盖该选项，请求快照保留实际值，清单补记 `localModelRetention`。 |
| Harness | 适配器为受控请求固定 `keep_alive:0` | [Codex](../../apps/desktop/src-tauri/src/benchmark/harness/session/bridge.rs)、[Claude Code](../../apps/desktop/src-tauri/src/benchmark/harness/session/anthropic.rs)、[Aider](../../apps/desktop/src-tauri/src/benchmark/harness/aider_channel.rs) 保留各自原有配额、超时与容器边界。 |
| 调度 | 批次独占租约与已确认的换模授权 | 批次排队、取消、续跑和转组规则继续生效。主动释放前核对模型名称与指纹，且只处理计划中授权的已有模型。单题结束后的保留参数随本题请求发送。 |

## 修改与验证

- [批量计划与调度](../../apps/desktop/src-tauri/src/benchmark/personalized/batch.rs) 和[运行请求与快照](../../apps/desktop/src-tauri/src/benchmark/repository.rs) 接收可选的 `localKeepAliveSeconds`。后端限制为本地 Core 60 v2、0–300 秒。界面仅提供明确的「每题回答结束后释放本次模型」开关，保持原默认行为。空字段按原格式省略，旧计划哈希与请求快照继续用原值解释。没有数据库迁移、快照更新或者隐式重跑。
- [标准评测设置](../../apps/desktop/src/benchmark/StandardSetup.tsx)、[计划与报告](../../apps/desktop/src/benchmark/PersonalizedReport.tsx)、[前端类型](../../apps/desktop/src/benchmark/personalized.ts) 在发题前展示此次选择，重新进入计划可核对。现有 Rust 基准测试调用处补齐空值以保持兼容。
- [标准评测原生专项](../../apps/desktop/tests/standard-benchmark.spec.ts) 通过，1 条，整轮 20.3 秒。固定回环记录 9 次请求，其中旧规则占用资源的请求为 `"5m"`，新计划剩余 8 个为 `0`。重开后取消等待没有发送新题，资源释放后仅发送计划中的题。读取真实 SQLite 快照，核对其请求哈希与回环收到的请求字节一致，并检查资源保留的上下文记录。原始日志 `artifacts/phase-h0/mock.log`，非活动 Windows 桌面、隔离测试库。
- `npm.cmd run build` 及离线锁定的 Rust 调试构建通过。未运行 Rust 单元测试或全面回归。本轮以原生的跨 UI、IPC、调度、HTTP 与持久化专项验证该改动。

## 真实模型复测

经 H0 单独授权，真实 Ollama 请求 1 次：Core 60 v2 `math-001` 回答 `95`，状态 `Passed`，上游收到 `keep_alive:0`。保存的请求与清单均记录 `0`，Ollama 在下一项开始前报告已加载模型为 0。原 G5 的 Execution Error 仍保留。单题验证覆盖新请求、实际评分和退出后释放，详细配置、图谱并行及显存见[性能复核](h0-performance-review.md)。

## 2026-11-21 旧草稿回归

在独立桌面用最终 H1 调试构建运行 `playwright test --project=desktop standard-benchmark.spec.ts`，两项通过，1.2 分钟。先写入使用旧结构的草稿，再检查立即释放默认未勾选。默认模型保留、勾选后的每题即时释放、排队取消、恢复、请求哈希和实际快照继续由固定回环验证。测试日志位于 `artifacts/phase-h1/h14/standard-final.log`。本轮复核沿用 H0 既有真实模型证据。

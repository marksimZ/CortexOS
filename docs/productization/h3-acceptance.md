# H3 可靠性与诊断验收

## 交付内容

- 统一任务状态投影，区分等待、执行、停止中和各业务终态。窗口退出纳入存储任务的取消和收尾。
- 增加「设置 → 关于 → 本机诊断」与 JSON 导出。最近问题以已知类别呈现处理建议，备份恢复、库锁与取消仍由原业务入口完成。
- 复用 Docker 控制器进行只读引擎检查，兼容此前安装于当前 Windows 用户目录的 Docker Desktop。已有 Docker 隔离参数及模型权限继续适用。
- 建立固定数据、原生采样和异常恢复的重复验证入口。测试数据使用单独的 SQLite 和 WebView2 目录。

## 修改位置

- [任务投影](../../apps/desktop/src-tauri/src/tasks.rs)、[应用状态](../../apps/desktop/src-tauri/src/commands.rs)、[关闭与命令](../../apps/desktop/src-tauri/src/lib.rs)、[存储扫描](../../apps/desktop/src-tauri/src/storage.rs)。
- [诊断与报告](../../apps/desktop/src-tauri/src/diagnostics.rs)、[Docker 本机查询](../../apps/desktop/src-tauri/src/benchmark/python.rs)、[Ollama 错误状态](../../apps/desktop/src-tauri/src/ollama.rs)、[Windows 主窗口权限](../../apps/desktop/src-tauri/capabilities/default.json)、[命令清单](../../apps/desktop/src-tauri/build.rs)。
- [前端诊断](../../apps/desktop/src/components/LocalDiagnostics.tsx)、[IPC 类型](../../apps/desktop/src/api/diagnostics.ts)、[设置入口](../../apps/desktop/src/Settings.tsx)。
- [原生诊断](../../apps/desktop/tests/diagnostics-native.spec.ts)、[性能样本](../../apps/desktop/tests/h3-baseline.spec.ts)、[资源采样](../../apps/desktop/tests/h3-sample.ps1)、[RAG](../../apps/desktop/tests/rag-native.spec.ts)、[项目工具](../../apps/desktop/tests/harness-experiment.spec.ts)、[测试项目](../../apps/desktop/playwright.config.ts)。
- 后端异常专项位于 [云端用量](../../apps/desktop/src-tauri/src/runtime_usage.rs) 和 [Docker 评分器测试](../../apps/desktop/src-tauri/src/benchmark/python_tests.rs)。

数据库保持 Schema 26，依赖版本与锁文件保持原有版本。诊断读取由用户主动触发。行为与数据边界见 [H3 可靠性](h3-reliability.md)，采样和重放步骤见 [H3 性能回归](h3-performance-regression.md)。

## 验收矩阵

| 范围 | 原生或故障注入结果 |
| --- | --- |
| 空闲、长对话及图谱 | 原生采样 91.85 秒空闲、120.60 秒固定流式，12 次切换工作区并打开图谱，另完成 1,000 节点图谱。前后资源数据和保留占用见性能报告。 |
| 排队、取消与恢复 | Rust 队列专项、两条原生标准评测及一条三工具队列通过。队列取消不会影响其他任务。Codex 正常停止，Claude 的固定失败后 Aider 接手。强制退出后无等待项重发，保留已完成状态和失败原因。 |
| 数据库写入、容量、索引 | SQLite 写锁阻塞时原工作区保持可读，释放后重新写入成功。容量不足采用注入，原数据仍可读。固定回环验证索引排队取消、主动取消、错误维度和保留旧索引。 |
| Ollama 与网络异常 | 本机回环模拟服务不可用、无响应、返回 503 和流式断线。部分回答保留。503 的最终提示由独立本机 HTTP 专项核对。 |
| 云端状态与重试 | 回环验证 401、限流 429、503 和超时。鉴权及服务错误仅发一次，限流按设置有上限地重试，费用记账保持可查。 |
| Docker 中断与资源 | 用测试替身在第二条断言中断引擎，保留已完成断言和待清理标识。真实 Linux Docker 队列验收后新增残留容器为 0。按授权临时启动，完成后恢复关闭。 |
| 诊断与隐私 | 独立 Windows 桌面通过页面读取、Windows 保存对话框和 JSON 校验。植入合成密钥、端点、聊天、代码、知识和工作区标识，导出均未包含。1366×768 截图见 `artifacts/phase-h3/diagnostics-final/`。 |

### 执行命令

- 前端在 `apps/desktop` 执行 `npm.cmd run build`、`npm.cmd run lint`、`npm.cmd run format:check`。TypeScript、生产构建和格式检查通过，ESLint 有 63 条已有警告。
- 桌面执行 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`，最终构建通过。
- 同一 manifest 以 `cargo test --offline --locked --lib <专项名称>` 运行云端 401/503/超时、429 计费重试、队列归属、Docker 评分器故障、容量不足恢复和本机 HTTP 503，六项通过。
- 原生按 [性能回归](h3-performance-regression.md) 的入口分别运行，另运行 `playwright test --project=desktop diagnostics-native.spec.ts rag-native.spec.ts` 并重跑诊断专项。合计通过 9 项原生用例，重复执行按一项统计。

首跑性能测试遇到两处测试夹具问题。隔离导入文件须放在测试数据目录内，切换工作区也需要通过界面创建以更新工作区选项。修正后 90 秒周期和完整场景复验通过。Docker 未启动时先观察到队列预检失败，获准临时启动后复验通过。

真实模型多轮加载与卸载、正式云端请求、物理切断正在使用的 Docker Engine、重启系统后的冷启动和多小时资源趋势尚未实测。本轮 Docker 中断使用故障注入。当前停在 H3 验收点，后续为 H4 正式安装、签名与发布工程，再进行 H5 产品体验验收。

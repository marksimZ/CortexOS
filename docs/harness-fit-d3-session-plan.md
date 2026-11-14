# D3-2c：受控本地模型与单题执行

2026-11-01。承接已验收的固定 Codex 隔离策略。复用原 Docker 控制器、Ollama Provider、TaskSlot、任务副本清单和独立评分，不更换 Runner、数据库或原任务版本。

## 本片步骤

1. 复核真实协议，建立有界 Responses → Ollama chat 转接；固定本地模型身份、4096 上下文、512 输出 Token、每任务最多 4 次请求、总计 180 秒、每次最多 120 秒。容器无网络，仅容器内回环 HTTP；通过本次 Docker 标准输入输出交接，由 Rust 校验后发往已有 Provider。禁止透传 URL、凭据或任意请求参数。
2. 固定 Codex 镜像和兼容策略启动单个已审核函数任务，保存实际 JSONL、请求/响应及终止原因。工具不携带答案和测试；只允许原函数文件，保留原目录权限。模型返回的普通文本不会转换为工具调用。只支持实际函数工具，其他协议明确不支持。
3. 严格收集候选、保存不可变修改，清理执行副本后使用原固定 Python 镜像独立逐条评分。正常停止/异常退出只清理已记录且归属匹配的容器，不自动重试推理，不覆盖历史检查与成绩。
4. 类型检查、构建和最小协议/预算/归属验证；独立 QA 数据做真实 WebView2 检查。实际模型验证前弹窗说明实际模型、唯一任务和有界批次；未获确认时保留未运行。
5. 交付本片并停止验收。D3-3 下一 Harness、D4 组合、D5 报告/图谱后置；跨文件/完整仓库任务仍未实现。每步交付列出剩余步骤和阻断。

## 已核实的协议限制

- 同限制的无模型复现确认：Codex 初始化 state_5.sqlite-wal 达到原 1MiB 单文件限制后收到 SIGXFSZ，尚未发模型请求。仅 Codex 会话的 fsize 改为 4MiB，仍受原 4MiB HOME / 16MiB tmpfs 总额度约束；不修改其他容器、系统设置、CPU/内存/PID 和网络限制。实际配置写入执行记录。
- 原 D3-1 的只读目录没有 `.git/.agents/.codex/.aws`，真实工具的 bubblewrap 保护路径准备因无法建目录而失败。仅 Codex 副本在锁定目录前预建这四个空的 root/0555 目录，IO 使用固定 `codex-protected-empty-1` 校验；仍只有题目和目标函数两个文件，额外文件或子目录一律拒绝。普通人工副本继续使用原规则。

- 官方 Ollama OpenAI 接口不能直接设置上下文长度，故复用原生 `/api/chat` 的 `options.num_ctx`，不把 Codex 的窗口元数据当成显存限制。实际显存仍受模型及运行环境影响。
- 2026-11-01 固定 Linux CLI 0.166.0 的无模型观察中，`exec` 即使收到顶层 on-request 也采用自动拒绝提权的非交互审批；保留 workspace-write 与内层沙盒，不采用全权限或跳过沙盒。需要额外权限的任务在本片不可执行。
- 转接只支持文本消息与 JSON 函数调用/结果。缓冲原生响应后发送 Responses 事件，不声称测得逐 Token 首字延迟；实际工具调用只来自结构化 `tool_calls`。
- CLI 工具清单由关闭联网搜索、多代理和目标自动续跑的固定配置产生。身份、能力、环境、预算与模型回答失败分开记录；通过协议检查不等于模型完成任务。

依据：[Codex 配置](https://learn.chatgpt.com/docs/config-file/config-reference)、[Provider 配置](https://learn.chatgpt.com/docs/config-file/config-advanced)、[Ollama 兼容接口及上下文限制](https://docs.ollama.com/api/openai-compatibility)。实际无模型观察保存在 artifacts/harness-codex-transport/protocol.json，不作为模型成绩。

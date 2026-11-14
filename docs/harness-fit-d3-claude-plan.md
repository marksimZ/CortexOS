# D3-3：Claude Code 增量接入

2026-11-02。用户在 D3-2c 交付后要求继续，按 D3 顺序先接 Claude Code，再单独接 Aider。复用现有 TaskSlot、Docker 控制器、固定 Python 任务/评分和追加式存储，不创建新 Runner 或修改知识、聊天、题库。

2026-11-03更新：[D3-3b方案](harness-fit-d3-claude-sandbox-plan.md)已按两次明确许可完成，见[交付](harness-fit-d3-claude-sandbox.md)。预算参数解析、依赖及真实Bash固定检查已核验，旧D3-3a结果保留。停在3b验收；3c模型通道/实际预算及单题仍未实现，没有模型调用。下文是初始分片边界，不据已解阻断重写历史。

## 本轮 D3-3a：程序与安全条件检查

2026-11-02交付更新：用户已明确批准固定工具准备。官方2.1.309大小/SHA-256/签名通过，离线镜像与真实无模型检查、停止/恢复和数据兼容证据已完成，见[实际状态](harness-fit-d3-claude.md)。普通帮助未列出`--max-turns`、缺bwrap/socat、默认策略下用户命名空间拒绝均保留阻断；Claude内层沙盒及模型通道未验证。停在D3-3a验收，不自动进入3b或模型调用。

1. 核对官方文档及稳定发布清单，固定 Linux x64 Claude Code **2.1.309**。官方清单二进制 293,469,552 字节，SHA-256 `4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658`。只准备项目缓存和专用镜像，不安装宿主程序，不使用个人登录。下载来源/清单/hash 与实际执行观察分开保存；签名若未验证明确记录。
2. 基于已有固定 Python 镜像 `sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb` 离线构建，镜像执行保持 Docker 默认策略、无网络、只读根、非 root、无宿主挂载/Socket、CPU/内存/PID/时间/输出限制。Codex 专用 seccomp 策略不自动用于 Claude。
3. 只运行实际 `--version`、`--help` 及固定环境探针，核对自动化/空配置/严格沙盒参数和 bubblewrap/socat 等依赖。确认版本/参数不代表沙盒或任务已通过；缺依赖/命名空间权限则明确阻断，不启动模型或项目任务。禁止用关闭沙盒、自动批准或扩大权限获得通过。
4. 复用原运行检查流程和记录校验，以明确的 `harness_claude_runtime` 用途隔离。绑定当前工作区/分析和已发布函数版本/hash，避免把“宿主 Codex 已安装”作为 Claude 的条件。所有创建、读写、恢复、清理均限制用途与归属；不改旧 Codex 记录格式或策略。
5. UI 沿用任务页折叠段、SelectMenu、FitStatus、AppDialog；列出程序/参数/依赖/沙盒/模型通道各自的真实状态。没有任务运行按钮或虚构成绩。类型检查、构建、必要的原生无模型检查、截图和恢复核对后停止验收，不进入 D3-3b。

## 文件与复用方案

- `scripts/harness/prepare_claude.py`、`claude.Dockerfile`：固定来源/清单/hash、缓存与离线镜像，不运行下载的安装脚本，不触发模型下载。
- `benchmark/python/claude.rs`、`claude_probe.py`：复用原固定 Docker 参数和归属清理，添加精确版本/参数及严格环境观察；默认策略与 Codex 模型通道不改。
- `benchmark/harness/runtime.rs`、`runtime/records.rs`：在已有运行检查中增加两个固定用途的内部枚举；保留 Codex API 包装器及旧记录校验，复用事件/历史/终态和恢复，避免另写同类存储。
- `benchmark/harness/runtime/claude.rs`：Claude 专属已发布任务准入与四个精确业务接口，不接受任意命令/端点/路径或保护策略。
- `ClaudeRuntime.tsx` 与任务页必要接入：直接选已发布任务，不依赖 Codex 检查；保留全部旧入口。
- 复用 schema11，不新增迁移、前端依赖或通用终端能力。实际变更及命令记录于 milestone-log。

## 后续门槛与阶段剩余

- **D3-3b**：解决实际发现的 Claude 内层沙盒依赖/权限门槛。新增下载、系统安装或策略变化按具体范围确认；没有安全证据就保留阻断。
- **D3-3c**：Anthropic Messages → 已有 Ollama Provider 的受控通道、预算、真实 JSONL/工具/候选/独立评分；先核实固定版本实际请求。实际模型逐批弹窗，不使用个人 Claude 登录或云端付费接口。Ollama 文档推荐大仓库64k+容量，本项目不据此提高8GB设备的4096配置或擅自下载模型。
- **D3-3d**：单独核对并接入 Aider，同样先核实协议、认证、安全和版本，不视两者能力相同。
- **D4 / D5**：模型×执行器组合对照，再做报告与图谱。跨文件/完整仓库任务、更多模型及云端仍后置。

## 官方依据（本轮实际阅读）

- [Claude Code 程序化入口](https://code.claude.com/docs/en/headless)：bare 模式及隔离自动发现；不把文档示例的自动批准视作本项目授权。
- [CLI 参数](https://code.claude.com/docs/en/cli-reference)：JSON 输出、最大回合、settings 来源和会话持久化；需实际二进制复核。
- [Linux 沙盒](https://code.claude.com/docs/en/sandboxing)：bubblewrap/socat；默认缺依赖可能回退，故后续执行必须启用 failIfUnavailable，禁用 allowUnsandboxedCommands 和弱化模式。文件工具也有独立权限，不以 Bash 沙盒替代完整任务隔离。
- [官方安装及完整性](https://code.claude.com/docs/en/setup)：固定发布清单、SHA-256 与签名说明；本片只下载明确的平台文件。
- [Ollama 的 Claude Code 接入](https://docs.ollama.com/integrations/claude-code)：说明 Anthropic 兼容通道，不证明本机版本/实际模型/4k上下文完成任务。其 launch 可能下载模型，本片不运行 launch。

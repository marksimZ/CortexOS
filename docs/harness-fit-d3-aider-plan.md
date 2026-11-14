# D3-3d：Aider 增量接入

2026-11-09。用户在D3-3c交付后要求继续，进入下一Harness。本轮先完成工具环境及无模型检查；用户已批准 Python 3.12、固定 Aider 及基础依赖下载和专用镜像准备，真实模型另批确认。沿用ponytail最小复用原则，不增加通用执行器、数据库迁移或新页面。

## 已核对的差异

- 官方PyPI当前发布0.86.2，Requires-Python为`>=3.10,<3.13`。现有Python3.13镜像不能作为其安装环境。Aider独立使用官方`python:3.12-slim-bookworm`，准备时记录并固定实际镜像digest及Image ID；原Python3.13独立评分环境保持。
- 官方wheel：`aider_chat-0.86.2-py3-none-any.whl`，377009字节，SHA-256 `64f6a0c66c9f4633ad9f479bca3e64ebcba02b9da03c6b604b74a44736b2416e`。该hash用于内容完整性，不宣称验证了作者签名。依赖需另解析、逐文件固定版本/来源/hash，不把主包hash当依赖证明。
- 官方支持一次消息后退出及禁用Git、自动提交、自动lint/test、分析统计和更新检查。实际固定版本参数和行为仍需运行验证。未确认其具备Codex/Claude同类内部沙盒或JSONL事件，状态分别记录，不能伪造等价能力。
- 当前只查阅元数据和文档，未下载包/镜像，未运行Aider或模型。宿主PyPI请求首次TLS认证失败；通过正常升级权限读取成功，未关闭证书验证。

## 切片与验收门槛

### D3-3d.1：工具环境与无模型检查（本片）

1. 获准后下载官方Python3.12基础镜像、固定Aider wheel及基础依赖，仅项目缓存/专用Docker镜像。缓存上限1GB，超出先报告；不安装Windows/WSL系统包，不包含help/browser/playwright extras、模型权重或个人登录。
2. 依赖解析和下载在临时准备容器完成，不传入仓库、任务或密钥，不暴露Socket。只接收wheel，缺少wheel不自动改用源码编译。记录实际依赖锁和hash；最终镜像离线安装，保留pip check结果与镜像指纹。构建阶段与任务运行阶段区分。
3. 无模型探针：空配置/空HOME、非root、无网络、只读根、cap-drop ALL、no-new-privileges、默认seccomp、CPU1/内存256MiB/PID64/有界输出及超时，检查实际`--version`、`--help`。如资源不够记录真实原因，不擅自提高限额或套用Claude/Codex专用规则。
4. 确认配置和网络副作用边界：显式禁用更新、分析统计、Git、自动lint/test和会话恢复；环境仅白名单。没有API Key或模型地址，不执行题目。Aider不具备内层隔离时如实标记，外层Docker检查与工具自身能力分开。
5. 复用`runtime.rs`/`runtime/records.rs`检查、归属、清理及恢复，新增固定`harness_aider_runtime`用途。绑定同工作区已发布任务，但只检查工具，不执行函数；状态分别列程序、参数、外层隔离、内层能力、模型通道。
6. 复用任务页折叠区和SelectMenu/FitStatus；只增加精确业务命令。代码检查、必要信任边界项、类型检查、构建、无模型原生界面与停止恢复检查后停止验收。

### D3-3d.2：受控协议与实际单题（下一片）

验收d.1后才实施。核对固定Aider实际请求/编辑协议，通过容器回环与stdio接已有Ollama Provider，后端硬限制实际模型指纹、请求数、4096上下文、512输出、超时和取消。关闭自动提交/lint/test；不直接开放宿主Ollama或云端。以固定回复验证真实文件修改、越界拒绝及独立评分；实际模型单题另弹窗。

Aider输出文本只能作为日志，文件变化通过真实收集和hash确认；日志中自称工具成功不能变成真实工具事件。编辑范围仍是已审核的单函数副本。失败、预算耗尽和不可用均保留，不自动重试。

## 文件方案

- 本计划及`milestone-log.md`：决策、已验证事实、实际命令和限制。
- `scripts/harness/prepare_aider.py`、`aider.Dockerfile`、`aider_probe.py`：在工具下载许可后实现固定依赖准备和无模型探针，不运行远端安装脚本。
- `benchmark/python/aider.rs`：复用Docker控制器，核对独立3.12工具镜像；不修改3.13评分约束。
- `benchmark/harness/runtime.rs`、`runtime/records.rs`及`runtime/aider.rs`：新增固定用途分支，保留旧版本记录校验。
- 复用 `ClaudeRuntime.tsx` 中的检查视图并导出 `AiderRuntime`、任务页嵌入、三个或四个精确命令/权限、恢复入口：以实际需要为准，无通用终端能力。
- 专项验证：无模型真实CLI、工作区/用途隔离、停止/中断清理、旧Codex/Claude历史；普通聊天、知识与图谱不改。

## 当前阻断与剩余

工具准备已获确认并完成，2026-11-10 [实际交付](harness-fit-d3-aider.md)记录了版本/帮助、256MiB外层检查、停止恢复与原生验收。工具自身隔离、完整任务资源与受控模型通道仍未验证。之后顺序：d.1验收 → d.2模型通道与单题 → D4组合对照 → D5报告/图谱。完整仓库与跨文件仍后置。

## 本次官方依据

- [固定发布元数据](https://pypi.org/pypi/aider-chat/0.86.2/json)
- [脚本调用](https://aider.chat/docs/scripting.html)
- [参数参考](https://aider.chat/docs/config/options.html)
- [官方Docker说明](https://aider.chat/docs/install/docker.html)

文档中的宿主目录挂载示例不用于本项目运行环境。实际版本和配置以准备后证据为准。

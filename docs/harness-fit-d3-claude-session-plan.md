# D3-3c：Claude 受控本地单题

2026-11-03。用户验收D3-3b并要求继续；已关闭原验收窗口。只接入当前已审核Python纯函数的一个实际任务，完成后停在本片验收。Aider、组合对照、完整项目与更多模型留待后续。

## 范围与复用

1. 现有session存储、任务准入、副本/候选、独立Python评分、停止/恢复与IO流程增加内部Codex/Claude两种固定用途。Codex公开接口/输入和历史保持；Claude新增三个精确业务命令，不提供任意进程、URL、路径或通用代理。
2. 仅Claude新增Messages→Ollama原生chat适配和容器回环/stdio relay。复用现有Ollama Provider显式4096/512/temperature0/keep_alive0/单次120秒，最多4请求，编程阶段180秒、无自动重试。每次转发前核对实际模型指纹；原始请求、原生响应、JSONL、真实工具/文件与错误追加记录。
3. Claude使用已验收的固定依赖镜像及同一已批准内核规则；Session描述明确新用途，不改写D3-3b的策略描述或历史。无外部网络、宿主挂载、Docker Socket；非root、只读根、原CPU/256MiB内存/PID/输出限额。只有可信初始化以容器内root写入只读题目/目录，实际CLI和模型工具始终UID65534。
4. 首版只开放Bash中的python3/cat任务命令，明确允许它们在已验证的严格沙盒内执行；不启用bypassPermissions、acceptEdits、后台工具、提权或未隔离重试。只允许原地修改已有目标文件，目录/题目/配置保护、允许文件清单/hash再校验。没有原项目写回、参考实现或评分断言进入CLI。
5. Messages仅支持文本、原生Bash tool_use/tool_result；严格绑定ID/工具名/参数Schema，拒绝图像、MCP/远程工具、后台运行及未声明能力。普通文本描述不会变工具调用。SSE从实际原生结构化结果生成；原生Token缺失不编造用量，截断停止并保留已有文件。
6. 复用schema11，新增purpose=harness_claude_session；所有读写、恢复、清理和历史按明确用途/工作区限制，新旧记录不混合。新任务必须绑定当前工作区内已完成且已清理、同任务版本/hash的Claude兼容检查。重开不自动推理，保留中断并由用户清理。
7. 复用现有Codex单题表单/历史/确认组件，内部仅两种固定适配器。Claude入口放在原Claude检查内，未就绪仍可读历史；弹窗显示实际模型/digest、唯一函数及预算。无新CSS/页面/依赖或数据库重构。

## 验证顺序

- 先检查代码逻辑；只跑协议/用途/预算信任边界必要项。类型检查、离线Rust检查、桌面构建。
- 固定本机HTTP回复替身配合真实Claude/Docker/WebView2验证工具修改/独立评分、四请求截停、错误工具/参数、不支持能力、取消、强杀恢复/手动清理及旧Codex/Claude历史。替身不是模型成绩。三尺寸浅深截图及只读数据库/容器审计。
- 实际只读模型列表/元数据核对后，通过交互弹窗说明模型/digest、唯一任务、4请求/4096/512/120/180秒上限，等待新许可，再运行最多一会话。不下载模型、不用云端/个人认证、不自动重试。模型若失败仍保留真实结果，不用替身冒充通过。
- 打开独立QA验收窗口展示已有结果；命令与初次失败写入milestone-log。完成D3-3c后停止。

## 实施文件

- `benchmark/harness/session.rs`、`session/records.rs`：共用任务/存储/终态，固定内部用途和Codex包装器。
- 新`session/claude.rs`、`session/anthropic.rs`：Claude专属准入/命令、纯协议适配，不复制评分或新增Runner。
- `benchmark/python/session.rs`、`workspace_io.py`及新`claude_session.py`：复用原Docker/stdio/file IO，按固定工具选择程序、策略和relay。
- `python/claude_policy.rs`：会话专属描述，保留原检查描述/规则。
- `CodexSession.tsx`及`ClaudeRuntime.tsx`：共用现有交互，明确Claude名称/真实状态；旧Codex选择器及行为兼容。
- `lib.rs`、`db.rs`、`build.rs`、capabilities：仅三个具体命令及本用途关闭/中断恢复。
- 显式启用的原生专项和本片交付/数据/UI/测试文档，无新增依赖。

## 边界与下一步

官方[Messages兼容](https://docs.ollama.com/api/anthropic-compatibility)支持子集，不证明本机模型兼容或任务完成；本片仍通过原生chat显式执行预算，不把容器直接接到宿主Ollama端口。官方64k大仓库建议不应用到本机4096配置。[CLI参考](https://code.claude.com/docs/en/cli-reference)与当前2.1.309实际行为共同核对，不执行可能下载模型的ollama launch。

本片后剩余：D3-3d Aider、D4组合对照、D5报告与图谱。实际模型的每个新批次仍按AGENTS另弹窗确认。

兼容参数核对（2026-11-05）：固定 CLI 首次实际发送 thinking=adaptive；适配器拒绝并保留失败。采用[官方环境变量](https://code.claude.com/docs/en/env-vars)在容器专属环境关闭该参数并设 MAX_THINKING_TOKENS=0、输出512、effort=unset。原生 Provider 仍独立固定 think=false/4096/512；不依赖 CLI 参数单独实施预算。

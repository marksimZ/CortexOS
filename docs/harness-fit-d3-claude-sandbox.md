# D3-3b：Claude 沙盒条件交付

2026-11-03。按[本片方案](harness-fit-d3-claude-sandbox-plan.md)完成固定Claude Code 2.1.309的参数解析、依赖准备、独立兼容策略和真实Bash隔离检查。**停在D3-3b验收；没有调用模型或运行项目任务，不进入D3-3c。**

## 本片结果

| 项目 | 实际结果 |
| --- | --- |
| 预算参数 | 普通帮助未列出的`--max-turns 1`实际被解析器接受。控制组和预算组均在固定无效选项处退出1；没有创建模型会话。只证明参数识别，真实模型回合限制未验证 |
| 依赖镜像 | 用户单独批准后，从Debian官方签名HTTPS索引获取约10.1MB索引和536kB包，新增bubblewrap 0.12.0-1~deb13u1、socat 1.8.0.3-1+deb13u1、libwrap0 7.6.q-36；0升级、0删除，不改Windows/WSL |
| 默认策略 | 依赖已存在仍无法创建用户命名空间，bwrap退出1；原默认策略继续保留，结果不改成通过 |
| 独立Claude候选 | 用户另行批准最多三组固定规则对照。显式官方基线和仅unshare组仍失败；bubblewrap候选能执行真实Claude Bash工具。新增接口/hash与已审批方案相同，无新capabilities或弱化配置 |
| 严格隔离 | 固定回复夹具驱动真实CLI/Bash，13项反向保护通过；夹具没有连接本地或云端模型。目录外、home保护目录、系统目录写入及直接网络访问被拒绝，mount/BPF和再次创建用户命名空间均被拒绝 |
| 运行状态 | 新记录`sandboxReady=true`仅表示本片固定环境检查通过，`canRun=false`、模型通道未验证、项目任务Not Run。没有模型能力成绩 |

原CLI镜像ID仍为`sha256:55de5386970372e49b2d221e15d870e36902c1e5d3e37eab9118fe4e9919f32e`；新依赖镜像为`sha256:2258d386fb20be9cc16f1d6867e1e34cc6d3c2e13f33ca07af7fbdc370dba89f`，标签cortexos-claude:2.1.309-sandbox。APT准备阶段联网，实际所有执行检查容器network=none。包清单、下载包SHA-256和来源保存在镜像/opt/claude/evidence，观察副本在artifacts/harness-claude-sandbox/。

## 实际证据与判断边界

`claude-bwrap-1`仅用于明确选择并确认的Claude检查。官方Moby源提交4b8214fc4971e06172938a1f1554ccff6647ee80，候选hash46a3ec985105a85a8d6bb5d21974a5dd5093f176411d766f8acf51bf5f2d9f13。规则字节与此前Codex候选相同，来源文件和临时文件机制复用；Claude拥有独立许可、策略ID/描述、临时文件名、用途及真实观察。Codex API仍拒绝Claude策略，Claude也拒绝Codex策略；默认Python评分/副本保护不变。

三个配置组之后，同一候选组补读能力明细和子命名空间反向保护，没有增加第四种规则或权限。初轮检查将内层CapBnd非零一起算成“可用权限非零”，因此未通过，原记录保留。补读确认：外层CapEff/CapBnd均0；内层UID65534，CapEff/CapPrm/CapAmb均0，而CapBnd为000001ffffffffff。按[Linux用户命名空间语义](https://man7.org/linux/man-pages/man7/user_namespaces.7.html)及[no_new_privs](https://docs.kernel.org/userspace-api/no_new_privs.html)，权限上限不等于进程已有权限，需分开呈现，不能把内层CapBnd写成0或忽略权限再获取路径。

因此使用新的固定检查协议cortexos-claude-fixed-sandbox-2，保留四种实际能力字段；要求外层能力全0、内层有效/允许/环境能力全0、no_new_privs=1、UID65534，并额外实测unshare(NEWUSER)和clone(NEWUSER)均EPERM。13项全部通过后才接入。旧协议初轮未通过的记录没有覆盖或重标。实际OOM/oom_kill均0；部分检查曾触及容器内存上限产生回收事件，不扩大256MiB限额。

夹具只在容器内回环监听，最多两条固定Messages回复，唯一允许命令为固定Python隔离探针；没有连接Ollama或外部服务，不使用个人登录或真实API密钥。不使用bypassPermissions、acceptEdits或任意工具批准。enabled=true、failIfUnavailable=true、allowUnsandboxedCommands=false，弱化嵌套/网络配置为false、excludedCommands为空。前两组CLI最终退出0但其Bash结果失败，评分仍保持失败，不能凭CLI退出码判通过。

原始JSONL里CLI产生的Token/费用/模型名是固定回复夹具的元数据和估算，**不是真实模型用量、计费或成绩**，仅保存于本用途原始记录。

## 数据与界面

- schema仍11，无迁移、依赖或锁文件变化。新增检查计划claude-runtime-check-2、探针claude-program-2，绑定程序、检查源码hash、任务版本/hash、镜像及固定策略描述。旧check-1/program-1按原定义读取，不重算评分或修改历史。
- 复用原worker、用途/工作区/任务准入、事件hash、停止、关闭和中断恢复。两个工具共用临时文件机制，固定作用域及文件指纹防止互相删除，关闭/恢复仍按用途处理。
- 沿用原任务折叠段、SelectMenu、FitStatus和AppDialog，无CSS或页面结构重做。基础/含依赖镜像、默认/Claude兼容选择清楚区分；兼容确认说明额外接口风险。当前记录显示它实际使用的策略和旧版标志，新选择不会重写当前记录。
- 模型通道始终显示未验证，没有真实任务按钮。参数、沙盒和模型能力分别反馈；夹具语义与原始记录按需展开。

## 修改文件

- `scripts/harness/claude-sandbox.Dockerfile`、`prepare_claude_sandbox.py`：已批准的固定依赖镜像、来源及包清单。
- `scripts/harness/claude_sandbox_probe.py`、`check_claude_seccomp.py`：固定无模型回复、实际CLI/Bash、反向探针和有界对照/独立证据文件。
- `apps/desktop/src-tauri/src/benchmark/python/{claude.rs,claude_probe.py,claude_policy.rs,codex_policy.rs}`及`python.rs`：新版参数/隔离分类、固定作用域策略与共用临时文件机制。
- `benchmark/harness/runtime.rs`、`runtime/{claude,records}.rs`：新旧计划兼容、明确策略、停止/恢复/清理；无新Tauri命令或能力。
- `apps/desktop/src/benchmark/harness/ClaudeRuntime.tsx`：现有表单、确认及真实结果。
- `apps/desktop/tests/harness-claude-runtime.spec.ts`：从旧Claude QA库备份验证新旧结果及恢复，显式CORTEXOS_CLAUDE_COMPATIBLE分支，无模型。
- 本片计划/说明及实施、数据/UI/测试规范、文档索引和里程碑。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| TypeScript / Rust | npm.cmd run typecheck、cargo check --offline通过；最终桌面构建也编译了最新源文件 |
| 必要信任边界 | cargo test claude_gate_：2通过、96过滤；cargo test codex_policy_is_fixed：1通过、97过滤。不跑无关全套单元 |
| 桌面构建 | npm.cmd run tauri -- build --debug --no-bundle通过，Vite15.84秒、Rust41.19秒；既有chunk大小警告保留 |
| 原生兼容专项 | 1通过，场景32.3秒/总34.8秒；真实WebView2/实际CLI/固定回复/独立QA库，新旧记录、输入/hash/用途/归属、停止、强杀恢复、手动清理及三尺寸浅深6图通过 |
| 只读交付核对 | integrity ok、FK空、schema11/迁移checksum不变；11条旧Claude/Codex检查与会话及事件逐列相等。6条Claude计划/事件hash正确，新探针源码hash相符；自有容器、临时策略文件无残留，普通聊天/知识/执行/评测数据计数不变 |

证据在artifacts/harness-claude-sandbox/和artifacts/harness-claude-sandbox-native/。人工查看1366浅色与1920深色截图，其他尺寸留图；CSS视口不等于系统缩放或读屏实测。真实模型回合耗尽、完整项目/跨文件、长期压力和更多模型未验证。实际命令、初次Docker未启动及各轮证据见[里程碑](milestone-log.md)。

## 剩余与实际阻断

本片固定程序/镜像/策略的参数与隔离门槛已验证，等待D3-3b验收。**D3-3c受控本地Messages通道和实际模型预算/唯一单题尚未实现**，因此真实编程仍阻断，不能把环境检查当执行授权。之后是D3-3d Aider、D4组合对照、D5报告与图谱。每批模型推理依AGENTS另弹窗确认，不沿用本片无模型许可。

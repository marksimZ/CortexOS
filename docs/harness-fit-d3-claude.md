# D3-3a：Claude Code 运行前检查

> 2026-11-03：本片已验收并继续交付[D3-3b沙盒条件](harness-fit-d3-claude-sandbox.md)。本文保留D3-3a当时的默认策略观察和阻断；新结果产生独立v2记录，不重算下面的v1历史。

2026-11-02。按[本片计划](harness-fit-d3-claude-plan.md)实施。**用户已批准固定工具下载及无模型检查，D3-3a 的程序准备、实际检查、停止与中断恢复已完成，停在本片验收。检查完成不等于任务已就绪：自动化参数、沙盒依赖和命名空间仍有明确阻断，真正内层沙盒和模型通道未验证。**

## 已实现

- 工作区内四个精确接口：读取专用镜像、读取检查历史、创建检查、停止/清理检查。不接受任意命令、URL、文件路径或保护策略。
- 直接选择已发布的 Python 函数及版本/hash，复用原 `preflight_task` 核对来源、人工审核与实际验证；不要求先安装宿主 Codex 或完成 Codex 检查。
- 以 `harness_claude_runtime` 复用 schema11 现有表，绑定工作区、分析/任务、固定程序定义、镜像、默认策略、端点与容器归属。每条事件带 hash，只追加；不写普通对话、知识、原项目或模型成绩。
- 两个固定内部用途共用原运行检查的事件、worker、停止、清理和终态；Codex 对外接口、v1/v2 记录、策略描述及原始时间保持。Claude 不接受或套用 `codex-bwrap-1`。
- 正常关闭清理两个用途的未完成检查；强杀恢复将 Claude checking 标为 interrupted，等待手动清理，不自动执行。实际 Claude 容器的停止、强杀恢复和手动清理已验证。
- 固定探针只运行 CLI 版本、帮助及环境观察，不传 prompt、项目文件、参考答案或个人认证。Docker 默认策略、无外部网络、非 root、只读根目录、无宿主挂载/Socket和原 CPU/内存/PID/输出限额保持。
- 将程序指纹、版本/参数、依赖、命名空间观察、真正内层沙盒及模型通道分开；本片 `sandboxReady=false`、`canRun=false`。依赖存在不等于安全运行验证通过。
- 任务页沿用折叠段、SelectMenu、FitStatus、AppDialog和现有样式；函数名/版本、专用镜像、检查及历史操作明确分开。镜像未准备时禁用检查，显示下一步，不生成演示记录。

## 工具准备

经用户明确批准，已下载官方 Claude Code 2.1.309 的 Linux x64 文件、发布清单、清单签名及公钥，仅放项目 `artifacts/harness-claude-runtime/`。程序 293,469,552 字节，SHA-256 `4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658`；实际校验大小、hash及官方签名通过，签名主公钥指纹 `31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE`。使用现有 Git GPG 和隔离公钥环，未读取个人密钥或登录。

`prepare_claude.py` 在文件已获准下载到项目缓存后，要求固定大小/hash、官方清单和隔离公钥环的签名校验，固定主公钥指纹 `31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE`；校验失败不执行程序或构建。它不下载安装器、不读取个人 GPG 或 Claude 认证。构建使用已固定的 Python 镜像，network=none、pull=false，无 RUN 安装步骤。

离线构建 `cortexos-claude:2.1.309` 成功，实际镜像 ID `sha256:55de5386970372e49b2d221e15d870e36902c1e5d3e37eab9118fe4e9919f32e`。沿用固定 Python 基础镜像，未下载 bwrap/socat、套用 Codex 策略或改动系统设置。准备脚本修正 Git MSYS GPG 对 Windows 路径的兼容：在受保护缓存目录使用固定相对文件名，保留全部签名检查。

## 实际程序观察

| 检查 | 观察与结论 |
| --- | --- |
| 程序指纹、版本 | 通过；`--version` 退出0，输出 `2.1.309 (Claude Code)` |
| 自动化参数 | 阻断；`--help` 退出0，但普通帮助中未列出要求的 `--max-turns`。仅记录公开帮助证据不足，不据此断言隐藏参数完全不受支持；未降低检查要求 |
| 沙盒依赖 | 阻断；bwrap、socat均未发现，本片没有安装 |
| 用户命名空间 | 阻断；默认策略下实际 `unshare(NEWUSER)` 返回 errno1；不借用 Codex 专用策略 |
| 外层容器条件 | 实际 UID65534、capEff/capBnd均0、noNewPrivileges=1、seccomp=2、无Docker Socket；配置核对无网络、无宿主挂载、只读根及原资源限制；OOM计数0 |
| Claude 内层沙盒、受控模型通道 | **未验证**；没有执行 Claude Bash 沙盒任务，通道未实现，`sandboxReady=false`、`canRun=false` |

## 本轮实际验证

| 项目 | 结果及范围 |
| --- | --- |
| TypeScript / Rust | `npm.cmd run typecheck`、`cargo check --offline`通过 |
| 桌面构建 | `npm.cmd run tauri -- build --debug --no-bundle`通过，已有 bundle 大小警告保留 |
| 必要边界单项 | `cargo test claude_gate_ --lib --offline -- --nocapture`：1通过、96过滤；构造依赖齐全也不得标记真正沙盒/模型通道已通过，指纹错误/混入模型调用拒绝。不跑无关全套 |
| Claude 未准备态原生专项 | 1通过，4.5秒/总6.4秒；真实 WebView2/独立 QA 库，确认空镜像、任务/hash/用途/工作区拒绝、旧 Codex 实跑记录不变、三尺寸浅深6图。没有 Claude 程序执行或模型调用 |
| 共用流程 Codex 回归 | 1通过，16.9秒/总18.8秒；原已批准环境、真实 Docker/无模型探针，旧v1/v2历史、停止、强杀恢复及清理通过。不是 Claude 隔离验证 |
| Claude 准备态原生专项 | **1通过，16.2秒/总19.2秒**；真实WebView2/独立QA库及实际CLI版本/帮助/环境探针；三条记录分别完成、停止、强杀恢复/手动清理，任务/hash/用途/工作区拒绝及旧Codex记录不变，三尺寸浅深6图/无横向溢出。流程检查通过不表示沙盒条件通过 |
| 准备态只读交付审计 | integrity ok、FK空、schema11及迁移checksum不变，三条Claude计划/事件hash正确且自有容器均无残留；8条旧Codex检查/会话及事件逐列相等，普通数据计数不变；两份Python语法检查通过 |
| 本地推理、付费云端、额外模型及全仓库任务 | **未执行** |

证据位于 `artifacts/harness-claude-runtime/`、`artifacts/harness-claude-runtime-prepared/`，以及此前的 `artifacts/harness-claude-runtime-empty/` 和 `artifacts/harness-claude-codex-regression/`。准备态专项实际截取1366×768、1600×900、1920×1080 CSS视口和浅深主题；人工查看1366浅色与1920深色，其他图留存，不等同于系统缩放或读屏实测。实际命令、初次失败与纠正见[里程碑](milestone-log.md)。

## 本轮文件

- `scripts/harness/prepare_claude.py`、`claude.Dockerfile`：固定来源/验签/缓存与离线镜像准备。
- `apps/desktop/src-tauri/src/benchmark/python/claude.rs`、`claude_probe.py`、`python.rs`：固定 Docker 检查及真实状态分类。
- `benchmark/harness/runtime.rs`、`runtime/{records,claude}.rs`：两个固定用途共用流程、Claude 任务准入及历史。
- `src-tauri/{build.rs,capabilities/default.json,src/lib.rs,src/db.rs}`：四个业务命令、关闭与中断恢复，无新迁移。
- `apps/desktop/src/benchmark/harness/{ClaudeRuntime,TaskWorkbench}.tsx`：增量界面及现有任务页入口，无依赖或 CSS 变更。
- `apps/desktop/tests/harness-claude-runtime.spec.ts`、`playwright.config.ts`：独立 QA 及显式准备态分支；无模型。
- 本片计划/说明及实施、数据、UI、测试、文档索引和里程碑。

## 当前剩余与实际阻断

1. **D3-3a已交付，等待用户验收**：固定程序下载/验签/离线构建、实际CLI/环境检查、停止和恢复、数据兼容与界面证据齐备。本片不以解除全部运行阻断为验收条件，不提供真实任务入口。
2. **D3-3b**：先核实未公开的预算参数，处理实际缺失的bwrap/socat和默认策略下命名空间限制，再验证Claude内层沙盒及严格失败策略。新增下载或策略变化按具体方案另行确认，不自动借用Codex策略。
3. **D3-3c**：受控 Anthropic 消息通道和预算，唯一真实单题；每批本地模型另弹窗。Ollama 文档不代替本机模型与4k配置兼容验证。
4. **D3-3d / D4 / D5**：Aider独立接入、组合对照、报告/图谱，继续分片。完整仓库/跨文件及云端不提前实施。

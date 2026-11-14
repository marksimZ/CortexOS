# D3-2a：Codex 镜像与真实沙盒门槛

> 后续进展（2026-10-31）：经用户批准的[专用策略及新版探针](harness-fit-d3-sandbox.md)已通过本机隔离检查；同时修正本页所述旧探针的多余linux参数及EROFS处理。原v1记录及本文的当时观察保留，不重算。D3-2真实任务仍未完成。

2026-10-31。D3-1 已获用户继续授权；本轮完成 D3-2 的工具准备和可复现的环境检查。**D3-2 真实编程任务尚未完成**：固定版本 CLI 的内层沙盒在现有 Docker 权限下无法建立，实际任务与模型调用保持未运行。下一片先解决兼容性，再另行确认模型调用范围。

## 当前结果与操作

评测 → 项目适配 → 4.测试任务 → Codex 执行前检查 → 选择已有检查 → **Codex 隔离检查**。

- 更新列表只读取本机镜像及已保存记录，不下载、不启动容器。
- “检查隔离环境”先显示确认窗，再运行固定的版本、参数和临时目录权限探针。没有传入项目代码、题目答案、个人登录或模型密钥。
- 真实观察：Linux Codex **0.166.0**；程序、附属文件指纹和自动化参数已核对。内层沙盒返回 `bwrap: No permissions to create a new namespace`，测试命令未执行，目录内允许写入/目录外拒绝写入均不能标为已验证。
- 检查流程正常完成与沙盒通过分别表达。界面显示“目录权限保护未就绪”，`sandboxReady=false`、`canRun=false`、`taskExecution=Not Run`、`modelCalls=0`。没有将环境阻断计入模型答错，也没有产生编程成绩。
- 每次检查新增独立历史。可停止、清理、查看原始输出和固定环境；重开只读取历史，不自动重查或运行模型。切换任务不混用记录。

## 镜像来源与实际边界

已获用户明确批准下载官方 Linux CLI 并构建镜像。官方 npm 包固定 `@openai/codex@0.166.0-linux-x64`，下载大小 **172325449 bytes**，SHA-512 与该版本官方 registry 元数据相符。完整性比较使用 SHA-512，未独立验证 npm 签名或构建来源证明，不把哈希核对称作签名验证。

`scripts/harness/prepare_codex.py` 仅从已下载且完整性正确的包中提取四个固定普通文件，不运行安装脚本或解包任意路径。二进制 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| bin/codex | b8a11fedf24b2dfb59a70c9e52b030f9a508dcd49cd2dd0d41956f4c785156b8 |
| codex-resources/bwrap | 77360cb751ccedc5971391444ac86a8a33c15b04d6b4a6fe45f5d25496e62c4c |
| codex-path/rg | e62198eb19b136b88c330af83647b5a962cb99b6b1f066758568f12de1974849 |
| codex-package.json | a734b829e73b56b59a1ed45e8669785c8e3d23290e60f40f4b5174f68e8ea7cd |

Dockerfile 复用现有 Python 3.13-slim，本机标签在构建前核对原固定 ID `sha256:2d44738ebf4ce5f63b9d9ea871e9c7cfdc0426f7b0c97cac915fd88ded1d86bb`；实际 BuildKit 记录绑定该 digest。构建 `--network=none --pull=false`，无 apt/npm 安装步骤。最终本机 Codex 镜像为 `sha256:71d63ab98eec120322e718fc0faf631cf1008bb3e938ee0554d830cecfbf480b`；没有更改用户宿主 Codex 安装，版本与原宿主预发行版分别保留。

探针复用原 `python::command/create_args/configuration_valid/cleanup_validation_container`，保持 network none、readonly root、UID65534、CPU1、256MiB、PID64、cap-drop ALL、no-new-privileges、无宿主目录和 Socket 挂载、默认 Docker seccomp。仅为固定测试增加 2MiB `/workspace` 和 4MiB 空 `/home/cortexos` tmpfs，均 noexec/nosuid/nodev；原 `/tmp` 16MiB 保留。空 CODEX_HOME 中没有个人配置或认证。

四个文件先全部核对 SHA-256，再启动 CLI。只执行 `--version`、`exec --help`、`sandbox linux` 下固定的临时写入探针；没有 `codex exec` 模型会话。每个 CLI 子命令8秒、探针整体30秒、原控制器总输出64KiB；创建、元数据和清理各自沿用5秒，整体30秒不包含这些前后操作。内层写入规则为 workspace-write，并排除默认可写 /tmp；没有开启全权限或旧沙盒回退。

## 数据与恢复

schema 仍11，复用 `benchmark_generation_jobs` 和 `benchmark_generated_cases`，用途 `harness_codex_runtime`。没有新表、迁移、依赖或锁文件变化。

- 创建前保存不可变计划和 hash：工作区、分析/检查/已审核任务版本、镜像、端点、owner 和准确容器名；原任务及 D3-0 快照不改写。
- 检查、错误和清理以带内容 hash 的观察记录追加保存。状态独立更新；清理未成功时保持 cleanup_required，不能标已结束。
- 一个数据库同时最多一个未结束的本用途检查，复用 TaskSlot 占用；跨工作区、跨用途、错误检查 hash、未确认输入在操作前拒绝。
- 应用正常关闭复用现有等待/取消流程，并清理本用途记录。异常退出后 checking→interrupted，不重跑；手动清理核对本地端点、名字、owner label 和固定镜像，不全局 prune。强杀/引擎离线仍可能暂留容器，记录保留。
- `get` 按当前工作区/分析/任务检查读取最近30条历史；活动记录帮助处理同工作区其他检查下的待清理项。清理完成不会改写原始探针和旧结果。
- 聊天、知识、execution_runs、普通 benchmark_runs 均不用于保存本用途结果。

四个精确业务接口：`get_harness_codex_images`、`get_harness_codex_runtime`、`check_harness_codex_runtime`、`cleanup_harness_codex_runtime`；前端不能提交命令、网络、挂载、宿主路径或模型参数。

## 修改文件与验证

- 工具：`scripts/harness/prepare_codex.py`、`scripts/harness/codex.Dockerfile`。
- 后台：新增 `benchmark/harness/runtime.rs`、`runtime/records.rs`、`benchmark/python/codex.rs`、`codex_probe.py`；修改 harness/mod.rs、python.rs、db.rs、lib.rs、build.rs、capabilities/default.json。
- 前端：新增 `src/benchmark/harness/CodexRuntime.tsx`，接入 ExecutionPreflight.tsx；复用当前 SelectMenu、AppDialog、FitStatus、列表和 Tokens，没有新 CSS 或页面重排。
- 检查：新增 `tests/harness-codex-runtime.spec.ts` 及显式开启的 Playwright 项目。说明/规范/实施计划与里程碑同步。

实际通过：TypeScript、离线 Rust 检查、桌面调试构建、单个必要判定边界专项、真实 WebView2 专项。原生专项核对实际版本/失败、假 hash/未确认/跨工作区/跨任务/跨用途拒绝、取消清理、历史不变、重开不重跑、测试进程强杀后中断与手动清理，以及1366×768、1600×900、1920×1080浅深六张截图。另只读核对独立库的用途隔离与 schema。没有无关全量单元。

最终证据：`artifacts/harness-codex-runtime-recovery/harness-codex-runtime-Linu-a2957-oped-cleanup-without-models-harness-codex-runtime/runtime.json` 和同目录六张 `runtime-{light,dark}-{width}.png`。命令、早期失败与验收窗口见 [里程碑](milestone-log.md)。

未运行：真实模型/Agent任务、CLI JSONL任务事件、模型预算/补丁自动交接及模型独立评分；沙盒环境门槛未通过，本轮也未申请或获得模型调用授权。Windows实际150%缩放、实体输入法和读屏未在本片重验。截图不替代这些人工检查。

## 下一片的明确前提

1. 先审查固定 Codex 与 Docker 默认 syscall 限制的兼容条件。当前证据只证明命名空间建立失败，尚不能排除或精确归因于 seccomp、内核、LSM 等具体层；不直接修改系统设置。
2. 若需调整容器安全配置，先形成可审阅的最小差异、风险和反向隔离检查，再单独请求用户确认。继续保留无网络、无挂载、无 Socket、非 root、资源限额及内层文件权限保护；不可满足时维持阻断。独立 Linux 环境是替代方案，也须明确安装范围。
3. 官方 [权限说明](https://learn.chatgpt.com/docs/permissions) 说明 Linux 文件系统限制依赖 bubblewrap，并限制旧 Landlock 路径；不把换旧开关当作本次兼容修复。[沙盒说明](https://learn.chatgpt.com/docs/sandboxing) 的系统设置建议不自动构成本项目授权。
4. 沙盒实测通过后再实现受控模型通道、请求/时间/输出预算与真实 JSONL，并弹窗确认实际模型和唯一审核任务的有界调用；不自动借用账号、现有密钥或下载模型。

本轮停在 D3-2a 环境检查验收；D3-2整体、D3-3及D4/D5均未完成。

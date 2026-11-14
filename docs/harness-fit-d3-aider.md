# D3-3d.1：Aider 运行前检查

2026-11-10。按 [本片计划](harness-fit-d3-aider-plan.md) 实施。用户已批准固定工具准备，实际模型调用另批确认。本片已完成无模型验证，停在人工验收。检查通过不等于已启用 Aider 项目执行。

## 实现范围

- Aider 0.86.2 使用独立 Python 3.12 工具镜像；原 Python 3.13 评分环境保持。
- `harness_aider_runtime` 复用 schema11 的检查计划、带 hash 事件、停止、清理和恢复。绑定当前工作区、分析版本及已审核发布的函数任务；不运行题目或改动知识、聊天、原项目。
- 四个精确业务接口：镜像列表、检查历史、发起检查、停止/清理。不接受任意命令、下载地址或额外保护策略。
- `ClaudeRuntime.tsx` 复用检查视图，分别导出 ClaudeRuntime 和 AiderRuntime，保留原 Claude 单题入口。Aider 只有检查入口，没有模型或项目执行按钮。
- 探针只运行实际 `--version`、`--help` 及可信外层容器检查。内层沙盒与模型通道单独标为未验证，`canRun=false`、任务 `Not Run`。
- 运行保护仍为非 root、只读根、无网络/宿主挂载/Socket、cap-drop ALL、no-new-privileges、默认 seccomp、CPU1、内存256MiB、PID64及有界输出/超时。准备容器的下载网络和1GB内存不用于运行检查。

## 修改文件

- `scripts/harness/{prepare_aider.py,aider.Dockerfile,aider_probe.py}`：工具准备、哈希锁定、离线构建、无模型探针。
- `apps/desktop/src-tauri/src/benchmark/python/{aider.rs}`、`python.rs`：固定镜像检查和结果分类。
- `apps/desktop/src-tauri/src/benchmark/harness/runtime/{aider.rs,claude.rs,records.rs}`、`runtime.rs`：固定用途、任务准入、共享历史及清理。
- `apps/desktop/src-tauri/{src/db.rs,src/lib.rs,build.rs,capabilities/default.json}`：恢复和四个业务接口；没有新迁移。
- `apps/desktop/src/benchmark/harness/{ClaudeRuntime.tsx,TaskWorkbench.tsx}`：增量检查区，复用现有样式和组件。
- `apps/desktop/tests/harness-aider-runtime.spec.ts`、`playwright.config.ts`：显式启用的无模型原生专项。
- 本片计划、交付说明、实施计划、里程碑和文档索引。

## 验证记录

| 项目 | 实际结果 |
| --- | --- |
| 依赖与构建 | 108 个 wheel 共 125,519,775 字节，逐文件对照官方 PyPI 文件名、大小、SHA-256 及非撤回状态；固定主包版本/hash。离线安装与 pip check 通过，未声称作者签名验证 |
| 前端 | 类型检查与生产构建通过；保留既有大包提示 |
| 后端 | 离线 check 通过，内嵌前端的桌面 build 通过 |
| Aider 版本 | 实际模块入口 `--version` 退出0，输出 `__main__.py 0.86.2`；已安装分发包版本0.86.2、Python3.12及依赖锁一致 |
| 参数 | `--help` 退出0，10项所需选项均出现；这不代替后续实际调用行为验证 |
| 容器 | 非root、无可用capabilities、NNP、seccomp、只读根、网络不可达、无Socket七项通过；实际Docker配置核对原限额、无宿主挂载和默认策略 |
| 首轮原生专项 | 1通过，30.7秒/总34.0秒；流程通过，但发现版本输出前缀误判，原始误报保留 |
| 修正后原生专项 | 1通过，18.0秒/总20.3秒；错误版本/镜像/确认/额外策略、跨工作区及跨用途拒绝，停止清理、强杀恢复、旧Claude与首轮Aider记录不变 |
| 数据审计 | integrity=ok、无外键违例、schema及迁移不变；23条旧任务及事件不变，其他业务表一致，3条新检查计划/事件hash正确 |
| 界面 | WebView2实际截取1366×768、1600×900、1920×1080浅深6图，无横向溢出；查看1366浅色和1920深色；不是系统缩放、读屏或手动拖动检查 |
| 清理 | 两轮自有检查与准备容器无残留；验收仅打开已保存结果 |

实际证据：`artifacts/harness-aider-runtime/`、`artifacts/harness-aider-native/`（首轮）、`artifacts/harness-aider-native-final/`、`artifacts/harness-aider-audit/` 和 `artifacts/harness-aider-acceptance/`。仅独立QA数据；没有调用本地模型、付费云端或下载模型，没有运行完整单测集。

专用镜像：`sha256:82b98ea28b075414873b52a67228fd9c0cb2b25ee564db0a3d7d673699f6cc3e`；依赖锁：`c956f9f95672c9303a7a0ed23aaa18822d2762f62435b5d0efe7d38f26cdd1c6`；基础镜像：`python@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df`。后端固定此镜像和锁，标签相同不代表任意重建镜像受信；后续更换需明确核对。Dockerfile的BASE参数在实际构建时已传入固定digest，仍有未设置默认值的静态提示。

准备期间保留Docker未启动、临时目录、日志解码、tmpfs复制失败证据。Docker cp不读取这次挂载内容，最终改用容器内有界归档导出、宿主逐项验证文件名后保存；归档删除。没有放宽运行检查权限。首轮版本误报只修正新检查的匹配规则，不回写历史结果。


## 剩余步骤

1. **D3-3d.1 已完成并打开验收窗口，等待用户验收。**
2. D3-3d.2：Aider 受控模型通道、固定回复验证、实际单题；真实推理先另弹窗。
3. D4：组合对照。
4. D5：报告与图谱。

实际剩余门槛：工具自身隔离未验证，受控模型通道与项目编辑/独立评分尚未接入；256MiB下版本和帮助可运行，不代表完整任务资源已足够。本片 `canRun=false`。未做本地推理、模型协议实测、全仓库或跨文件任务；下一片先做协议和固定回复检查，真实模型批次另弹窗。

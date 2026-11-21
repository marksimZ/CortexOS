# Phase H5 产品体验验收

## 交付范围

已交付首次使用引导、共用任务中心、按需帮助和相关页面定位。数据库仍为 Schema 26。未新增依赖、调度器或任务表。普通聊天和标准评测可全程使用本机模型。个性化评测保留有效 API 密钥准入。

## 内部走查

使用独立 Windows 桌面的 WebView2、测试工作区和固定回环回复。表内为自动化场景耗时，包含界面等待和部分样本准备。真实用户的求助次数、误操作和主观理解难度尚无数据。

| 场景 | 实测结果 | 用例耗时 |
| --- | --- | --- |
| 普通 AI 用户 | 识别本机模型，打开配置与备份入口，创建工作区并收到固定回复。从任务中心停止正在进行的回答，定位历史记录。合成 HTTP 503 保留可见错误，从原回答重试成功。另验证 Ollama 不可用时跳过引导，云端入口和个性化准入。 | 6.702 秒、2.505 秒。 |
| 知识管理用户 | 通过 Windows 文件选择器授权文件。验证导入快照、确认版本、旧证据、撤销后检索及无效向量恢复。使用模型组合对照核对引用及 3D 执行记录。 | 真实选文件 4.105 秒，来源快照 6.933 秒，知识版本 10.504 秒。两项检索 9.376 秒、7.206 秒。 |
| 开发者用户 | 实际选择项目规范文档，核对原文，查看并保存适配分析。标准评测从任务中心取消等待，运行中的批次保留资源。重新提交后完成处理，历史显示 8 / 8，跳回对应报告。 | 项目适配 15.234 秒，标准评测两项 47.715 秒、17.039 秒。 |
| 补充回归 | 两个隔离数据目录通过原生备份、恢复与重启。历史来源库升级通过，不合要求的文件被拒绝。 | 备份恢复 23.977 秒，来源库与文件边界 3.431 秒。 |

普通对话用例记录的步骤耗时为引导与设置 1.124 秒、新建并首次回答 0.513 秒、停止和定位 2.379 秒、失败和重试 0.673 秒、工作区隔离与帮助 1.003 秒。走查看到的主要问题是入口难找、任务状态分散和缺少直接返回路径，落地改动见 [产品体验](h5-product-experience.md)。

## 构建、运行与界面证据

- 在 `apps/desktop` 执行 `npm.cmd run build`、`npm.cmd run lint` 和 `npm.cmd run format:check`。类型检查、生产构建和格式检查通过，ESLint 为 0 个错误、63 条原有警告。
- 执行 `cargo check --manifest-path src-tauri/Cargo.toml --offline --locked` 与 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop`，通过。
- 通过 `artifacts/phase-h3/alternate-desktop-qa.py` 在非活动桌面运行 `playwright test --project=desktop h5-product-native.spec.ts standard-benchmark.spec.ts knowledge.spec.ts sources.spec.ts rag-native.spec.ts harness-fit.spec.ts data-portability-native.spec.ts cloud-native.spec.ts --workers=1`，13 项通过，0 失败、0 跳过。
- 通过同一非活动桌面入口复跑 H5、标准评测、项目适配和检索原生用例，7 项通过。核对旧项目分析版本定位，以及从任务中心回到原检索对照报告。
- `playwright test --project=edge personalized.spec.ts runtime-cloud.spec.ts --workers=1` 首跑发现 2 项切换工作区回归，原因是切换时丢失设置分类。修复后 5 项通过，包含三种视口下的个性化评测导入、发送预览、审核和报告，以及两条云端设置工作区切换路径。浏览器用例拦截 IPC，使用固定项目与凭据样本。
- 最终构建运行 `playwright test --project=desktop h5-product-native.spec.ts standard-benchmark.spec.ts --grep "H5|cancel resource wait" --workers=1`，3 项通过，覆盖引导、取消和任务历史的空状态。最终执行 `npm.cmd run lint -- --quiet` 通过。
- 检查 1366×768 浅色欢迎与任务抽屉、900×640 深色帮助。确认抽屉标题和关闭按钮可见，全局窗体按钮保持可见，Esc 关闭后焦点返回入口。项目适配与个性化连接另有 1366×768、1600×900、1920×1080 截图。测试数据、日志、截图位于 `artifacts/phase-h5/`。

## 资源记录

按入口 HTML 引用的 JavaScript 文件实测，H4 rc.3 为 530,276 字节，H5 最终开发构建为 537,599 字节，增量 7,323 字节（1.38%）。任务列表代码分包约 7.66 kB，打开抽屉才加载和轮询。3D 画布仍延迟加载，构建报告中画布分包继续超过 500 kB 提示阈值。本轮测量覆盖产物体积和功能耗时，尚无与 H4 同条件的进程占用、模型速度或大型历史库专项基线。

## 文件与接口

| 范围 | 修改文件 |
| --- | --- |
| 首次使用与布局 | `App.tsx`、`Chat.tsx`、`Knowledge.tsx`、`Settings.tsx`、`studio.ts`、`components/AppShell.tsx`、`components/EmptyState.tsx`、`components/WorkspaceSwitcher.tsx`、`styles/layout.css` |
| 任务与帮助 | `components/TaskCenter.tsx`、`components/TaskList.tsx`、`components/WorkspaceHelp.tsx`、`api/task-center.ts` |
| 评测记录定位 | `benchmark/BenchmarkPage.tsx`、`benchmark/PersonalizedWorkbench.tsx`、`benchmark/RagEvaluation.tsx`、`benchmark/RagRuntimeEvaluation.tsx`、`benchmark/harness/HarnessFitLab.tsx`、`benchmark/harness/HarnessExperiments.tsx` |
| 原生与权限 | `src-tauri/src/tasks.rs`、`src-tauri/src/task_center.rs`、`src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json` |
| 验证与记录 | `playwright.config.ts`、`tests/h5-product-native.spec.ts`、`tests/standard-benchmark.spec.ts`、`tests/rag-native.spec.ts`、`tests/harness-fit.spec.ts`、H5 三份交付报告、实施计划和里程碑记录 |

新增 `read_live_tasks`、`read_task_history`、`cancel_waiting_task` 三条主窗口命令。历史从原记录读取，取消等待需要匹配工作区、任务和任务种类，运行中的停止操作复用各业务命令。

## 后续验收与发布状态

H5 的可用性结论来自内部走查。真实本机模型和付费云端未调用，未重新运行 Docker 三工具真实任务或采集显存。H4 的隔离 Windows 安装证据对应 rc.3，H5 本轮验证对应 WebView2 开发构建。

正式发布继续暂缓。项目授权、正式 Authenticode、正式更新托管和物理多屏与 DPI 仍按 [H4 发布准入](../release/h4-release-checklist.md) 跟踪。当前交付等待维护者使用 H5 验收入口检查。正式安装包和远端发布仍由维护者单独批准。

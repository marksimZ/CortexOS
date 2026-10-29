# Benchmark 切片 5：耗时、Token 与资源

2026-10-28：用户确认本切片验收完毕。下一步为原计划切片 6；Docker、其他 GPU、读屏及完整 Smoke 等独立检查继续按下文记录保留。

2026-10-27。用户验收标准批量链路后授权继续原计划切片 5。本轮复用既有 Runner、评分器、批量任务与采样表，完成指标链路后停在验收门槛。Core60 v1/v2 题库及评分版本保持原样；Core60 仍是基础 Smoke 集。

## 本次交付

评测结果首页显示首字与响应用时中位数、生成速度；每个模型的“速度、Token 与资源”按需展开。单题可查看耗时、资源缺失原因及原始采样记录。复用浅深主题、表格、数字等宽和既有折叠组件，没有新增卡片布局或权限。

| 数据 | 来源与口径 |
| --- | --- |
| 首个可见文字 | 本机单调时钟，请求发送至首个非空白正文增量；不读取隐藏思维、不给工具调用冒充正文 |
| 首个工具响应 | 实际非空 `tool_calls` 帧首次处理时刻，独立于首字 |
| 响应用时 | 请求发送至成功、取消或错误终态；成功样本与全部已发送样本分别汇总 |
| 加载 / 处理输入 / 生成 | Ollama 终帧 `load_duration / prompt_eval_duration / eval_duration`，纳秒保存、毫秒展示 |
| 服务端总时长 | `total_duration` 原值保留于执行事件，与客户端响应用时分开 |
| 单题全程 | 从逻辑项准备至首次评分落盘的单调计时；另存准备、执行及保存、评分及保存分段；不含排队和模型分组间释放时间 |
| Token | 仅取终帧 `prompt_eval_count / eval_count / prompt_eval_cached_count`；流中间帧的计数不累加 |
| 生成速度 | 同一成功样本的输出 Token / 服务端生成时间；批次用完整配对样本的 Token 总和 / 生成时间总和，不拿端到端时间替代 |
| 请求前加载状态 | 实际预检 `/api/ps` 的已加载、未加载或未知及检查时间；按状态分别汇总，不宣称清除了系统/磁盘/输入缓存 |
| 整机 CPU | Windows `GetSystemTimes` 相邻读数差，内核时间含空闲时间；按当前处理器组总时间归一到 0–100%。超过 64 个逻辑处理器的设备须关注原始 `cpuScope` |
| 整机内存 | Windows `GlobalMemoryStatusEx` 的物理总量减可用量 |
| CortexOS 进程内存 | `K32GetProcessMemoryInfo` 工作集，原始采样及单题资源摘要保留；不包含 WebView2 和 Ollama 子进程 |
| 整卡显存 | 已安装 NVIDIA 驱动工具的 `memory.used`；按 GPU UUID 保存，包含其他程序，不是模型独占用量；`/api/ps size_vram` 仅随加载观察保留，不混为整卡峰值 |

无读数时为 `null / 不可用`。Token 合计附有记录的题次数；全缺失不再显示零，真实返回零仍是零，合计溢出保持不可用。缓存计数单独列出，不从输入数中擅自扣减。不同模型的 Token 不是等量文字。

## 采样和生命周期

仅本地 v2 / 个人本地评测启动采样。请求前采一份基线，默认每秒一次，响应结束再读 CPU/内存；采样范围包括请求前基线准备，不是只含模型生成的区间。首次 GPU 读取的准备耗时另存在 `resourceCollection.setupMs`；它包含在单题全程、不包含在请求响应计时。

显卡工具只查两个驱动安装位置，不执行工作区或任意 PATH 程序；固定只读参数，无 shell、无安装，隐藏控制台。每次查询限 750 ms、64 KiB、最多 64 个设备；停止/超时销毁本次子进程。不存在、超时、字段不支持等原因保留，不阻断正常答题评分。AMD/Intel 显卡读数本切片不可用。

CPU 首份读数仅作差分基线，使用率未知。原始采样记录相对毫秒、序号、来源、范围、采样周期、GPU 读取完成时刻、字段缺失；摘要包含采样数、字段有效数/缺失数、窗口和最大间隔。峰值只是已采到的最大值，短请求可能只有基线和终点。采样进程本身会产生开销，没有据此宣称模型独占资源或零扰动。

取消和错误也保存请求实际耗时、已观察首字和已有资源。没有完整终帧时，Token 与生成速度保持未知；这些项留在取消/错误统计，不进入成功延迟/生成速度分布。应用崩溃只能保留已落盘采样，不从系统时间或历史事件回填精确首字。

## 统计、隔离与历史

- 每项分布保存有效数 `n`、相对全部计划的缺失数、最小/最大、中位数和 P95。中位数在偶数样本取中间两值均值；P95 使用 nearest-rank。小样本不作稳定性能保证。
- 同时保留按重复轮次、请求前加载状态分组；不选择最快一轮作为最终成绩。生成速度只使用同一题中完整配对的计数与正时长。
- 正确率、执行覆盖率、执行成功率、评分覆盖率及六类状态继续复用既有确定性评分，不生成隐藏综合分。
- 采样按工作区和评测尝试关联，使用已有 `benchmark_resource_samples` 及不可变约束；硬件快照只写新运行。数据库仍为 schema 10，无迁移或历史表改写。
- 每题执行终态事件保留原始计时/Provider 字段；新增 `case_timing_recorded` 事件只记录真实单调计时。评分及旧执行事件不回写。
- 新报告的 `performance.version=1` 标识本次指标投影；旧记录按已有数据读取，旧终帧可计算有依据的速度，缺少客户端时刻或采样时保持未知。历史评分不重算。云端非流式返回无法测首字/本机模型资源，不作补造。

## 修改文件

以下路径相对 `apps/desktop/`：

- `src-tauri/src/benchmark/resources.rs`、`telemetry.rs`、`telemetry/tests.rs`：Windows/GPU 读取、受限采样与持久化、统计及必要边界专项。
- `src-tauri/src/benchmark/mod.rs`、`repository.rs`、`runner_v2.rs`：采样模块接入、新硬件快照、受归属校验的详情读取、逻辑项计时。
- `src-tauri/src/generation.rs`、`ollama.rs`：本地评测计时、全终态保存、空白增量处理、真实终帧输入耗时与缓存数。
- `src-tauri/src/benchmark/personalized/report.rs`：两类评测共享投影，修正速度和缺失 Token 合计，重复/加载分组及资源摘要。
- `src-tauri/Cargo.toml / Cargo.lock`：扩展已有 Tokio 的受限子进程读取及 Windows API features，新增一个间接锁定包 `signal-hook-registry`；无系统软件安装。
- `src/benchmark/PerformanceDetails.tsx`、`PersonalizedReport.tsx`、`personalized.ts`、`benchmark.css`：摘要、按需细节、原始记录读取及类型；沿用现有状态和样式。
- `tests/standard-benchmark.spec.ts`、`benchmark-metrics-real.spec.ts`、`playwright.config.ts`：受控计量、取消恢复、浅深截图、可显式启用的真实小范围验证和存档复查。

## 验证与验收

类型检查、前端及 Windows 调试桌面构建通过；必要的计量/存储 3 项、三尺寸浏览器个人流程 3 项通过。最终 WebView2 指标/恢复专项及真实存档复查 2 项通过；另有既有聊天取消/错误/重启回归 1 项通过。已查看实际截图，覆盖 1366×768、1600×900、1920×1080 浅深主题。

真实样本为当前已安装 `qwen3:4b-instruct` 的 `math-001` 两次，均回答 `95`，观察到输入 174 / 输出 6 Token、30 份资源采样。第一次请求前未加载，首字 24231 ms、请求响应 26008 ms；第二次已加载，首字 36 ms、响应 64 ms。这是两份原始观测，不是稳定速度或模型能力结论。后续截图失败修正均重用这两份存档，没有追加模型请求。

实际命令、结果及脚本失败修正见 [里程碑](milestone-log.md)。必要验证覆盖缺失与零、配对速度、缓存/终帧不重复计数、空白增量、采样用途隔离与不可变、重启历史、停止/恢复和普通聊天。

真实本地仅使用当前列表选出的一个模型、一题两次，保存原文及原始字段，不代表跨模型排名或完整 60 题验收。没有安装 Docker、下载模型或发起付费请求；真实容器隔离、其他 GPU、系统读屏和新评测页的 150% 人工验收仍待执行。现有外观菜单在鼠标选择后观察到仍展开，截图通过 Esc 收起；本切片未改共享菜单，留到界面切片复核。已有 Vite 大块提示及大计划详情读取成本保留到后续页面切片。

验收入口：标准评测 → 运行与历史 → 打开本次结果 → 速度、Token 与资源；展开某题 → 本题耗时与资源 → 查看原始采样记录。核对未知值、停止后用时与评分覆盖率可分别查看。完成本切片后暂停，下一步是原切片 6 的报告比较与大列表交互，不自动进入导出/复算或完整模型验收。

协议依据：[Ollama Chat 终帧字段](https://docs.ollama.com/api/chat)、[Windows CPU 时间](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes)、[Windows 物理内存](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)、[进程工作集](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo)、[NVIDIA SMI 字段及不支持值](https://docs.nvidia.com/deploy/nvidia-smi/index.html)。

# 检查与验收

## Benchmark 8 真实本地完整计划

显式设置 `CORTEXOS_BENCHMARK_ACCEPTANCE_REAL=1`，运行 `benchmark-acceptance-real.spec.ts --project=benchmark-acceptance-real`。只选择实际目录中至多两个不同指纹的已安装模型，使用独立SQLite/Profile，先两题两次重复并停止/恢复，再固定Core60 v2全计划。原文、各类状态、分母、资源与配置均保留，不断言模型必须答对。Docker缺失按真实环境阻断；不下载模型、安装Docker或执行宿主代码。

检查源运行全部终态、已完成项恢复时的runId不变、普通聊天为空及全局设置不变。全量已保存输出的独立重新评分和JSON/CSV系统保存可复查。若输出已完成而检查脚本后段失败，设置 `CORTEXOS_BENCHMARK_ACCEPTANCE_SAVED` 为仓库artifacts下原目录，复用其data、small.json及report.json，仅复查保存数据，不重复模型生成；新结果目录与原目录分开。该模式仍追加一轮显式独立评分，原分数不覆盖。

实际结果及Docker/跨厂商模型/系统缩放等环境门槛见 [切片8](benchmark-acceptance-slice8.md)，不把Core60 Smoke当能力排行榜。

## Benchmark 7 导出与独立重新评分

先审阅工作区/用途/批次归属、源数据指纹、追加事务及原评分选择。仅运行必要的 `report_transfer` 两个数据库/文件专项，验证取消/失败不写、旧成绩保留、全部尝试、哈希、CSV 转义及公式保护、结构化认证元数据剔除、拒绝覆盖和临时文件清理，不运行无关单元套件。

浏览器复用单题及个人向导检查，覆盖导出取消/失败重试、独立评分确认/取消/切页清理、三尺寸浅深布局。原生标准批次专项使用受控 HTTP 真实保存的8次回答，核对追加8条新评分但原统计与模型请求数不变，轮次定位正确；系统保存窗口实际保存 JSON/CSV 并取消。UIA 脚本使用本机现有 `pwsh.exe`（PowerShell7），只操作指定进程及仓库内路径；本机旧 Windows PowerShell 无法取得完整保存控件，不将它的失败记录为通过。JSON/CSV 实际解析核对结果。自动化环境依赖不增加产品运行依赖。

实际命令、失败修正及截图见里程碑和 [切片7](benchmark-report-slice7.md)。Docker实测、真实付费模型、长时跨硬件、磁盘拔出/断电、系统150%与读屏另列未测；内容视口检查不代替系统缩放。

## Benchmark 6.5 页面与性能专项

仅前端增量，先逻辑审阅、类型检查及调试构建，不新增或运行单元套件。`benchmark-result-drawer.spec.ts` 补加载计数、三尺寸浅深长内容、固定栏、菜单/Esc/焦点；`personalized.spec.ts` 使用长文件名检查扩展名保留；复查共享菜单和导航。

新增原生 `benchmark-report-usability.spec.ts` 使用独立 SQLite 的 900 项未运行计划（3 模型×60 题×5 次），各读取 5 次概览/页面/单题，记录 WebView2 内部 IPC 等待时间及序列化后 UTF-8 字节，另记录含自动化往返的翻页耗时。验证 60 个可见列表项、统计不变、读取时零模型请求，再发送一条受控未结束响应检查滚动后的停止入口。测试 HTTP 端点不是真实模型。实测结果、样本数与环境边界见 [6.5](benchmark-report-slice6-5.md)。

最终原生检查复用 300 项定位与标准批量/实际执行图/停止恢复专项。两组 Playwright 若共用 1420 开发服务须顺序运行，避免服务所有者退出导致另一组连接拒绝。实际系统缩放和读屏保留人工检查，不以 CSS 缩放或内容视口截图替代。

## Benchmark 6.4 导航与返回定位专项

先审查 URL 参数、异步响应身份和所有只读查询的工作区/用途边界。只运行必要的两个 `cargo test report_ --lib --offline -- --nocapture` 数据库专项，不跑完整单元套件；新增定位命令需要验证执行与题目配对、跨域拒绝、筛选后页码和不匹配状态。

浏览器专项覆盖复制/打开内部链接、刷新、前进后退、错误链接、晚到批次、个人入口重载及无自动任务调用。原生专项以真实 SQLite 的 300 项计划核对末题定位、筛选恢复和关闭焦点，以受控模型 HTTP 响应核对真实已保存执行图往返、原样输出及停止/恢复。共享弹窗或导航修改后按需复查全局窗口按钮、知识历史与既有菜单。三尺寸浅深截图是应用内容视口检查，不等同于 150% 系统缩放或读屏验收。实际命令和失败过程见里程碑与 [6.4 交付](benchmark-report-slice6-4.md)。

## Benchmark 6.3 单题详情专项

先审阅只读调用与响应身份；无需新增单元测试。`benchmark-result-drawer.spec.ts --project=edge` 使用可控晚到请求检查切题、长原文、读取/运行错误、未运行/不支持和焦点；`ui-usability.spec.ts` 回归开发 StrictMode 下的共享弹窗与菜单。

Windows 调试构建后，`benchmark-report-pages.spec.ts` 覆盖实际 SQLite 的 300 项分页、页内切题、固定资料及三尺寸浅深抽屉；`standard-benchmark.spec.ts` 保留受控标准批次、已保存评分/采样和停止恢复。共享 AppDialog 有修正时按需补 `ui-usability-native.spec.ts` 及 `window-drag.spec.ts --grep "native dialogs"`，不把它等同于物理鼠标拖动通过。实际命令、结果与截图见里程碑。

## Benchmark 6.2 与弹窗专项

按需要运行 `cargo test report_ --lib -- --nocapture` 两项数据库专项，验证筛选在分页前完成、固定轮次/题库元数据、空匹配、非法参数及隔离；不扩大为全部单元测试。原生 `benchmark-report-pages.spec.ts` 覆盖 300 项组合筛选/重置/三尺寸主题；`standard-benchmark.spec.ts` 使用受控模型检查缺失速度排序、同分稳定次序、模型详情筛选不改全计划表和停止恢复。

`window-drag.spec.ts --grep "native dialogs"` 独立检查弹窗下移、正文 inert、标题栏真实命中、最大化/最小化/还原/全局关闭、各页 Esc 和焦点；`ui-usability-native.spec.ts` 保留长记录/历史/嵌套草稿/来源菜单。物理鼠标拖动测试仍单列原有专项；本轮命中另一 Edge 窗口而被归属保护拒绝，不把其他通过项当作其通过。实际结果见里程碑。

## 窗口标题拖动专项

调试构建后可运行 `npx.cmd playwright test window-drag.spec.ts --project=desktop`。使用独立数据库，原生鼠标和实际窗口坐标检查五页标题/聊天空白、双击、最大化/还原、最小化和关闭；辅助脚本只操作此检查启动的窗口，并恢复光标。此专项会短暂置前测试窗口，按窗口交互改动需要执行，不属于每次修改的必跑项。实际结果见 2026-10-28 里程碑。

## Benchmark 6.1 只读与分页专项

`cargo test --lib report_paging --offline` 核对完整/概览的统计和性能一致、回答不进入列表、分页范围与归属、未运行详情及普通聊天隔离。`benchmark-report-pages.spec.ts --project=desktop` 使用真实 Tauri/SQLite 的 300 项未运行计划，验证 15 页、每页 20 项、单题展开及三尺寸浅深截图；`standard-benchmark.spec.ts` 检查已完成回答、计量、采样与停止恢复兼容。`personalized.spec.ts --project=edge` 保留三尺寸浏览器流程兼容。按需读取触发由组件挂载/effect 代码审阅及实际返回内容共同检查，不依赖修改 WebView2 内置 invoke。实际命令与结果见里程碑。

## Benchmark 切片 5 专项

先检查代码逻辑。计量与存储边界使用 `cargo test --lib telemetry --offline`，包含缺失/零、配对速度、分位数、溢出、GPU 不支持字段、工作区隔离、追加约束与重启历史。前端类型/构建及原生 `standard-benchmark.spec.ts --project=desktop` 检查终帧不重复、空白不算首字、缺字段、取消/恢复、原始采样查看与三尺寸浅深截图。共享执行改动另做 `chat.spec.ts` 必要回归。

真实小范围显式设置 `CORTEXOS_BENCHMARK_METRICS_REAL=1` 后运行 `benchmark-metrics-real.spec.ts --project=benchmark-metrics-real`，使用当前实际模型列表，一题两次、64 输出 Token 请求上限，不下载或释放模型。已有记录复查可设置 `CORTEXOS_BENCHMARK_METRICS_SAVED` 为该检查结果目录，仅重开其独立数据库和核对采样，不重复调用模型。实际结果及脚本失败修正在 [里程碑](milestone-log.md)，边界见 [指标交付](benchmark-metrics-slice5.md)。

> 阅读说明（2026-10-22）：本页是检查方法及分阶段覆盖记录，不是每次修改的必跑清单。按 [开发指南](../CONTRIBUTING.md) 选择相关专项；纯文档变更检查事实、链接和差异。实际运行结果以 [里程碑](milestone-log.md) 为准。

2026-10-23 验收补充：用户已在Windows150%系统缩放下验证所有功能，反馈无问题，该项通过。真实读屏、干净系统安装和长期真实模型等剩余检查继续保留。下文各阶段的“未测/待验”保留当时事实；当前状态见[Windows交付记录](windows-delivery.md)。

## 当前可运行命令

Core60 v2阶段B：`cargo test --lib scoring_ -- --nocapture`运行评分正反控制、分母及存储专项，其中真实Docker检查明确ignored。已有数据/用途兼容检查用`cargo test --lib benchmark_ -- --nocapture`，共享帧解析用`cargo test --lib ollama::tests -- --nocapture`。调试构建后设置`CORTEXOS_BENCHMARK_RUNNER=1`，原生`--grep 'native v2'`覆盖14种受控场景；它不是完整60题模型成绩。Docker尚未安装，用户决定保留实测待办，不回退宿主执行；具体命令和边界见[阶段B](benchmark-scoring-stage-b.md)。下段保留切片3检查历史。

Benchmark切片3：必要数据专项为`cargo test --lib benchmark_development -- --nocapture`。原生专项在调试构建后设置`CORTEXOS_BENCHMARK_RUNNER=1`，执行`npx.cmd playwright test tests/benchmark-runner.spec.ts --project=benchmark-runner`；默认只跑受控服务，用`CORTEXOS_BENCHMARK_REAL_RUN=1`另行启用一次真实本地模型问答（180秒请求上限、64输出Token请求值）。首次实施已获用户确认并执行。共享执行变化还需相关聊天/知识/检索/来源/执行图回归；实际命令和结果见里程碑。不运行批量或代码题。

Benchmark切片2：必要数据检查为`cargo test --lib benchmark_bundle -- --nocapture`；原生检查先构建调试应用，再设置`CORTEXOS_BENCHMARK_PREFLIGHT=1`运行`npx.cmd playwright test tests/benchmark-preflight.spec.ts --project=benchmark-preflight`。仅当明确允许只读访问本机模型时再加`CORTEXOS_BENCHMARK_REAL_MODELS=1`，会读取实际两个Qwen身份，不下载/生成/卸载。默认测试集不运行该专项；题目人工审核、实际能力探针和评分执行另验。实际结果见里程碑。

Benchmark Phase 1切片1的必要检查已完成：`src-tauri`中执行`cargo test --lib benchmark_foundation_tests -- --nocapture`及单项既有聊天恢复测试；`apps/desktop`中执行`npm.cmd run tauri -- build --debug --no-bundle`，再运行聊天/知识/检索/来源/执行图五项原生专项。覆盖用途拒绝、空库/7→8升级、备份失败、事务回滚与备份恢复、旧数据逐表相等和中断对账；细节和实际失败修正见[milestone-log](milestone-log.md)。全部使用隔离数据，没有运行完整测试集、Docker或真实模型基准。

从 `apps/desktop` 执行：

```powershell
npm.cmd ci
npm.cmd run typecheck
npm.cmd run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm.cmd run tauri -- build --debug --no-bundle
npm.cmd run test:smoke
```

`test:smoke` 先用已安装 Microsoft Edge 检查网页，再启动刚构建的桌面 exe，连接该测试进程的 WebView2 调试端口检查真实窗口。桌面测试需要先构建 debug exe。只运行浏览器检查可加 `-- --project=edge`。测试使用临时浏览器配置，不访问用户的浏览记录或个人配置。

## 自动检查范围

- 四页导航、当前项、页面标题、直接路径、刷新、返回和未知路径回退。
- 键盘跳转主内容、导航后聚焦、图谱类型键盘切换。
- 浏览器明确为预览，发送与连接功能禁用；桌面实际接入业务。
- 1440×900、1280×800、1024×768、800×600、375×812 四页均无页面级横向溢出，主导航始终可达。
- 真实桌面窗口加载生产资源、四页可切换、生产 CSP 存在，浏览过程无脚本错误或控制台错误。
- 每个窗口尺寸保存页面截图；桌面测试保存原始 viewport、pixelRatio 和 CSP 记录。截图与报告在忽略目录 `test-results` 中，不混入源码提交。

Rust 检查使用真实临时 SQLite：迁移、工作区隔离、独占数据目录、事件不可修改、唯一终态、迟到检查点及中断恢复；另检查 URL 约束、UTF-8 分片、同一分片中的有效与畸形 JSON。测试不会访问正式工作区。

原生聊天集成检查使用隔离 SQLite 和可控 loopback HTTP 服务，覆盖完整回答、重命名、跨工作区生成与草稿、停止保留、畸形流保留、重试不重复上下文、强制退出恢复、连接失败与重连、30 秒超时，以及云端模型别名和未完成下载的模型被阻止。测试服务只用于重现错误分支；真实模型效果需单独验证。

输出预算耗尽单独检查：完成帧中的 `done_reason=length` 保存到执行事件并投影到消息，显示“达到输出上限”，可提高参数重试；截断轮次不重复纳入下一次请求。

正常关窗检查通过 Windows `CloseMainWindow()` 作用于测试自行启动的独立进程，验证生成先取消并保存，重启后为 cancelled；强制结束测试进程则恢复为 interrupted。新建会话期间输入保持禁用，切换完成后再接受输入，防止文字在异步切换中被覆盖。

## 真实本地模型

先启动官方 Ollama 并完整下载 `qwen3:4b-instruct`。执行 `ollama show qwen3:4b-instruct` 核对实际量化；服务仅监听 127.0.0.1，关闭云端功能。从应用目录执行：

```powershell
$env:CORTEXOS_REAL_OLLAMA='1'
npm.cmd run test:smoke -- --project=ollama
Remove-Item Env:CORTEXOS_REAL_OLLAMA
```

检查真实回答、实际输出 token 统计、停止第二次生成、重新打开后完整与部分回答一致。使用独立测试数据库、WebView2 配置和合成问题。截图与原始性能 JSON 保存于根目录 `artifacts/phase-1`；时间是单次冷/热运行的观测，不是稳定基准。配合 `ollama ps` 和 `nvidia-smi` 记录模型加载与 GPU 瞬时读数。

默认真实模型测试保留现有 Ollama 服务。仅当测试调用方明确管理该服务时，额外设置 `CORTEXOS_MANAGED_OLLAMA_PID` 为所启动服务的 PID；测试核对其可执行文件与端口归属后，停止该进程，验证真实断连提示，再以 loopback、关闭云端和单任务配置恢复后台服务。恢复后的 PID 写入验收 JSON。该选项不加入日常默认命令。

开发模式复测可以设置 `$env:CORTEXOS_SMOKE_DEV='1'` 后运行 `npm.cmd run test:smoke -- --project=desktop`。测试复用本机 Vite 服务并执行 `tauri dev --no-watch`；检查真实开发窗口导航和 Vite 直接响应的策略，后者不等同于原生代理响应的 CSP。完成后清除环境变量并重新构建独立 exe，再运行默认完整检查。

## 手动验收项

1. 可手动运行 `npm.cmd run tauri -- dev` 观察开发窗口；该启动路径也由上述开发模式自动检查覆盖。发布时是否出现控制台窗口需要另做 release 验收。
2. 在 Windows 系统设置中分别选择 125% 与 150% 缩放，重新打开应用并检查文字和主操作；记录实际结果。浏览器 viewport 变化不能替代这项实测。
3. 用 Windows 键盘和读屏检查焦点顺序及中文播报。自动 DOM 检查不能代表完整读屏验收。
4. 对实际截图检查对齐、空白比例、正文宽度及窄窗口布局。

## 后续阶段

Phase 2B 增加关系修订与 3D 操作检查；Phase 3 增加事件投影和目标 GPU 图谱性能；Phase 4 增加文件权限、引用、版本重放及备份恢复。

实际命令、失败与限制统一记录在 `milestone-log.md`。


## Phase 2A 知识检查

Rust 的 knowledge 测试覆盖：草案/生效指针分离、并发 revision 冲突、插入失败后的事务回滚、旧版不可变、审批记录不可变、工作区隔离、来源摘录校验、重启保存；升级检查从真实 v1 schema 和已有工作区开始，验证备份保持 v1、升级保留数据，并拒绝早期迁移 checksum 不匹配。

`npm.cmd run test:smoke -- --project=desktop knowledge.spec.ts` 运行隔离原生知识闭环：受控回答中选择连续片段 → 保存草案 → 审批 → 修改草案保留旧生效版 → 窗口关闭继续编辑 → 模拟另一处修改 → 对照最新内容解决冲突 → 版本切换 → 原会话回溯 → 工作区隔离 → 搜索 → 放弃修改 → 重启 → 窄视口详情/编辑 → 手动新建 → 关闭前保存 → 重启后待确认记录存在。使用正常桌面权限运行 WebView2；375px 是 viewport 模拟，不等于 Windows 系统缩放。

知识内容和测试响应均为确定性合成数据。此测试不评估实际模型质量，也不依赖正式工作区。


## Phase 2B 图谱检查

Rust graph 检查覆盖关系创建/重复拒绝/修订/停用、跨工作区拒绝、旧预期版本拒绝、版本不可变、插入失败事务回滚、有限坐标/重复位置验证、布局乐观并发、重启数据一致性和位置保存不增加知识版本。

新增两个图谱检查：

- Edge 演示：等待实际 WebGL 首帧、按已知相机投影点击画布节点、聚焦/适应、按类型筛选、闲置 250ms 帧计数不增长，以及注入 contextlost 后使用列表查看详情。
- 原生桌面：隔离工作区创建 7 条知识，添加关系、正常关窗继续编辑、位置保存、模拟并发关系修订并比较、图谱中修改/确认知识、旋转并保存视图、停用关系、工作区切换、状态筛选、375px 详情布局、WebGL 降级和重启恢复；重开后不移动相机再次保存，逐轴核对相机与目标点。

使用 `npm.cmd run test:smoke` 运行完整检查，图谱专项使用 `npm.cmd run test:smoke -- graph-preview.spec.ts graph.spec.ts`。graph-observation.json 记录单次 7 节点载入观测和原生渲染设备，不作为大图基准。100/500/1000 节点与真实模型并行性能、完整读屏及 Windows 150% 缩放仍待后续验收。


## P0-A 已加入的检查

Rust context_tests：生效 v1 / 草案 v2、审批冲突、重复和跨工作区、预算限制、预览不写库、快照等值/哈希/不可变、注入快照写入失败的事务回滚、恢复中断、v3 升级旧运行不补造快照、完整性与外键检查。

tests/context.spec.ts：原生 WebView2 选择知识→实际模拟 HTTP 捕获与快照逐项相等→审批冲突保留问题→刷新选择 v2→预览→下一次请求使用 v2→模型检查失败无 chat 请求→重启保留 v1 快照。tests/context-real.spec.ts 使用 CORTEXOS_REAL_CONTEXT=1 独立开启，运行已安装真实 Ollama，保存观察记录与截图。既有 chat 测试的事件预期更新为真实边界；图测试补 HTMLCanvasElement 类型标注。

执行记录、浏览器/原生/真实模型结果分别见 milestone-log。尚未实测新面板的 Windows 150% 缩放与屏幕阅读器，不以浏览器尺寸检查代替。


## P0-B 检查

Rust `execution::tests` 覆盖工作区隔离、分页、视角并发/无效参数、知识视角独立性、完成/失败/取消及恢复中断的读取、重启和外键完整性。

`tests/trace.spec.ts` 在原生 WebView2 验证：事件与投影逐项对应、legacy 无补造上下文、v1运行在知识更新到v2后仍定位v1、原消息键盘焦点、返回节点、保存视角、WebGL降级、375px布局、生成中刷新、跨工作区拒绝、关闭模拟服务后重启离线读取。`context-real.spec.ts` 增加真实模型回答到执行画布和知识快照检查，使用 CORTEXOS_REAL_CONTEXT=1 开启。

大图基准、Windows150%缩放和完整读屏待专项验收；浏览器窗口尺寸检查与原生测试分别记录。

## P1-A 验证范围

retrieval.rs固定查询集对比中文短词方案，并检查8个中文/英文/代码问题的预期首项；另测无词、无命中、查询语法字符。事务测试覆盖审批更新/草案排除/跨工作区隔离/手动重复/自动预算/手动超预算/关闭开关/不可变候选/注入候选写入失败/索引更新失败审批回滚/重启快照与开关。v5结构迁移测试验证生效版本回填、备份、20候选与5自动条数上限和只读预览。

原生retrieval.spec.ts捕获HTTP请求，与快照逐项比较，验证自动v1→审批v2、历史快照不变、检索图节点、关闭后无检索事件、会话偏好重启与新会话默认。context-real.spec.ts改为不手动勾选资料，实际Ollama必须使用自动纳入的知识。其他聊天/知识/执行图测试更新真实事件预期。实际执行结果见milestone-log。


## P1-B1必要集成检查

按用户新规范不额外跑单元测试。sources.spec.ts通过原生IPC和实际UI验证：选择/预览时不落库，确认保存当次预览而非静默重读、UTF-8规范化、固定行证据、新来源版本与旧知识独立、审核及请求等值、停用后自动/手动拒绝、历史不变、重启/重新授权、票据作用域/重放/取消、通用fs IPC无授权。旧v6数据库夹具检查升级保留知识及外键/完整性，拒绝非UTF-8、超限、空文本、敏感名称、重解析路径。

前两条使用debug隔离文件选择替身；第三条使用Windows UIAutomation操作同一测试进程的真正文件选择器，并检查1366×768和1920×1080来源窗口。测试文件均在隔离目录，敏感命名用合成夹具，不访问项目根目录真实令牌。实际命令和通过结果见milestone-log；未执行项不得据此声称通过。

## P1-B2 必要整链路检查

不运行单元或全量测试。citations.spec.ts 一条原生 WebView2 场景使用隔离数据库/合成文件和可控本地 HTTP 服务，捕获实际请求并比较保存请求及引用知识。覆盖全文/指定行/重复行、中文与 emoji 的 UTF-16 位置、未选知识编号/越界/错误起点/残缺标记、无引用提示、流式期间不显示已检查引用、取消保留部分输出、知识版本入口与返回焦点、真实位置检查事件在执行图展示。

更新知识到v2、文件到v2并停用后，重启仍必须读原知识v1和文件v1；跨工作区 get_run/get_source 拒绝。检查1366×768、1600×900、1920×1080并保存真实截图，Escape关闭后恢复触发焦点。仅证明处理链路及定位正确，不证明模型论断正确或保证模型遵守引用格式。旧协议和失败/启动恢复共用路径另外做代码逻辑检查；本轮未重跑真实Ollama、旧迁移、系统150%缩放或完整读屏。

## P1-C1 交互与绘制基线

graph-neighborhood.spec.ts一条原生场景通过作用域命令建立58节点、有效/停用/两跳关系，验证50项分页与范围、筛选及临时显示、端点选择/焦点、跨页保留/工作区隔离、WebGL降级、1366/1600/1920窗口及375px内容视口，最后比较完整GraphData确认浏览未修改存储。375px不代表原生外窗可小于最小宽度。

graph-baseline.spec.ts只在CORTEXOS_GRAPH_BASELINE=1时出现在graph-baseline项目；隔离合成规模为100/1000、500/1000、1000/5000节点/关系。seedGraph使用现有Rust命令，不直接写数据库。记录三次读取往返中位数、首画面、空闲绘制数、90步自动拖动绘制次数、渲染器主线程累计任务时间、JS堆和上一帧绘制提交数，输出JSON/截图。自动拖动速率受输入调度影响，不等于屏幕FPS；JS堆不等于进程内存或显存。不运行模型生成或推导完整CPU/GPU指标。本轮不跑单元或全量测试，实际命令见日志。

同模块原graph.spec.ts同步当前列表和关系折叠入口及标点，保留原有关系冲突、知识修订确认、视角保存和重启断言。清除筛选会移除按钮，焦点须回到首个可见节点（空列表则回列表按钮），避免键盘后续操作丢失。此路径属于本轮必要相关回归。

分页检查同时验证焦点进入新增的首个节点/关系摘要；最后一批关联展开完成及返回全部节点时，焦点分别回到范围返回按钮/列表按钮。

## P1-C2a 必要验证

graph-instances.spec.ts使用8个固定位置的隔离节点，鼠标按相机投影点击所有6种形状，检查实体ID、同类型实例筛选重排、移动后旧位置不命中/新位置命中、单击聚焦及Esc恢复原探索视角。该用例显式选择立即切换以固定投影；不把它作为动画平滑度验证。原graph/graph-neighborhood/trace专项保留业务断言，适配V3显式展开筛选及适应全图退出详情；列表Esc不得退出节点聚焦。

graph-baseline支持CORTEXOS_GRAPH_FIXTURE_DIR重用同一隔离库、CORTEXOS_GRAPH_VARIANT标记版本，记录完整GraphData哈希。V3版适应全图后等待500ms，再测350ms空闲；更新前数据保留原方法。不同视觉版本单列，完整方法与局限见性能报告。

CORTEXOS_REAL_GRAPH=1启用real-graph项目。graph-model.spec.ts使用已有Ollama与qwen3:4b-instruct，先短生成预热，再以默认512 Token上限输出纯数字合成表格；CORTEXOS_GRAPH_OUTPUT_LIMIT=2048用于独立长回答压力场景，结果分别保存。断言拖动画布和类型筛选前后仍在生成、文本增长、最终事件与请求配置落盘、GraphData哈希未变化。保存实际视口/DPR、宿主与WebView2累计CPU及工作集/私有内存、CDP主线程/JS堆、Ollama加载信息与设备整体GPU/显存。仅停止本检查启动的服务，不改变用户默认服务配置。不是模型答案正确性或屏幕FPS检查；流式更新长时开销、模型进程CPU、整机归因和打包仍另验。长回答超时保留失败证据，不用默认场景通过覆盖。

## 流式更新隔离检查

CORTEXOS_STREAM_PROFILE=1启用独立stream-profile项目：在原生WebView2和同一隔离库中，通过固定HTTP流发送200段文本（请求间隔20ms），观察未操作图谱的标签层重建、绘制回调、主线程任务和JS堆；CORTEXOS_EXPECT_STABLE_GRAPH=1对优化版要求标签零重建且绘制不超过2次。随后实际筛选必须更新图谱，最终完整文本逐字落盘且GraphData哈希不变。模拟响应不作为模型吞吐或屏幕FPS证据。

真实graph-model检查新增CORTEXOS_GRAPH_ACTIVITY=idle/interactive，预热后使用独立新会话，使两种活动的请求内容可按哈希核对。测试专用回环转发记录真实上游headers/首文字/流结束；检索耗时沿用事件elapsedMicros，模型生成速率从实际evalCount/evalDuration计算。性能标记启用准备IPC计时；与首个保存文字的轮询观测分开。记录服务由谁启动，结束仅清理检查拥有的进程。必要聊天专项保留取消/断流/重启断言，图谱与执行图专项验证稳定props未阻止业务刷新。实际执行与未通过项见日志。


## 图谱按需读取专项

按用户要求不运行单元或全套测试。graph-query.spec.ts使用原生WebView2、隔离数据库及真实IPC检查261节点跨页/Unicode搜索、稳定布局序号、工作区隔离、参数上限、快照变化拒绝、101个关系历史修订按50/50/1完整读取且重复展开不重复、跨范围手动坐标及未加载坐标保留、跨页关系端点选择。测试端延迟一条实际IPC响应再切换搜索，检查旧结果不能覆盖新范围。三桌面视口截图与溢出检查独立记录，不冒称系统缩放验收。

graph/neighborhood/instances/trace/preview沿用必要回归。独立graph-baseline对同一个隔离库分别记录旧全量命令与新首页+有效links的往返、JSON字节和实体等值；图谱打开时间对应初始最多200节点，loadAllMs另记明确加载全图的耗时，不与旧全图首帧混为一谈。固定流与真实模型专项先显式加载全部节点，继续验证已有刷新隔离，而非用200节点冒充千节点共存。

实际运行的场景、失败修复和未执行范围以milestone-log及graph-on-demand-reads为准。


## 多中心必要检查

graph-exploration.spec.ts在真实WebView2与隔离70节点库验证：A59/B17/并集65的去重与50项前缀、共享来源、环/停用/未展开两跳排除、SQL参数及工作区隔离、逐个/全部收起、选中详情/位置草稿保留、显式临时显示、真实旧IPC响应延迟后不覆盖新范围、跨页和工作区恢复、WebGL降级、演示隔离及三尺寸截图。

复用graph-neighborhood、graph-query、graph的必要原生兼容专项，保留原业务断言；graph-preview扩展纯浏览器演示的双中心5→4→0→7路径。发现的清除筛选异步焦点竞争从产品逻辑修复，原toBeFocused断言保留。实际执行及未验证范围见milestone-log与graph-multi-center，不运行单元或全套测试。


## 图谱阅读必要检查

graph-reading.spec.ts在真实WebView2使用80节点隔离夹具，检查密集与长中英文标题、末尾节点选中优先、草稿标记不被标题挤掉、屏幕/面板/标签矩形避让、相机后方/画布外裁剪、连线名称开关和真实关系历史、三尺寸、空闲350ms绘制不增长及完整GraphData不变。复用实例点击/位置/相机、多中心、执行图和浏览器演示专项。

三规模graph-baseline记录24个标签槽与实际可见数量，延用首页/全量分开计时和绘制提交观测；千节点stream-profile继续要求固定流零标签层重建/零静置绘制。实际命令、当前性能单位和未验证范围见graph-reading与milestone-log，不运行单元或全套测试。

## Windows内部交付专项

只在CORTEXOS_DELIVERY_CHECK=1时启用delivery项目。先以tests/fixtures/delivery-previous.json及delivery-current.json构建两份独立标识的debug NSIS包，再构建正式Release并恢复默认debug程序；命令见windows-delivery。实际安装路径限定仓库artifacts，/S /NS不建快捷方式。检查版本与文件哈希、重新启动后的GraphData、四条知识版本及旧生效/新草稿、审核、消息/事件/请求快照、来源及布局的全表哈希，卸载应用文件删除而数据保留。Python标准库脚本仅接受artifacts内数据库，检查完整性、外键和6→7合成库的一致性备份。

窗口用真实HWND缩放，不调用CDP视口仿真；最大化/还原用键盘Enter，可访问树检查有意义的控件名称。它们不代替鼠标拖动、系统150%和真实读屏。正式Release只构建；伴随包debug测试入口不作为生产特性。用户选择保留系统设置，人工表全部待执行。

chat.spec.ts可通过CORTEXOS_CHAT_SOAK_MS=60000/120000延长既有受控流，默认0。取消后检查SQLite内容与状态；新增真实HTTP连接销毁，确认有效部分输出及run_failed。原格式错误、30秒超时、关闭和中断重启断言保留。必要专项只运行delivery及chat，不跑单元或全套测试；不把合成流当作真实模型长期稳定性。实际命令与结果见milestone-log。

## Markdown与慢速相机必要检查

markdown-camera.spec.ts在desktop项目运行两个隔离原生场景：输入原文/预览/焦点/发送、流式未闭合围栏代码、标题/粗体/列表/表格、原文保留、UTF-16引用与实体转义、错误引用、脚本/危险URL不执行、图片无请求、保存知识原文及跨格式选择保护、重启；相机在reduce下默认平滑，120ms实际相机位置仍在起终点之间，420ms过渡从真实首帧起算，快速切换、真实空白拖动/滚轮可打断，旧动画不重新开详情，空闲停止绘制，选择跟随系统/reduce与明确立即模式才即时定位；另覆盖首帧前500ms同步阻塞、偏好重载和三尺寸选项。计时是UI操作到详情出现，包括IPC和调度，不能当纯动画时间。

保留citations、graph-instances及trace专项检查，不跑单元或全套测试；citations既有分隔点断言调整为当前V3逗号，未删除业务断言。三内容视口实际截图与命令见milestone-log。原生截图不代表系统150%或真实读屏；代码中引用为字面文本，原报告不改写。

实体点选的固定投影检查显式选用立即模式，不以其验证动画；相机专项另外检查真实中间位置。Windows SPI和WebView媒体查询分别记录，不混为同一事实。


## Phase C / C3–C5（2026-10-27）

必要的安全/迁移/数据隔离检查与 Mock Provider 全链路见 [交付报告](benchmark-phase-c3-c5.md)。`cargo test --lib personalized --offline` 11 项通过；浏览器 3 种尺寸流程与 4 项连接/问卷回归通过；原生准入、知识、执行图、标准评测 v1/v2 共 5 项通过。真实云端、Docker 和新增页面的系统缩放/读屏不计为已测。截图保留于 artifacts/phase-c-ui-final，原生证据按报告列示；界面替身数据不作为模型成绩。


## 标准批量与窗口控制（2026-10-27）

本地标准计划不依赖云端或画像，工作区/类型边界、重复计划、重启补首次评分与无重发由必要专项检查。原生标准流程观察12次受控答题、并发1、授权模型释放、停止/剩余恢复、既有数据隔离及三尺寸按钮归属；详见 [本轮交付](benchmark-standard-batches.md) 和里程碑。真实模型能力、1800项极限规模、资源采样、Docker与系统缩放/读屏不作为本轮已验证项。

# H1.1 正确性、草稿和可访问性验收

## 交付范围

H1.0 记录 30 项复核、性能数据与 40 张 H0 截图。H1.1 对应 C3、C11–C16。H1.2–H1.4 的性能重构、IPC 整理和视觉统一在本轮验收点之后。 [审计复核](h1-audit-verification.md) · [实施计划](h1-implementation-plan.md)

| 项 | 改动与核对路径 |
| --- | --- |
| C3 错误 | 同一解析入口处理字符串和结构化错误。结构化结果保留错误码、可展示技术信息、重试字段和服务明确返回的保存状态。普通错误不推定保存成功。显示前脱敏常见凭据、URL 和文件路径，避免回显未声明的原始字段。 [错误解析](../../apps/desktop/src/frontend-error.ts) · [对话](../../apps/desktop/src/studio.ts) · [评测](../../apps/desktop/src/benchmark/personalized.ts) |
| C11 草稿 | 对话使用工作区和会话键。知识与关系含文档身份及基础修订。评测和项目配置按工作区、编辑对象与已有修订关系恢复。本机草稿写入当前应用 WebView 的本地存储，存储失败时使用当前窗口内存。临时草稿由同一关闭入口询问，知识与关系编辑继续提供正式保存、放弃、继续编辑，并可保留已落盘的本机草稿。云端密钥只经现有凭据链路提交。 [草稿存储](../../apps/desktop/src/draft-store.ts) · [知识编辑](../../apps/desktop/src/Knowledge.tsx) · [关系编辑](../../apps/desktop/src/Graph.tsx) · [窗口控制](../../apps/desktop/src/components/WindowControls.tsx) |
| C12 下拉菜单 | 用真正的原生 select 处理选择、键盘、焦点和表单。支持 `base-select` 的 WebView 使用现有 Token 着色，较旧环境回退到系统选择器。兼容原 label、value、disabled、required 和 change。 [控件](../../apps/desktop/src/components/SelectMenu.tsx) · [样式](../../apps/desktop/src/styles/controls.css) · [Chromium 原生控件](https://developer.chrome.com/blog/a-customizable-select) |
| C13 聊天焦点 | 等待发送或切换会话时使用只读保持输入焦点，发送按钮继续受可发送状态和既有互斥限制控制。消息滚动区可用 Tab 到达，保留输入法、Enter 和 Shift+Enter。超过应用可容纳的总输入上限时就地显示说明，具体消息预算仍由后端校验。 [输入区](../../apps/desktop/src/components/ChatComposer.tsx) · [聊天区](../../apps/desktop/src/Chat.tsx) |
| C14 弹窗 | 改善初始焦点、叠层确认、回到编辑点、无触发器时的焦点退路。原生选择器打开时浏览器先处理 Escape。内容区域和下层弹窗 inert，现行原型要求保留自绘标题栏可达，因此继续采用内容范围弹窗。全局提示统一由一个 status 区域播报。 [共享弹窗](../../apps/desktop/src/components/AppDialog.tsx) · [状态提示](../../apps/desktop/src/components/Toast.tsx) · [产品标题栏规范](../../design.md) · [W3C 交互参考](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) |
| C15 表单 | 数字取 `valueAsNumber`，空值不转换为 0。设置及评测核对范围、整数和总容量，字段解释由 `aria-describedby` 关联。云端可选限额支持空值和上下限，候选 JSON 在提交之前检查语法与外层结构，项目任务行号检查整数、边界和顺序。后端独立执行原有校验。 [数字字段](../../apps/desktop/src/components/NumberField.tsx) · [设置](../../apps/desktop/src/Settings.tsx) · [标准评测](../../apps/desktop/src/benchmark/StandardSetup.tsx) · [专属评测](../../apps/desktop/src/benchmark/BatchSetup.tsx) · [云端配置](../../apps/desktop/src/components/CloudRuntimeSettings.tsx) · [候选审核](../../apps/desktop/src/benchmark/DatasetReview.tsx) |
| C16 渲染恢复 | 根边界提供重试和重载。当前页另有恢复边界，保留现有懒载重试。回退提示描述实际故障与恢复动作，保存结果由相应流程显示。 [根入口](../../apps/desktop/src/main.tsx) · [错误边界](../../apps/desktop/src/components/ErrorBoundary.tsx) |

## 草稿和关闭边界

本机草稿在相同 WebView 配置下恢复。用户切换 Windows 帐户、清理 WebView 数据或更换安装身份后，不保证可访问该份草稿；正式业务数据仍由原 SQLite 流程管理。已编辑内容与基础版本不匹配时，不自动把旧草稿套到新版本。设置刷新时的合并策略归 C10。外部系统关闭与运行中任务联动、真实读屏、高对比度、不同 DPI、实体跨屏拖动需要独立桌面手验。

## 验收结果

- `npm.cmd run build` 通过，含 `tsc --noEmit`。`cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。Vite 仍提示按需 3D 块大于 500 kB。
- 原生核心专项通过 2 条，49.2 秒，浏览器专用项按项目跳过。覆盖对话发送互斥、取消和超时、SQLite 重启、工作区草稿隔离、空数字或容量错误、下拉键盘 Escape、知识编辑初始和返回焦点、保留本机草稿并重启、放弃和正式记录核对。早先原生批次发现关闭监听经默认强制关闭接口触发权限拒绝，修正为确认后解除监听并使用现有常规关闭权限。后续加入知识标题初始焦点，最终用例通过。
- 浏览器回归通过 13 条，31.1 秒，原生专用项按项目跳过。范围为评测报告和返回定位、云端配置与费用提示，以及浅暗原生菜单布局和键盘交互。另一次 4 条通过，16.0 秒，覆盖根级恢复和三个视口的个性化评测流程，1366×768 增加三种无效 JSON 输入情形。所有云端和模型数据来自固定测试输入。
- 日志在 `artifacts/phase-h1/core-recheck.log`、`edge.log`、`edge-final.log`。未运行全套测试、真实读屏、真实云端调用或本地模型推理。外部关闭和运行中任务联动、实际不同 DPI 和物理跨屏操作仍待验证。

补充回归通过 3 条，40.7 秒。真实原生 WebView2 覆盖 RAG 引文准确与历史证据、从实际开发文档导入项目适配与审核版本、标准评测资源等待取消和按计划发送。模型端均使用受控响应。另一次原生回归通过 5 条，49.5 秒，覆盖前后同条件性能用例、知识审核及重启、输入冷却、保持阅读位置、窄窗分屏与可用的编辑和资料文件菜单。日志为 `artifacts/phase-h1/supplement.log` 与 `after.log`。

真实读屏与物理窗口尚待人工验收，重点核对焦点进入和返回、弹窗内外的播报顺序、缩放后的下拉边界及标题栏关闭路径。现行标题栏允许独立的窗口操作，保留这一规则是本轮的无障碍取舍。

## 同条件性能与截图复核

最终标题栏菜单在深浅主题中保持固定宽度，文字与图标垂直对齐。最后一批 5 条通过，38.4 秒。包含 WebView2 同条件截图和历史测量、两条 Edge 下拉/弹窗检查与原生菜单回归。五页、四视口、两主题复拍 40 张，最终前后各 40 张均无文档级横向溢出。评测首画面保留加载态，计划实际加载和可用性由标准评测回归独立确认。

| 指标 | H1.0 基线 | H1.1 最终版本 |
| --- | ---: | ---: |
| 流式根 / 外壳 / 会话列表提交观测 | 119 / 60 / 119 | 119 / 60 / 119 |
| 3,000 字流式 / 终止后 Markdown 入口调用 | 60 / 2 | 60 / 2 |
| Markdown 入口累计 CPU 采样估计 | 466.80 ms | 480.37 ms |
| 首屏入口及 HTML 显式预加载 JS | 520,059 字节 | 522,796 字节 |
| 100 / 500 / 1,000 条历史进入，P50 | 51.74 / 147.70 / 277.93 ms | 50.37 / 158.58 / 272.48 ms |
| 100 / 500 / 1,000 条历史，滚动帧间隔 P95 | 7.10 / 7.10 / 7.10 ms | 7.10 / 7.10 / 7.10 ms |
| 图谱拖动帧间隔 P95 | 7.10 ms | 7.10 ms |

单次流的累计 CPU 和每规模三次打开不足以判断稳定的性能变化。首次明列脚本增加 2,737 字节。两个版本的五页切换 P50/P95、帧数据与测试输入见 `artifacts/phase-h1/comparison.json`。最终截图、CDP 采样、历史数据及测试日志分别在 `final-baseline/`、`after-final/` 和 `after-final.log`。H1.2 以此作为优化起点。 [测量口径](h1-performance-baseline.md) · [截图基线](h1-ui-baseline.md)

# H1.2 流式渲染与状态管理验收

## 交付范围

- [流式状态](../../apps/desktop/src/chat-stream.ts) 按消息订阅，核对运行归属及事件序号。首段和终止立即显示，中间累计正文最多每 90 毫秒刷新一次。终态与 SQLite 回读协调。AppShell、会话侧栏和当前时间线仍响应会话、准备、结束等低频状态。
- [工作区与会话](../../apps/desktop/src/chat-workspace.ts)、[设置草稿](../../apps/desktop/src/studio-settings.ts) 和[本地界面偏好](../../apps/desktop/src/preferences.ts) 按职责拆出。[应用状态](../../apps/desktop/src/studio.ts) 负责组织流程，错误提示通过独立状态管理。
- [消息渲染](../../apps/desktop/src/components/ChatMessage.tsx) 和 [Markdown](../../apps/desktop/src/components/MarkdownContent.tsx) 保留代码块、表格、链接、原文和引用。按合并后的累计正文解析，有引用时才加入引用转换。
- [会话历史](../../apps/desktop/src/Chat.tsx) 首次挂载最近 60 条，向前加载保留阅读位置。旧消息或引用链接打开目标附近的 60 条，可继续加载更新的消息或回到最新内容。[滚动逻辑](../../apps/desktop/src/components/useChatScroll.ts) 将连续尺寸变化合并到动画帧处理，保留上滚暂停跟随和回到底部操作。

## 原生对照

2026-11-20，在同一台 Windows 11 电脑的独立桌面上，比较 H1.1 留档与 H1.2 调试程序。硬件为 i7-12800HX、RTX 4070 Laptop，使用 WebView2 157、同一套合成 SQLite 数据和测试配置。单轮流式样本为 3,000 字符、60 段、每段约 30 毫秒，另有终止帧。两版均验证最终写入 SQLite 的文本与输入逐字一致。用例和统计规则见 [H1 原生对照用例](../../apps/desktop/tests/h1-baseline.spec.ts) 与 [H1 基线口径](h1-performance-baseline.md)。

| 流式指标 | H1.1 | H1.2 |
| --- | ---: | ---: |
| React 根树提交 | 119 | 20 |
| 外壳 / 会话侧栏 / 聊天容器更新观测 | 60 / 60 / 60 | 0 / 0 / 0 |
| Markdown 流式 / 终止入口调用 | 60 / 2 | 18 / 3 |
| Markdown 入口累计 CPU 采样估计 | 458.73 ms | 181.70 ms |
| 首屏 HTML 入口及预加载 JS 合计 | 522,796 B | 526,927 B |

根树提交包含单条消息自己的提交。容器数字取对应 DOM Fiber 已提交的 props 引用变化，作为外壳、侧栏、聊天容器的更新观测。未观察到非当前页面挂载。CDP CPU 采样包含解析入口中的完整处理，单次样本用于诊断。首屏 JS 增加 4,131 B，约 0.8%。

| 会话总条数 | 首批消息 DOM，H1.1 → H1.2 | 打开至最新消息挂载 P50/P95，H1.1 → H1.2 | 持续上滚帧间隔 P50/P95 |
| --- | ---: | ---: | ---: |
| 100 | 100 → 60 | 55.43 / 148.79 → 45.15 / 75.91 ms | 两版 7.00 / 7.10 ms |
| 500 | 500 → 60 | 163.17 / 163.25 → 47.99 / 49.40 ms | 两版 7.00 / 7.10 ms |
| 1,000 | 1,000 → 60 | 307.65 / 313.42 → 54.36 / 63.51 ms | 两版 7.00 / 7.10 ms |

会话规模各测 3 次，P95 取最近秩。聊天、知识、图谱、评测、设置各测 8 次切换。P50 分别为 29.45→32.31、15.07→18.20、122.70→128.15、17.72→17.00、18.06→15.56 ms。P95 分别为 33.50→39.41、330.04→326.69、847.64→842.93、335.15→339.14、326.25→327.95 ms。切换混合首次及后续打开，包含等待可见标记的轮询。小图谱浅暗拖动合计 216→215 个帧间隔，两版 P50/P95 均为 7.00/7.10 ms。

原生截图在两版各生成 40 张，覆盖 800×600、1366×768、1600×900、1920×1080，五页浅暗主题。两版可视区域均未检测到页面级横向溢出，另对看了两版 800×600 暗色对话截图。原始结果和统计在本地 `artifacts/phase-h1/h12/{before-final,after-scroll-final}/` 和 `comparison.json`。

## 验证与边界

- 在 `apps/desktop` 执行 `npm.cmd run build`，TypeScript 及 Vite 通过。离线执行 `cargo build --manifest-path src-tauri/Cargo.toml --offline --locked --features tauri/custom-protocol --bin cortexos-desktop` 通过。Vite 仍提示既有图谱资源超过 500 kB。
- 独立 Windows 桌面执行 `playwright test --project=h1-baseline`，H1.1 留档 2 条通过，用时 27.3 秒。H1.2 交付版本 2 条通过，用时 24.5 秒。H1.2 另验证长历史前插后的阅读锚点、深链接聚焦、加载较新消息和回到最新内容。
- 初次 `ui-micro-native.spec.ts` 捕获「回到最新内容」后下一段回复未跟随的问题。布局整数与 WebView2 实际滚动位置存在舍入差异，已修正为以实际位置更新滚动基准。最终原生专项复跑 1 条通过，用时 10.4 秒。
- 最终版执行 `playwright test --project=desktop --project=stream-profile chat.spec.ts markdown-camera.spec.ts prototype-ui.spec.ts citations.spec.ts context.spec.ts trace.spec.ts stream-profile.spec.ts`，8 条通过，用时 1.7 分钟。覆盖取消、错误及重开后的 SQLite 输出，Markdown 与引用偏移、上下文记录、图谱镜头及执行图定位。固定 1,000 节点、5,000 条关系的图谱在 200 段合成回复期间记录 0 次额外绘制和 0 次标签增删，随后验证过滤与恢复。回归报告在本地 `artifacts/phase-h1/h12/regression-{scroll,other}-final/`。
- 历史分页在界面渲染层，`load_messages` 仍从 SQLite 经原有 IPC 读取完整会话。反复展开旧消息后，该次访问保留已展开的 DOM。磁盘读取、跨进程序列化和完整历史内存占用仍需独立测量。
- 测试使用固定回环回复和隔离数据库。真实模型吞吐、系统读屏与物理高 DPI 操作未验证。

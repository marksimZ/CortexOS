# H1.0 前端性能基线

## 环境与口径

2026-11-20，Windows 11 build 26200，Intel Core i7-12800HX，NVIDIA RTX 4070 Laptop GPU，驱动 610.74。H0 调试 custom-protocol 程序，WebView2/Edge 157，非活动桌面。内容视口 1600×900、DPR 约 1。截图同时覆盖另外三个尺寸。每个 Playwright 用例有独立 WebView2 配置和测试库。

流式输入为单条合成 3,000 字符回答，60×50 字符，约每 30ms 一段，另有一条终止帧。固定本地回环的响应由测试脚本产生。历史组通过隔离 SQLite 工作区构造 100、500 和 1,000 条带短 Markdown 的消息，打开真实聊天页。每种规模 3 次，每次向上滚动约 600ms。图谱为 3 个已确认合成节点，无关系，浅色与暗色各操作一次。图谱大型场景的历史参考见 [G5](../optimization/phase-g5-release-validation.md)。

以 React DevTools 提交钩子记录根提交和本轮访问的外壳、会话列表 Fiber；用浏览器 CDP Precise Coverage 统计生产 Bundle 中包含 `processor.runSync(processor.parse(…))` 的 ReactMarkdown 入口调用。CPU profile 将对应调用栈的采样间隔合计，包含处理器准备、Markdown 解析、插件、转换和元素构造。Chrome 主线程采样的估计值会受插桩影响。单独解析耗时与每次解析 P95 未测。

## 流与首次加载

| 指标 | H0 基线 |
| --- | ---: |
| 发送已进入受控服务后，60 段期间根提交 | 119 |
| 同一窗口内外壳提交 | 60 |
| 会话列表 Fiber 的提交观测 | 119 |
| 此窗口中实际挂载的非当前业务页 | 0 |
| 流式 / 终止之后的 Markdown 入口调用 | 60 / 2 |
| 本次 Markdown 入口的累计 CPU 采样估计 | 466.80 ms |
| 首屏入口 JS / HTML 明列的入口和预加载 JS 合计 | 494,472 / 520,059 字节 |

这组 JS 数值是 WebView2 调试协议读到的内容原始字节。按需加载的 `GraphViewport` 资源未列于首屏 HTML 预加载清单。会话列表计数按观测到的 Fiber 更新，覆盖已进入固定服务的流期间。非当前路由同时有源码条件渲染作为证据。

## 历史滚动与页面切换

| 历史规模 | 路由至全部消息 DOM P50 / P95 | 持续上滚帧间隔 P50 / P95 |
| --- | ---: | ---: |
| 100 条 | 51.74 / 81.34 ms | 7.00 / 7.10 ms |
| 500 条 | 147.70 / 153.25 ms | 7.00 / 7.10 ms |
| 1,000 条 | 277.93 / 294.56 ms | 7.00 / 7.10 ms |

| 页面 | 切换至页面可见标记 P50 / P95，8 次 |
| --- | ---: |
| 对话 | 27.49 / 35.69 ms |
| 知识 | 15.35 / 334.07 ms |
| 图谱（WebGL ready） | 121.25 / 840.38 ms |
| 评测 | 15.75 / 335.91 ms |
| 设置 | 16.42 / 326.79 ms |

P95 用最近秩，小样本表中通常等于最大值。切换数据混合首次和重复进入，包含 Playwright 轮询。图谱在 1600×900 短时拖动，浅暗 P50/P95 都是 7.00/7.10ms。`requestAnimationFrame` 统计反映页面回调间隔，GPU 单帧成本和长会话真实内容长尾待另测。

原始记录在 `artifacts/phase-h1/final-baseline/` 下两个 Playwright 用例目录，包含 `baseline.json`、`stream.cpuprofile`、`history.json` 和 40 张 PNG。运行入口为 [基线用例](../../apps/desktop/tests/h1-baseline.spec.ts) 与 [长历史固定数据](../../apps/desktop/tests/h1-history-fixture.py)。运行时设置 `CORTEXOS_H1_BASELINE=1`、`CORTEXOS_H1_EXECUTABLE` 指向待比较的调试可执行文件，再执行 Playwright 的 `h1-baseline` 项目。两条通过，共 25.1 秒。

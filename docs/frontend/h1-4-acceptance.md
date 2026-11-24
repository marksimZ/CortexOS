# H1.4 视觉系统与交互验收

## 对应审计项

| 范围 | 交付与证据 |
| --- | --- |
| V1–V3 字体、阅读和控件 | 在[语义变量](../../apps/desktop/src/styles/tokens.css)统一 12–28 px 字号、36 / 30 px 控件高度和三档主圆角。聊天使用 720 px 阅读列，知识正文限制为 76ch，代码和数据保留局部滚动。见[共享控件](../../apps/desktop/src/styles/controls.css)、[知识和设置](../../apps/desktop/src/styles/features.css)、[Markdown](../../apps/desktop/src/markdown.css)。 |
| V4–V6 标题、玻璃与操作层级 | 页面标题保持在全局栏，知识空态只留选择提示。导航当前项、主要按钮、普通内容和浮层分别使用已有的主题色与表面层级。新建工作区突出为主要操作。 |
| V7–V8 表格、原始记录和反馈 | [评测结果行](../../apps/desktop/src/benchmark/PagedResults.tsx)将题目、重复轮次和状态分别显示，窄窗换行，宽表独立滚动。知识、评测单题与项目适配复用[状态标签](../../apps/desktop/src/components/StatusLabel.tsx)，失败、待确认和通过各有文字。分页保留独立读取状态及重试操作。 |
| V9–V11 浅暗、窄窗与跨页一致性 | 在 800×600、1366×768、1600×900、1920×1080 实拍五个页面及两套主题，验证窄窗设置标签换行、评测表格局部滚动，以及 Windows 窗口栏和详情关闭可达。修复问卷固定底栏遮挡末项滑块，保存区随表单滚动。 |
| V12–V13 动效、定位和图谱 | 原生专项覆盖流式时的滚动跟随、分屏取消、Markdown 预览与引文、图谱镜头插值与切换。浏览器覆盖 WebGL 降级。 |
| V14 视觉验收 | 前后截图逐项比对，核对知识当前导航与状态、800×600 暗色聊天、评测表格和抽屉。WebView2 模拟强制配色下，设置项有可见焦点，可用 Enter 选中，当前项带下划线。真实 Windows 读屏以及系统 DPI 和多屏操作仍由独立安装验收覆盖。 |

现行视觉规则见 [HTML 原型设计规范](../../design.md)。

## 验证结果

- `npm.cmd run build`、`npm.cmd run format:check`、ESLint 检查和离线锁定的 Rust custom-protocol 调试构建通过。ESLint 记录 0 错误、63 条提示。
- Edge 最终复验 17 项通过，用时 49.0 秒。覆盖评测报告、图谱降级、窗口栏、弹窗键盘、云端连接表单、工作区模型配置和知识范围。问卷在 800×600 用键盘调节末项滑块、滚动至保存按钮并保存，另验证切页保留草稿。首次检查发现问卷固定底栏遮挡与分页断言使用旧文案，分别修正后复验。一次中间复跑复用了即将退出的网页测试服务，最终用独立服务重跑全部 17 项。见 `artifacts/phase-h1/h14/browser-release.log`。
- 原生业务回归 7 项通过。命令为 `playwright test --project=desktop ui-micro-native.spec.ts knowledge-layout.spec.ts markdown-camera.spec.ts harness-fit.spec.ts benchmark-report-pages.spec.ts benchmark-report-usability.spec.ts`。
- 最终原生基线 3 项通过。以 `CORTEXOS_H1_BASELINE=1` 和 `CORTEXOS_H14_CHECK=1` 执行 `playwright test --project=h1-baseline`。40 张常规截图均无页面级横向溢出。另留强制配色截图和对比度数据，浅暗各 5 组语义底色与文字组合均高于 4.5∶1，最低为浅色错误文字对基础背景的 5.23∶1。此对照采样为纯色变量，玻璃和渐变叠色仍按实拍逐页检查。 [WCAG 对比度标准](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)

前后截图、原生运行记录与固定数据的详情见本地 `artifacts/phase-h1/`，基线目录为 `final-baseline`、`h13/baseline`，本切片目录为 `h14/baseline-release`、`h14/native-final`、`h14/browser-release`。业务回归通过隔离工作区与固定回环回复完成。独立 Windows 验收仍需覆盖实际安装身份、读屏、系统 DPI 与多屏。 [H0 安装矩阵](../release/h0-windows-validation.md)

## 性能对照

同机按 H1.3、H1.4、H1.3、H1.4 交替运行。每轮生成一条固定长回复，分别打开 100、500、1,000 条合成会话各 3 次。两版首批均挂载 60 条。H1.4 性能数据采于问卷底栏单独修正前，基线脚本使用标准评测入口。P95 取最近秩，小样本受当次波动影响。

| 指标 | H1.3 | H1.4 |
| --- | ---: | ---: |
| 60 段回复的根树提交，两轮 | 20、20 | 19、19 |
| Markdown 流式解析入口，两轮 | 18、18 | 18、18 |
| 首屏入口及预加载 JavaScript | 528,209 B | 528,278 B |
| 100 条会话打开 P50 / P95，毫秒 | 69.04 / 110.31 | 78.98 / 122.43 |
| 500 条会话打开 P50 / P95，毫秒 | 59.22 / 64.12 | 64.51 / 120.48 |
| 1,000 条会话打开 P50 / P95，毫秒 | 62.16 / 79.11 | 62.50 / 70.79 |
| 三节点图谱帧间隔 P50 / P95，毫秒 | 7.00 / 7.10 | 7.00 / 7.10 |

100 和 500 条样本在最终版有较大的单次耗时，1,000 条中位数在本组接近。一次未交替的最终版样本中，1,000 条打开中位 94.77 毫秒。对照原始日志单独保留该样本与四组交替数据。各页面切换统计及原始帧间隔见本地 `artifacts/phase-h1/h14/comparison.json` 与 `compare-{a1,b1,a2,b2}/`。

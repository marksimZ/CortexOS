# H1.0 前端审计复核

审计基线为 H0 的原生调试可执行文件和 H1 修改前的工作区源码。根目录 `design.md` 与另一位 Agent 的独立 30 项报告均未在仓库找到。C1–C16 按 H1 附件明确给出的编号复核。附件只给出 V1–V14 的范围与视觉验收重点，下表按这些重点重建十四个检查项，编号是本次复核索引。原始逐项结论少了可核对的材料。

已阅读项目总纲、开发规范、实施计划、G5 验收和现行 HTML 原型规范。统计、截图和复现见 [性能基线](h1-performance-baseline.md) 和 [界面基线](h1-ui-baseline.md)。

## 视觉与交互

| 编号 | 检查项与结论 | 证据及处理 |
| --- | --- | --- |
| V1 | 字体层级，部分成立 | 全局用语义字体，评测与项目适配的局部字号仍分散。H1.4 抽取实际复用层级。 [基础令牌](../../apps/desktop/src/styles/tokens.css) · [评测样式](../../apps/desktop/src/benchmark/benchmark.css) |
| V2 | 阅读宽度，部分成立 | 聊天与知识正文具有限宽规则，图谱和对照表有不同宽度需求。H1.4 以长内容截图分别验证。 [布局](../../apps/desktop/src/styles/layout.css) · [正文](../../apps/desktop/src/markdown.css) |
| V3 | 控件尺寸与圆角，部分成立 | 已有圆角、间隙与箭头样式，局部控件仍以数值控制尺寸。H1.4 处理令牌覆盖。 [控件](../../apps/desktop/src/styles/controls.css) |
| V4 | 页面标题与视觉焦点，部分成立 | 全局标题栏已按页面切换。部分评测段落另有任务标题，应根据层级决定保留。 [应用外壳](../../apps/desktop/src/components/AppShell.tsx) · [评测入口](../../apps/desktop/src/benchmark/BenchmarkPage.tsx) |
| V5 | 内容面与玻璃，部分成立 | 统一背景和表面变量已覆盖主框架。弹层与业务局部容器仍混用不同深度和透明表现。 [令牌](../../apps/desktop/src/styles/tokens.css) · [控件](../../apps/desktop/src/styles/controls.css) |
| V6 | 主次按钮，部分成立 | 通用主次/文字按钮已具备。流程内并列高优先级动作仍需按截图和任务频率复核。 [控件](../../apps/desktop/src/styles/controls.css) · [批量配置](../../apps/desktop/src/benchmark/BatchSetup.tsx) |
| V7 | 表格与原始数据，部分成立 | 结果已分页并能打开详情。大段 JSON 与长列的可读性仍需按窄窗和字体放大检查。 [结果分页](../../apps/desktop/src/benchmark/PagedResults.tsx) · [评测样式](../../apps/desktop/src/benchmark/benchmark.css) |
| V8 | 状态、错误、空态，已确认 | 聊天与评测有不同的错误降级，局部反馈结构不一致。H1.1 先统一高风险错误语义，视觉留 H1.4。 [对话状态](../../apps/desktop/src/studio.ts) · [评测处理](../../apps/desktop/src/benchmark/personalized.ts) |
| V9 | 浅暗主题，部分成立 | 双主题应用到五页，颜色由共享变量驱动。系统高对比、长表及错误态仍需独立验收。 [令牌](../../apps/desktop/src/styles/tokens.css) · [外观控件](../../apps/desktop/src/components/AppearanceControl.tsx) |
| V10 | 800×600 窄窗，部分成立 | 已有折叠导航、抽屉与滚动处理。完整验证要覆盖内容区、长表和打开弹层后的键盘可达。 [布局](../../apps/desktop/src/styles/layout.css) · [应用外壳](../../apps/desktop/src/components/AppShell.tsx) |
| V11 | 跨页一致性，部分成立 | 五页共用外壳和令牌，评测与项目任务局部样式独立。H1.4 统一通用交互状态。 [外壳](../../apps/desktop/src/components/AppShell.tsx) · [项目适配](../../apps/desktop/src/benchmark/harness/harness.css) |
| V12 | 动效与滚动，部分成立 | 已有减少动态效果、流式动画和跟随规则；每次内容变化仍读布局。H1.2 用测量决定调整。 [动效](../../apps/desktop/src/styles/motion.css) · [滚动](../../apps/desktop/src/components/useChatScroll.ts) |
| V13 | 3D 交互，已在后续代码中修复一部分 | 已有按需画布、实例几何、镜头偏好、无 WebGL 的列表入口。大型图谱的实际绘制与交互压力仍需持续监测。 [图谱](../../apps/desktop/src/Graph.tsx) · [视口](../../apps/desktop/src/GraphViewport.tsx) |
| V14 | 完成视觉规范的统一，需进一步测试 | 本阶段记录真实视口。系统缩放、读屏、高对比度与局部超长文字还需要 Windows 手验。 [设计规范](../design/html-prototype-ui.md) · [截图](h1-ui-baseline.md) |

## 前端实现

| 编号 | 结论 | 证据及处理 |
| --- | --- | --- |
| C1 | 已确认 | `App` 持有 `useStudio`，同一个 Hook 管理导航数据、设置、网络及 `setUpdates`。流式时根和外壳重复提交。H1.2 测细粒度订阅。 [App](../../apps/desktop/src/App.tsx) · [useStudio](../../apps/desktop/src/studio.ts) |
| C2 | 已确认 | `invoke<T>` 泛型由各调用点手工声明，命令与参数跨领域分散。原知识详情请求封装已在 H1.3 迁入按领域划分的契约。 [模型设置](../../apps/desktop/src/components/CloudRuntimeSettings.tsx) · [知识契约](../../apps/desktop/src/api/knowledge.ts) |
| C3 | 已确认 | `studio.ts` 接受 Error.message，`personalized.ts` 通用提示推定已保存。H1.1 共用解析，结构化状态才进入提示。 [对话](../../apps/desktop/src/studio.ts) · [评测](../../apps/desktop/src/benchmark/personalized.ts) |
| C4 | 部分成立 | 已有统一 hash 解析、深链和 `useSyncExternalStore`；部分报告筛选和返回仍有独立参数逻辑。H1.3 以既有定位测试保护。 [导航](../../apps/desktop/src/navigation.ts) · [报告导航](../../apps/desktop/src/benchmark/reportNavigation.ts) |
| C5 | 已确认 | 消息列表全量挂载，流时 `useLayoutEffect` 与 ResizeObserver 可重复调用距离/尺寸读取。原有取消跟随行为有效。 [聊天](../../apps/desktop/src/Chat.tsx) · [滚动](../../apps/desktop/src/components/useChatScroll.ts) |
| C6 | 已确认 | 历史卡片已有 memo。增长中的回答仍把完整字符串交给 ReactMarkdown，remark 参与每次新内容处理。H1.2 按实测改善。 [回答](../../apps/desktop/src/components/AnswerContent.tsx) · [解析](../../apps/desktop/src/components/MarkdownContent.tsx) |
| C7 | 部分成立 | Graph、Knowledge 同文件包含多个独立职责；现有画布、视角、查询、详情已有拆分。H1.3 按依赖和副作用分界。 [图谱](../../apps/desktop/src/Graph.tsx) · [知识](../../apps/desktop/src/Knowledge.tsx) |
| C8 | 部分成立 | TypeScript、npm 锁文件和 Playwright 已存在。未见统一 lint、格式检查和前端单元测试入口。H1.3 增补。 [工具清单](../../apps/desktop/package.json) · [测试配置](../../apps/desktop/playwright.config.ts) |
| C9 | 部分成立 | 多处重复 active/current、轮询和防过期模式；部分有工作区校验、请求身份及定时器清理。H1.3 保持各侧业务语义。 [模型连接](../../apps/desktop/src/components/CloudRuntimeSettings.tsx) · [评测结果](../../apps/desktop/src/benchmark/PersonalizedReport.tsx) |
| C10 | 已确认 | `settingsDraft` 与已保存配置分离，但 `reload()` 直接重置草稿。H1.1 处理设置表单校验，刷新时的合并策略归 H1.3。 [设置](../../apps/desktop/src/Settings.tsx) · [状态](../../apps/desktop/src/studio.ts) |
| C11 | 已确认 | 聊天、知识、评测与 Harness 采用不同内存草稿，知识与图谱各装关闭回调，其他表单有 `beforeunload`。H1.1 统一本机草稿和关闭入口。 [知识](../../apps/desktop/src/Knowledge.tsx) · [图谱](../../apps/desktop/src/Graph.tsx) · [编程草案](../../apps/desktop/src/benchmark/harness/ProgrammingDrafts.tsx) |
| C12 | 部分成立 | 真实 select 保留了表单和值，但由 JS 接管弹层和方向键，读屏语义在两套选项间交叠。H1.1 采用 CSS 增强原生 picker。 [下拉组件](../../apps/desktop/src/components/SelectMenu.tsx) |
| C13 | 已确认 | 临时 `disabled` 会让当前文本框离开焦点，消息容器原来没有独立焦点入口。H1.1 保留只读时焦点和发送互斥。 [输入框](../../apps/desktop/src/components/ChatComposer.tsx) · [消息区](../../apps/desktop/src/Chat.tsx) |
| C14 | 部分成立 | 已有内容 inert、栈、Escape 和标题栏独立操作。负 tabindex、初始/回退焦点、picker 优先关闭及重复播报仍需处理。 [共享弹窗](../../apps/desktop/src/components/AppDialog.tsx) · [通知](../../apps/desktop/src/components/Toast.tsx) |
| C15 | 已确认 | `Number("")` 可转为 0，部分未 `required`，候选 JSON 只有断言类型。Rust 已有范围与契约校验。H1.1 在高风险入口加字段级反馈。 [设置](../../apps/desktop/src/Settings.tsx) · [专属评测](../../apps/desktop/src/benchmark/BatchSetup.tsx) · [候选审核](../../apps/desktop/src/benchmark/DatasetReview.tsx) |
| C16 | 已确认 / 部分成立 | 入口原先挂载 App 无根级错误边界，懒加载只覆盖自身加载失败。已存在类型检查和回归测试；工程规范归 H1.3。 [入口](../../apps/desktop/src/main.tsx) · [懒加载](../../apps/desktop/src/components/lazyView.tsx) |

## 实施取舍

整页重写、全局状态单 Context、手工模拟下拉选项及文档级弹窗模态化会分别扩大回归范围、订阅范围、键盘兼容成本或挡住 CortexOS 自绘标题栏。H1.1 使用原生表单、保留局部弹层范围，并在密钥表单之外保存本机草稿。页面结构、接口分层与样式令牌按后续切片验收。 [现行 HTML 原型规范](../design/html-prototype-ui.md)

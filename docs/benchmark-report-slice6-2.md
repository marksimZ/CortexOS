# Benchmark 6.2：模型比较与结果筛选

2026-10-28。用户在 6.1 交付后授权继续原计划，同时暂选 Logo 02，并明确应用内弹窗应下移、拥有自己的关闭入口，且全局标题栏始终可见、可操作。本次不进入 6.3 详情抽屉、6.4 导航、6.5 收尾或发布。

## 实际变更

| 文件（相对仓库根） | 修改 |
| --- | --- |
| `apps/desktop/src-tauri/src/benchmark/report_read.rs` | 增加可选筛选参数，校验状态、能力、难度和冻结轮次；COUNT 和分页共享数据库筛选投影及绑定参数。保留未筛选请求兼容、用途/工作区校验和每页上限；只读查询不取回答正文 |
| `apps/desktop/src-tauri/src/benchmark/standard_tests.rs` | 必要的筛选/分页/隔离专项；全题库跨页计数、组合筛选、非法输入与原统计等价 |
| `apps/desktop/src/benchmark/ReportOptions.tsx` | 稳定排序、逐题筛选与冻结比较条件；未知值放最后，相同值按计划模型序号排序 |
| `apps/desktop/src/benchmark/PersonalizedReport.tsx` | 标准/个人入口共用排序及筛选；全计划比较表不受逐题筛选改变 |
| `apps/desktop/src/benchmark/PagedResults.tsx` | 将筛选传给后端、显示匹配/计划数量、空结果与返回首页入口；切换筛选重置页码/展开项并隔离旧响应 |
| `apps/desktop/src/benchmark/benchmark.css` | 沿用现有 Tokens 和 SelectMenu，使用可换行表单排列筛选 |
| `apps/desktop/src/components/AppDialog.tsx` | 内容范围的共享弹窗：正文与侧栏 inert，标题栏可操作；保留原生 close/cancel、Esc、焦点返回及嵌套草稿确认。弹窗挂到 body，避免玻璃面板裁切/层叠 |
| `apps/desktop/src/App.tsx`、`Chat.tsx`、`Graph.tsx`、`Knowledge.tsx`、`SourceBrowser.tsx` | 仅将弹窗标签/打开入口接入共享容器，保留各自保存、审核、错误与取消保护 |
| `apps/desktop/src/components/AppShell.tsx`、`AnswerContent.tsx`、`ChatEvidence.tsx`、`CognitiveInspector.tsx` | 导航、引用、依据和处理记录接入同一弹窗规则 |
| `apps/desktop/src/styles/controls.css`、`layout.css` | 弹窗顶部预留 76px；抽屉与弹窗在该范围内布局；全局标题栏高于遮罩；紧凑弹窗补自己的关闭按钮，长面板继续固定标题和操作区 |
| `apps/desktop/public/cortex.svg`、`apps/desktop/src/components/BrandMark.tsx` | 将已选“知识节点”方向整理为透明几何 SVG，供导航、欢迎页和 favicon 共用 |
| `apps/desktop/src/components/EmptyState.tsx`、`Sidebar.tsx` | 复用新标识；旧侧栏组件保留相同标识 |
| `apps/desktop/src-tauri/icons/` 下原有六个图标文件 | 使用现有 Tauri CLI 从 SVG 转换桌面图标；多余移动端生成文件已移除 |
| `apps/desktop/tests/benchmark-report-pages.spec.ts`、`standard-benchmark.spec.ts`、`window-drag.spec.ts`、`window-drag.ps1` | 真实分页筛选、双模型排序/停止恢复、弹窗期间窗口操作与截图；物理鼠标操作仍先核对本测试窗口归属 |

文档同步：本文件、阶段计划、实施计划、HTML UI 规范、测试计划、Logo 方案说明及里程碑。

## 统计与数据边界

- 排序提供计划顺序、已评分正确率、评分覆盖率和已记录生成速度；不产生综合分。零值有效，缺失值不补零。
- 模型筛选只控制下方逐模型详情。状态/能力/难度/轮次按数据库全部结果筛选，再分页；上方统计分母仍为整个冻结计划。
- “比较条件”显示固定题库版本/内容标识、计划锁定的模型标识、Provider 和请求上下文/回答上限/温度/超时。参数不同时明确标记差异；不推测量化方式、实际上下文用量或硬件条件相同。
- 无新增 SQL 迁移、题库/评分变更或依赖。原 v1/v2 文件、历史评分、聊天/知识/图谱数据与模型调用逻辑保持原样。生产数据库未用于检查。
- 仅应用内弹窗采用内容范围限制；操作系统文件选择器继续由 Windows 管理。没有重复绘制窗口控制按钮。

## 验证与限制

实际命令、成功及失败后的修正均记录在 `milestone-log.md`。最终构建通过；两项必要数据库专项通过；标准双模型受控专项通过；最终三个原生弹窗/长面板/筛选专项通过（19.8 秒）。验证包括前端类型与构建、Windows 调试构建、两项必要数据库专项及隔离 WebView2 检查。没有运行无关全套单元测试、付费模型请求或下载模型。

物理鼠标拖动检查当前命中另一 Edge 窗口，归属保护因此拒绝发送鼠标输入；不能据此声称本轮物理拖动实测通过。标题栏命中、最小化/最大化/还原/关闭、焦点与弹窗关闭由独立原生专项检查。实际鼠标拖动、系统 150% 和读屏仍需当前增量人工复核，旧页面验收不替代新页面验收。

Docker、完整真实 Core60、不同显卡与长期大计划实测保持既有待办。原 Vite 主包/图谱大块提示仍在。下一步建议 6.3：右侧按需单题详情；需先验收本增量。

# Benchmark 6.1：报告按需读取与分页

2026-10-28。用户要求先列出完整阶段计划再继续实施，已形成 [6–8 实施顺序](benchmark-phase1-remaining-plan.md)。本轮完成 6.1 后暂停验收；排序筛选、右侧详情面板、页码深链和最终交互收尾按 6.2–6.5 继续。

## 可用行为

- 打开标准或个人评测报告、刷新进度时读取概览：整个计划的统计、模型比较、速度/Token/资源、偏好及选择依据保持可见。
- 每个模型的逐题列表默认每页 20 项，支持前后翻页。页数、总项数、冻结序号及重复轮次来自后端。列表只含状态和问题预览，不含回答。
- 展开某题才读取完整问题、回答、评分、处理记录、配置及采样；同时保留原始采样查看和 3D 执行定位。未运行项明确没有模型输出，错误保留原文。
- 运行中的已展开题目每 2 秒读取自身；收起或翻页后停止该题读取。其他关闭题目的正文不轮询。
- 快速翻页、切换批次或工作区时忽略过期响应；读取失败可重试。翻页保持全计划分母，原停止/恢复及发送范围确认继续使用冻结计划。

现有完整报告接口与个人 JSON 导出保留原行为，历史数据及评分没有改写。概览保留启动/恢复确认所需的计划及云端发送清单，不擅自裁掉用户需要授权的内容；它不是个人计划大小的恒定上限保证。

## 读取边界

| 命令 | 输入 | 返回 |
| --- | --- | --- |
| `get_benchmark_overview` | workspaceId、batchId、scope | `pagedResults=true`，全计划模型汇总；省略 items，各模型 results 为空，明细由分页命令读取 |
| `get_benchmark_result_page` | 同上及 modelIndex、page、pageSize | 当前页 metadata、total；page 从 0 开始，pageSize 限 1–50，UI 默认 20 |
| `get_benchmark_result_detail` | 同上及 ordinal | 冻结批次所属单题的完整已有详情；未执行时 execution/output 为 null |

scope 为 standard / personalized。后端验证工作区、批次、评测类型、模型、页码和题目归属；个人入口还沿用连接准入锁，标准入口不需要云端密钥。查询均为只读，没有模型调用、重新评分或通用数据库权限。

概览通过原统计/计量方法聚合小字段；读取终态 metrics 及必要计时事件，不读取 messages、请求快照、逐帧事件列表。资源峰值及读数数量由 SQLite 聚合，原始采样不送往概览。题库仍按既有不可变版本读取校验。单题详情使用原完整读取，保证原文和评分依据可追溯。

列表问题预览最多 240 字，超出加省略号；错误预览最多 1000 字，展开后读取完整错误。ordinal 是冻结计划中的全局序号，列表显示加 1；它不是 ExecutionRun ID。

## 实际修改文件

以下路径相对 `apps/desktop/`：

- `src-tauri/src/benchmark/report_read.rs`：新增概览轻量字段读取、资源聚合、分页/单题归属校验及 3 个受限业务读命令。
- `src-tauri/src/benchmark/personalized/report.rs`：复用原汇总过程的概览模式；完整报告继续读取全部详情。
- `src-tauri/src/benchmark/mod.rs`、`src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json`：登记已实现的业务读命令。
- `src/benchmark/PagedResults.tsx`：分页列表、单题懒加载、运行题目刷新及过期响应保护。
- `src/benchmark/PersonalizedReport.tsx`、`StandardBenchmarks.tsx`、`PersonalizedWorkbench.tsx`、`personalized.ts`：两类入口/轮询改读概览，接入分页并保留完整报告兼容。
- `src/benchmark/PerformanceDetails.tsx`：单题已有采样可直接展示，避免点击原始记录时重复取整题。
- `src/benchmark/benchmark.css`：复用现有 Tokens、列表和按钮，补充分页间距与长问题换行。
- `src-tauri/src/benchmark/standard_tests.rs`、`tests/benchmark-report-pages.spec.ts`、`tests/standard-benchmark.spec.ts`、`tests/personalized.spec.ts`、`playwright.config.ts`：必要只读归属/分页专项、原生分页与既有流程兼容。

没有新增依赖或迁移；数据库仍为 schema 10。没有修改题库、评分规则、知识引擎、模型执行或图谱实现。

## 验证与保留项

类型检查和 Windows 调试桌面构建通过；必要只读/分页专项 1 项、既有个人评测浏览器流程 3 项、最终原生分页与标准批量兼容 2 项通过。最终原生耗时 30.5 秒，证据在 `artifacts/benchmark-report-acceptance`。初次脚本的内置 invoke 拦截不生效已移除，改为真实返回内容与代码触发逻辑核对；没有将失效计数当成检查结果。

实际命令和修正过程见 [里程碑](milestone-log.md)。本轮验证新旧统计/性能一致、20 万字符回答不进入概览或列表、分页边界、归属拒绝、未运行详情、300 项计划的实际 WebView2 页面及既有标准停止/恢复流程。受控题目/响应明确作为检查数据，不作为模型能力成绩。

三尺寸沿用项目 HTML 风格及真实截图；未新增设计体系。本地设计资料库没有直接匹配的分页规则，分页细节按项目组件、语义化按钮和实际截图验证。

保留边界：概览仍聚合全计划的小字段及题库校验，尚未建立大型已完成报告的缓存；300 项样本为未运行计划，不代表 1800 份完整回答的耗时保证。个人分页真实受控全流程、跨页查询筛选、页码/单题深链、右侧固定标题详情、共享外观菜单问题及 150%/读屏按后续步骤检查。真实 Docker、付费云端、额外模型和完整 60 题未运行。

验收：打开既有标准报告 → 检查模型汇总 → 在“逐题结果”翻页 → 展开某题查看原文/评分/采样及图谱入口。无历史时可选择一个模型和全部题目 → 预览计划 → 刷新结果，查看未运行项的分页，无需点击确认运行。6.1 验收后进入 6.2 的比较排序与筛选。

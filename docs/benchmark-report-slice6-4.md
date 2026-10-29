# Benchmark 6.4：导航与返回定位

验收状态：2026-10-28，用户确认“验收完”，本增量已验收。以下保留实际交付、验证和未测边界；用户验收不替代尚未执行的专项。

2026-10-28。用户授权“做导航与返回定位”。本轮复用报告分页、单题详情、执行图和调度，不改变题库、评分或模型调用。交付后停在 6.4 验收；6.5 页面收尾、7 导出/复算、8 真实模型验收分别推进。

## 实际文件

路径相对仓库根。

| 文件 | 本轮修改 |
| --- | --- |
| `apps/desktop/src/navigation.ts` | 支持 push/replace 及 popstate，抽出内部链接构造；旧页面调用方式兼容 |
| `apps/desktop/src/benchmark/reportNavigation.ts`（新增） | 解析、校验、保存报告筛选/页码/题目；定位读取与过期响应保护；复制链接校验及执行图返回目标 |
| `apps/desktop/src/benchmark/useBatchOverview.ts`（新增） | 标准/个人共用的 URL 驱动概览，只读轮询、重新读取及当前目标身份检查 |
| `apps/desktop/src/benchmark/BenchmarkLink.tsx`（新增） | 通过已有共享弹窗打开内部定位链接，错误保留且可修改 |
| `apps/desktop/src/benchmark/BenchmarkPage.tsx` | 打开链接入口、工作区不匹配保护、已获准个人报告的重载入口 |
| `apps/desktop/src/benchmark/StandardBenchmarks.tsx` | 从 URL 读取批次；历史/新建明确区分；历史导航不被晚到请求覆写 |
| `apps/desktop/src/benchmark/PersonalizedWorkbench.tsx` | 接入共用概览、个人报告刷新恢复，保留原向导及生成任务轮询 |
| `apps/desktop/src/benchmark/PersonalizedReport.tsx` | 筛选/排序/模型/页码接入 URL；错误定位重试和清除条件；启动/停止方法沿用原逻辑 |
| `apps/desktop/src/benchmark/PagedResults.tsx` | 当前页和题目由导航控制；响应绑定查询身份，滚动定位及焦点返回；保留分页/计数/重试 |
| `apps/desktop/src/benchmark/ResultDrawer.tsx` | 固定标题增加复制定位链接；真实执行图携带返回信息；支持直接链接打开后的焦点返回 |
| `apps/desktop/src/components/AppDialog.tsx` | 可选焦点返回解析函数，其他弹窗沿用原返回规则 |
| `apps/desktop/src/App.tsx` | 执行图提供“返回评测结果”内部链接，拒绝跨工作区返回目标 |
| `apps/desktop/src/ExecutionTrace.tsx` | 按真实执行 ID 返回原报告/题目，节点切换保留原返回信息；独立图入口也能定位题目 |
| `apps/desktop/src-tauri/src/benchmark/report_read.rs` | 复用相同 SQL 筛选投影，新增限定工作区/用途/批次的只读题目位置查询 |
| `apps/desktop/src-tauri/src/{lib.rs,benchmark/standard_tests.rs}` | 登记只读命令；在已有两项数据库专项中补归属与定位检查 |
| `apps/desktop/src-tauri/{build.rs,capabilities/default.json}` | 仅登记 `get_benchmark_result_location` 业务读能力，保留已有图标构建修正 |
| `apps/desktop/tests/{benchmark-result-drawer,benchmark-report-pages,standard-benchmark}.spec.ts` | 导航/迟到响应/错误链接/真实 SQLite 定位/执行图往返专项 |

同步实施计划、阶段顺序、测试计划、HTML UI 规范和里程碑。无新依赖、表、索引或迁移；已有大量未提交改动保持原样，没有提交、推送或发布。

## 内部链接规范

示例：`#/benchmark?workspace=<工作区ID>&batch=<批次ID>&order=speed&model=0&repeat=3&page0=2&item=135`。

| 参数 | 含义 |
| --- | --- |
| `workspace` / `batch` | 工作区与固定批次 ID；后端校验归属 |
| `view=personalized` | 个人评测；省略表示标准评测 |
| `tab=setup/history` | 无批次时的新建/历史入口，避免返回列表后自动跳回活动任务 |
| `order=plan/correctness/coverage/speed` | 全计划比较排序 |
| `model=all/0/1/...` | 逐题区显示的模型，编号来自冻结计划 |
| `state` / `ability` / `difficulty` / `repeat` | 结果状态、评分器、难度和重复轮次，遵循现有筛选定义 |
| `page0` / `page1` / ... | 各模型独立页码，链接从 1 开始；接口从 0 开始 |
| `item` | 批次中的原始全局序号，从 0 开始；不以列表第几项或题目文本定位 |
| `trace` | 可选实际 ExecutionRun ID；返回时与批次/题目配对校验 |

复制保存应用内 hash；“打开定位链接”只接受内部 `#/benchmark?...`，拒绝外站链接和重复参数。没有注册 OS 深链接或将主机临时开发地址当作稳定地址。链接须在同一份仍包含该工作区/批次的本地数据库使用；不是跨机器分享报告功能。

## 行为与边界

1. 翻页、筛选、排序写入历史；换筛选从第一页开始，清除当前单题。各模型页码独立，表格比较分母仍是整个冻结计划。
2. 点击题目打开抽屉；页内切题替换当前题目历史，避免逐题堆积。关闭可返回打开前列表；直接链接打开时清除题目定位并保留筛选和页码。焦点回到触发项；没有触发项时回到本题或结果区。列表滚动到对应题目，正文读取仍按需。
3. 直接定位题目时先查真实所属模型、当前筛选下的页码，再加载列表及单题；不会遍历下载所有回答。有效题目不符合筛选时明确提示，可清除条件定位；错误 ID、过期页码、未知参数值保留重试或返回入口。
4. 抽屉进入执行图时带原查看条件。刷新图谱后仍可返回原题；图谱中切换同一运行的节点保留返回信息。独立执行图从 contextManifest 的真实 batchId/scope 和 ExecutionRun ID 查找所属项，不能把另一个执行 ID 配到原题。
5. 工作区、用途、批次、题目及执行配对均由后端查询限定；个人入口继续经过现有 access gate。标准入口不需要云端密钥。新查询不读取普通聊天历史，不修改知识或记忆。
6. 概览及分页响应与当前查询身份绑定，换目标后不展示旧批次/旧题；迟到响应不能重写新 URL。进行中批次仍只读轮询，原启动、停止和继续动作需要用户点击。刷新、定位、复制、返回不会自动发送、重新评分或重新批准。
7. 个人向导未保存的项目选择等仍是原本的页面状态；本轮恢复的是已保存报告，不序列化整个生成向导。原运行中的失败/未运行/不支持仍按既有状态展示。

## 验证

实际命令、结果和失败修正过程见本轮里程碑。

- 类型检查、离线 Rust 检查和最终桌面调试构建通过。
- 必要数据库专项 2 项通过；未运行其余 77 项。
- 浏览器 6 个相关场景分别通过：最终导航单项 1 条，前次组合中的个人恢复/隔离、原详情和三尺寸向导 5 条；前次名称错误的失败及修正完整保留。
- 原生 300 项 SQLite 定位专项 1 项通过；执行图/停止恢复、知识历史与共享弹窗专项 3 项通过。
- 已实际生成三尺寸浅深截图，查看代表图；验收窗口已打开且可见，复用原数据，未自动评测。

测试使用独立数据库；浏览器合成数据、受控模型 HTTP 响应不冒充真实模型成绩。桌面构建不等于安装包发布。

## 验收建议

- 标准报告筛选重复轮次、翻页、打开单题，复制定位链接；刷新与历史前进后退，核对条件/页码/题目。
- 从一条已有实际执行的结果进入 3D 图，刷新并返回，核对原模型/轮次/题目。
- 关闭抽屉后粘贴定位链接重新打开；错误链接应显示可恢复错误，不能重发模型请求。
- 验收后再推进 6.5；完整系统缩放、读屏、长时任务、真实个人云端与 Docker 保持独立待办。

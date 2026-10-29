# Core60 v2：阶段A修订与验收

2026-10-24。用户已批准三级设计，并将本次实施限定为阶段A：题库版本化修订、静态校验与相关测试。Level 2的120题及Level 3的32个场景继续保留设计。阶段B的运行时评分器等待本阶段验收。

后续状态：用户已验收阶段A，后续[阶段B运行时评分](benchmark-scoring-stage-b.md)另行交付；本页保留阶段A检查范围及当时未实现项，原题库文件及hash保持不变。

## 交付与文件

| 文件 | 实际用途 |
| --- | --- |
| [v2 cases](../apps/desktop/src-tauri/datasets/cortex-core-60-v2/cases.json) | 修订后的60题，ID、类别与难度分布保持原样；93条Python断言 |
| [v2 manifest](../apps/desktop/src-tauri/datasets/cortex-core-60-v2/manifest.json) | 数据版本2、schema版本2、Smoke定位、文件hash与未开放运行状态 |
| [v2 case.schema](../apps/desktop/src-tauri/datasets/cortex-core-60-v2/case.schema.json) | 新版数据结构；评分契约必填，RAG改用结构化JSON标准 |
| [v2 scoring-rules](../apps/desktop/src-tauri/datasets/cortex-core-60-v2/scoring-rules.json) | 六类评分规格版本2、独立状态与分母约定；标明运行时尚未实现 |
| [完整逐题差异](../apps/desktop/src-tauri/datasets/cortex-core-60-v2/revision-notes.json) | 60个ID逐项记录原因、旧/新hash及各字段旧/新值；包括全部断言变化 |
| [可复现编制检查](../scripts/benchmark_core60_v2.py) | 读取固定v1编制v2，默认逐字节检查；`--write-new`仅允许创建新目录，拒绝覆盖已有版本 |
| [dataset.rs](../apps/desktop/src-tauri/src/benchmark/dataset.rs) | 显式注册v1/v2、读取和事务安装；旧字段的序列化保持原样 |
| [dataset_v2.rs](../apps/desktop/src-tauri/src/benchmark/dataset_v2.rs) | 作者标准、契约、定义域、引用标注和时间条件的有限静态校验 |
| [dataset_v2_tests.rs](../apps/desktop/src-tauri/src/benchmark/dataset_v2_tests.rs) | 新版数据反例、版本冲突、失败回滚和合成历史兼容性专项 |
| [repository.rs](../apps/desktop/src-tauri/src/benchmark/repository.rs) | 开发执行入口明确按版本1查找，避免目录同时提供v2后误绑定 |
| [原生预检检查](../apps/desktop/tests/benchmark-preflight.spec.ts) | 更新双版本目录预期，增加v2事实及契约的IPC读取检查 |

同步文档为[索引](README.md)、[实施计划](implementation-plan.md)、[Benchmark计划](benchmark-implementation-plan.md)、[测量方法](benchmark-evaluation-methodology.md)、[预检契约](benchmark-preflight.md)、[三级设计](benchmark-tiers-design.md)、[评分提案](benchmark-scoring-v2.md)、[里程碑](milestone-log.md)及本页；原审计与原题目审阅表保留历史结论。

## 12项问题复核与处理

以下依据实际v1文件和当前代码核对，不把阶段B尚未完成的内容勾为已解决。

| 审计项 | 重新核对的依据 | 阶段A处理与剩余门槛 |
| --- | --- | --- |
| Q-01 | manifest的35 easy/25 medium/0 hard；60题题面 | v2明确Smoke，保持题量/难度，不生成正式能力题 |
| Q-02 | cases中的math-006与tool-002；规则integer_equal/native_tool_equal | 数列限定等差；工具题明确limit=3，参数Schema仍定义该工具通用范围1–10，任务语义明确要求3 |
| Q-03 | scoring-rules的evidence_match完全别名/跨度匹配；rag-001/002/008 | 所有RAG标准改为status/facts/reason/citations；时间有显式别名，天数/楼层等用整数；逐事实标注可接受证据跨度 |
| Q-04 | rag-004/005及原规则的nonempty explanation | 不可答固定facts={}、reason=insufficient_evidence、citations=[]，取消任意自由说明字段；拒答与不补造事实的运行判定留阶段B |
| Q-05 | repository::finish_benchmark仅两种判定；Ollama FrameMessage缺工具字段 | 新增的仍是评分规格及数据校验，没有假冒运行时评分器；四类完整评分/原生调用保存/Docker均待阶段B |
| Q-06 | preflight::inspection/能力失败处理及Runner的两题白名单 | 保留python_sandbox为环境需求，不能直接传入模型能力预检；规则明确六种状态，不支持/未运行/执行错误/评分错误不进正确率分母。需求分流、状态映射及汇总实现待阶段B |
| Q-07 | zh-014别名及exact_alias的逐字规则 | 增添补入；题目明确从三词中选，封闭合法集合；书名接受论语/論語；单位补公尺、秒钟；其余明确简体及ASCII格式，保持化学式大小写 |
| Q-08 | code-003/008题面及dataset对函数/断言的原结构检查 | 逆序明确码点；空白采用Python3.13规则并固定29个码点；返回类型和输入不可修改写进契约与题面 |
| Q-09 | 原10题32条断言 | 保留原断言前缀，补至93条；静态验证参数/返回值类型与合法域。真实类型/输入修改/资源检查仍待Docker评分实现 |
| Q-10 | rag-010新旧生效通知；rag-006/007同条件冲突 | 所有场景固定as_of；rag-010显式生效/取代关系；冲突题无权威优先，两方事实/引用均必须提供 |
| Q-11 | dataset::example_matches的小词汇示例检查及schema-004 | 五份JSON固定2020-12，日历日期增加format=date；所有原字段仍必填。阶段A检查作者标准的类型/日期，不宣称完整通用Schema运行验证器已完成 |
| Q-12 | dataset原固定版本1、DB复合主键及不可变触发器 | 新目录与显式版本注册；同事务安装、同版本冲突拒绝；输入/标准/评分按版本读取，历史评分保持原样 |

## 保守规则及具体差异

### 中文与数学

所有题目仅去首尾Unicode空白，化学式及拼音保持大小写敏感，不做全局简繁转换、NFKC或标点删除。zh-003显式列出两个书名字形，其他中文题面明确简体。zh-014用明确三选一保留原两个答案并补“增添”，避免声称已穷举开放中文反义词。

15道数学题仍用原标准整数；math-006只增加等差条件。全部明确可选ASCII负号与半角数字、禁止加号/小数/解释。语法与数值的实际判定仍使用各版本自己的评分器，v2不借用v1的已保存分数。

### Python

| ID | 原/新断言数 | 新增覆盖重点 |
| --- | ---: | --- |
| code-001 | 3 / 7 | 单元素、全负、重复、64个10^9之和 |
| code-002 | 3 / 8 | 重复偶数、单偶/负奇、全偶、64个0 |
| code-003 | 3 / 10 | 单码点、空白、重复、非BMP、组合符、ZWJ、长度上界 |
| code-004 | 3 / 7 | 负数重复、单元素、全不同且顺序保留、长度上界 |
| code-005 | 3 / 10 | Aa区分错误lower实现、偶数回文、单字符、空白、标点、非BMP、长度上界 |
| code-006 | 4 / 10 | 等于上下界、负区间、相同界、最小最大输入；不测非法low>high |
| code-007 | 4 / 13 | 穷举题面定义域0–12，不暗测负数或13 |
| code-008 | 3 / 11 | 全空白、无空白中文、单词、全角空格/NBSP、29种空白、非空白U+200B、长度上界 |
| code-009 | 3 / 8 | 右空左非空、重复负数、长度不均、全相等、两列表长度上界 |
| code-010 | 3 / 9 | 单个0/负数、全负/全0、重复、累计超过32位整数 |

统一输入域为整数绝对值≤10^9、单个列表/字符串长度≤64，阶乘保持0–12，clamp及merge原前提保留。整数返回不接受bool/float，布尔必须bool，列表必须list且整数元素为int；不得修改任意输入。Python3.13是新契约目标，不表示已下载对应Docker镜像或验证镜像身份。镜像digest、资源额度与隔离实测在阶段B冻结；当前只验证可信作者的固定数据，未执行模型返回代码。

### RAG

每道题给出明确事实槽位。示例：rag-003使用retention_days=30，rag-008使用floor=2；冲突题分别保存两来源的预算/限额。拒答没有可夹带编造的自由说明字段，reason使用固定枚举。

事实与引用分开评分：引用以claim_id关联事实；source_id及1基闭区间必须存在，并属于该事实预审的支持范围集合。允许包含必要证据的相邻无冲突行，例如rag-008允许map:2–2、1–2、2–3、1–3；只有map:1–1不能支持服务台楼层。此处是作者预先标注的支持关系，不是评分器读懂任意文本的语义证明。

所有场景时刻固定`2026-07-02T12:00:00+08:00`；rag-010中新通知已生效且取代旧通知，不能按运行日变更标准。两道不可答题的引用准确率为N/A；可答题无证据不得完整通过。冲突题需要两方事实及两方证据；答案正确不能抵消引用失败。

RAG的response_schema是**评分用结构定义**，契约标schema_use=grading_only，模型需求仍completion；避免把资料问答误当原生Schema能力。schema-*才要求Provider的JSON Schema能力。阶段B必须按此区分请求构造。

### JSON与工具

5道JSON的原字段全部必填，没有新增可选业务字段，note=null仍必须出现；额外字段禁止。对象键顺序不计，数组顺序保留，布尔/数字/字符串/null/缺失区分；整数允许数学上等价的整数表示，拒绝重复键。schema-004需要真实日期而不只是匹配数字形状。完整2020-12验证及数字语义控制响应在阶段B实现；静态作者标准采用规范整数写法，不将作者侧i64检查当通用Schema实现。

工具题保留一个调用的Smoke范围，参数键顺序不计，整数语义等价形式可接受。tool-002题意明确3，无需猜测1/2是否满足“最多”；tool-001固定日期。五题只核对真正Provider调用对象，预定调用数恰好1，不运行工具或模拟宣称查询成功。阶段A未增加原生tool_calls解析。

## 版本与存储兼容

| 项目 | v1 | v2 |
| --- | --- | --- |
| 数据目录 | cortex-core-60，原文件不变 | cortex-core-60-v2，新建5个文件 |
| 数据/schema/评分规则版本 | 1 / 1 / 1 | 2 / 2 / 2 |
| manifest SHA-256 | 3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc | b8d462c2408f948c9a1f74d1e044cb4f78dbfabe52f76f93e6d135ee171edb70 |
| cases SHA-256 | 784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806 | 8a575711712d352d9e61d915c6b95b01e54c4b280e3cb3fb8175404c4e168174 |
| 审阅状态 | 原needs_human_review | needs_human_review，等待本阶段验收/独立人工签审 |
| 运行入口 | 仍仅原两道开发题 | 目录与单题读取可用；运行未开放 |

Scoring.contract存入现有scoring_json；不加SQL字段、不改迁移。新增可选Rust字段在v1序列化时省略，因此历史题目及配置保持原内容。目录返回按版本1/2排列；执行端明确查找版本1，v2 hash传给旧开发入口会被拒绝。

双版本定义在同一个事务中安装，已存在内容逐字段核对；新版本中途失败不影响已有v1。新schema并不升级用户数据库schema版本；仍为开发schema8、正式v0.2.0为schema7。本轮未打开正式用户库；没有新SQL迁移，因而没有声称本轮做了新迁移备份。现有迁移与恢复路径保留并由专项回归核对。

没有重新评分接口或覆盖历史评分的操作。阶段B若加入重评分，将追加独立evaluator版本及结果；不得借安装新题库重写历史记录。

## 验证与边界

已运行的命令、最终结果、初次失败和范围校验见[本阶段里程碑](milestone-log.md)。数据静态检查与运行时评分测试分别记录；构建通过不代表60题模型能力或Docker隔离通过。

已知剩余：六类v2运行时评分、原生调用捕获、Docker、状态汇总/报表、真实模型完整Smoke均未实现或未执行；没有将这些填成通过。阶段A的引用支持集合和别名仍需要用户/独立审阅者确认，技术校验不能替代人工语义审阅。

本阶段完成后暂停。验收后才进入阶段B：实现真实评分器及正反控制响应，优先保证确定性、失败分类、逐断言结果和历史评分隔离；不自动进入原切片4、生成新能力题、接入云端或发布版本。

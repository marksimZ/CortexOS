# Cortex Core 60 v1：题库与评分审计

2026-10-23。按用户最新决定，Cortex Core 60只定位为 **Level 1 Smoke Test**，用于基础功能、输出协议及评分器验证，不用作正式模型能力排行榜。本报告独立于原审阅表保存；这是AI技术复核，不是第三方盲审或独立人工认证。用户已阅读原审阅表，本轮没有把题库状态改成“全部通过”。

## 审计范围和证据

完整读取了[cases.json](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json)、[manifest](../apps/desktop/src-tauri/datasets/cortex-core-60/manifest.json)、[题目schema](../apps/desktop/src-tauri/datasets/cortex-core-60/case.schema.json)、[评分规则规格](../apps/desktop/src-tauri/datasets/cortex-core-60/scoring-rules.json)，并逐项追到实际Rust入口。源码和数据本轮保持原样，没有生成v2或新增题目。

| 项目 | 检查事实 | 不能据此声称 |
| --- | --- | --- |
| 60道题 | 全部逐条阅读题目、标准、别名、规则、能力要求和材料 | 全部已获人工验收、全部有运行时评分器 |
| 数学15题 | 固定算式复算，数值都与标准一致；math-006需额外假设 | 数列题无歧义、构成困难推理测量 |
| 代码10题/32条断言 | 用可信审核脚本的固定控制计算核对全部JSON输入/预期值，32条值一致 | 已执行待测函数、通过Docker隔离或边界覆盖充分 |
| RAG 10题 | 逐项阅读全部13份虚构材料、22行；10个标准引用跨度均在范围内 | 行号有效就代表语义被支持；检索效果已测 |
| JSON 5题 | 已存标准符合当前简单schema词汇，字段和值匹配题意 | 已存在通用JSON Schema响应验证器 |
| 工具5题 | 工具名及标准参数符合所附声明 | 已验证原生调用对象或真实执行成功 |

版本1的manifest SHA-256：`3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc`；cases SHA-256：`784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806`。分布15知识/15数学/10代码/10资料/5 JSON/5工具；标签为35 easy、25 medium、**0 hard**。来源与许可只有整包声明，知识事实缺逐题来源与适用条件，需要补齐。

审核脚本及完整逐断言记录在本机`artifacts/benchmark-quality-audit/audit_core60.py`和`audit-data.json`。脚本只读取现有数据并做固定控制计算，不加载模型代码或待测函数；没有运行模型、Docker、单元套件或构建。

## 评分器实际实现边界

| 规则 | 规格和实际实现 | 审计结论 |
| --- | --- | --- |
| exact_alias v1 | [repository.rs:396](../apps/desktop/src-tauri/src/benchmark/repository.rs#L396)去首尾空白后逐字比较 | 已实现，但[启动入口](../apps/desktop/src-tauri/src/benchmark/repository.rs#L94)只允许zh-006，不能宣称15道中文题都已运行验收 |
| integer_equal v1 | [repository.rs:397](../apps/desktop/src-tauri/src/benchmark/repository.rs#L397)可选ASCII负号、ASCII数字、i64比较 | 已实现，只开放math-001；`+95`不接受，题面却没有约定这种词法限制 |
| python_assertions v1 | 数据有32条断言；[dataset.rs:227](../apps/desktop/src-tauri/src/benchmark/dataset.rs#L227)只检查标识符和至少3条等结构 | Docker执行器、类型/输入修改检查和实际断言评分未实现；“32条值一致”不是编程题通过率 |
| evidence_match v1 | 规格要求状态、别名、完全相同的引用跨度；[dataset.rs:239](../apps/desktop/src-tauri/src/benchmark/dataset.rs#L239)检查材料ID/行范围 | 运行时证据评分未实现。原规格有误判及漏判风险，见Q-03/Q-04 |
| schema_semantic v1 | 规格要求先结构校验再语义比较；[example_matches](../apps/desktop/src-tauri/src/benchmark/dataset.rs#L121)只核对作者给出的简单示例 | **当前没有模型响应的通用结构化评分器，也不是已经在用字符串比较评分**；不能混淆“设计正确”与“实现完成” |
| native_tool_equal v1 | 规格明确只接受真正Provider tool_calls | [FrameMessage](../apps/desktop/src-tauri/src/ollama.rs#L30)只收role/content，[generation](../apps/desktop/src-tauri/src/generation.rs#L125)只累加文字，空文字终帧判失败；原生工具字段尚未保存/验证，不能说已验证工具成功 |

### 能力标记的现状

[preflight.rs:170](../apps/desktop/src-tauri/src/benchmark/preflight.rs#L170)可表示supported/unsupported/unknown，保留api_show_advertised与not_probed。服务明确列出能力但未含tools时，按自报标“不支持”；缺列表是“未知”。JSON Schema当前保持未知，不能等同不支持。[能力失败处理](../apps/desktop/src-tauri/src/benchmark/preflight.rs#L340)返回blocked及capability_unsupported，或needs_verification及capability_unknown。

这还不是逐题统计：Runner只开放两道题，其余58题是**当前实现范围未开放**，不能算58道“模型不支持”。数据库允许unsupported状态，不代表已有完整写入/汇总路径。code题的python_sandbox是执行环境需求，而[模型请求校验](../apps/desktop/src-tauri/src/benchmark/preflight.rs#L35)只接受completion/json_schema/tool_calls，不能直接把整份题目requirements塞给模型预检。缺Docker、缺评分器、模型不支持和能力未知要分开。

## 问题清单与修订优先级

P0表示正式能力比较前的阻断项，不表示本轮立即修改代码。

| 编号/优先级 | 证据与问题 | 建议及验收条件 |
| --- | --- | --- |
| Q-01 / P0 | Core60难度低且无hard；同类短题与显式给选项题较多 | 保留为Smoke；Level 2另建并独立审阅/校准，禁止从这60题推导正式排名 |
| Q-02 / P0 | math-006只列四项，不能唯一确定下一项；tool-002“最多3条”与只接受limit=3不一致 | 新版限定等差数列；工具题指定limit必须3，或评分允许满足1≤limit≤3的参数。旧版不覆盖 |
| Q-03 / P0 | RAG“答案或说明”未要求逐字短答案，规格却只接受固定字符串和完全相同跨度 | 结构化事实/单位/日期与预审别名；接受审阅过的等价证据组合；将支持性与引用冗余分开计量 |
| Q-04 / P0 | rag-004/005不可答只要求answer非空；即使写错误断言，只要status正确也可能满足文字规格 | 不可答采用明确拒答结构/原因枚举，检查不额外捏造；无引用时precision为N/A，单列拒答正确率 |
| Q-05 / P0 | JSON/工具/RAG/代码评分尚未运行实现 | 正负控制响应、独立oracle、Schema一致性、真正tool_calls解析、Docker隔离验证完成前不报告这些维度成功率 |
| Q-06 / P0 | 未开放/缺环境/未知能力可能被混成unsupported或模型答错 | 分离模型能力、系统能力、评分器可用性和运行/评分状态；先冻结计划及分母，再报告覆盖率 |
| Q-07 / P1 | zh-014漏合理反义词“增添”；中文简繁/大小写/数字表示政策未逐题声明 | 扩充经核对别名或限制题面范围；化学式保持大小写敏感，不能全局无差别折叠 |
| Q-08 / P1 | code-003“Unicode字符”含义不定；code-008空白定义未固定；输入修改/返回类型政策缺失 | 声明码点或字素簇、空白规则和Python/Unicode版本；补返回类型、输入不变和失败分类 |
| Q-09 / P1 | 10道代码仅3或4条断言；当前不区分bool/int/float或序列返回类型 | 补边界、等价错误实现反例及类型检查；不能只用Python宽松相等或先JSON化后比对 |
| Q-10 / P1 | rag-010缺固定as_of，最新生效取决于执行日期；来源无时区/权威规则 | 固定场景时刻和生效条件；相冲突材料预设权威关系，禁止用机器当前时间改变标准 |
| Q-11 / P1 | 示例检查用as_i64，非完整JSON Schema；日期只声明string | 成熟且声明方言的结构验证，独立语义断言，显式format断言政策；不要将作者示例检查直接升级成通用评分器 |
| Q-12 / P1 | v1包和加载器固定版本/字段；仅换文件会撞不可变hash/反序列化校验 | 新版目录及显式版本注册，必要时新schema/评分器版本；旧快照可读、不被新标准静默重判 |

## 逐条审计：中文15题

“标准可成立”只评价当前题意与值；不代表运行通过或人工认证。别名精确规则目前只在zh-006开放；其余中文题仍待后续实际评分接入。

| 题目与代码位置 | 题意/标准与别名核对 | 问题及修订建议 |
| --- | --- | --- |
| [zh-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L3) | shān去声调为shan，标准可成立；[教育部字典](https://dict.revised.moe.edu.tw/dictView.jsp?ID=8926&la=0)。 | 题意明确；补明小写或大小写政策。适合拼音格式Smoke，不测复杂中文理解。 |
| [zh-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L30) | 日部可核对于[教育部字典](https://dict.revised.moe.edu.tw/dictView.jsp?ID=1483&la=0&powerMode=0)。 | 未发现该标准错误；为减少字典分类口径差异，新版明确所据字典/字形，不把“通常”当精确规范。 |
| [zh-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L57) | 《论语·为政》有该句，论语为正确选项；[原文](https://ctext.org/analects/wei-zheng/zh)。 | 未限制简体，繁体“論語”可能被误拒；新版声明目标字形或补别名。二选一只测基础识别。 |
| [zh-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L84) | 孟浩然正确；[教育局材料](https://www.edb.gov.hk/attachment/tc/curriculum-development/kla/chi-edu/resources/secondary-edu/pth/20160528_cuhk.pdf)含作者与诗句。 | 题意明确，姓名无须任意模糊匹配；保留。 |
| [zh-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L111) | 常规四季顺序下秋季/秋天/秋均合理。 | 明确采用通常四季划分即可；三种别名覆盖常用表达。 |
| [zh-006](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L140) | 100正确，已开放开发执行。 | 只写数字但未约定半角；新版明确ASCII十进制。是格式/存储Smoke，不是能力证据。 |
| [zh-007](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L167) | H2O正确；[PubChem Water](https://pubchem.ncbi.nlm.nih.gov/compound/WATER)。 | 题面已要求普通字母数字，下标字符不属当前格式；不要为扩别名接受错误大小写h2o。 |
| [zh-008](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L194) | 月球/月亮为通常教科书答案；[NASA](https://solarsystem.nasa.gov/moons/earths-moon/in-depth.amp)支持这一常规表述。 | 建议限定“长期稳定的天然卫星”，减少临时捕获天体的术语争议；不是发现现有月球答案错误。 |
| [zh-009](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L222) | 水星正确；[NASA Mercury](https://science.nasa.gov/mercury/facts/)。 | 太阳距离基准清晰，保留；不混成“离地球平均最近”问题。 |
| [zh-010](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L249) | 二氧化碳正确；[NASA光合作用材料](https://spaceplace.nasa.gov/coloring-pages/en/Carbon_Cycle.pdf)。 | 题意清楚，保留中文标准；CO2不在题目要求的中文格式内。 |
| [zh-011](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L276) | 米正确；[BIPM SI基本单位](https://www.bipm.org/en/measurement-units/si-base-units)。 | 可保留；是否接受公尺需依中文地区政策预先决定，不能运行后补别名。 |
| [zh-012](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L303) | 秒正确；[BIPM](https://www.bipm.org/en/measurement-units/si-base-units)。 | 题意清晰，保留；“秒钟”等是否接受应预先声明，不能让文字变体主导能力分。 |
| [zh-013](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L330) | 快乐/高兴均合理，题面明确只从二者选。 | 其他近义词不在当前选择范围，并非漏标错误；该题接近遵循选项指令，不宜称开放语义理解。 |
| [zh-014](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L358) | 增加/增多都正确，但集合不完整。 | [教育部字典](https://dict.concised.moe.edu.tw/dictView.jsp?ID=22896&la=0)还列“增添”；它符合双字词要求。新增版本补别名或明确二选一，当前不可据此把增添判语义错误。 |
| [zh-015](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L386) | 二选一时到达正确，年龄不符合先来后到。 | 题意明确但几乎暴露答案，适合指令Smoke；正式集另测上下文中的顺序与例外。 |

## 逐条审计：数学15题

全部标准数值均复算一致，math-006只是在等差假设下成立。整数评分v1限制ASCII词法且拒绝加号，v2应将此写入题面或扩大有审计依据的解析，数值正确和格式正确分开。以下题目不要求模型提供隐藏思维链。

| 题目与代码位置 | 独立复算/标准 | 题意与建议 |
| --- | --- | --- |
| [math-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L413) | 37+58=95 | 明确；已开放执行，仅基础加法。 |
| [math-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L438) | 144−79=65 | 明确；未开放运行，不标已通过。 |
| [math-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L463) | 17×6=102 | 明确；保留Smoke。 |
| [math-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L488) | 156÷12=13，余数0 | 明确；不涉及有理数/容差评分。 |
| [math-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L513) | (8+7)×3−9=36 | 运算顺序清晰，属于短多运算而非困难推理。 |
| [math-006](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L538) | 假定公差3时11+3=14 | **需修订**：有限前缀有多个延伸规律；应明确“等差数列”。不能把其他合理延伸一律判错。 |
| [math-007](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L563) | 12×7=84平方厘米 | 题面已给输出单位且要求省略，明确。 |
| [math-008](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L588) | 200×8/10=160元 | 八折语义清楚，标准正确；无需浮点误差容忍。 |
| [math-009](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L613) | 18×3−12−20=22 | 平均数/已知数条件完整，明确。 |
| [math-010](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L638) | 5+3+2=10 | 明确但非常基础，不构成中高难度证据。 |
| [math-011](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L663) | (30+4)/2=17；乙13 | 两条件一致，明确；medium标签尚无经验校准。 |
| [math-012](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L688) | 交集{3,4}，大小2 | 问数量不是集合输出，明确。 |
| [math-013](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L713) | 11×60+15−(9×60+40)=95分钟 | 当天条件排除跨日，明确；是基础时间差。 |
| [math-014](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L738) | C(12,2)=66 | 无向、每对一次语义明确，保留。 |
| [math-015](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L763) | ceil(24/5)=5 | 每盒最多5和至少几盒条件明确；四盒最多20不足。 |

## 逐条审计：Python10题及全部32条断言

表内列出全部现有输入→输出，固定控制计算与这些值一致。**所有10题均有边界/协议补测项，均未执行Docker或待测实现**。新增反例是修订建议，不是已经加入数据的题目。

Python字符串与用户感知字素不是同一口径；参见[Python str文档](https://docs.python.org/3.13/library/stdtypes.html#text-sequence-type-str)与[Unicode字素分割](https://www.unicode.org/reports/tr29/)。Python布尔与整数的比较关系也要求显式类型检查，不能拿简单`==`充当类型契约。[Python类型文档](https://docs.python.org/3.13/library/stdtypes.html#boolean-type-bool)

| 题目与代码位置 | 全部现有断言及值核对 | 缺口/应补边界 |
| --- | --- | --- |
| [code-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L788) | sum_numbers：`[]→0`；`[1,2,3]→6`；`[-3,0,2]→-1`，3/3值一致 | 加单元素、全负、重复值、大整数、输入不变。声明输入长度/数值范围，避免性能标准隐含。 |
| [code-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L842) | count_even：`[]→0`；`[0,1,2,-4]→3`；`[1,3]→0`，3/3 | 已含0、负偶数和全奇；缺重复偶数`[2,2,2]→3`、单元素、全偶、负奇。错误的“不同偶数个数”逻辑会漏检。 |
| [code-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L896) | reverse_text：`""→""`；`"abc"→"cba"`；`"你好a"→"a好你"`，3/3 | “Unicode字符”需限定码点或字素簇。若选码点，`e + U+0301`逆序不同于人眼字素；另补单字符、空白、重复字符及非BMP码点。 |
| [code-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L942) | dedupe：`[]→[]`；`[2,1,2,3,1]→[2,1,3]`；`[0,0]→[0]`，3/3 | 已测顺序/重复；补负数、全不同、单元素、返回list类型及不修改输入。先JSON化会掩盖tuple/list差异。 |
| [code-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1003) | is_palindrome：`""→true`；`"上海海上"→true`；`"Ab"→false`，3/3 | **大小写要求尚未被有效区分**：错误地先lower仍能过这3例；加`"Aa"→false`、`"abba"→true`、单字符和带空白/标点，布尔返回不可被1/0代替。 |
| [code-006](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1049) | clamp：`(5,0,10)→5`；`(-1,0,10)→0`；`(12,0,10)→10`；`(4,3,3)→3`，4/4 | 已含内/外和相等上下界；补x恰等low/high、负区间、相等界且x等界、大整数。low>high已不在题目定义域，不应暗加错误断言。 |
| [code-007](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1109) | factorial：`0→1`；`1→1`；`5→120`；`12→479001600`，4/4 | 0、1与上界已有；定义域仅13个值，建议全覆盖0–12及整数返回。负数和13不在定义域，若要测试需先改题目。 |
| [code-008](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1161) | word_count：`""→0`；`"  a\tb\n c  "→3`；`"你好 世界"→2`，3/3 | 补纯空白、无空白中文、单词、U+3000/NBSP；明确按固定Python版本`str.split()`空白定义，而不是另加未说明的语言分词。 |
| [code-009](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1207) | merge_sorted：`([],[])→[]`；`([1,3],[2,3])→[1,2,3,3]`；`([],[-1,0])→[-1,0]`，3/3 | 缺右边空且左非空、偏斜长度、重复负数/全相等、返回list、不修改两输入。当前标准值正确。 |
| [code-010](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1273) | prefix_sums：`[]→[]`；`[1,2,3]→[1,3,6]`；`[-2,2,5]→[-2,0,5]`，3/3 | 补单元素0/负数、全负、全0、重复和大整数；返回等长list、不修改输入。 |

共同协议缺口：题目写“纯函数”但没有明确输入修改检查；类型、异常、stdin/stdout、导入和资源边界需要在评分契约固定。增加断言数只是起点，还需验证能否拒绝已知错误实现。非法输入只有题面明确定义行为才可作为扣分项。

## 逐条审计：RAG10题

这些材料直接随题提供，不经过CortexOS知识库检索，测的是**固定上下文问答/引用**，不是检索召回或认知记忆。下表按虚构材料内部语义核对，不把其日期/人名当真实世界事实。

| 题目与代码位置 | 标准及证据核对 | 缺口/修订建议 |
| --- | --- | --- |
| [rag-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1335) | notice:1明确周一09:00，支持答案 | 9点/上午九点也表达同一事实，但规格只认09:00；需类型化时间或明确固定格式。notice:1–2仍包含充分证据，应区分支持性与冗余。 |
| [rag-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1379) | team:1负责人林澄，team:2只是复核人 | 标准支持充分；“负责人是林澄”的说明句会被精确别名误拒。事实主体/属性应明确，引用复核人不能算正确支持。 |
| [rag-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1423) | policy:1保留30天，标准正确 | 30天/30日有同义单位，需预审归一。引用policy:2的归档操作不能支持时长。 |
| [rag-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1467) | 仅周六开放和工作日20元，不能推出周六票价；unanswerable成立 | 非空说明不是拒答语义检验，status不可答却填“42元”不能得分。答案字段/原因需结构化，无引用不记precision=1。 |
| [rag-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1503) | 只有阶段进展，无启动年；unanswerable成立 | 同上，必须禁止补造年份；不能把任意非空字符串当正确解释。 |
| [rag-006](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1539) | a:1预算80万元，b:1预算95万元，同日期同项目，conflict成立 | 两份材料应共同引用；“80与95不一致”不能因多了说明而判错。v2规定是否必须列出双方值和无权威优先关系。 |
| [rag-007](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1595) | a:1每人2份，b:1每人3份，同生效日，conflict成立 | 同上；需区分识别冲突与擅自选一份。原精确别名只能测标签，不足测理由完整性。 |
| [rag-008](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1651) | map:2服务台二层，二层/2层均正确 | 可补“第二层”等审阅别名或用楼层整数；不能把入口一层当服务台。map:1–3支持但较冗余，不宜与错误行号同罚。 |
| [rag-009](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1697) | task:2交付2026-06-18，与task:3复盘20日区分；两个现有日期别名合理 | 建议ISO日期值与合法日期校验；“6月18日”缺年份是否允许必须声明，不能隐式采用机器年份。 |
| [rag-010](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1743) | 在2026-07-01及之后，新通知取代旧通知，new:1支持周二 | **有条件成立**：题中无固定as_of，未来生效通知在不同执行日含义不同。v2冻结场景日期；需旧新对照时另定义所需证据，别随运行日改标准。 |

## 逐条审计：JSON5题

标准均为对象，不应与原始输出字符串逐字比。通用评分尚未实现，以下是数据/schema审查。JSON Schema的integer允许数学上为整数的1.0；日期format在2020-12中的断言能力也要明确启用。[数字规则](https://json-schema.org/understanding-json-schema/reference/numeric)、[验证规范](https://json-schema.org/draft/2020-12/json-schema-validation)

| 题目与代码位置 | 标准/schema核对 | 缺口/修订建议 |
| --- | --- | --- |
| [schema-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1794) | name=林禾，age=28；必填、禁止多字段、年龄非负，标准自洽 | 分开schema_valid与semantic_equal；字段调换顺序应等价。28.0与28的整数语义不能被宿主数值类型误判。 |
| [schema-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1839) | done=true，seconds=12；布尔与非负整数分明 | 不能让1代替true或字符串"12"代替数值。当前只是简单类型Smoke，不是复杂Schema。 |
| [schema-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1884) | colors=[红,蓝,绿]，数组长度3，标准顺序与题面一致 | item只限string，语义另管颜色/顺序；不能将数组当集合。需控制响应覆盖多/少元素和错误顺序。 |
| [schema-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1932) | date=2026-08-09，city=杭州，标准正确 | date只是string，不能声称测试了日历日期约束。新版声明date格式断言或独立日期语义检查，不能只加pattern便称日历有效。 |
| [schema-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L1976) | sku=A-17，stock=0，note=null；字段全必填且禁止额外字段 | 0/false、null/缺失/空字符串必须区分；适合保留为结构边界Smoke。 |

重复键要在解析阶段发现；单靠解析后的对象无法知道是否出现两次age。项目可选择严格拒绝重复键，但应说明这是本协议的严格政策；RFC8259要求成员名应唯一，并未把所有重复键文档都定义成语法非法。[RFC8259 §4](https://www.rfc-editor.org/rfc/rfc8259#section-4)

## 逐条审计：工具5题

当前都只有一个声明、工具名和参数几乎直接给出，适合作调用对象契约Smoke，不测多工具规划。元数据里的tools是请求声明，不能当作模型响应已产生tool_calls；纯文字JSON不得冒充原生调用。[Ollama原生调用协议](https://docs.ollama.com/capabilities/tool-calling)

| 题目与代码位置 | 标准/参数核对 | 缺口/修订建议 |
| --- | --- | --- |
| [tool-001](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L2026) | get_temperature(city=杭州)，参数与声明一致 | 没有温度返回值或工具执行，所以不能算查温度任务成功；只验证调用对象。若需要“当天”，固定场景时钟，不去调用真实天气API。 |
| [tool-002](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L2075) | search_notes(query=发布清单,limit=3)是合法参数之一 | **需修订**：“最多3条”同样允许1或2；函数schema还允许至10。应改明“limit=3”，或将期望改约束并拒绝>3。 |
| [tool-003](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L2131) | create_todo(title=检查报告,priority=2)，范围1–3合法 | 可保留契约Smoke；“创建”在此仅构造调用，没执行待办写入，不能计真实任务成功。 |
| [tool-004](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L2187) | convert_length(value=5,from_unit=m,to_unit=cm)，标准合法且明确 | 没有断言转换结果500cm，因此不测试换算执行；多参数类型核对可保留。 |
| [tool-005](../apps/desktop/src-tauri/datasets/cortex-core-60/cases.json#L2254) | get_document(document_id=DOC-12)，合法明确 | 没有返回文档或错误处理，只是参数转录；正式集再测返回值依赖和多步流程。 |

## 本轮结论与待批准修订

已逐条审查60题，并核对全部现有32条代码断言值；这不等于把60题标记为已通过。主要问题是若干题面/评分不一致、别名及边界不足，以及四类评分器尚未运行实现；没有发现可无条件宣布“整套正确完整”的依据。

建议后续经确认创建`cortex-core-60@2`，保持60题和旧版本可读，优先修正Q-02–Q-10。题目内容变化增dataset版本；判定语义变化增评分器版本，必要时增schema版本，旧输出只追加新评分、不覆盖历史。当前加载器固定v1，须显式加入版本注册/兼容读取，不能直接替换include_str文件。**本轮没有创建v2、修改cases/manifest/规则或勾选旧审阅表。**

新体系与执行依赖见[三级设计](benchmark-tiers-design.md)，分母与评分准入见[评分方案](benchmark-scoring-v2.md)。等待用户确认后再实施修订或继续原开发切片。

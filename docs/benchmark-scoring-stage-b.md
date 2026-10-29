# Core60 v2：阶段B运行时评分

2026-10-24。用户验收阶段A后授权继续阶段B，并确认本机尚未安装Docker：先交付实现，真实容器验证保留待办。本轮实现六类评分及单题闭环，不进入批量排程、评测页面、导出或新测试集开发。

## 实际文件

应用文件以下均相对`apps/desktop`。

| 文件 | 实际改动 |
| --- | --- |
| `src-tauri/src/benchmark/evaluation.rs`、`evaluation_tests.rs`（新增） | 独立结果状态、五类纯评分、分母统计、正反控制；Python交给隔离控制器 |
| `src-tauri/src/benchmark/evaluation_json.rs`（新增） | 严格JSON解码、重复键检查、精确十进制比较、真实Schema校验及原生调用片段归并 |
| `src-tauri/src/benchmark/python.rs`、`python_runner.py`、`python_tests.rs`（新增） | 本机Linux Docker检测、固定镜像ID/资源策略、逐断言隔离运行、返回类型/输入观察、宿主判定及清理；真实Docker检查显式ignored |
| `src-tauri/src/benchmark/scoring_repository.rs`、`scoring_repository_tests.rs`（新增） | 保存v2评分、输出与原生字段hash、状态/覆盖率/难度与规则分组；版本、幂等、错误保存专项 |
| `src-tauri/src/benchmark/runner_v2.rs`（新增）、`runner.rs` | v2专用单题执行；模型及代码环境预检、互斥/取消、真实调用、保存后评分；v1继续原路径 |
| `src-tauri/src/benchmark/repository.rs` | 显式数据版本、v2快照/资料装配、format/tools请求、按版本读取及源码/依赖指纹 |
| `src-tauri/src/benchmark/dataset.rs`、`mod.rs` | 不可变Case再次核验、运行能力目录与模块登记；创建时manifest状态和当前引擎能力分开 |
| `src-tauri/src/benchmark/preflight.rs` | 仅允许已固定身份且唯一待确认项为能力未知的v2任务，作一次当前题目的实际探针 |
| `src-tauri/src/generation.rs`、`ollama.rs` | v2严格帧解析、真实tool_calls记录、无文字调用完成、客户端单调计时；聊天/v1仍走原文字规则 |
| `src-tauri/src/benchmark/repository/tests.rs`、`dataset_v2_tests.rs` | 原夹具显式使用版本1，保留旧入口拒绝v2 hash的检查 |
| `tests/benchmark-runner.spec.ts` | 增加受控服务下的v2原生闭环，保留v1与真实模型检查开关 |
| `src-tauri/Cargo.toml`、`Cargo.lock` | 固定jsonschema 0.60.0、关闭默认外部解析功能；serde_json启用原数值文本和RawValue |

本页、文档索引、实施计划、预检/执行/评分方法及里程碑同步状态。前端src、现有UI、SQL迁移、知识引擎、正式用户数据库和两个版本的题库文件保持原样。

## 接口与版本绑定

沿用现有`start_benchmark / get_benchmark_run / cancel_benchmark`，未增加通用进程、文件或HTTP权限。start仍要求developmentOnly=true，且一次只运行一个模型的一道题。

```typescript
{
  launchKey: uniqueKey,
  developmentOnly: true,
  datasetVersion: 2,
  datasetHash: selectedDataset.contentHash,
  caseId: selectedCase.case_id,
  model: {
    model: selectedActualModel.name,
    expectedDigest: selectedActualModel.digest,
    parameters: selectedParameters,
    requirements: selectedCase.requirements.filter(r => r !== "python_sandbox")
  },
  // 仅编程题；来自已准备的本地镜像inspect结果，不接受可变tag，也不会pull。
  pythonImage: selectedLocalImageId // sha256:<64位小写hex>；非编程题可省略
}
```

省略datasetVersion仍为版本1，保持旧请求及幂等配置的序列化；v1只开放原两道开发题。版本2可选择已存在的60个ID。python_sandbox是环境需求，不传入模型能力预检。没有根据Qwen品牌推断工具或Schema能力。

模型自报明确不支持时记录Unsupported；缺失能力信息保持unknown。仅能力未知、而身份/上下文/服务检查均已通过的v2任务可实际请求当前题目；结果保留“未知+本题观察”，不宣称已验证该模型的全部能力。运行前再检查一次，身份变化或明确不支持会停止发送并保存实际预检证据。

运行明确绑定datasetId/version/hash、caseHash、规则/契约、模型digest及请求快照。v2评分引擎为`core60-runtime-2.1`，评分配置hash包含引擎、规则、逐题契约、容器策略及镜像身份；源码指纹覆盖执行/评分模块和Cargo清单/锁文件。v1评分读取与旧版本名称保持独立。

题库v1/v2的字节和hash没有修改。v2 manifest内“stage_a”字样是不可变的创建阶段元信息；目录返回的runtime说明当前引擎及环境条件，不能据旧manifest字段误判当前实现状态。数据内needs_human_review不被静默改写为第三方审阅通过。

## 六类实际评分

| 类别 | 实际输入及判定 | 分开保存的结果 |
| --- | --- | --- |
| 中文知识 | 首尾空白去除后匹配逐题合法别名；大小写/字形按契约，不做全局折叠 | 是否通过、文本协议及所用规范化 |
| 整数数学 | 题面声明的ASCII负号/数字，i64范围内数值相等；不执行表达式 | 数值正确、格式有效 |
| JSON | 单个合法JSON、拒绝重复键；2020-12验证器及date断言；对象键无序、数组有序、类型和值一致 | JSON有效、Schema有效、语义相等 |
| RAG | status/facts/reason/citations结构；逐事实值/别名、可答/拒答/冲突、固定as_of及标注证据 | 答案、可答性、拒答/冲突、引用位置/支持/覆盖、版本有效性 |
| Python | Docker逐断言运行；返回类型在JSON编码前观察，宿主比较预期值及实参不变；原答案不送容器 | 每条通过/失败及原因、类型/值/输入变更、计划/运行/通过断言数、环境/清理结果 |
| 工具 | 实际Provider tool_calls对象，经片段归并后检查调用数、工具名、必填/额外参数、类型和值 | 原生调用存在、数量、名称、Schema及参数语义；工具实际执行固定false |

JSON保留原数值文本并比较规范十进制系数/指数，28与28.0可等价，28.0000000000000000001不能被舍入成28。布尔不等于数值，null不等于缺失。内部解码保留结构并拒绝重复/保留实现键；没有把字符串完全相等当JSON评分。

Schema采用真实[jsonschema验证器](https://docs.rs/jsonschema/latest/jsonschema/)，启用format断言；当前只接纳Core60已声明的词汇。未知关键词、外部引用或无效评分配置记Scoring Error，不偷偷忽略。Level 2复杂关键词扩展仍需独立版本与控制样本；不把本轮范围说成所有JSON Schema特性的验收。

RAG答案正确不抵消证据失败。没有引用时precision为null/N/A，可答题coverage为0；不可答题无必需事实时coverage也为null。支持关系由固定题库逐事实标注，范围必须在资料内且列入允许集合；更宽的预审范围可接受，正确行号但无关事实不能通过。等价数值表示的重复引用也拒绝。冲突需要两方事实与证据；版本检查针对实际引用的生效/取代关系，不能声称读取了模型内部选用知识的过程。

工具正文中即使写出正确JSON也不计调用。原始响应对象按实际帧记录，索引明确的对象片段可归并；无索引的多个对象仍计多个调用。帧包含重复字段、非法结构或冲突片段时保留错误，不补造调用。协议依据为[Ollama流式工具调用](https://ollama.com/blog/streaming-tool)；当前没有OpenAI-compatible适配、多步调度或真实工具副作用。

## Python隔离实现与实测边界

只连接本机npipe/unix Docker端点并确认Linux引擎。镜像必须是本地不可变内容ID；不自动拉取、不运行宿主Python、不挂宿主目录。每个断言创建独属容器，覆写入口为Python3，使用隔离/不加载site配置，并核对实际Python3.13。

冻结策略：网络none、非root 65534:65534、cap-drop ALL、禁止提权、只读根文件系统、1 CPU、256MiB内存/同额度swap限制、64进程、受限tmpfs、无宿主/Docker Socket挂载；校验创建后的容器配置。单次函数容器最多5秒，整题最多60秒，最后清理另最多5秒；stdout+stderr合计64KiB。关闭容器日志落盘并限制文件大小/句柄，结束、取消和异常都清理本尝试的唯一容器名。策略基于[Docker运行参数](https://github.com/docker/cli/blob/master/docs/reference/commandline/container_run.md)，不能以配置存在代替真实资源约束实测。

源码限函数定义和标准库导入；不接受顶层可执行脚本、global/nonlocal。返回int/bool/str/list按真实Python类型观察，保留输入副本并将实际参数的类型和值返回宿主复核；bool不能冒充int，tuple不能冒充list。提交及实参可以进入容器，预期答案留在宿主；宿主不信任程序自报的passed字段。

类型与输入变更属于容器内观察及宿主复核，不是对任意恶意Python的形式化纯函数证明。Core60是公开Smoke，不能据此声称具备防作弊排行榜能力。容器不是绝对安全保证；Windows/WSL后端的隔离、超时、内存/进程/网络与清理仍需真实环境验收。

**本机未安装Docker，用户明确要求保留实测待办。** 当前代码题在模型请求前检查环境，返回Execution Error及具体缺项，保留计划项，正确率为null。控制器正反测试使用受控CLI观察，不执行待测源码；真实容器检查显式ignored，没有计为通过。

以后准备好可信的本地Python3.13镜像后，先核对镜像ID及引擎，再从`apps/desktop/src-tauri`运行：

```powershell
$env:CORTEXOS_BENCHMARK_TEST_IMAGE = 'sha256:<实际本地镜像内容ID>'
cargo test --lib scoring_python_real_docker_types_mutation_and_cleanup -- --ignored --nocapture
```

该检查会真正运行正确函数、错误类型、输入修改、Socket缺失和根目录只读探针，并核对容器清理；不能把它当作尚未实现的全部隔离压力验收。安装/准备运行时、镜像和压力检查另按用户确认推进。

## 状态、统计与性能

| 状态 | 含义 | 进入正确率分母 |
| --- | --- | --- |
| Passed | 已完成执行且全部本题判定通过 | 是 |
| Failed | 已完成执行但答案/格式/证据/代码契约未通过 | 是 |
| Unsupported | 有模型/Provider能力不支持的预检证据 | 否 |
| Not Run | 已计划但未开始；本阶段无批量队列，不补造未执行60题 | 否 |
| Execution Error | 服务/环境/传输/协议等使执行未能完成 | 否 |
| Scoring Error | 评分器/契约/保存异常，答案未知 | 否 |
| Needs Review | 输出截断或完整输出尚未可靠评分 | 否 |
| Cancelled / In Progress | 独立生命周期，不混成答题错误 | 否 |

标准六种状态之外保留复核、取消及进行中，与设计的W/C/L对应。评测状态存入现有评分JSON，底层数据库原状态继续表达执行生命周期，不破坏旧约束。模型答错但正确记录并判0时，实验可以completed；这不表示答案通过。

`get_benchmark_run.statistics`同时返回：

- planned、attempted、executionSucceeded、scored、passed和全部状态数量。
- executionCoverage=实际发出请求/计划项，scoringCoverage=有效评分/计划项。
- executionSuccessRate=模型完整输出/实际已发出请求；Python环境预检失败不会冒充一次模型调用，错误数量仍完整展示。
- correctness=Passed/(Passed+Failed)，分母0为null。
- datasetCases=60、distinctCasesScored及datasetScoringCoverage，避免单题开发样本被描述成全套100%。
- byEvaluator和byDifficulty分组，无样本显示null；Core60无hard，不生成困难结果。

目前是一个实验的一道题，不是已实现多模型/批量结果表。模型能力、代码、引用等指标在各评分metrics中分别保存，没有加权黑箱总分。调用契约通过率不等于工具任务执行成功率。

实际最终响应的Token计数及服务端时长原样保存，缺失为null。v2另用客户端单调时钟记录HTTP响应头、首文字、首工具事件及完整输出；响应头不是首Token，工具事件也不是首字。失败不估造Token或延迟，普通聊天/v1不新增这些字段。资源峰值尚未采集，不填0。单题不足以支撑稳定p95或模型性能排名。

## 存储、恢复与验收

保留v1的两种旧判定和历史版本，新规则只处理明确版本2的运行。评分先读已保存输出及实际原生帧，事务插入评分/状态；重复完成幂等，模型请求不增加。输出hash同时覆盖文字和原生帧，工具无文字时也有实际内容身份。

没有重新评分接口，不覆盖旧评价；今后若新增重评须追加独立版本记录。现有恢复只对账执行，不自动重发。重启后已有完整输出但评分未完成时，明确为待复核，不伪装有效分数；批量恢复/手动续跑仍属于原切片4。

评分插入失败保留原输出；已有错误事件时统计显示Scoring Error，不能自动填模型0分。整个数据库不可写时也可能无法记录错误自身，仍只能恢复最后成功落盘内容；本轮采用触发器模拟局部评分写入失败，没有做磁盘耗尽实验。

验证结果：37项独立Rust检查通过、真实Docker1项ignored；v2原生专项1项通过，覆盖14种受控场景；旧v1及聊天/知识/检索/来源/执行图原生回归6项通过；类型检查、桌面构建和不可变数据/源码指纹核对通过。原生受控服务共有11次生成请求，不能当成11次实际模型能力测试。50个非编程标准响应及93条编程控制器观察也是正例控制，不是60题模型成绩。

Ollama的外部标签在最后检查之后仍可能被其他程序修改，保存的快照不等于获得跨进程模型锁；资源环境与缺失计数继续按已观察事实报告。

最终实际命令、早期失败和未运行项见[里程碑](milestone-log.md)。阶段B交付后停止等待验收；真实Docker待办按用户决定保留，不自动进入原切片4、生成Level 2/3题目或发布版本。

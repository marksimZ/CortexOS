# Benchmark 切片2：题目与模型预检

2026-10-23。此切片提供数据和Rust接口，页面按原计划在切片6接入。没有发送生成请求，没有分数、Runner、工具执行、Docker、云端或密钥管理。开发库仍为schema8；本轮没有DDL迁移，不触发新迁移备份。既有0008备份/恢复验证沿用切片1，正式用户库未打开。

后续状态：切片2已获用户确认，切片3另行实现开发单题执行，见[执行契约](benchmark-execution-slice3.md)。本页保留切片2当时的只读验证范围；预检自身仍不发送生成请求。

阶段B更新（2026-10-24）：目录runtime字段反映当前v2引擎及Docker条件，冻结manifest继续保留创建时的状态；明确datasetVersion=2可进入[v2单题评分](benchmark-scoring-stage-b.md)。模型身份/上下文失败仍阻断；仅能力未知时可通过显式选中的v2题做一次当前任务探针，保留unknown，不推断通用支持。python_sandbox单独检查Docker，未安装时不发代码题模型请求；旧v1入口不变。预检命令本身仍只读元数据。

2026-10-24接口更新：[Core60 v2阶段A](benchmark-core60-v2.md)已登记版本2，`list_benchmark_datasets`按版本1/2返回两项，`get_benchmark_case`支持明确版本读取。两个版本在同一事务安装；v1的字节/hash与历史评分不变。v2的Scoring.contract保存在现有scoring_json，无SQL迁移；manifest额外覆盖revision-notes.json。开发执行入口仍明确绑定v1，v2评分与执行等待阶段B。下文固定文件及原验证记录均是切片2时的v1。

## 固定题目

文件位于`apps/desktop/src-tauri/datasets/cortex-core-60/`：

| 文件 | 内容 |
| --- | --- |
| manifest.json | 整数版本1、顺序、数量、模板、来源/许可声明及其他文件SHA-256 |
| cases.json | 60题，15知识/15数学/10代码/10固定资料/5结构化/5工具；含本地标准及逐题hash |
| case.schema.json | JSON Schema 2020-12结构说明；Rust serde与语义检查是本轮加载校验入口 |
| scoring-rules.json | 六种规则的ID、版本和判定规格，document_type=scoring_specification；不表示已存在引擎 |

manifest原始UTF-8字节的SHA-256作为数据集contentHash；manifest包含其他三个文件原始字节hash。逐题hash排除content_hash字段，使用serde_json默认按键排序、紧凑UTF-8序列化后的字节（不是通用RFC8785实现）。换行固定LF，数组顺序有意义。

首次`list_benchmark_datasets`在有效工作区下原子安装两个表中的内置草案；每次读取重新校验文件、固定ID/分布、题目/标准格式、规则版本及引用的一基闭区间。已有版本逐行相等才接受，失败返回错误，不覆盖。入库失败整体回滚，重试不产生重复记录。内置定义全局共享，聊天和评测运行仍按工作区/用途隔离。`get_benchmark_case`从数据库读题，返回的标准仅用于本地复核。未来发送请求只能选择input/fixture/模板，不能序列化整个Case外发。

本轮示例/schema一致性检查只处理所用的object/array/string/integer/boolean/null、enum、数值和长度界限；不是通用模型输出Schema评分引擎。不存在执行模型输出、导入外部任意schema或宿主机运行Python的路径。

全部内容明确为`needs_human_review`。结构校验不是人工事实审核；请按[审阅表](benchmark-dataset-review.md)逐题核查。版本1为开发草案，标准、材料、模板或审阅状态变化时发布新版本并保留旧版。小型公开题目可能已被模型见过，不能据此宣布学术排名或全领域质量。

## 已实现的命令

四个命令都由Rust验证workspaceId存在；未授予任意HTTP、文件、shell或数据库权限。

| 命令 | 参数与返回 |
| --- | --- |
| list_benchmark_datasets | workspaceId → 内置版本、contentHash、manifest |
| get_benchmark_case | workspaceId、datasetId、version、caseId → 数据库中的完整题目及hash |
| list_benchmark_models | workspaceId → 实际标签/digest/量化/大小、服务版本、当前加载列表或null及原因 |
| inspect_benchmark_model | workspaceId、request → 独立参数、检查快照、状态与原因 |

request示例（model和expectedDigest必须来自本机实际读取，下面不虚构标签）：

```typescript
{
  model: selected.name,
  expectedDigest: selected.digest, // 首次查看可为null，返回需确认；正式运行前必须固定
  parameters: {numCtx: 4096, numPredict: 512, temperature: 0.3, timeoutSeconds: 180},
  requirements: ["completion"] // 也可声明json_schema或tool_calls
}
```

参数以请求值保存和检查；复用已有应用参数范围校验，不读写全局聊天配置。范围1024–16384上下文、32–2048输出、0–2温度、30–600秒是应用当前限制，不是模型已接受的参数。检查模型声明上下文上限，同时保留未知；不把自报最大值当作设备可用容量。示例4096/512是产品起点，不是推断模型规格。

## 预检的含义

| 状态 | 操作含义 |
| --- | --- |
| ready_for_probe | 身份与所需元数据检查通过，可以进入后续实际探针；不是“生成已验证” |
| needs_verification | 未固定/缺失digest、缺失上下文元数据或能力未知，原因逐项返回 |
| not_installed | 本机列表没有所选模型，不自动下载 |
| identity_changed | 预期digest不匹配、检查期间被替换/删除或show与tags指纹矛盾，需重新确认 |
| blocked | 非本地GGUF、未知格式、声明不支持所需能力、上下文越界或show失败 |

服务不可连接、非法参数或非法工作区直接返回明确错误。`/ps`不可用时loaded=null且保留warning；绝不当作“没有模型加载”。size_vram是Provider报告的模型估计字节数，不是整卡峰值；/ps.context_length是当前加载配置，不替代模型元数据上限。

沿用127.0.0.1、禁代理/重定向、连接及请求超时和4MiB响应限制。元数据快照只保留选定字段，排除模型模板/许可证正文。capabilities分supported/unsupported/unknown，并标记api_show_advertised或not_probed；completion/tools/thinking仅记录自报。JSON Schema没有本轮生成探针，保持unknown，不能以能生成普通JSON推断支持。

tags在show前后各读一次检查同标签digest，并比较用户预期值。此处只是检查快照，无法锁住外部Ollama的可变标签；后续Runner应在每次调用前重新检查并记录结果。此轮没有发送chat/generate/pull/keep_alive=0，不会预热、下载或卸载模型。

字段依据：[Ollama模型列表](https://docs.ollama.com/api/tags)、[模型详情](https://docs.ollama.com/api-reference/show-model-details)、[加载列表](https://docs.ollama.com/api/ps)，2026-10-23核对；实际返回缺失时保留未知，不套用品牌默认值。

## 验证与停止点

必要数据专项覆盖60题结构/hash、坏标准、引用边界、规则版本、原子回滚、重复读取、重启及聊天状态不变。原生专项覆盖四个IPC、非法工作区不访问服务、独立参数、支持/未知/不支持、缺失指纹、模型替换、非本地格式和服务失败。本机真实Qwen仅只读身份/元数据；没有生成实测、Docker或新评测UI验收。

实际命令、结果与限制见[milestone-log](milestone-log.md)。本轮等待切片2验收；之后才能进入切片3的少量题真实执行闭环。完整60题运行、可执行评分、代码隔离和全部界面不在本轮完成范围。

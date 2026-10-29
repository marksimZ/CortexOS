# 模型接口、能力、密钥与预算

2026-10-25 补充：已实现 [C1 云端目录认证与安全存储](benchmark-phase-c1.md)，代码在 `src-tauri/src/cloud/`。三家官方目录共享窄 ProviderAdapter；现有 Ollama 生成流程不变。推理、费用/预算、云端任务恢复与统一生成接口仍是后续设计，C1 不发送推理请求。以下为原始设计基线。

2026-10-23 · 切片2在现有`Ollama`上实现列表/预检；切片3复用chat_request和流解析形成共享generation服务，运行前再次检查固定身份，仅支持开发单题文本输出。见[预检契约](benchmark-preflight.md)及[执行切片](benchmark-execution-slice3.md)。下表仍是完整统一接口设计；新协议、工具/Schema输出、正式能力探针、卸载、云端和费用接口尚未实现。Phase 2按实际接入需要扩展。

## 统一接口保留能力差异

模型身份由`provider_id + model_id + revision/digest`组成，供应商名称或可变别名不足以表示固定权重。每次实验保存元数据原始字段的受限快照、来源和检查时间，运行时再次核实。

| 操作 | 输入/输出契约 | 适配规则 |
| --- | --- | --- |
| list_models | 配置ID → 模型列表与元数据来源 | 没有列表接口的服务允许手填ID，显示尚未验证；不伪造完整目录 |
| inspect_model | 模型ID → 身份、上下文、参数/输出能力 | 每项能力为支持/不支持/未知，另带advertised/probe证据；文档或自报不冒充生成实测 |
| prepare_request | 冻结的公共意图及Provider扩展参数 → 精确wire body、实际生效参数、排除原因 | 额外参数按白名单验证；不支持温度/seed/JSON Schema等时明确阻止或让用户选择兼容子集，不静默删除 |
| stream | 已准备请求、取消信号 → 有界增量事件 | 标准事件含TextDelta、ToolCallDelta、Usage、Finish和Error；保留Provider请求ID、停止原因和必要原字段 |
| count_or_bound_input | 实际请求 → 精确计数或有来源的保守上限 | 无法确定时返回未知；现有文本预算单位不能当精确Token数 |
| price_bound | 价格快照、Token上限、额外收费能力 → 金额上限或不可用 | 币种一致、覆盖全部收费项；未知价格不自动记零 |
| cancel | 任务ID → 客户端已停止及远端状态（如可知） | 断开HTTP不等于远端已停止，更不等于不计费 |

Ollama专属的`list_loaded/unload`只在本地实现，不要求所有Provider假装支持模型卸载。公共流式事件允许不产生文本而直接返回工具调用；当前`commands.rs::generate`“空正文即失败”的条件只适用于现有文字聊天，评测服务须按输出类型判定。

Phase 1可先用Ollama专用结构，公共边界保持上表语义；Phase 2实际加入第二个协议时再建立Rust Provider enum/trait。无需提前加入全套SDK或Python代理。

## 本地Ollama

既有HTTP客户端禁代理/重定向且只连接127.0.0.1，继续保留。新增能力由Rust读取：

| 接口 | 用途 | 本轮核实与实施边界 |
| --- | --- | --- |
| `/api/version`、`/api/tags` | 服务版本、标签、大小、digest、量化 | 本机已读取；现有health未向前端暴露digest，需扩展只读DTO |
| `POST /api/show` | 模型详情、上下文元数据、能力 | 本机已读取两个Qwen标签；现有inspect仅保留格式/来源和可能缺失的digest，新增受限元数据快照 |
| `/api/ps` | 已加载模型、size_vram、上下文配置 | 本机本次为空；模型内存估计与整卡显存分别记录 |
| `/api/chat` | 实际流式调用、工具/结构化请求及终帧指标 | 保留NDJSON解析；补`prompt_eval_duration`、工具调用、输出类型及配置能力适配 |
| 显式释放 | 使用经当前服务验证的keep_alive=0协议释放指定模型 | 本轮未发送释放请求；实施时记录请求与确认结果，不能只改变UI状态 |

以上协议依据[模型列表](https://docs.ollama.com/api/tags)、[模型详情](https://docs.ollama.com/api-reference/show-model-details)、[加载列表](https://docs.ollama.com/api/ps)及[Chat协议](https://docs.ollama.com/api/chat)，核对日2026-10-23。实际模型能力与服务版本仍须逐次预检。

`/tags`的digest与`/show`详情共同构成检查快照，分别标记字段来源和缺失。模型标签在排队期间被替换时暂停实验，用户选择新实验或保留明确分段，不能混成同一固定版本。

默认标准配置从4096上下文、512输出和单请求开始，记录模型实际接受的参数。模型自报最大上下文不代表8GB设备可行容量。thinking参数能力单独核实；关闭可见思考输出只是请求选项，不代表能够读取模型隐藏内部过程。

## 云端接入顺序与差异

| Provider | Phase 2适配重点 | 待实际验证事项 |
| --- | --- | --- |
| OpenAI | Responses的事件、usage、工具输出及实际模型标识 | 模型参数支持、计费/缓存项、请求限额；不能把Chat Completions兼容性当Responses兼容性 |
| OpenAI-compatible | 首版明确实现哪一种协议及端点 | 模型列表、SSE结束方式、usage和错误结构常有差异；逐服务探测 |
| Anthropic | Messages、内容块、tool_use及流式usage | 终态计数语义、版本头、工具与最大输出支持 |
| Gemini | contents/parts、工具参数、流式响应及usageMetadata | 模型能力、缓存/思考Token计费与结束原因 |
| 自托管 | 用户明确保存的服务地址与协议类型 | 认证、TLS、能力及隐私范围均按该端点检查 |

官方协议分别见[OpenAI流式响应](https://developers.openai.com/api/docs/guides/streaming-responses)、[OpenAI Token计数](https://developers.openai.com/api/docs/guides/token-counting)、[Anthropic API总览](https://platform.claude.com/docs/en/api/overview)、[Anthropic流式消息](https://platform.claude.com/docs/en/build-with-claude/streaming)、[Gemini generateContent](https://ai.google.dev/api/generate-content)。本轮核查公开文档，没有配置密钥或调用这些模型，兼容性均未实测。Responses和旧Anthropic大页面抓取失败后改查较小官方页面，不将抓取失败当作服务不可用。

## 安全存储与发送授权

Windows首选系统Credential Manager，通过Rust保存/读取/删除；SQLite只保存凭据引用和是否已配置。实现时比较最小维护成本的Rust封装与直接Win32调用，失败时提示重新配置，禁止回退明文。[CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew)是本次核查的系统能力，不是项目已有实现。

- 密钥只在用户输入与后端请求的必要内存中存在；不进app_settings、URL、请求快照、事件、异常正文或报告。Git凭据与应用Provider密钥完全分开。
- 云端默认HTTPS、固定保存的origin/base path；禁自动重定向和凭据跨origin转发。loopback自托管HTTP单独允许；其他不安全端点需单独设计和明确授权。
- 当前本地地址校验保持原样；新的云端地址策略单独实现，不把旧本地校验放宽为任意URL代理。只提供具体推理命令。
- 每次启动展示Provider、实际端点和会发送的题目/模板/资料范围。判分标准默认留在本地；私有知识快照外发须单独确认，授权绑定内容hash、目标端点和用途，变化后重新确认。
- 外部资料和模型输出是数据，不能改变系统指令、调度、预算、评分规则或工具权限。工具测试只接受数据集列明的模拟工具。

## 预算、重试和未知计费

价格快照保存来源URL、取得时间、适用模型、币种、计价单位及输入/输出/缓存/其他收费规则，使用十进制定点或整数最小货币单位。Phase 0不写固定价格，也不把估算写成实际账单。

发送前在同一事务为本次尝试预留保守费用上限：已确认估算费用 + 未决预留 + 本次上限 ≤ 用户预算。每个请求有明确输出上限和超时，自动重试预算必须包含在内。输入计数或价格/收费模式无法保守界定时，不启动受预算保护的付费实验，要求补充资料或更换服务。

实际usage返回后按所保存价格计算“按用量估算”；只有供应商账单证据才标“实际计费”。取消、网络超时、流断开、崩溃且未取得完整usage时，保留预留并标“费用未确认”，不能记零或马上释放预算。预算防护限制本应用新增发送，不能保证供应商最终账单完全一致。

默认自动重试0；用户可有限增加。429等可恢复错误按供应商明确规则、Retry-After和有上限退避处理；请求可能已接受但结果未知时，禁止自动重发。只有服务明确支持且已验证的幂等或结果查询才能安全去重，不能把自建attempt_id当通用付费幂等保证。

服务端usage若为累计值则取最终已确认值，不逐帧累加；若为增量则按协议累计。缓存、推理/可见思考、工具和输入/输出Token分别保留原语义。跨Provider的计数口径差异进入比较条件。

## 接口验收

本地实测：改名标签/缺失字段/不支持参数、拆分UTF-8帧、无正文工具调用、取消/断流/终帧和指标。云端先用受控协议替身检查密钥不泄漏、usage不重复、预算原子预留和恢复不重复扣费；用户明确同意后才用小额真实API验证。记录协议和模型版本，不把替身通过写成真实兼容。

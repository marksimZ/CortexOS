# Benchmark 切片3：单题开发执行

2026-10-23。用户确认切片2后实施本片。现有后端支持“固定题目与模型 → 保存精确请求 → 实际调用 → 保存原文 → 基础判定 → 详情读取”。新评测页面在切片6接入，本轮没有新导航、实验配置UI或3D评测投影。

2026-10-24后续状态：本页的两题白名单与判定仍是v1契约；[阶段B](benchmark-scoring-stage-b.md)为显式版本2增加六类单题评分、结构化请求/真实工具响应及状态统计。批量队列、手动续跑和新页面仍未接入，Docker真实验证按用户决定保留待办。旧输出及规则没有被新版静默覆盖。

## 当前范围

每次一个本机模型、一道题、一次尝试。明确developmentOnly=true，仅开放`math-001`（37+58）与`zh-006`（一百的数字写法）。数据集仍是cortex-core-60整数版本1审阅草案，题目/hash/模板保持切片2原值；切片2验收不代表60题已独立人工审核。开发检查不能当正式60题评测、跨模型质量排名或学术结论。

已有Ollama连接、NDJSON解析、250ms脏输出检查点、1MiB输出保护和超时继续复用。模型标签与digest从实际列表选择，初次检查和调用前复核；没有硬编码新模型标签或修改聊天默认值。

## 已实现接口

命令带workspaceId，由Rust验证工作区和记录归属。评测读写与普通聊天用途隔离，没有新增任意文件、网络、shell或SQL权限。

| 命令 | 输入/结果 |
| --- | --- |
| start_benchmark | workspaceId、input → 含实验/逻辑项/尝试/执行ID的Summary；保存完毕后后台执行 |
| get_benchmark_run | workspaceId、experimentId → Summary、固定配置/模型/题目、精确请求和执行事件、已保存输出、尝试状态/发送情况、评分与实验事件 |
| cancel_benchmark | workspaceId、experimentId → 请求停止当前评测；不会发送下一题或自动重试 |
| bootstrap.activeTask | id、workspaceId、kind=chat/benchmark、executionRunId；准备阶段执行ID为null |

bootstrap.activeRun继续仅投影日常聊天。当前聊天页面没有评测任务条，尝试发送时后端给出“已有聊天或评测正在进行，请先停止或等待完成”；新摘要留给后续统一任务UI使用。Benchmark当前通过命令与数据库轮询读取，聊天原有Channel保持原样。

启动参数示意（身份和datasetHash来自实际预检，不使用虚构标签）：

```typescript
const input = {
  launchKey: crypto.randomUUID(),
  developmentOnly: true,
  datasetHash: selectedDataset.contentHash,
  caseId: "math-001",
  model: {
    model: selectedModel.name,
    expectedDigest: selectedModel.digest,
    parameters: {numCtx: 4096, numPredict: 64, temperature: 0, timeoutSeconds: 180},
    requirements: ["completion"]
  }
};
```

同工作区、同launchKey、同配置返回原实验，不重复发送；配置不同时拒绝复用该标识。两个启动并发时只有一个获得占用，另一个拒绝，不创建等待队列。失败、取消或已中断实验用原标识读取原记录，当前没有自动恢复或手动续跑命令。

## 数据与占用

`tasks.rs::TaskSlot/Lease`替换原聊天独占状态，聊天和评测共用。占用覆盖准备、实际调用、原文保存、评分及实验终态；共享generation只完成单次模型请求，释放由调用者持有的Lease负责。关闭应用取消整个任务，等待结束；关闭标志同时拦截新启动。

`db.rs::insert_execution`是两种用途共用的事务记录函数。聊天保留已有上下文组装和来源语义；评测单独创建purpose=benchmark的会话，不读日常消息、长期知识或生产检索。相同事务保存：

1. 实验的单题计划、配置/hash、开发标记与规则/模板。
2. 实际预检快照及模型槽位/hash。
3. 专用会话、用户/回答消息、ExecutionRun。
4. context_snapshots中的精确请求、字节hash和清单。
5. 逻辑项、一次尝试与真实执行的绑定，以及已经发生的准备事件。

事务失败不调用模型。请求只有固定system及本题prompt两条消息，标准答案/评分规则不发给模型。发送时复用已保存的同一Value；受控原生检查捕获HTTP正文，验证对象及原始字节SHA-256与快照一致。

用途和完整身份校验覆盖共享事件、检查点、终态和内部详情。普通会话/消息/运行/图谱仍拒绝评测执行；共享执行的迟到写入在终态后忽略，不能改写输出或追加伪造事件。没有虚构检索或验证步骤，也不记录模型隐藏思维链。

开发源码仍schema8，无新DDL/迁移备份。实验表保存app版本、相关十个执行实现Rust文件的内容指纹（slice3-core-sha256，非完整仓库/Git版本）、评分实现版本及明确未采集的硬件快照。源码与配置/模板/标准hash是不同口径。参数清单标为requested_not_verified_as_effective，不能因HTTP成功就称模型采用了全部请求参数。

## 保存、判定与失败

共享generation先事务保存原始输出和执行终态，仓储再读取这些已保存事实，事务追加评分并结束尝试/逻辑项/实验。模型成功与答案正确分开；评测完成不等于答对。

| 判定 | 实际规则 |
| --- | --- |
| integer_equal v1 | 去首尾Unicode空白，接受可选ASCII负号及ASCII数字，解析i64后比较；解释、加号或其他格式按不匹配处理 |
| exact_alias v1 | 去首尾Unicode空白后与固定别名逐字比较，区分大小写 |
| 输出上限 | Provider明确done_reason=length时保留原文，correctness=null、needs_review，不当作已评分成功 |
| 失败/取消 | 保留收到的部分，不生成正确性分数；模型失败与答错分开 |

规则文件保持规格，实际实现只对上述两题启用；其余题目、JSON Schema、原生工具调用及Python题在启动前拒绝。题目或模型生成的代码不会在宿主机执行；Docker、自动卸载、云端和资源采样未接入。已有服务端终帧字段照实记录，缺失保持null；没有新增精确首字延迟、整卡峰值等结论。

发送情况初始not_sent，实际进入请求阶段时保守标为unknown，收到完整成功输出后finished。取消只表示本地已停止接收，不承诺远端已即时停止计算。准备阶段取消可在activeTask取得ID，此时尚无实验记录，启动返回“已停止”。

评分存储失败时原始输出不覆盖，实验记录result_save_failed并置失败，不自动再问模型。数据库全面不可写时错误可能也无法入库，进程输出错误、保留最后可写检查点；启动沿用既有中断对账，不承诺未落盘内容可恢复。本轮故障注入验证了准备回滚与评分写入失败，没有做磁盘真正耗尽实验。

## 验收证据与停止点

必要SQLite专项3项通过；受控原生验证HTTP请求字节、已存在聊天和可命中知识不混入、两种任务互斥、同标识不重发、流中/准备中取消、部分保存、迟到无覆盖、调用前身份变化、坏流和跨域拒绝。既有聊天/知识/检索/来源/执行图5项原生回归通过。

真实本机qwen3:4b-instruct只执行math-001一次，返回95，integer_equal v1得到1，实际库中执行/尝试/评分各1条，没有生产检索/知识引用。此结果只证明开发闭环可用，不能外推模型综合准确率。实际命令、指纹与证据位置见[milestone-log](milestone-log.md)。

本轮停在切片3等待验收。下一片才实现多项串行计划与恢复；完整60题、模型切换、正式评分和评测界面继续按原计划后续切片推进。
